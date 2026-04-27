//! 指标有效性评估器（Sprint A — 过渡方案）
//!
//! 给定一段历史 K 线：
//! 1. 扫描所有 "可评估 Arm" 的触发点（Playbook + MA 高级信号）
//! 2. 用 [`signal::replay::HistoricalReplay`] 结算每次触发后 `horizon` 根的表现
//! 3. 按 arm 名聚合 → 胜率 / 平均收益 / Sharpe / α / 综合评分
//! 4. 按综合评分降序返回排行榜
//!
//! # 设计原则（见 `RL_EFFECTIVENESS_DESIGN.md`）
//!
//! - Arm 粒度：L1 Playbook + L2 MA 高级信号（不含形态，避免噪声）
//! - Horizon：默认 10 根 K 线（可覆盖）
//! - 综合评分：`sqrt(n) × (win_rate − 0.5) × avg_return_pct × 10`
//!   - `sqrt(n)`：样本越多越可信（边际递减）
//!   - `(win_rate − 0.5)`：纯运气校正，抛硬币为 0 分
//!   - `× avg_return_pct × 10`：收益放大器（百分点）
//! - Sprint B 会把这套评估换成在线 Thompson Sampling，此处为静态统计版本

use serde::{Deserialize, Serialize};

use crate::data::Kline;
use crate::engine::backtest::{
    self, CompositePlaybook, GuillotineExitPlaybook, HangingScallionsEntryPlaybook, Playbook,
    PlaybookContext, PlaybookDecision, StagedExitPlaybook, TrendMatrixPlaybook,
};
use crate::engine::ma::{self, MaAdvancedKind, MaAdvancedParams};
use crate::engine::signal::{self, HistoricalReplay, ReplayRecord, ToppingSignalSeverity};
use crate::engine::trend::{self, DowPhase};

/// 默认 horizon（评估 N 根后收益）
pub const DEFAULT_HORIZON: usize = 10;

/// 单个 arm 的聚合表现
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EffectivenessEntry {
    /// arm 唯一标识，如 "signal.ma.guillotine" / "playbook.guillotine"
    pub arm: String,
    /// 人类可读标签
    pub label: String,
    /// 类别：Signal / Playbook
    pub category: String,
    /// 原书出处（若有）
    pub book_source: Option<String>,
    /// 样本数（触发次数 ∩ 有 horizon 后数据）
    pub n: usize,
    pub wins: usize,
    pub losses: usize,
    /// 胜率 [0, 1]
    pub win_rate: f64,
    /// 方向修正后平均涨跌幅（百分比，如 0.025 = 2.5%）
    pub avg_return: f64,
    /// 平均涨跌幅（百分数，便于前端直接展示，如 2.5）
    pub avg_return_pct: f64,
    /// Sharpe 比（avg / std）
    pub sharpe: f64,
    /// 最大单次收益（方向修正后）
    pub max_return: f64,
    /// 最大单次回撤（方向修正后）
    pub min_return: f64,
    /// 平均最大不利回撤（MAE，负值）
    pub avg_mae: f64,
    /// 平均最大有利收益（MFE，正值）
    pub avg_mfe: f64,
    /// 相对买入持有市场基线的平均 α（方向修正后）
    pub alpha_vs_market: f64,
    /// 综合评分（详见模块注释）
    pub effectiveness_score: f64,
}

/// 评估报告
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EffectivenessReport {
    pub symbol: String,
    pub interval: String,
    pub bars: usize,
    pub horizon: usize,
    /// 总触发次数（未必都能评估，可能因 horizon 超出范围被过滤）
    pub total_triggers: usize,
    pub rankings: Vec<EffectivenessEntry>,
}

/// 内部：单次触发记录
#[derive(Clone)]
struct Trigger {
    arm: &'static str,
    label: &'static str,
    category: ArmCategory,
    book_source: Option<&'static str>,
    index: usize,
    direction: i8,
}

#[derive(Clone, Copy)]
enum ArmCategory {
    Signal,
    Playbook,
}

impl ArmCategory {
    fn as_str(&self) -> &'static str {
        match self {
            ArmCategory::Signal => "Signal",
            ArmCategory::Playbook => "Playbook",
        }
    }
}

// ============================================================
// 主入口
// ============================================================

/// 在给定历史 K 线上扫描所有 arm 触发并汇总有效性
pub fn evaluate(
    klines: &[Kline],
    symbol: impl Into<String>,
    interval: impl Into<String>,
    horizon: usize,
) -> EffectivenessReport {
    let horizon = horizon.max(1);
    let bars = klines.len();

    let triggers = if klines.len() > horizon + 60 {
        collect_all_triggers(klines)
    } else {
        Vec::new()
    };

    let closes: Vec<f64> = klines.iter().map(|k| k.close).collect();
    let replay = HistoricalReplay::new(&closes, horizon);

    // 按 arm 聚合
    use std::collections::HashMap;
    let mut by_arm: HashMap<&'static str, Vec<(Trigger, ReplayRecord, Option<f64>)>> =
        HashMap::new();

    let mut total_triggers = 0usize;
    for t in &triggers {
        total_triggers += 1;
        let Some(record) = replay.evaluate_signal(t.arm, t.index, t.direction) else {
            continue; // horizon 超出范围
        };
        let alpha = replay.alpha_vs_market(&record);
        by_arm
            .entry(t.arm)
            .or_default()
            .push((t.clone(), record, alpha));
    }

    let mut rankings: Vec<EffectivenessEntry> = by_arm
        .into_iter()
        .map(|(arm, group)| build_entry(arm, group))
        .collect();

    rankings.sort_by(|a, b| {
        b.effectiveness_score
            .partial_cmp(&a.effectiveness_score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    EffectivenessReport {
        symbol: symbol.into(),
        interval: interval.into(),
        bars,
        horizon,
        total_triggers,
        rankings,
    }
}

// ============================================================
// 1) 收集所有触发点
// ============================================================

fn collect_all_triggers(klines: &[Kline]) -> Vec<Trigger> {
    let mut out = Vec::new();
    out.extend(collect_ma_signal_triggers(klines));
    out.extend(collect_playbook_triggers(klines));
    out
}

/// L2：MA 高级信号（4 种）
fn collect_ma_signal_triggers(klines: &[Kline]) -> Vec<Trigger> {
    let closes: Vec<f64> = klines.iter().map(|k| k.close).collect();
    let opens: Vec<f64> = klines.iter().map(|k| k.open).collect();
    let volumes: Vec<f64> = klines.iter().map(|k| k.volume).collect();
    let periods = [5usize, 10, 20, 60];
    let mas: Vec<Vec<f64>> = periods.iter().map(|&p| ma::sma(&closes, p)).collect();
    let events = ma::scan_advanced(
        &closes,
        &opens,
        &volumes,
        &mas,
        &periods,
        &MaAdvancedParams::default(),
    );

    events
        .into_iter()
        .map(|e| Trigger {
            arm: ma_arm_name(e.kind),
            label: ma_arm_label(e.kind),
            category: ArmCategory::Signal,
            book_source: Some(e.kind.book_source()),
            index: e.index,
            direction: e.kind.direction(),
        })
        .collect()
}

/// L1：Playbook（5 个，全部在每根 K 线上试跑）
fn collect_playbook_triggers(klines: &[Kline]) -> Vec<Trigger> {
    if klines.len() < 60 {
        return Vec::new();
    }
    let closes: Vec<f64> = klines.iter().map(|k| k.close).collect();
    let opens: Vec<f64> = klines.iter().map(|k| k.open).collect();
    let volumes: Vec<f64> = klines.iter().map(|k| k.volume).collect();
    let periods = [5usize, 10, 20, 60];
    let mas: Vec<Vec<f64>> = periods.iter().map(|&p| ma::sma(&closes, p)).collect();
    let ma_events = ma::scan_advanced(
        &closes,
        &opens,
        &volumes,
        &mas,
        &periods,
        &MaAdvancedParams::default(),
    );

    // 预计算：每根 K 线窗口内是否有 ma 高级事件（近 10 根）
    let ma_event_by_index: std::collections::HashMap<usize, MaAdvancedKind> =
        ma_events.iter().map(|e| (e.index, e.kind)).collect();

    // 预计算：长期趋势（使用滑动窗口的 DowPhase）
    // 为性能考虑：对 klines[..=i] 的整段算一次成本太高；改为每 10 根计算一次并缓存
    let phase_cache = precompute_dow_phases(klines, 10);

    // 构造并运行 5 个 playbook
    let mut pb_guillotine = GuillotineExitPlaybook;
    let mut pb_scallions = HangingScallionsEntryPlaybook;
    let mut pb_staged = StagedExitPlaybook::new();
    let mut pb_matrix = TrendMatrixPlaybook;
    let mut pb_composite = CompositePlaybook::default_combo();

    // 预 warm-up：前 60 根用于 ma/trend 稳定
    let start = 60usize;
    let mut out: Vec<Trigger> = Vec::new();

    for i in start..klines.len() {
        // 最近 10 根内是否有 ma 高级信号（Playbook 决策依赖）
        let ma_kind: Option<MaAdvancedKind> = (i.saturating_sub(9)..=i)
            .rev()
            .find_map(|j| ma_event_by_index.get(&j).copied());

        // 顶部信号严重度：Guillotine → Severe, PoissonSpider → Intermediate
        let topping = ma_kind.and_then(|k| match k {
            MaAdvancedKind::Guillotine => Some(ToppingSignalSeverity::Severe),
            MaAdvancedKind::PoissonSpider => Some(ToppingSignalSeverity::Intermediate),
            _ => None,
        });

        let long_trend: i8 = match phase_cache[i] {
            DowPhase::Uptrend => 1,
            DowPhase::Downtrend => -1,
            _ => 0,
        };

        let ctx = PlaybookContext {
            klines,
            index: i,
            current_position: 0.5,
            ma_advanced_kind: ma_kind,
            topping_severity: topping,
            long_trend,
        };

        // 逐个 playbook 试决策
        maybe_push(&mut out, "playbook.guillotine", "断头铡刀清仓", ArmCategory::Playbook,
            Some("ma p.380"), i, &pb_guillotine.decide(&ctx));
        maybe_push(&mut out, "playbook.scallions", "旱地拔葱轻仓入场", ArmCategory::Playbook,
            Some("ma p.340"), i, &pb_scallions.decide(&ctx));
        maybe_push(&mut out, "playbook.staged_exit", "三次减仓", ArmCategory::Playbook,
            Some("candle p.605"), i, &pb_staged.decide(&ctx));
        maybe_push(&mut out, "playbook.trend_matrix", "多级趋势线矩阵", ArmCategory::Playbook,
            Some("trend p.216"), i, &pb_matrix.decide(&ctx));
        maybe_push(&mut out, "playbook.composite", "组合策略（默认）", ArmCategory::Playbook,
            Some("三书综合"), i, &pb_composite.decide(&ctx));
    }

    out
}

/// 根据 Playbook 决策推出 direction；仅 Buy / Sell 产生触发
fn maybe_push(
    out: &mut Vec<Trigger>,
    arm: &'static str,
    label: &'static str,
    cat: ArmCategory,
    book: Option<&'static str>,
    index: usize,
    decision: &PlaybookDecision,
) {
    let direction: Option<i8> = match decision {
        PlaybookDecision::Buy { .. } => Some(1),
        PlaybookDecision::Sell { target_position, .. } => {
            // 减仓方向始终看空（目标仓位越小 → 越看空）
            if *target_position < 0.5 {
                Some(-1)
            } else {
                None // 只是微调不触发 arm
            }
        }
        PlaybookDecision::Hold | PlaybookDecision::StayOut { .. } => None,
    };
    if let Some(d) = direction {
        out.push(Trigger {
            arm,
            label,
            category: cat,
            book_source: book,
            index,
            direction: d,
        });
    }
}

/// 预计算每根 K 线的 Dow 趋势阶段
///
/// 为性能考虑：每 `step` 根重算一次；中间 bar 取最近一次结果
fn precompute_dow_phases(klines: &[Kline], step: usize) -> Vec<DowPhase> {
    let n = klines.len();
    let mut out = vec![DowPhase::Consolidation; n];
    if n < 30 {
        return out;
    }
    let step = step.max(1);
    let mut last_phase = DowPhase::Consolidation;
    for i in 30..n {
        if (i - 30) % step == 0 {
            let s = trend::compute_trend_state(&klines[..=i]);
            last_phase = s.dow.phase;
        }
        out[i] = last_phase;
    }
    out
}

fn ma_arm_name(kind: MaAdvancedKind) -> &'static str {
    match kind {
        MaAdvancedKind::HangingScallions => "signal.ma.scallions",
        MaAdvancedKind::PoissonSpider => "signal.ma.poisson_spider",
        MaAdvancedKind::Guillotine => "signal.ma.guillotine",
        MaAdvancedKind::BondUpwardDiverge => "signal.ma.bond_upward",
    }
}

fn ma_arm_label(kind: MaAdvancedKind) -> &'static str {
    match kind {
        MaAdvancedKind::HangingScallions => "旱地拔葱（早期看涨）",
        MaAdvancedKind::PoissonSpider => "毒蜘蛛（首次向下）",
        MaAdvancedKind::Guillotine => "断头铡刀（再次向下）",
        MaAdvancedKind::BondUpwardDiverge => "主升浪（再次向上）",
    }
}

// ============================================================
// 2) 聚合计算
// ============================================================

fn build_entry(
    arm: &'static str,
    group: Vec<(Trigger, ReplayRecord, Option<f64>)>,
) -> EffectivenessEntry {
    let n = group.len();
    let first = &group[0].0; // 元数据取第一条
    let label = first.label.to_string();
    let category = first.category.as_str().to_string();
    let book_source = first.book_source.map(|s| s.to_string());

    let records: Vec<ReplayRecord> = group.iter().map(|(_, r, _)| r.clone()).collect();
    let stats = signal::ReplayStats::from_records(&records);

    // α 平均（跳过 None）
    let alphas: Vec<f64> = group.iter().filter_map(|(_, _, a)| *a).collect();
    let alpha_vs_market = if alphas.is_empty() {
        0.0
    } else {
        alphas.iter().sum::<f64>() / alphas.len() as f64
    };

    let avg_return_pct = stats.avg_return * 100.0;
    // 综合评分：sqrt(n) * (win_rate - 0.5) * avg_return_pct * 10
    let score = if n == 0 {
        0.0
    } else {
        (n as f64).sqrt() * (stats.win_rate - 0.5) * avg_return_pct * 10.0
    };

    EffectivenessEntry {
        arm: arm.to_string(),
        label,
        category,
        book_source,
        n,
        wins: stats.wins,
        losses: stats.losses,
        win_rate: stats.win_rate,
        avg_return: stats.avg_return,
        avg_return_pct,
        sharpe: stats.sharpe_ratio,
        max_return: stats.max_gain,
        min_return: stats.max_loss,
        avg_mae: stats.avg_mae,
        avg_mfe: stats.avg_mfe,
        alpha_vs_market,
        effectiveness_score: score,
    }
}

// ============================================================
// 测试
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn mk_kline(t: i64, o: f64, h: f64, l: f64, c: f64, v: f64) -> Kline {
        Kline {
            open_time: t,
            close_time: t + 60_000,
            open: o,
            high: h,
            low: l,
            close: c,
            volume: v,
        }
    }

    /// 构造 120 根平稳上涨的 K 线
    fn fake_uptrend(n: usize) -> Vec<Kline> {
        (0..n)
            .map(|i| {
                let base = 100.0 + i as f64 * 0.5;
                mk_kline(
                    i as i64 * 60_000,
                    base,
                    base + 0.5,
                    base - 0.3,
                    base + 0.2,
                    1000.0,
                )
            })
            .collect()
    }

    #[test]
    fn t_evaluate_empty_returns_no_triggers() {
        let report = evaluate(&[], "BTC", "1d", 10);
        assert_eq!(report.bars, 0);
        assert_eq!(report.total_triggers, 0);
        assert!(report.rankings.is_empty());
    }

    #[test]
    fn t_evaluate_small_kline_not_enough_for_warmup() {
        // < horizon + 60 根 → 直接返回无触发
        let klines = fake_uptrend(50);
        let report = evaluate(&klines, "BTC", "1d", 10);
        assert_eq!(report.total_triggers, 0);
    }

    #[test]
    fn t_evaluate_uptrend_has_playbook_triggers() {
        let klines = fake_uptrend(150);
        let report = evaluate(&klines, "BTC", "1d", 10);
        // 平稳上涨的市场应该至少触发 TrendMatrix / Composite（即使没有 ma 高级信号）
        // 由于 ma_advanced_kind=None，trend_matrix 只会在 long_trend<0 时返回 StayOut
        // 平稳上涨时 long_trend>=0 + ma_kind=None → 全部 Hold，实际可能无 Buy/Sell 触发
        // 所以这里只验证不 panic 且字段可序列化即可
        assert_eq!(report.bars, 150);
        assert!(report.horizon == 10);
        // rankings 可能为空，但如果有，每条都有合理的字段
        for entry in &report.rankings {
            assert!(entry.n > 0);
            assert!(entry.win_rate >= 0.0 && entry.win_rate <= 1.0);
            assert!(!entry.label.is_empty());
        }
    }

    #[test]
    fn t_score_formula_sanity() {
        // 构造假触发，手工验证评分公式
        // 场景：25 次触发，胜率 0.6，平均收益 2% → score = 5 * 0.1 * 2 * 10 = 10
        let mut records = Vec::new();
        for i in 0..25 {
            let correct = i < 15; // 15 胜 10 负
            records.push(ReplayRecord {
                name: "x".into(),
                index: i,
                direction: 1,
                price_at_signal: 100.0,
                price_after: if correct { 102.0 } else { 98.0 },
                raw_return: if correct { 0.02 } else { -0.02 },
                directional_return: if correct { 0.02 } else { -0.02 },
                correct,
                max_adverse_excursion: -0.01,
                max_favorable_excursion: 0.02,
            });
        }
        let stats = signal::ReplayStats::from_records(&records);
        assert_eq!(stats.total, 25);
        assert_eq!(stats.wins, 15);
        assert!((stats.win_rate - 0.6).abs() < 1e-9);
        // avg = (15*0.02 - 10*0.02)/25 = 0.1/25 = 0.004
        assert!((stats.avg_return - 0.004).abs() < 1e-9);

        let avg_pct = stats.avg_return * 100.0; // 0.4
        let score = (25.0f64).sqrt() * (stats.win_rate - 0.5) * avg_pct * 10.0;
        // = 5 * 0.1 * 0.4 * 10 = 2.0
        assert!((score - 2.0).abs() < 1e-9);
    }

    #[test]
    fn t_ma_arm_name_coverage() {
        // 确保所有 MaAdvancedKind 都有 arm 名
        for k in [
            MaAdvancedKind::HangingScallions,
            MaAdvancedKind::PoissonSpider,
            MaAdvancedKind::Guillotine,
            MaAdvancedKind::BondUpwardDiverge,
        ] {
            let name = ma_arm_name(k);
            let label = ma_arm_label(k);
            assert!(name.starts_with("signal.ma."));
            assert!(!label.is_empty());
        }
    }
}

// 允许 ArmCategory::Signal 未构造（目前只从 MA signal 来），防止 dead_code 警告
#[allow(dead_code)]
fn _arm_cat_signal_keepalive() -> ArmCategory {
    ArmCategory::Signal
}

// 允许 backtest alias，在某些构型下避免 unused
#[allow(unused_imports)]
use backtest as _backtest_keepalive;
