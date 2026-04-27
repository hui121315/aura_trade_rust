//! 贪心搜索自动发现 Top-K 高尤组合（M6）
//!
//! # 问题规模
//!
//! 从 32 个组件中选 2..5 个 × 2 种聚合规则 ≈ 50 万候选体系。完整枚举
//! 不现实，因此采用 **Beam Search**：
//!
//! - Stage 1：用所有单组件跑，保留 Top-K
//! - Stage k：在 Stage k-1 的 Top-K 基础上，贪心添加一个组件
//! - 每个体系在 Stage 里用**单次回测 Sharpe** 排序（快）
//! - 最终从所有 Stage 的 Top-K 汇总中挑前 K，用 **Walk-Forward**
//!   做精排（`wf_avg_sharpe × consistency_ratio`）
//!
//! # 设计原则
//!
//! - **方向一致性**：所有组件必须同向（`direction_bias` 相同），否则
//!   `AllAligned` 永不触发
//! - **去重**：组合按集合比较，不关心顺序
//! - **可重现**：通过固定组件注册表的顺序保证结果确定

use std::collections::HashSet;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Instant;

use rayon::prelude::*;

use serde::{Deserialize, Serialize};

use crate::data::Kline;
use crate::engine::system::{
    all_components, run_walkforward, BacktestParams, CombineRule, Component, CostModel, RiskParams,
    SystemDefinition, SystemMeta, SystemOrigin, WalkForwardConfig,
};

use super::runner;

// ============================================================
// 输入 / 输出
// ============================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoveryConfig {
    /// +1 搜做多体系 / -1 搜做空体系
    pub direction: i8,
    /// 组合大小范围：`[min_size..=max_size]`；单组件太弱故默认从 2 开始
    #[serde(default = "default_min_size")]
    pub min_size: usize,
    pub max_size: usize,
    /// 每个 stage 保留的 beam 宽度
    #[serde(default = "default_top_k")]
    pub top_k: usize,
    /// walk-forward 精排折数（0 表示跳过 WF 精排，仅按单次回测排序）
    #[serde(default = "default_wf_folds")]
    pub wf_folds: usize,
    /// 是否启用 `MajorityK` 规则（大小 ≥ 3 时才启用）
    #[serde(default = "default_enable_majority")]
    pub enable_majority: bool,
    /// 是否启用 `AllAligned` 规则
    #[serde(default = "default_enable_all_aligned")]
    pub enable_all_aligned: bool,
}

fn default_min_size() -> usize { 2 }
fn default_top_k() -> usize { 10 }
fn default_wf_folds() -> usize { 4 }
fn default_enable_majority() -> bool { true }
fn default_enable_all_aligned() -> bool { true }

impl Default for DiscoveryConfig {
    fn default() -> Self {
        Self {
            direction: 1,
            min_size: 2,
            max_size: 3,
            top_k: 10,
            wf_folds: 4,
            enable_majority: true,
            enable_all_aligned: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoveryCandidate {
    pub rank: usize,
    pub definition: SystemDefinition,
    /// 单次回测指标
    pub single_sharpe: f64,
    pub single_return_pct: f64,
    pub single_trades: usize,
    pub single_max_dd_pct: f64,
    /// Walk-forward 指标（若 `wf_folds=0` 则全 NaN）
    pub wf_consistency: f64,
    pub wf_avg_sharpe: f64,
    pub wf_sharpe_std: f64,
    pub wf_avg_return_pct: f64,
    /// 交叉 symbols 上的 WF 结果（按输入顺序）
    #[serde(default)]
    pub cross_validation: Vec<CrossValidationResult>,
    /// 所有 symbol（主 + cross）的 WF Sharpe 均值
    pub cross_sharpe_mean: f64,
    /// 所有 symbol（主 + cross）的 WF consistency 均值
    pub cross_consistency_mean: f64,
    /// 综合评分：
    /// - 有 cross 时：`mean(wf_sharpe × consistency across symbols)`（惩罚单 symbol 过拟合）
    /// - 仅主 symbol：`wf_avg_sharpe × consistency_ratio`
    /// - 无 WF：`single_sharpe`
    pub composite_score: f64,
}

/// 单个 (symbol, interval) 验证点的 WF 汇总
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrossValidationResult {
    pub symbol: String,
    /// M11：验证点的周期（可能不同于主周期）
    #[serde(default = "default_interval_field")]
    pub interval: String,
    pub wf_consistency: f64,
    pub wf_avg_sharpe: f64,
    pub wf_sharpe_std: f64,
    pub wf_avg_return_pct: f64,
    pub total_trades: usize,
}

fn default_interval_field() -> String {
    "1d".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoveryReport {
    pub symbol: String,
    pub interval: String,
    pub config: DiscoveryConfig,
    pub total_combinations_tried: usize,
    pub elapsed_ms: u128,
    pub top_k: Vec<DiscoveryCandidate>,
}

// ============================================================
// 核心算法
// ============================================================

/// 运行 Discovery。
///
/// - `klines` / `symbol` / `interval`：主市场/主周期，用于贪心搜索和精排
/// - `cross`：额外的 **(symbol, interval, klines)** 验证点（可为空）。
///   包括：跨 symbol 相同周期、跨周期相同 symbol、或任意组合。
///   composite_score 用所有验证点均值，惩罚单点过拟合。
pub fn discover(
    klines: &[Kline],
    symbol: &str,
    interval: &str,
    cfg: &DiscoveryConfig,
    cross: &[(&str, &str, &[Kline])],
) -> Result<DiscoveryReport, String> {
    // 参数校验
    if cfg.direction != 1 && cfg.direction != -1 {
        return Err(format!("direction 必须 +1 或 -1，实际 {}", cfg.direction));
    }
    if cfg.min_size < 2 || cfg.max_size > 5 || cfg.min_size > cfg.max_size {
        return Err(format!(
            "组合大小范围非法：min={} max={}（要求 2 ≤ min ≤ max ≤ 5）",
            cfg.min_size, cfg.max_size,
        ));
    }
    if cfg.top_k == 0 || cfg.top_k > 50 {
        return Err(format!("top_k 必须在 1..=50，实际 {}", cfg.top_k));
    }
    if !(cfg.enable_all_aligned || cfg.enable_majority) {
        return Err("至少启用一种聚合规则".into());
    }

    let t0 = Instant::now();

    // 过滤同向组件
    let candidates: Vec<&'static Component> = all_components()
        .iter()
        .filter(|c| c.direction_bias == cfg.direction)
        .collect();
    if candidates.len() < cfg.min_size {
        return Err(format!(
            "同方向组件仅 {} 个，不足 min_size={}",
            candidates.len(),
            cfg.min_size,
        ));
    }

    let total_tried = AtomicUsize::new(0);

    // Stage 1: 单组件种子（只用于扩展，不直接入 Top-K）
    let mut beam: Vec<Scored> = candidates
        .iter()
        .map(|c| Scored {
            ids: vec![c.id.to_string()],
            rule: None,
            sharpe: 0.0,
            total_return_pct: 0.0,
            trades: 0,
            max_dd: 0.0,
        })
        .collect();

    // 收集所有有效候选（不同大小都参与最终排序）
    let mut harvest: Vec<Scored> = Vec::new();

    // Stage 2..=max_size: 扩展
    for size in cfg.min_size..=cfg.max_size {
        // M8: 先**串行**生成去重后的任务列表（快），再**并行**跑 runner::run
        let base_beam: Vec<Vec<String>> =
            beam.iter().map(|s| s.ids.clone()).collect();

        let mut tasks: Vec<(Vec<String>, CombineRule)> = Vec::new();
        let mut seen: HashSet<Vec<String>> = HashSet::new();
        for base in &base_beam {
            for add_c in &candidates {
                if base.iter().any(|x| x == add_c.id) {
                    continue;
                }
                let mut new_ids: Vec<String> =
                    base.iter().cloned().chain(std::iter::once(add_c.id.to_string())).collect();
                new_ids.sort();
                if new_ids.len() != size {
                    continue;
                }
                if !seen.insert(new_ids.clone()) {
                    continue;
                }
                for rule in applicable_rules(new_ids.len(), cfg) {
                    tasks.push((new_ids.clone(), rule));
                }
            }
        }

        // 并行跑所有 runner::run
        let stage_results: Vec<Scored> = tasks
            .par_iter()
            .filter_map(|(ids, rule)| {
                let def = build_definition(ids, rule, cfg.direction);
                total_tried.fetch_add(1, Ordering::Relaxed);
                match runner::run(&def, klines, symbol, interval) {
                    Ok(r) if r.performance.total_trades > 0 => Some(Scored {
                        ids: ids.clone(),
                        rule: Some(rule.clone()),
                        sharpe: r.performance.sharpe,
                        total_return_pct: r.performance.total_return_pct,
                        trades: r.performance.total_trades,
                        max_dd: r.performance.max_drawdown_pct,
                    }),
                    _ => None,
                }
            })
            .collect();

        // harvest（跨所有 size） + new_beam（本 size 剪枝）
        harvest.extend(stage_results.iter().cloned());
        let mut new_beam = stage_results;
        new_beam.sort_by(|a, b| {
            b.sharpe.partial_cmp(&a.sharpe).unwrap_or(std::cmp::Ordering::Equal)
        });
        new_beam.truncate(cfg.top_k);
        beam = new_beam;
    }

    let total_tried = total_tried.load(Ordering::Relaxed);

    // 从 harvest 中按 sharpe 降序取 top_k（跨所有 size）
    harvest.sort_by(|a, b| b.sharpe.partial_cmp(&a.sharpe).unwrap_or(std::cmp::Ordering::Equal));
    let mut finalists: Vec<Scored> = Vec::new();
    let mut seen_final: HashSet<Vec<String>> = HashSet::new();
    for s in harvest {
        let mut key = s.ids.clone();
        key.push(rule_tag(&s.rule));
        if seen_final.insert(key) {
            finalists.push(s);
            if finalists.len() >= cfg.top_k {
                break;
            }
        }
    }

    // Walk-forward 精排（可选）。M8：finalists 之间独立，全部并行跑
    let mut results: Vec<DiscoveryCandidate> = finalists
        .par_iter()
        .enumerate()
        .map(|(i, s)| {
            let def = build_definition(&s.ids, s.rule.as_ref().unwrap(), cfg.direction);
            // 主 symbol WF（内部已并行 folds）
            let (wf_cons, wf_sh, wf_std, wf_ret) = if cfg.wf_folds >= 2 {
                let wf_cfg = WalkForwardConfig { folds: cfg.wf_folds, prewarm_bars: 0 };
                match run_walkforward(&def, klines, symbol, interval, &wf_cfg) {
                    Ok(r) => (
                        r.aggregate.consistency_ratio,
                        r.aggregate.avg_sharpe,
                        r.aggregate.sharpe_std,
                        r.aggregate.avg_return_pct,
                    ),
                    Err(_) => (f64::NAN, f64::NAN, f64::NAN, f64::NAN),
                }
            } else {
                (f64::NAN, f64::NAN, f64::NAN, f64::NAN)
            };

            // 交叉 (symbol, interval) 验证（每个验证点之间独立，可继续并行）
            let cross_results: Vec<CrossValidationResult> = if cfg.wf_folds >= 2 {
                cross
                    .par_iter()
                    .map(|(csym, ctf, ckl)| {
                        let wf_cfg = WalkForwardConfig {
                            folds: cfg.wf_folds,
                            prewarm_bars: 0,
                        };
                        match run_walkforward(&def, ckl, csym, ctf, &wf_cfg) {
                            Ok(r) => CrossValidationResult {
                                symbol: csym.to_string(),
                                interval: ctf.to_string(),
                                wf_consistency: r.aggregate.consistency_ratio,
                                wf_avg_sharpe: r.aggregate.avg_sharpe,
                                wf_sharpe_std: r.aggregate.sharpe_std,
                                wf_avg_return_pct: r.aggregate.avg_return_pct,
                                total_trades: r.aggregate.total_trades,
                            },
                            Err(_) => CrossValidationResult {
                                symbol: csym.to_string(),
                                interval: ctf.to_string(),
                                wf_consistency: f64::NAN,
                                wf_avg_sharpe: f64::NAN,
                                wf_sharpe_std: f64::NAN,
                                wf_avg_return_pct: f64::NAN,
                                total_trades: 0,
                            },
                        }
                    })
                    .collect()
            } else {
                Vec::new()
            };

            let (cross_sh_mean, cross_cons_mean, composite) =
                compute_composite(wf_sh, wf_cons, &cross_results, s.sharpe);

            DiscoveryCandidate {
                rank: i + 1,
                definition: def,
                single_sharpe: s.sharpe,
                single_return_pct: s.total_return_pct,
                single_trades: s.trades,
                single_max_dd_pct: s.max_dd,
                wf_consistency: wf_cons,
                wf_avg_sharpe: wf_sh,
                wf_sharpe_std: wf_std,
                wf_avg_return_pct: wf_ret,
                cross_validation: cross_results,
                cross_sharpe_mean: cross_sh_mean,
                cross_consistency_mean: cross_cons_mean,
                composite_score: composite,
            }
        })
        .collect();

    // 按 composite_score 重排
    results.sort_by(|a, b| {
        b.composite_score
            .partial_cmp(&a.composite_score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    for (i, r) in results.iter_mut().enumerate() {
        r.rank = i + 1;
    }

    Ok(DiscoveryReport {
        symbol: symbol.to_string(),
        interval: interval.to_string(),
        config: cfg.clone(),
        total_combinations_tried: total_tried,
        elapsed_ms: t0.elapsed().as_millis(),
        top_k: results,
    })
}

// ============================================================
// 辅助
// ============================================================

#[derive(Debug, Clone)]
struct Scored {
    ids: Vec<String>,
    rule: Option<CombineRule>,
    sharpe: f64,
    total_return_pct: f64,
    trades: usize,
    max_dd: f64,
}

/// 根据主 symbol + cross symbols 的 WF 指标计算综合评分。
///
/// 返回 `(cross_sharpe_mean, cross_consistency_mean, composite_score)`。
///
/// 规则：
/// - 有 cross 时：只把**有效**（non-NaN）的指标纳入均值；composite =
///   `mean(sharpe_i × cons_i)`，惩罚单 symbol 过拟合
/// - 仅主 symbol：`composite = wf_sh × wf_cons`
/// - 无 WF（wf_sh 为 NaN）：`composite = single_sharpe`
fn compute_composite(
    wf_sh: f64,
    wf_cons: f64,
    cross: &[CrossValidationResult],
    single_sharpe: f64,
) -> (f64, f64, f64) {
    let mut products: Vec<f64> = Vec::new();
    let mut sharpes: Vec<f64> = Vec::new();
    let mut conss: Vec<f64> = Vec::new();
    if wf_sh.is_finite() && wf_cons.is_finite() {
        products.push(wf_sh * wf_cons);
        sharpes.push(wf_sh);
        conss.push(wf_cons);
    }
    for c in cross {
        if c.wf_avg_sharpe.is_finite() && c.wf_consistency.is_finite() {
            products.push(c.wf_avg_sharpe * c.wf_consistency);
            sharpes.push(c.wf_avg_sharpe);
            conss.push(c.wf_consistency);
        }
    }
    if products.is_empty() {
        return (f64::NAN, f64::NAN, single_sharpe);
    }
    let composite = products.iter().sum::<f64>() / products.len() as f64;
    let sh_mean = sharpes.iter().sum::<f64>() / sharpes.len() as f64;
    let cons_mean = conss.iter().sum::<f64>() / conss.len() as f64;
    (sh_mean, cons_mean, composite)
}

fn rule_tag(rule: &Option<CombineRule>) -> String {
    match rule {
        Some(CombineRule::AllAligned) => "AA".into(),
        Some(CombineRule::MajorityK { k }) => format!("MK{}", k),
        Some(CombineRule::WeightedScore { threshold }) => format!("WS{:.2}", threshold),
        Some(CombineRule::SequentialCascade { window_bars }) => format!("SC{}", window_bars),
        None => "-".into(),
    }
}

fn applicable_rules(size: usize, cfg: &DiscoveryConfig) -> Vec<CombineRule> {
    let mut out = Vec::new();
    if cfg.enable_all_aligned {
        out.push(CombineRule::AllAligned);
    }
    if cfg.enable_majority && size >= 3 {
        // MajorityK 只在 size≥3 才有意义（size=2 时 k=2 等同 AllAligned；k=1 太宽松）
        let k = (size + 1) / 2; // 多数派：2/3, 3/5...
        out.push(CombineRule::MajorityK { k });
    }
    out
}

fn build_definition(ids: &[String], rule: &CombineRule, direction: i8) -> SystemDefinition {
    let id = format!("discovered.{}", ids.join("+"));
    let name = format!(
        "Discovered [{}] {}",
        if direction > 0 { "Long" } else { "Short" },
        ids.join(" + "),
    );
    SystemDefinition {
        id,
        name,
        origin: SystemOrigin::Discovered,
        description: Some(format!(
            "自动发现的 {} 组件体系（direction={}, rule={:?}）",
            ids.len(),
            direction,
            rule,
        )),
        components: ids.to_vec(),
        combine: rule.clone(),
        weights: Default::default(),
        risk: RiskParams {
            stop_atr_mult: 2.0,
            target_r: 3.0,
            max_hold_bars: 30,
            max_position_pct: 0.5,
        },
        backtest: BacktestParams {
            warmup_bars: 60,
            cost_model: CostModel::default(),
        },
        meta: SystemMeta { schema_version: 1, ..Default::default() },
    }
}

// ============================================================
// 测试
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::Kline;

    fn mk_kline(t: i64, o: f64, h: f64, l: f64, c: f64) -> Kline {
        Kline {
            open_time: t,
            close_time: t + 60_000,
            open: o,
            high: h,
            low: l,
            close: c,
            volume: 1000.0,
        }
    }

    fn synthetic_uptrend(n: usize) -> Vec<Kline> {
        (0..n)
            .map(|i| {
                let p = 100.0 + i as f64 * 0.3;
                mk_kline(
                    i as i64 * 86_400_000,
                    p,
                    p + 0.6,
                    p - 0.2,
                    p + ((i as f64 * 0.1) % 0.5),
                )
            })
            .collect()
    }

    #[test]
    fn t_discovery_basic_long() {
        let klines = synthetic_uptrend(1200);
        let cfg = DiscoveryConfig {
            direction: 1,
            min_size: 2,
            max_size: 3,
            top_k: 5,
            wf_folds: 0, // 跳过 WF 让测试快
            enable_majority: true,
            enable_all_aligned: true,
        };
        let r = discover(&klines, "TEST", "1d", &cfg, &[]).expect("ok");
        assert!(!r.top_k.is_empty(), "至少应找到一些组合");
        assert!(r.top_k.len() <= 5);
        // 单调上涨合成数据：多头组件组合 sharpe 大多为正
        let pos_count = r.top_k.iter().filter(|c| c.single_sharpe > 0.0).count();
        assert!(pos_count >= 1);
        // rank 递增
        for (i, c) in r.top_k.iter().enumerate() {
            assert_eq!(c.rank, i + 1);
        }
    }

    #[test]
    fn t_discovery_rejects_bad_config() {
        let klines = synthetic_uptrend(500);
        let bad = DiscoveryConfig {
            direction: 0, // 非法
            ..DiscoveryConfig::default()
        };
        assert!(discover(&klines, "T", "1d", &bad, &[]).is_err());

        let bad2 = DiscoveryConfig {
            direction: 1,
            min_size: 3,
            max_size: 2, // min > max
            ..DiscoveryConfig::default()
        };
        assert!(discover(&klines, "T", "1d", &bad2, &[]).is_err());

        let bad3 = DiscoveryConfig {
            direction: 1,
            enable_all_aligned: false,
            enable_majority: false,
            ..DiscoveryConfig::default()
        };
        assert!(discover(&klines, "T", "1d", &bad3, &[]).is_err());
    }

    #[test]
    fn t_discovery_with_cross_validation() {
        let klines = synthetic_uptrend(1200);
        let cross1 = synthetic_uptrend(1200);
        let cross2 = synthetic_uptrend(1200);
        let cfg = DiscoveryConfig {
            direction: 1,
            min_size: 2,
            max_size: 2,
            top_k: 3,
            wf_folds: 4,
            enable_majority: false,
            enable_all_aligned: true,
        };
        let r = discover(
            &klines,
            "BTC",
            "1d",
            &cfg,
            &[("ETH", "1d", cross1.as_slice()), ("SOL", "1d", cross2.as_slice())],
        )
        .expect("ok");
        for cand in &r.top_k {
            // 每个候选都应该有 2 个 cross 结果
            assert_eq!(cand.cross_validation.len(), 2);
            assert_eq!(cand.cross_validation[0].symbol, "ETH");
            assert_eq!(cand.cross_validation[0].interval, "1d");
            assert_eq!(cand.cross_validation[1].symbol, "SOL");
            assert_eq!(cand.cross_validation[1].interval, "1d");
            // cross_sharpe_mean 和 cross_consistency_mean 应是有效数
            assert!(cand.cross_sharpe_mean.is_finite());
            assert!(cand.cross_consistency_mean.is_finite());
            assert!(cand.composite_score.is_finite());
        }
    }

    #[test]
    fn t_discovery_all_components_same_direction() {
        // 确保搜到的所有组件都同向
        let klines = synthetic_uptrend(800);
        let cfg = DiscoveryConfig {
            direction: 1,
            min_size: 2,
            max_size: 2,
            top_k: 5,
            wf_folds: 0,
            enable_majority: false,
            enable_all_aligned: true,
        };
        let r = discover(&klines, "T", "1d", &cfg, &[]).expect("ok");
        for cand in &r.top_k {
            for cid in &cand.definition.components {
                let c = crate::engine::system::find_component(cid).unwrap();
                assert_eq!(c.direction_bias, 1, "组件 {} 方向不一致", cid);
            }
        }
    }
}
