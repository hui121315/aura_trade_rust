//! A5：均线排列识别
//!
//! 对应 PRD §A5：多头/空头排列 + 别名（上山爬坡/逐浪上升 / 下山滑坡/逐浪下降）+
//! 粘合/收敛/发散 + 金叉/死叉。

use serde::{Deserialize, Serialize};

/// 均线排列状态
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Alignment {
    /// 多头排列（上山爬坡 / 逐浪上升）
    Bullish,
    /// 空头排列（下山滑坡 / 逐浪下降）
    Bearish,
    /// 均线粘合（所有均线极差小）
    Stuck,
    /// 均线收敛（间距持续缩窄）
    Converging,
    /// 均线发散（间距持续扩大）
    Diverging,
    /// 其它（交错，方向不明）
    Mixed,
}

impl Alignment {
    /// 返回原书别名列表
    pub fn aliases(&self) -> &'static [&'static str] {
        match self {
            Alignment::Bullish => &["多头排列", "上山爬坡", "逐浪上升"],
            Alignment::Bearish => &["空头排列", "下山滑坡", "逐浪下降"],
            Alignment::Stuck => &["均线粘合"],
            Alignment::Converging => &["均线收敛"],
            Alignment::Diverging => &["均线发散"],
            Alignment::Mixed => &["交错"],
        }
    }
}

/// 在索引 `i` 处的排列判定。
/// `ma_stack` 必须按周期升序（如 [MA5, MA10, MA20, MA60]），每条序列长度相等。
///
/// `stuck_threshold` = 粘合阈值：最大值-最小值 / 最小值 < 此阈值 则视为粘合。
/// 默认 0.005（0.5%）。
pub fn classify(ma_stack: &[&[f64]], i: usize, stuck_threshold: f64) -> Alignment {
    if ma_stack.len() < 2 {
        return Alignment::Mixed;
    }
    let vals: Vec<f64> = ma_stack
        .iter()
        .filter_map(|m| m.get(i).copied())
        .filter(|v| v.is_finite())
        .collect();
    if vals.len() != ma_stack.len() {
        return Alignment::Mixed;
    }
    let (lo, hi) = vals
        .iter()
        .fold((f64::INFINITY, f64::NEG_INFINITY), |(a, b), &v| (a.min(v), b.max(v)));
    if lo > 0.0 && (hi - lo) / lo < stuck_threshold {
        return Alignment::Stuck;
    }
    // 多头：vals 严格降序（周期小的在上）
    let bull = vals.windows(2).all(|w| w[0] >= w[1]);
    // 空头：严格升序（周期小的在下）
    let bear = vals.windows(2).all(|w| w[0] <= w[1]);
    match (bull, bear) {
        (true, false) => Alignment::Bullish,
        (false, true) => Alignment::Bearish,
        _ => Alignment::Mixed,
    }
}

/// 判断收敛 vs 发散：比较 i 与 i-lookback 两个时刻的均线极差。
pub fn spread_trend(ma_stack: &[&[f64]], i: usize, lookback: usize) -> Option<Alignment> {
    if i < lookback || ma_stack.len() < 2 {
        return None;
    }
    let spread_at = |t: usize| -> Option<f64> {
        let vals: Vec<f64> = ma_stack
            .iter()
            .filter_map(|m| m.get(t).copied())
            .filter(|v| v.is_finite())
            .collect();
        if vals.len() != ma_stack.len() {
            return None;
        }
        let lo = vals.iter().copied().fold(f64::INFINITY, f64::min);
        let hi = vals.iter().copied().fold(f64::NEG_INFINITY, f64::max);
        Some(hi - lo)
    };
    let s_now = spread_at(i)?;
    let s_prev = spread_at(i - lookback)?;
    if s_now < s_prev * 0.95 {
        Some(Alignment::Converging)
    } else if s_now > s_prev * 1.05 {
        Some(Alignment::Diverging)
    } else {
        None
    }
}

/// 金叉/死叉事件
///
/// 原书 ma p.224 明确：黄金/死亡交叉必须满足 ①短穿/破长 **AND** ②两条均线同向。
/// 否则仅为"普通交叉"（PlainUp / PlainDown），**不具备交易信号的技术含义**。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CrossKind {
    /// 黄金交叉：短穿长 + 两线同时上行 → 买入信号
    Golden,
    /// 死亡交叉：短破长 + 两线同时下行 → 卖出信号
    Death,
    /// 普通交叉（向上）：短穿长，但至少一条不同向 → 无信号意义
    PlainUp,
    /// 普通交叉（向下）：短破长，但至少一条不同向 → 无信号意义
    PlainDown,
}

impl CrossKind {
    /// 是否为原书明确的交易信号（Golden/Death），用于 filter 普通交叉。
    pub fn is_signal(&self) -> bool {
        matches!(self, CrossKind::Golden | CrossKind::Death)
    }
}

/// 一次交叉事件
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Cross {
    pub index: usize,
    pub fast_period: usize,
    pub slow_period: usize,
    pub kind: CrossKind,
}

/// 扫描 `fast / slow` 两条均线，返回所有交叉事件（含普通交叉）。
///
/// 默认 slope_lookback = 5 根 K 线用于判定两条均线方向。
pub fn find_crosses(
    fast: &[f64],
    slow: &[f64],
    fast_period: usize,
    slow_period: usize,
) -> Vec<Cross> {
    find_crosses_with_lookback(fast, slow, fast_period, slow_period, 5)
}

/// `slope_lookback` 参数化版本：
/// 使用过去 `slope_lookback` 根 K 线判定两条均线方向（上行/下行）。
pub fn find_crosses_with_lookback(
    fast: &[f64],
    slow: &[f64],
    fast_period: usize,
    slow_period: usize,
    slope_lookback: usize,
) -> Vec<Cross> {
    let mut out = Vec::new();
    let n = fast.len().min(slow.len());
    let lb = slope_lookback.max(1);
    for i in 1..n {
        let (p_fast, p_slow, c_fast, c_slow) = (fast[i - 1], slow[i - 1], fast[i], slow[i]);
        if !(p_fast.is_finite() && p_slow.is_finite() && c_fast.is_finite() && c_slow.is_finite()) {
            continue;
        }
        // 两条均线方向：若回望不到 lb 根就用 i-1 根做 best-effort（避免序列开头漏检）
        let back = i.saturating_sub(lb).max(0);
        let fast_dir_ref = fast.get(back).copied().unwrap_or(f64::NAN);
        let slow_dir_ref = slow.get(back).copied().unwrap_or(f64::NAN);
        let fast_up = fast_dir_ref.is_finite() && c_fast > fast_dir_ref;
        let slow_up = slow_dir_ref.is_finite() && c_slow > slow_dir_ref;
        let fast_down = fast_dir_ref.is_finite() && c_fast < fast_dir_ref;
        let slow_down = slow_dir_ref.is_finite() && c_slow < slow_dir_ref;

        let up_cross = p_fast <= p_slow && c_fast > c_slow;
        let down_cross = p_fast >= p_slow && c_fast < c_slow;

        let kind = if up_cross {
            if fast_up && slow_up {
                CrossKind::Golden
            } else {
                CrossKind::PlainUp
            }
        } else if down_cross {
            if fast_down && slow_down {
                CrossKind::Death
            } else {
                CrossKind::PlainDown
            }
        } else {
            continue;
        };

        out.push(Cross { index: i, fast_period, slow_period, kind });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    // -------- classify --------

    #[test]
    fn t_classify_bullish_when_shorter_ma_higher() {
        // 多头排列：MA5=10, MA10=8, MA20=6（短周期在上，降序）
        let ma5 = [10.0];
        let ma10 = [8.0];
        let ma20 = [6.0];
        let stack: Vec<&[f64]> = vec![&ma5, &ma10, &ma20];
        assert_eq!(classify(&stack, 0, 0.005), Alignment::Bullish);
    }

    #[test]
    fn t_classify_bearish_when_shorter_ma_lower() {
        // 空头排列：MA5=6, MA10=8, MA20=10（短周期在下，升序）
        let ma5 = [6.0];
        let ma10 = [8.0];
        let ma20 = [10.0];
        let stack: Vec<&[f64]> = vec![&ma5, &ma10, &ma20];
        assert_eq!(classify(&stack, 0, 0.005), Alignment::Bearish);
    }

    #[test]
    fn t_classify_stuck_when_all_close() {
        // 粘合：spread/lo < 阈值
        let ma5 = [100.0];
        let ma10 = [100.1];
        let ma20 = [99.95];
        let stack: Vec<&[f64]> = vec![&ma5, &ma10, &ma20];
        // spread = 0.15/99.95 ≈ 0.0015 < 0.005 → Stuck
        assert_eq!(classify(&stack, 0, 0.005), Alignment::Stuck);
    }

    #[test]
    fn t_classify_mixed_when_interleaved() {
        // 交错：[8, 10, 6] 既不单调降也不单调升
        let ma5 = [8.0];
        let ma10 = [10.0];
        let ma20 = [6.0];
        let stack: Vec<&[f64]> = vec![&ma5, &ma10, &ma20];
        assert_eq!(classify(&stack, 0, 0.005), Alignment::Mixed);
    }

    #[test]
    fn t_classify_mixed_on_insufficient_stack() {
        let ma5 = [10.0];
        let stack: Vec<&[f64]> = vec![&ma5];
        assert_eq!(classify(&stack, 0, 0.005), Alignment::Mixed);
    }

    #[test]
    fn t_classify_mixed_on_nan_value() {
        let ma5 = [f64::NAN];
        let ma10 = [8.0];
        let stack: Vec<&[f64]> = vec![&ma5, &ma10];
        assert_eq!(classify(&stack, 0, 0.005), Alignment::Mixed);
    }

    #[test]
    fn t_alignment_aliases_contains_chinese_labels() {
        assert!(Alignment::Bullish.aliases().contains(&"多头排列"));
        assert!(Alignment::Bullish.aliases().contains(&"上山爬坡"));
        assert!(Alignment::Bearish.aliases().contains(&"空头排列"));
        assert!(Alignment::Stuck.aliases().contains(&"均线粘合"));
        assert!(Alignment::Converging.aliases().contains(&"均线收敛"));
        assert!(Alignment::Diverging.aliases().contains(&"均线发散"));
    }

    // -------- spread_trend --------

    #[test]
    fn t_spread_converging_when_gap_shrinks() {
        // t=0 spread=10, t=5 spread=4 → Converging
        let ma5: Vec<f64> = vec![110.0, 108.0, 106.0, 104.0, 102.0, 101.0];
        let ma20: Vec<f64> = vec![100.0, 100.5, 100.8, 100.9, 100.95, 100.98];
        let stack: Vec<&[f64]> = vec![&ma5, &ma20];
        let trend = spread_trend(&stack, 5, 5);
        assert_eq!(trend, Some(Alignment::Converging));
    }

    #[test]
    fn t_spread_diverging_when_gap_grows() {
        // t=0 spread=2, t=5 spread=20 → Diverging
        let ma5: Vec<f64> = vec![101.0, 103.0, 105.0, 110.0, 115.0, 120.0];
        let ma20: Vec<f64> = vec![100.0, 100.5, 100.8, 100.9, 100.95, 100.98];
        let stack: Vec<&[f64]> = vec![&ma5, &ma20];
        let trend = spread_trend(&stack, 5, 5);
        assert_eq!(trend, Some(Alignment::Diverging));
    }

    #[test]
    fn t_spread_returns_none_on_insufficient_history() {
        let ma5: Vec<f64> = vec![100.0, 101.0];
        let ma20: Vec<f64> = vec![100.0, 100.5];
        let stack: Vec<&[f64]> = vec![&ma5, &ma20];
        // i=0, lookback=5 → 不够 → None
        assert_eq!(spread_trend(&stack, 0, 5), None);
    }

    // -------- CrossKind metadata --------

    #[test]
    fn t_crosskind_is_signal_flag() {
        assert!(CrossKind::Golden.is_signal());
        assert!(CrossKind::Death.is_signal());
        assert!(!CrossKind::PlainUp.is_signal());
        assert!(!CrossKind::PlainDown.is_signal());
    }

    // -------- find_crosses --------

    #[test]
    fn t_find_crosses_golden_when_both_up() {
        // fast 线性 96→105，slow 线性 99→103.5，两者都上行，i=7 处 fast 穿越 slow
        let fast = vec![96.0, 97.0, 98.0, 99.0, 100.0, 101.0, 102.0, 103.0, 104.0, 105.0];
        let slow = vec![99.0, 99.5, 100.0, 100.5, 101.0, 101.5, 102.0, 102.5, 103.0, 103.5];
        let crosses = find_crosses(&fast, &slow, 5, 20);
        assert!(!crosses.is_empty(), "应找到交叉");
        assert_eq!(crosses[0].kind, CrossKind::Golden);
        assert_eq!(crosses[0].fast_period, 5);
        assert_eq!(crosses[0].slow_period, 20);
    }

    #[test]
    fn t_find_crosses_death_when_both_down() {
        // fast 线性 105→96，slow 线性 103.5→99，两者都下行，某处 fast 跌破 slow
        let fast = vec![105.0, 104.0, 103.0, 102.0, 101.0, 100.0, 99.0, 98.0, 97.0, 96.0];
        let slow = vec![103.5, 103.0, 102.5, 102.0, 101.5, 101.0, 100.5, 100.0, 99.5, 99.0];
        let crosses = find_crosses(&fast, &slow, 5, 20);
        assert!(!crosses.is_empty());
        assert_eq!(crosses[0].kind, CrossKind::Death);
    }

    #[test]
    fn t_find_crosses_plain_up_when_only_fast_rising() {
        // fast 上穿 slow，但 slow 横盘不上行 → PlainUp（非金叉）
        let fast = vec![96.0, 97.0, 98.0, 99.0, 100.0, 101.0, 102.0, 103.0];
        let slow = vec![100.0; 8]; // 完全横盘
        let crosses = find_crosses(&fast, &slow, 5, 20);
        assert!(!crosses.is_empty(), "应找到交叉");
        assert_eq!(crosses[0].kind, CrossKind::PlainUp,
            "slow 不上行时不应为 Golden");
    }

    #[test]
    fn t_find_crosses_plain_down_when_slow_flat() {
        // fast 跌穿 slow，但 slow 横盘 → PlainDown
        let fast = vec![104.0, 103.0, 102.0, 101.0, 100.0, 99.0, 98.0, 97.0];
        let slow = vec![100.0; 8];
        let crosses = find_crosses(&fast, &slow, 5, 20);
        assert!(!crosses.is_empty());
        assert_eq!(crosses[0].kind, CrossKind::PlainDown);
    }

    #[test]
    fn t_find_crosses_no_event_when_no_cross() {
        // fast 一直在 slow 上方 → 无交叉
        let fast = vec![105.0; 10];
        let slow = vec![100.0; 10];
        let crosses = find_crosses(&fast, &slow, 5, 20);
        assert!(crosses.is_empty());
    }

    #[test]
    fn t_find_crosses_skips_nan_points() {
        // 含 NaN 的点被跳过，不应 panic
        let fast = vec![f64::NAN, 97.0, 98.0, 99.0, 100.0, 101.0];
        let slow = vec![f64::NAN, 99.5, 100.0, 100.5, 101.0, 101.5];
        let _ = find_crosses(&fast, &slow, 5, 20); // 主要验证不 panic
    }

    #[test]
    fn t_find_crosses_empty_input() {
        let crosses = find_crosses(&[], &[], 5, 20);
        assert!(crosses.is_empty());
    }
}
