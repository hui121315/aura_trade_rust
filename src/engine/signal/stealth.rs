//! F5：主力潜伏突破 + 通道穿头破脚（R-P1-30 / R-P1-31）
//!
//! # R-P1-30 主力潜伏式突破（StealthBreakout）
//!
//! 原书 **trend p.274** 附近：
//! > "主力利用**小阳线缩量**的方式隐蔽地进行吸筹 + 突破，避免引起公众跟风。
//! > 股价在小阳线缩量中不断走高，常常是真突破的前兆。"
//!
//! 与常规"放量大阳线突破"不同：
//! - **缩量**（非放量）
//! - **小阳线**（涨幅 < 2%）
//! - **连续 N 根**持续新高
//!
//! # R-P1-31 通道穿头破脚（ChannelPiercing）
//!
//! 原书 **trend p.260**：上升通道末端，**价格短暂穿越通道上沿（"穿头"）
//! 然后跌破通道下沿（"破脚"）= 反转最强信号**。
//!
//! - "穿头"：向上穿越上轨 < N 根内
//! - "破脚"：随后向下跌破下轨

use serde::{Deserialize, Serialize};

use crate::data::Kline;
use crate::engine::chartpattern::MarketMakerBehavior;

/// 主力潜伏突破事件
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StealthBreakoutEvent {
    /// 突破确认的 K 线索引
    pub index: usize,
    /// 连续小阳线的根数
    pub small_bull_count: usize,
    /// 成交量缩减比率（后段均量 / 前段均量）
    pub volume_shrink_ratio: f64,
    /// 累计涨幅
    pub cumulative_rise_pct: f64,
}

impl StealthBreakoutEvent {
    /// 主力行为学分类（原书 trend p.274：潜伏式吸筹 = 主力 Stealth 行为）
    pub fn market_maker_behavior(&self) -> MarketMakerBehavior {
        MarketMakerBehavior::Stealth
    }
}

/// 通道穿头破脚事件
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelPiercingEvent {
    /// "穿头"的 K 线索引
    pub pierce_top_index: usize,
    /// "破脚"的 K 线索引
    pub pierce_bottom_index: usize,
    /// 通道上沿价
    pub upper_line: f64,
    /// 通道下沿价
    pub lower_line: f64,
}

/// 主力恐慌抛售事件（R-P1-58）
///
/// 原书 **ma p.310** 附近："均线粘合后瞬间跌破 + 成交量暴增 = 主力恐慌出逃"
///
/// 与 `MaAdvancedKind::Guillotine`（断头铡刀）的区别：
/// - Guillotine：再次粘合向下发散（含 60 日），属于"趋势性空头"
/// - Panic：**单次瞬间**跌破粘合 + 显著放量，属于"瞬发崩溃"
///
/// 两者可能同时出现（Guillotine 通常伴随 Panic），但语义层次不同。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PanicCapitulationEvent {
    /// 跌破确认的 K 线索引
    pub index: usize,
    /// 粘合状态下的均线 spread（绝对值）
    pub ma_tight_spread: f64,
    /// 跌破时的收盘价
    pub break_close: f64,
    /// 粘合区下沿（所有均线的最低值）
    pub tight_lower: f64,
    /// 放量倍数（当前 volume / 前 N 根均量）
    pub volume_surge_ratio: f64,
}

impl PanicCapitulationEvent {
    /// 主力行为学分类（原书：粘合放量跌破 = 主力 Panic 出逃）
    pub fn market_maker_behavior(&self) -> MarketMakerBehavior {
        MarketMakerBehavior::Panic
    }
}

/// 主力恐慌抛售参数
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct PanicParams {
    /// 粘合容差：(max_ma - min_ma) / mean_ma < tolerance 视为粘合
    pub tight_tolerance: f64,
    /// 粘合最少持续根数
    pub tight_min_bars: usize,
    /// 放量阈值（当前 volume / 前 N 根均量）
    pub volume_surge_factor: f64,
    /// 成交量对比回看窗口
    pub volume_lookback: usize,
    /// 跌破缓冲（close < tight_lower × (1 - break_pct)）
    pub break_pct: f64,
}

impl Default for PanicParams {
    fn default() -> Self {
        Self {
            tight_tolerance: 0.015,
            tight_min_bars: 3,
            volume_surge_factor: 1.5,
            volume_lookback: 10,
            break_pct: 0.005,
        }
    }
}

/// 参数
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct StealthParams {
    /// 小阳线最大涨幅（默认 2%）
    pub small_bull_max_pct: f64,
    /// 最小连续根数
    pub min_consecutive_bars: usize,
    /// 累计涨幅下限
    pub min_cumulative_rise: f64,
    /// 成交量缩减上限（后段 / 前段）
    pub max_volume_shrink: f64,
    /// 成交量对比窗口
    pub volume_window: usize,
}

impl Default for StealthParams {
    fn default() -> Self {
        Self {
            small_bull_max_pct: 0.02,    // 2% 以内的小阳
            min_consecutive_bars: 5,      // 连续 5 根以上
            min_cumulative_rise: 0.05,    // 累计至少 5%
            max_volume_shrink: 0.8,       // 后段均量 ≤ 前段 × 0.8
            volume_window: 10,
        }
    }
}

/// 检测主力潜伏突破（R-P1-30）
///
/// # 参数
/// - `opens` / `closes`：K 线开收盘
/// - `volumes`：成交量
/// - `params`：参数
///
/// # 算法
/// 1. 滑动窗口扫描每 N+ 根 K 线
/// 2. 要求每根都是小阳线（close > open 且涨幅 ≤ small_bull_max_pct）
/// 3. 累计涨幅 ≥ min_cumulative_rise
/// 4. 后段成交量均量 ≤ 前段均量 × max_volume_shrink
pub fn detect_stealth_breakouts(
    opens: &[f64],
    closes: &[f64],
    volumes: &[f64],
    params: &StealthParams,
) -> Vec<StealthBreakoutEvent> {
    let n = opens.len().min(closes.len()).min(volumes.len());
    let min_bars = params.min_consecutive_bars.max(2);
    if n < min_bars + params.volume_window {
        return Vec::new();
    }
    let mut out = Vec::new();
    let mut i = params.volume_window;

    while i + min_bars <= n {
        // 检查 [i, i+min_bars) 是否均为小阳线
        let mut valid = true;
        let mut total_rise = 0.0;
        for k in i..i + min_bars {
            let o = opens[k];
            let c = closes[k];
            if !o.is_finite() || !c.is_finite() || o.abs() < 1e-9 {
                valid = false;
                break;
            }
            if c <= o {
                valid = false;
                break;
            }
            let pct = (c - o) / o.abs();
            if pct > params.small_bull_max_pct {
                valid = false;
                break;
            }
            total_rise += pct;
        }
        if !valid || total_rise < params.min_cumulative_rise {
            i += 1;
            continue;
        }
        // 检查成交量缩减
        let prev_start = i.saturating_sub(params.volume_window);
        let prev_avg: f64 = volumes[prev_start..i].iter().sum::<f64>() / params.volume_window as f64;
        let curr_avg: f64 = volumes[i..i + min_bars].iter().sum::<f64>() / min_bars as f64;
        if prev_avg < 1e-9 {
            i += 1;
            continue;
        }
        let ratio = curr_avg / prev_avg;
        if ratio > params.max_volume_shrink {
            i += 1;
            continue;
        }
        // 确认
        out.push(StealthBreakoutEvent {
            index: i + min_bars - 1,
            small_bull_count: min_bars,
            volume_shrink_ratio: ratio,
            cumulative_rise_pct: total_rise,
        });
        // 跳到 episode 结束，避免重复
        i += min_bars;
    }

    out
}

/// 检测主力恐慌抛售（R-P1-58）
///
/// 三要素必须同时满足：
///
/// 1. **均线粘合**：前 `tight_min_bars` 根内，所有均线的 spread 都 < `tight_tolerance`
/// 2. **跌破下沿**：当前根 close < 粘合区下沿 × (1 − break_pct)
/// 3. **放量确认**：当前根 volume > 前 `volume_lookback` 根均量 × `volume_surge_factor`
///
/// # 参数
/// - `closes`：收盘价序列
/// - `volumes`：成交量序列
/// - `mas`：多条均线（至少 2 条）
/// - `params`：参数
pub fn detect_panic_capitulation(
    closes: &[f64],
    volumes: &[f64],
    mas: &[Vec<f64>],
    params: &PanicParams,
) -> Vec<PanicCapitulationEvent> {
    if mas.len() < 2 {
        return Vec::new();
    }
    let n = closes
        .len()
        .min(volumes.len())
        .min(mas.iter().map(|m| m.len()).min().unwrap_or(0));
    if n == 0 {
        return Vec::new();
    }
    let start = params.tight_min_bars.max(params.volume_lookback);
    if n <= start {
        return Vec::new();
    }
    let mut out = Vec::new();
    let mut last_emit: Option<usize> = None;

    for i in start..n {
        // 1. 粘合：前 tight_min_bars 根（不含 i）都处于粘合状态
        let mut all_tight = true;
        let mut tight_lower = f64::INFINITY;
        let mut tight_spread_sum = 0.0;
        for j in (i - params.tight_min_bars)..i {
            let (mut mx, mut mn, mut sum) = (f64::NEG_INFINITY, f64::INFINITY, 0.0);
            for m in mas {
                let v = m[j];
                if !v.is_finite() {
                    all_tight = false;
                    break;
                }
                if v > mx { mx = v; }
                if v < mn { mn = v; }
                sum += v;
            }
            if !all_tight {
                break;
            }
            let mean = sum / mas.len() as f64;
            if mean.abs() < 1e-9 {
                all_tight = false;
                break;
            }
            let spread = (mx - mn) / mean.abs();
            if spread >= params.tight_tolerance {
                all_tight = false;
                break;
            }
            tight_spread_sum += spread;
            if mn < tight_lower {
                tight_lower = mn;
            }
        }
        if !all_tight || !tight_lower.is_finite() {
            continue;
        }
        let avg_tight_spread = tight_spread_sum / params.tight_min_bars as f64;

        // 2. 跌破下沿
        let c = closes[i];
        if !c.is_finite() || c >= tight_lower * (1.0 - params.break_pct) {
            continue;
        }

        // 3. 放量
        let vol_start = i.saturating_sub(params.volume_lookback);
        let avg_vol: f64 =
            volumes[vol_start..i].iter().sum::<f64>() / params.volume_lookback as f64;
        if avg_vol < 1e-9 {
            continue;
        }
        let v = volumes[i];
        if !v.is_finite() {
            continue;
        }
        let surge_ratio = v / avg_vol;
        if surge_ratio < params.volume_surge_factor {
            continue;
        }

        // 避免同一次崩溃重复 emit：与上次事件间隔 < tight_min_bars 则跳过
        if let Some(prev) = last_emit {
            if i - prev < params.tight_min_bars {
                continue;
            }
        }
        out.push(PanicCapitulationEvent {
            index: i,
            ma_tight_spread: avg_tight_spread,
            break_close: c,
            tight_lower,
            volume_surge_ratio: surge_ratio,
        });
        last_emit = Some(i);
    }
    out
}

/// 检测通道穿头破脚（R-P1-31）
///
/// # 参数
/// - `klines`：K 线序列
/// - `upper_line_at`：闭包，返回索引 i 处的通道上沿价
/// - `lower_line_at`：闭包，返回索引 i 处的通道下沿价
/// - `pierce_window`：穿头后在多少根内跌破下沿才算"穿头破脚"
pub fn detect_channel_piercing<U, L>(
    klines: &[Kline],
    mut upper_line_at: U,
    mut lower_line_at: L,
    pierce_window: usize,
) -> Vec<ChannelPiercingEvent>
where
    U: FnMut(usize) -> f64,
    L: FnMut(usize) -> f64,
{
    if klines.is_empty() {
        return Vec::new();
    }
    let mut out = Vec::new();
    let n = klines.len();
    let mut i = 0;
    while i < n {
        let upper = upper_line_at(i);
        let lower = lower_line_at(i);
        if !upper.is_finite() || !lower.is_finite() || upper <= lower {
            i += 1;
            continue;
        }
        let k = &klines[i];
        // 穿头：高点超过上沿
        if k.high > upper {
            let end = (i + pierce_window).min(n - 1);
            for j in (i + 1)..=end {
                let low_j = klines[j].low;
                let lower_j = lower_line_at(j);
                if !lower_j.is_finite() {
                    continue;
                }
                if low_j < lower_j {
                    out.push(ChannelPiercingEvent {
                        pierce_top_index: i,
                        pierce_bottom_index: j,
                        upper_line: upper,
                        lower_line: lower_j,
                    });
                    // 跳到 j 之后避免重复
                    i = j;
                    break;
                }
            }
        }
        i += 1;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mk_kline(idx: i64, o: f64, c: f64, h: f64, l: f64, v: f64) -> Kline {
        Kline {
            open_time: idx * 86_400_000,
            close_time: (idx + 1) * 86_400_000 - 1,
            open: o,
            high: h,
            low: l,
            close: c,
            volume: v,
        }
    }

    #[test]
    fn t_stealth_breakout_detected() {
        // 前 10 根放量（5），后 5 根小阳缩量（涨 1.5% 每根，量 = 2）
        let opens = vec![
            100.0, 100.0, 100.0, 100.0, 100.0, 100.0, 100.0, 100.0, 100.0, 100.0,
            100.0, 101.5, 103.0, 104.5, 106.0,
        ];
        let closes = vec![
            100.0, 100.0, 100.0, 100.0, 100.0, 100.0, 100.0, 100.0, 100.0, 100.0,
            101.5, 103.0, 104.5, 106.0, 107.5,
        ];
        let volumes = vec![
            5.0, 5.0, 5.0, 5.0, 5.0, 5.0, 5.0, 5.0, 5.0, 5.0, 2.0, 2.0, 2.0, 2.0, 2.0,
        ];
        let params = StealthParams::default();
        let events = detect_stealth_breakouts(&opens, &closes, &volumes, &params);
        assert!(!events.is_empty(), "应识别主力潜伏突破；实际：{:?}", events);
    }

    #[test]
    fn t_stealth_breakout_rejected_if_volume_not_shrunk() {
        // 小阳线但**放量** → 不符合潜伏条件
        let opens = vec![
            100.0, 100.0, 100.0, 100.0, 100.0, 100.0, 100.0, 100.0, 100.0, 100.0,
            100.0, 101.5, 103.0, 104.5, 106.0,
        ];
        let closes = vec![
            100.0, 100.0, 100.0, 100.0, 100.0, 100.0, 100.0, 100.0, 100.0, 100.0,
            101.5, 103.0, 104.5, 106.0, 107.5,
        ];
        // 后段放量（10 > 前段 5）
        let volumes = vec![
            5.0, 5.0, 5.0, 5.0, 5.0, 5.0, 5.0, 5.0, 5.0, 5.0, 10.0, 10.0, 10.0, 10.0, 10.0,
        ];
        let events = detect_stealth_breakouts(&opens, &closes, &volumes, &StealthParams::default());
        assert!(events.is_empty(), "放量不应触发潜伏突破");
    }

    #[test]
    fn t_stealth_breakout_rejected_if_large_bull() {
        // 大阳线 → 非"小阳"
        let opens = vec![
            100.0, 100.0, 100.0, 100.0, 100.0, 100.0, 100.0, 100.0, 100.0, 100.0,
            100.0, 105.0, 110.0, 115.0, 120.0,
        ];
        let closes = vec![
            100.0, 100.0, 100.0, 100.0, 100.0, 100.0, 100.0, 100.0, 100.0, 100.0,
            105.0, 110.0, 115.0, 120.0, 125.0,
        ];
        let volumes = vec![5.0; 15];
        let events = detect_stealth_breakouts(&opens, &closes, &volumes, &StealthParams::default());
        assert!(events.is_empty(), "大阳线不应触发潜伏突破");
    }

    #[test]
    fn t_channel_piercing_detected() {
        // 上沿 110，下沿 90。第 5 根 high=112（穿头）→ 第 8 根 low=85（破脚）
        let klines: Vec<_> = (0..15)
            .map(|i| {
                if i == 5 {
                    mk_kline(i as i64, 108.0, 109.0, 112.0, 107.0, 1.0) // 穿头
                } else if i == 8 {
                    mk_kline(i as i64, 95.0, 88.0, 96.0, 85.0, 1.0) // 破脚
                } else {
                    mk_kline(i as i64, 100.0, 100.0, 102.0, 98.0, 1.0)
                }
            })
            .collect();
        let events = detect_channel_piercing(&klines, |_| 110.0, |_| 90.0, 5);
        assert!(!events.is_empty(), "应识别穿头破脚");
        assert_eq!(events[0].pierce_top_index, 5);
        assert_eq!(events[0].pierce_bottom_index, 8);
    }

    #[test]
    fn t_channel_piercing_no_event_when_only_pierce_top() {
        // 仅穿头，无破脚
        let klines: Vec<_> = (0..15)
            .map(|i| {
                if i == 5 {
                    mk_kline(i as i64, 108.0, 109.0, 112.0, 107.0, 1.0)
                } else {
                    mk_kline(i as i64, 100.0, 100.0, 102.0, 98.0, 1.0)
                }
            })
            .collect();
        let events = detect_channel_piercing(&klines, |_| 110.0, |_| 90.0, 5);
        assert!(events.is_empty());
    }

    #[test]
    fn t_channel_piercing_empty_input() {
        let events: Vec<ChannelPiercingEvent> =
            detect_channel_piercing(&[], |_| 0.0, |_| 0.0, 5);
        assert!(events.is_empty());
    }

    // ==================== 主力行为学分类测试（R-P1-37） ====================

    #[test]
    fn t_stealth_event_reports_market_maker_stealth() {
        // 复用 t_stealth_breakout_detected 的构造，验证返回 MarketMakerBehavior::Stealth
        let opens = vec![
            100.0, 100.0, 100.0, 100.0, 100.0, 100.0, 100.0, 100.0, 100.0, 100.0,
            100.0, 101.5, 103.0, 104.5, 106.0,
        ];
        let closes = vec![
            100.0, 100.0, 100.0, 100.0, 100.0, 100.0, 100.0, 100.0, 100.0, 100.0,
            101.5, 103.0, 104.5, 106.0, 107.5,
        ];
        let volumes = vec![
            5.0, 5.0, 5.0, 5.0, 5.0, 5.0, 5.0, 5.0, 5.0, 5.0, 2.0, 2.0, 2.0, 2.0, 2.0,
        ];
        let events =
            detect_stealth_breakouts(&opens, &closes, &volumes, &StealthParams::default());
        assert!(!events.is_empty());
        assert_eq!(
            events[0].market_maker_behavior(),
            MarketMakerBehavior::Stealth
        );
    }

    // --------- 主力恐慌抛售 (Panic) ---------

    fn tight_mas_and_base(n: usize) -> (Vec<Vec<f64>>, Vec<f64>) {
        // 3 条均线粘合在 100 附近（spread = 0.6/100 = 0.006 < 0.015）
        let ma5: Vec<f64> = (0..n).map(|_| 100.0).collect();
        let ma10: Vec<f64> = (0..n).map(|_| 100.3).collect();
        let ma20: Vec<f64> = (0..n).map(|_| 99.7).collect();
        let mut closes = vec![100.0; n - 1];
        closes.push(100.0); // 先占位，调用方覆盖末根
        (vec![ma5, ma10, ma20], closes)
    }

    #[test]
    fn t_panic_capitulation_detected() {
        let n = 15;
        let (mas, mut closes) = tight_mas_and_base(n);
        closes[n - 1] = 98.0; // 跌破 tight_lower=99.7×(1-0.005)=99.20
        let mut volumes = vec![1.0; n];
        volumes[n - 1] = 2.0; // 放量 2x
        let events =
            detect_panic_capitulation(&closes, &volumes, &mas, &PanicParams::default());
        assert_eq!(events.len(), 1, "应识别到恐慌抛售；实际：{:?}", events);
        assert_eq!(events[0].index, n - 1);
        assert_eq!(
            events[0].market_maker_behavior(),
            MarketMakerBehavior::Panic
        );
    }

    #[test]
    fn t_panic_rejected_without_tight_bond() {
        // 均线未粘合（spread 10% >> 1.5%）→ 不触发
        let n = 15;
        let ma5: Vec<f64> = (0..n).map(|_| 105.0).collect();
        let ma10: Vec<f64> = (0..n).map(|_| 100.0).collect();
        let ma20: Vec<f64> = (0..n).map(|_| 95.0).collect();
        let mas = vec![ma5, ma10, ma20];
        let mut closes = vec![100.0; n];
        closes[n - 1] = 80.0;
        let mut volumes = vec![1.0; n];
        volumes[n - 1] = 10.0;
        let events =
            detect_panic_capitulation(&closes, &volumes, &mas, &PanicParams::default());
        assert!(events.is_empty(), "无粘合不应触发 Panic");
    }

    #[test]
    fn t_panic_rejected_without_volume_surge() {
        // 粘合 + 跌破，但无放量 → 不触发
        let n = 15;
        let (mas, mut closes) = tight_mas_and_base(n);
        closes[n - 1] = 98.0;
        let volumes = vec![1.0; n]; // volume_surge_ratio = 1.0 < 1.5
        let events =
            detect_panic_capitulation(&closes, &volumes, &mas, &PanicParams::default());
        assert!(events.is_empty(), "无放量不应触发 Panic");
    }

    #[test]
    fn t_panic_rejected_without_break() {
        // 粘合 + 放量，但未跌破粘合下沿 → 不触发
        let n = 15;
        let (mas, mut closes) = tight_mas_and_base(n);
        closes[n - 1] = 99.5; // 仍在粘合区
        let mut volumes = vec![1.0; n];
        volumes[n - 1] = 5.0;
        let events =
            detect_panic_capitulation(&closes, &volumes, &mas, &PanicParams::default());
        assert!(events.is_empty(), "close 未跌破粘合下沿不应触发 Panic");
    }

    #[test]
    fn t_panic_empty_input_no_panic() {
        let events = detect_panic_capitulation(&[], &[], &[], &PanicParams::default());
        assert!(events.is_empty());
    }

    #[test]
    fn t_panic_single_ma_rejected() {
        // 参数要求至少 2 条均线
        let closes = vec![100.0; 20];
        let volumes = vec![1.0; 20];
        let mas = vec![vec![100.0; 20]];
        let events =
            detect_panic_capitulation(&closes, &volumes, &mas, &PanicParams::default());
        assert!(events.is_empty());
    }
}
