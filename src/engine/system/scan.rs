//! 预扫描器：一次性把所有组件在所有 bar 上的触发事件索引好
//!
//! 设计理由：各 engine 模块（`ma::granville`、`ma::scan_advanced`、`candle::scan`
//! 等）本来就是批量扫描接口，逐组件 wrapper 徒增开销。本模块一次跑完后，runner
//! 只需 `O(1)` 查表即可知道"在 bar i 上组件 X 有没有触发"。
//!
//! # 输出结构
//!
//! `ScanResult.triggers: HashMap<component_id, Vec<TriggerEvent>>`
//!
//! 按时间升序；runner 按 bar 索引做二分/线性扫描。
//!
//! # M2 实现范围（已完整覆盖 21 个 MVP 组件）
//!
//! - ✅ `ma.granville.b1/b2/b3/s1/s2`（5 个）
//! - ✅ `ma_advanced.hanging_scallions / guillotine`（2 个）
//! - ✅ `ma_special.bull_arrangement / bear_arrangement`（通过 MA 排列直接判断）
//! - ✅ `ma_special.golden_valley / death_valley`（短中长三均线金叉/死叉后三角结构）
//! - ✅ `ma_special.accelerating_up / accelerating_down`（MA20 斜率相对历史均值显著加速）
//! - ✅ `candle.*`（6 个）
//! - ✅ `trend.dow_uptrend / dow_downtrend`（基于 swing 的 HH/HL 结构）

use std::collections::HashMap;

use crate::data::Kline;
use crate::engine::candle::{self, PatternKind};
use crate::engine::chartpattern::{self, ChartPatternKind};
use crate::engine::ma::{self, GranvilleRule, MaAdvancedKind, MaAdvancedParams};
use crate::engine::trend::{dow, swing, DowPhase, SwingKind};

/// 单次触发事件
#[derive(Debug, Clone)]
pub struct TriggerEvent {
    pub bar_index: usize,
    /// +1 看多 / -1 看空
    pub direction: i8,
    /// [0, 1] 强度（M1 暂全部给 1.0）
    pub confidence: f64,
    /// 人类可读诊断串（调试 / 前端 tooltip 用）
    pub reason: String,
}

/// 扫描结果：每个组件 ID → 它在各根 K 线上触发的事件列表（按 bar_index 升序）
#[derive(Debug, Clone, Default)]
pub struct ScanResult {
    pub triggers: HashMap<&'static str, Vec<TriggerEvent>>,
    /// 预计算的 ATR（Wilder 平滑 14），runner 用于止损距离计算
    pub atr: Vec<f64>,
}

impl ScanResult {
    /// 查询某组件在给定 bar 上是否有触发
    pub fn get_trigger(&self, cid: &str, bar: usize) -> Option<&TriggerEvent> {
        self.triggers
            .get(cid)?
            .iter()
            .find(|e| e.bar_index == bar)
    }

    /// 查询某组件在整段上的触发总数（归因统计用）
    pub fn count(&self, cid: &str) -> usize {
        self.triggers.get(cid).map(|v| v.len()).unwrap_or(0)
    }
}

// ============================================================
// 主入口
// ============================================================

/// 扫描一段 K 线上所有 MVP 组件的触发事件
pub fn scan_all_triggers(klines: &[Kline]) -> ScanResult {
    let n = klines.len();
    if n == 0 {
        return ScanResult::default();
    }

    // 预计算基础序列
    let closes: Vec<f64> = klines.iter().map(|k| k.close).collect();
    let opens: Vec<f64> = klines.iter().map(|k| k.open).collect();
    let volumes: Vec<f64> = klines.iter().map(|k| k.volume).collect();

    let periods: [usize; 4] = [5, 10, 20, 60];
    let mas: Vec<Vec<f64>> = periods.iter().map(|&p| ma::sma(&closes, p)).collect();

    let mut triggers: HashMap<&'static str, Vec<TriggerEvent>> = HashMap::new();

    // -------- 1) 葛南维八法（基准 MA20） --------
    scan_granville(&closes, &mas, &periods, &mut triggers);

    // -------- 2) MA 高级形态（旱地拔葱 / 断头铡刀） --------
    scan_ma_advanced(&closes, &opens, &volumes, &mas, &periods, &mut triggers);

    // -------- 3) MA 排列（多头 / 空头） --------
    scan_ma_arrangement(&mas, n, &mut triggers);

    // -------- 4) 金山谷 / 死亡谷 / 加速上下行 --------
    scan_ma_special_extra(&closes, &mas, &mut triggers);

    // -------- 5) K 线形态 --------
    scan_candle_patterns(klines, &mut triggers);

    // -------- 6) 道氏趋势（HH/HL） --------
    scan_trend_dow(klines, &mut triggers);

    // -------- 7) 技术图形（头肩 / 双顶底 / 菱形顶 等） --------
    scan_chart_patterns(klines, &mut triggers);

    // 对每个组件的触发列表按 bar 升序（某些扫描可能乱序）
    for evs in triggers.values_mut() {
        evs.sort_by_key(|e| e.bar_index);
    }

    let atr = compute_atr(klines, 14);

    ScanResult { triggers, atr }
}

// ============================================================
// 各维度扫描
// ============================================================

fn scan_granville(
    closes: &[f64],
    mas: &[Vec<f64>],
    periods: &[usize],
    out: &mut HashMap<&'static str, Vec<TriggerEvent>>,
) {
    // 使用 MA20 作为葛南维基准（加密市场更适用；原书 60 日是 A 股场景）
    let Some(base_idx) = periods.iter().position(|&p| p == 20) else {
        return;
    };
    let base_ma = &mas[base_idx];
    let slope = ma::slope(base_ma, 5);
    let bias = ma::bias(closes, base_ma);

    let mut params = ma::granville::GranvilleParams::default();
    params.period = 20; // 本项目默认基准 MA20
    let signals = ma::granville::scan(closes, base_ma, &slope, &bias, &params);

    for s in signals {
        let (cid, dir) = match s.rule {
            GranvilleRule::B1BreakoutBuy => ("ma.granville.b1_breakout", 1i8),
            GranvilleRule::B2PullbackBuy => ("ma.granville.b2_pullback", 1i8),
            GranvilleRule::B3FalseBreakBuy => ("ma.granville.b3_false_break", 1i8),
            GranvilleRule::S1BreakdownSell => ("ma.granville.s1_breakdown", -1i8),
            GranvilleRule::S2ReboundSell => ("ma.granville.s2_rebound", -1i8),
            // 其他规则本 M1 未注册组件，忽略
            _ => continue,
        };
        out.entry(cid).or_default().push(TriggerEvent {
            bar_index: s.index,
            direction: dir,
            confidence: 1.0,
            reason: format!("葛南维 {}", s.rule.code()),
        });
    }
}

fn scan_ma_advanced(
    closes: &[f64],
    opens: &[f64],
    volumes: &[f64],
    mas: &[Vec<f64>],
    periods: &[usize],
    out: &mut HashMap<&'static str, Vec<TriggerEvent>>,
) {
    let events = ma::scan_advanced(
        closes,
        opens,
        volumes,
        mas,
        periods,
        &MaAdvancedParams::default(),
    );
    for e in events {
        let cid = match e.kind {
            MaAdvancedKind::HangingScallions => "ma_advanced.hanging_scallions",
            MaAdvancedKind::Guillotine => "ma_advanced.guillotine",
            // PoissonSpider / BondUpwardDiverge 在 M1 组件注册表中尚未单独列出，忽略
            _ => continue,
        };
        out.entry(cid).or_default().push(TriggerEvent {
            bar_index: e.index,
            direction: e.kind.direction(),
            confidence: 1.0,
            reason: e.kind.label().to_string(),
        });
    }
}

/// 扫描均线多头/空头排列
///
/// 多头排列：MA5 > MA10 > MA20 > MA60 且全部有限；空头相反。
/// 简化版：直接看 MA 值大小关系，无最小持续根数要求。
fn scan_ma_arrangement(
    mas: &[Vec<f64>],
    n: usize,
    out: &mut HashMap<&'static str, Vec<TriggerEvent>>,
) {
    if mas.len() < 4 {
        return;
    }
    for i in 0..n {
        let (m5, m10, m20, m60) = match (
            mas[0].get(i).copied(),
            mas[1].get(i).copied(),
            mas[2].get(i).copied(),
            mas[3].get(i).copied(),
        ) {
            (Some(a), Some(b), Some(c), Some(d))
                if a.is_finite() && b.is_finite() && c.is_finite() && d.is_finite() =>
            {
                (a, b, c, d)
            }
            _ => continue,
        };

        if m5 > m10 && m10 > m20 && m20 > m60 {
            out.entry("ma_special.bull_arrangement")
                .or_default()
                .push(TriggerEvent {
                    bar_index: i,
                    direction: 1,
                    confidence: 1.0,
                    reason: "MA5>MA10>MA20>MA60".into(),
                });
        } else if m5 < m10 && m10 < m20 && m20 < m60 {
            out.entry("ma_special.bear_arrangement")
                .or_default()
                .push(TriggerEvent {
                    bar_index: i,
                    direction: -1,
                    confidence: 1.0,
                    reason: "MA5<MA10<MA20<MA60".into(),
                });
        }
    }
}

/// 金山谷 / 死亡谷 / 加速上行 / 加速下行
///
/// 基于 MA5/MA10/MA20 的几何结构 + 斜率；逻辑简化自 `ma::special::scan_at`
/// 中的对应分支，避免调用大而全的 `scan_at`（那个函数参数繁杂且包含不必要的形态）。
fn scan_ma_special_extra(
    closes: &[f64],
    mas: &[Vec<f64>],
    out: &mut HashMap<&'static str, Vec<TriggerEvent>>,
) {
    if mas.len() < 4 {
        return;
    }
    let short = &mas[0]; // MA5
    let mid = &mas[1]; // MA10
    let long = &mas[2]; // MA20 —— 同时作为斜率评估对象
    let n = closes.len().min(long.len());

    // 预计算 MA20 斜率（单位：每根 K 线的价格变化）
    let slope_lookback = 5usize;
    let slope: Vec<f64> = (0..n)
        .map(|i| {
            if i < slope_lookback || !long[i].is_finite() || !long[i - slope_lookback].is_finite() {
                f64::NAN
            } else {
                (long[i] - long[i - slope_lookback]) / slope_lookback as f64
            }
        })
        .collect();

    // 预计算 "短线最近一次上穿/下穿长线" 的 bar 索引（用于金山谷/死亡谷的最近交叉判定）
    let mut last_cross_up: Vec<Option<usize>> = vec![None; n]; // 短上穿长
    let mut last_cross_dn: Vec<Option<usize>> = vec![None; n]; // 短下穿长
    for i in 1..n {
        let (sv, sv_prev, lv, lv_prev) = (short[i], short[i - 1], long[i], long[i - 1]);
        if !(sv.is_finite() && sv_prev.is_finite() && lv.is_finite() && lv_prev.is_finite()) {
            last_cross_up[i] = last_cross_up[i - 1];
            last_cross_dn[i] = last_cross_dn[i - 1];
            continue;
        }
        let up_cross = sv_prev <= lv_prev && sv > lv;
        let dn_cross = sv_prev >= lv_prev && sv < lv;
        last_cross_up[i] = if up_cross { Some(i) } else { last_cross_up[i - 1] };
        last_cross_dn[i] = if dn_cross { Some(i) } else { last_cross_dn[i - 1] };
    }

    const RECENT_BARS: usize = 10;
    const ACCEL_FACTOR: f64 = 2.0;

    for i in 0..n {
        let (sv, mv, lv) = match (short.get(i).copied(), mid.get(i).copied(), long.get(i).copied())
        {
            (Some(a), Some(b), Some(c)) if a.is_finite() && b.is_finite() && c.is_finite() => (a, b, c),
            _ => continue,
        };

        // 金山谷：MA5 > MA10 > MA20 + 最近 RECENT_BARS 根内短上穿长
        if sv > mv && mv > lv {
            if let Some(cross_bar) = last_cross_up[i] {
                if i.saturating_sub(cross_bar) <= RECENT_BARS {
                    out.entry("ma_special.golden_valley").or_default().push(TriggerEvent {
                        bar_index: i,
                        direction: 1,
                        confidence: 1.0,
                        reason: format!("MA5>MA10>MA20 且近期上穿 @bar{}", cross_bar),
                    });
                }
            }
        }

        // 死亡谷：MA5 < MA10 < MA20 + 最近 RECENT_BARS 根内短下穿长
        if sv < mv && mv < lv {
            if let Some(cross_bar) = last_cross_dn[i] {
                if i.saturating_sub(cross_bar) <= RECENT_BARS {
                    out.entry("ma_special.death_valley").or_default().push(TriggerEvent {
                        bar_index: i,
                        direction: -1,
                        confidence: 1.0,
                        reason: format!("MA5<MA10<MA20 且近期下穿 @bar{}", cross_bar),
                    });
                }
            }
        }

        // 加速上行 / 加速下行：MA20 斜率相对近 20 根斜率均值（绝对值）的 ACCEL_FACTOR 倍
        let s = slope[i];
        if !s.is_finite() {
            continue;
        }
        let lookback = 20.min(i + 1);
        let recent_slopes: Vec<f64> =
            (i + 1 - lookback..=i).filter_map(|j| slope.get(j).copied().filter(|v| v.is_finite())).collect();
        if recent_slopes.len() < 5 {
            continue;
        }
        let avg_abs_slope: f64 =
            recent_slopes.iter().map(|v| v.abs()).sum::<f64>() / recent_slopes.len() as f64;
        if avg_abs_slope < 1e-9 {
            continue;
        }

        // 必须同时满足多头/空头排列（避免盘整时误触）
        let bull_aligned = sv > mv && mv > lv;
        let bear_aligned = sv < mv && mv < lv;
        if bull_aligned && s > 0.0 && s > avg_abs_slope * ACCEL_FACTOR {
            out.entry("ma_special.accelerating_up").or_default().push(TriggerEvent {
                bar_index: i,
                direction: 1,
                confidence: (s / (avg_abs_slope * ACCEL_FACTOR)).min(2.0) / 2.0,
                reason: format!("MA20 斜率 {:.4} > {:.1}×近 20 根均值", s, ACCEL_FACTOR),
            });
        } else if bear_aligned && s < 0.0 && s.abs() > avg_abs_slope * ACCEL_FACTOR {
            out.entry("ma_special.accelerating_down").or_default().push(TriggerEvent {
                bar_index: i,
                direction: -1,
                confidence: (s.abs() / (avg_abs_slope * ACCEL_FACTOR)).min(2.0) / 2.0,
                reason: format!("MA20 斜率 {:.4} < -{:.1}×近 20 根均值", s, ACCEL_FACTOR),
            });
        }
    }
}

/// 道氏趋势（HH/HL = 上升 / LH/LL = 下降）
///
/// 一次性扫描 swing points，然后对每根 bar 截取"截止该 bar 的最近 4-6 个 swing"
/// 调用 [`dow::classify`]。通过"指针跟踪"避免 O(n²)。
fn scan_trend_dow(klines: &[Kline], out: &mut HashMap<&'static str, Vec<TriggerEvent>>) {
    let n = klines.len();
    if n == 0 {
        return;
    }
    let swings = swing::detect(klines, &swing::SwingParams::default());
    if swings.is_empty() {
        return;
    }

    // 按 index 升序
    let mut sorted_swings = swings.clone();
    sorted_swings.sort_by_key(|s| s.index);

    // 滑动窗口：保留最后 6 个（含 2 高 + 2 低 即可）
    let mut window: Vec<swing::SwingPoint> = Vec::with_capacity(6);
    let mut swing_ptr = 0usize;

    for i in 0..n {
        // 吸收所有 index <= i 的 swing
        while swing_ptr < sorted_swings.len() && sorted_swings[swing_ptr].index <= i {
            window.push(sorted_swings[swing_ptr]);
            if window.len() > 6 {
                window.remove(0);
            }
            swing_ptr += 1;
        }
        if window.len() < 4 {
            continue;
        }
        // 取其中最后的 4-6 个 swing 做分类
        let state = dow::classify(&window, i);
        match state.phase {
            DowPhase::Uptrend => {
                out.entry("trend.dow_uptrend").or_default().push(TriggerEvent {
                    bar_index: i,
                    direction: 1,
                    confidence: 1.0,
                    reason: "HH + HL 结构".into(),
                });
            }
            DowPhase::Downtrend => {
                out.entry("trend.dow_downtrend").or_default().push(TriggerEvent {
                    bar_index: i,
                    direction: -1,
                    confidence: 1.0,
                    reason: "LH + LL 结构".into(),
                });
            }
            _ => {}
        }
    }
    // 抑制未用变量警告
    let _ = SwingKind::High;
}

fn scan_candle_patterns(
    klines: &[Kline],
    out: &mut HashMap<&'static str, Vec<TriggerEvent>>,
) {
    let hits = candle::scan(klines);
    for h in hits {
        let cid: Option<&'static str> = match h.kind {
            // M1 6 个
            PatternKind::BullishEngulfing => Some("candle.bullish_engulfing"),
            PatternKind::BearishEngulfing => Some("candle.bearish_engulfing"),
            PatternKind::MorningStar => Some("candle.morning_star"),
            PatternKind::EveningStar => Some("candle.evening_star"),
            PatternKind::ThreeWhiteSoldiers => Some("candle.three_white_soldiers"),
            PatternKind::ThreeBlackCrows => Some("candle.three_black_crows"),
            // M3 新增 6 个
            PatternKind::PiercingLine => Some("candle.piercing_line"),
            PatternKind::DarkCloudCover => Some("candle.dark_cloud_cover"),
            PatternKind::TweezersBottom => Some("candle.tweezers_bottom"),
            PatternKind::TweezersTop => Some("candle.tweezers_top"),
            PatternKind::CloseMarubozuBull => Some("candle.close_marubozu_bull"),
            PatternKind::CloseMarubozuBear => Some("candle.close_marubozu_bear"),
            _ => None,
        };
        if let Some(id) = cid {
            out.entry(id).or_default().push(TriggerEvent {
                bar_index: h.index,
                direction: h.direction,
                confidence: 1.0,
                reason: h.kind.label().to_string(),
            });
        }
    }
}

/// 技术图形（头肩、双顶底、菱形顶等）
///
/// 使用 [`chartpattern::detect_all`] 批量识别，按 `ChartPatternKind`
/// 映射到注册的 5 个组件 ID。触发发生在图形的 `completion_index`（颈线突破根）。
fn scan_chart_patterns(
    klines: &[Kline],
    out: &mut HashMap<&'static str, Vec<TriggerEvent>>,
) {
    let patterns = chartpattern::detect_all(klines);
    for p in patterns {
        let cid: Option<&'static str> = match p.kind {
            ChartPatternKind::HeadAndShoulders => Some("chart.head_and_shoulders_top"),
            ChartPatternKind::InverseHeadAndShoulders => Some("chart.head_and_shoulders_bottom"),
            ChartPatternKind::DoubleTop => Some("chart.double_top"),
            ChartPatternKind::DoubleBottom => Some("chart.double_bottom"),
            ChartPatternKind::DiamondTop => Some("chart.diamond_top"),
            _ => None,
        };
        if let Some(id) = cid {
            out.entry(id).or_default().push(TriggerEvent {
                bar_index: p.completion_index,
                direction: p.direction,
                confidence: 1.0,
                reason: p.label.clone(),
            });
        }
    }
}

// ============================================================
// ATR（Wilder 14，供 runner 计算止损距离）
// ============================================================

fn compute_atr(klines: &[Kline], period: usize) -> Vec<f64> {
    let n = klines.len();
    let mut atr = vec![f64::NAN; n];
    if n == 0 || period == 0 {
        return atr;
    }
    let mut trs: Vec<f64> = Vec::with_capacity(n);
    for i in 0..n {
        let k = &klines[i];
        let tr = if i == 0 {
            k.high - k.low
        } else {
            let pc = klines[i - 1].close;
            (k.high - k.low).max((k.high - pc).abs()).max((k.low - pc).abs())
        };
        trs.push(tr);
    }
    // Wilder 平滑：第一个 ATR = 前 period 个 TR 的均值；之后 ATR_i = (ATR_{i-1}*(p-1) + TR_i)/p
    if n < period {
        return atr;
    }
    let seed: f64 = trs[..period].iter().sum::<f64>() / period as f64;
    atr[period - 1] = seed;
    for i in period..n {
        atr[i] = (atr[i - 1] * (period as f64 - 1.0) + trs[i]) / period as f64;
    }
    atr
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

    fn flat_bars(n: usize, price: f64) -> Vec<Kline> {
        (0..n)
            .map(|i| mk_kline(i as i64 * 60_000, price, price + 0.2, price - 0.2, price, 1000.0))
            .collect()
    }

    #[test]
    fn t_empty_input() {
        let r = scan_all_triggers(&[]);
        assert!(r.triggers.is_empty());
        assert!(r.atr.is_empty());
    }

    #[test]
    fn t_flat_market_no_triggers() {
        // 完全平盘 → 关键趋势/葛南维/图形组件不应触发
        // （镊子底/顶这类纯"相等 low/high" 的形态在全平数据上可能命中，属于识别器设计，不在本测试范围）
        let klines = flat_bars(150, 100.0);
        let r = scan_all_triggers(&klines);
        let critical_ids = [
            "ma.granville.b1_breakout",
            "ma.granville.b2_pullback",
            "ma.granville.s1_breakdown",
            "ma_special.bull_arrangement",
            "ma_special.bear_arrangement",
            "ma_special.golden_valley",
            "ma_special.death_valley",
            "ma_special.accelerating_up",
            "ma_special.accelerating_down",
            "trend.dow_uptrend",
            "trend.dow_downtrend",
            "chart.head_and_shoulders_top",
            "chart.double_top",
        ];
        for id in critical_ids {
            assert_eq!(r.count(id), 0, "平盘 → {} 不应触发", id);
        }
    }

    #[test]
    fn t_uptrend_produces_bull_arrangement() {
        // 构造稳定上涨 200 根 → 多头排列应触发
        let klines: Vec<Kline> = (0..200)
            .map(|i| {
                let p = 100.0 + i as f64 * 0.5;
                mk_kline(i as i64 * 60_000, p, p + 0.4, p - 0.2, p + 0.2, 1000.0)
            })
            .collect();
        let r = scan_all_triggers(&klines);
        let bull = r.count("ma_special.bull_arrangement");
        assert!(bull > 100, "稳定上涨应产生大量多头排列触发，实际 {}", bull);
        // 空头排列应为 0
        assert_eq!(r.count("ma_special.bear_arrangement"), 0);
    }

    #[test]
    fn t_atr_computed_for_sufficient_data() {
        let klines = flat_bars(50, 100.0);
        let r = scan_all_triggers(&klines);
        assert_eq!(r.atr.len(), 50);
        // 平盘 ATR 应很小（只剩 high-low = 0.4）
        assert!(r.atr[20].is_finite());
        assert!((r.atr[20] - 0.4).abs() < 0.1);
    }

    #[test]
    fn t_triggers_sorted_by_bar() {
        let klines: Vec<Kline> = (0..200)
            .map(|i| {
                let p = 100.0 + (i as f64).sin() * 5.0 + i as f64 * 0.1;
                mk_kline(i as i64 * 60_000, p, p + 0.5, p - 0.5, p, 1000.0)
            })
            .collect();
        let r = scan_all_triggers(&klines);
        for (cid, events) in &r.triggers {
            for w in events.windows(2) {
                assert!(
                    w[0].bar_index <= w[1].bar_index,
                    "{} 触发序列未排序: {} vs {}",
                    cid,
                    w[0].bar_index,
                    w[1].bar_index
                );
            }
        }
    }
}
