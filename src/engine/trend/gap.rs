//! B6：缺口识别与分类
//!
//! - 缺口定义：当前 K 线的开盘价与前一 K 线的最高/最低完全脱离
//!   - 向上缺口：`low[i] > high[i-1]`
//!   - 向下缺口：`high[i] < low[i-1]`
//! - 分类（需结合趋势位置）：
//!   - **Common** 普通缺口：盘整中出现
//!   - **Breakaway** 突破缺口：上升 / 下降趋势启动点（结合道氏趋势）
//!   - **Runaway** 中继缺口：趋势中段
//!   - **Exhaustion** 衰竭缺口：趋势末端
//! - 回补检测：若后续 K 线高/低回到缺口内部 → filled

use serde::{Deserialize, Serialize};

use crate::data::Kline;

use super::dow::DowPhase;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum GapKind {
    Common,
    Breakaway,
    Runaway,
    Exhaustion,
}

impl GapKind {
    pub fn label(&self) -> &'static str {
        match self {
            GapKind::Common => "普通缺口",
            GapKind::Breakaway => "突破缺口",
            GapKind::Runaway => "中继缺口",
            GapKind::Exhaustion => "衰竭缺口",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum GapDir {
    Up,
    Down,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Gap {
    pub index: usize,
    pub time: i64,
    pub dir: GapDir,
    pub kind: GapKind,
    pub label: String,
    /// 缺口上下边界
    pub top: f64,
    pub bottom: f64,
    /// 缺口大小（相对价格的 %）
    pub size_pct: f64,
    /// 是否已被后续 K 线回补
    pub filled: bool,
    pub filled_index: Option<usize>,
}

pub fn detect(klines: &[Kline], min_size_pct: f64, dow: DowPhase, trend_len: usize) -> Vec<Gap> {
    let n = klines.len();
    let mut out = Vec::new();
    if n < 2 {
        return out;
    }
    for i in 1..n {
        let prev = &klines[i - 1];
        let cur = &klines[i];
        let (top, bottom, dir) = if cur.low > prev.high {
            (cur.low, prev.high, GapDir::Up)
        } else if cur.high < prev.low {
            (prev.low, cur.high, GapDir::Down)
        } else {
            continue;
        };
        let size_pct = (top - bottom) / prev.close.abs().max(1e-9);
        if size_pct < min_size_pct {
            continue;
        }
        // 分类：基于距离趋势起点的相对位置
        let kind = classify_gap(i, trend_len, dow, dir);
        // 是否已回补
        let mut filled = false;
        let mut filled_index: Option<usize> = None;
        for j in (i + 1)..n {
            let k = &klines[j];
            let filled_up = matches!(dir, GapDir::Up) && k.low <= bottom;
            let filled_down = matches!(dir, GapDir::Down) && k.high >= top;
            if filled_up || filled_down {
                filled = true;
                filled_index = Some(j);
                break;
            }
        }
        out.push(Gap {
            index: i,
            time: cur.open_time,
            dir,
            kind,
            label: kind.label().to_string(),
            top,
            bottom,
            size_pct,
            filled,
            filled_index,
        });
    }
    out
}

fn classify_gap(i: usize, trend_len: usize, dow: DowPhase, dir: GapDir) -> GapKind {
    if trend_len == 0 {
        return GapKind::Common;
    }
    let position = (i as f64) / (trend_len as f64);
    // 趋势与缺口方向一致
    let aligned = matches!(
        (dow, dir),
        (DowPhase::Uptrend, GapDir::Up) | (DowPhase::Downtrend, GapDir::Down)
    );
    if !aligned {
        return GapKind::Common;
    }
    if position < 0.25 {
        GapKind::Breakaway
    } else if position > 0.80 {
        GapKind::Exhaustion
    } else {
        GapKind::Runaway
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mk_kline(idx: i64, o: f64, h: f64, l: f64, c: f64) -> Kline {
        Kline {
            open_time: idx * 86_400_000,
            close_time: (idx + 1) * 86_400_000 - 1,
            open: o,
            high: h,
            low: l,
            close: c,
            volume: 1.0,
        }
    }

    // -------- GapKind metadata --------

    #[test]
    fn t_gap_kind_labels_complete() {
        assert_eq!(GapKind::Common.label(), "普通缺口");
        assert_eq!(GapKind::Breakaway.label(), "突破缺口");
        assert_eq!(GapKind::Runaway.label(), "中继缺口");
        assert_eq!(GapKind::Exhaustion.label(), "衰竭缺口");
    }

    // -------- classify_gap --------

    #[test]
    fn t_classify_gap_common_when_trend_len_zero() {
        let k = classify_gap(5, 0, DowPhase::Uptrend, GapDir::Up);
        assert_eq!(k, GapKind::Common);
    }

    #[test]
    fn t_classify_gap_common_when_direction_opposite_to_trend() {
        // 上涨趋势 + 向下缺口 → 不 aligned → Common
        let k = classify_gap(5, 100, DowPhase::Uptrend, GapDir::Down);
        assert_eq!(k, GapKind::Common);
        // 下跌趋势 + 向上缺口 → Common
        let k = classify_gap(5, 100, DowPhase::Downtrend, GapDir::Up);
        assert_eq!(k, GapKind::Common);
        // 震荡阶段 → 永远 Common
        let k = classify_gap(5, 100, DowPhase::Consolidation, GapDir::Up);
        assert_eq!(k, GapKind::Common);
    }

    #[test]
    fn t_classify_gap_breakaway_at_trend_start() {
        // position = 10/100 = 0.10 < 0.25 → Breakaway
        let k = classify_gap(10, 100, DowPhase::Uptrend, GapDir::Up);
        assert_eq!(k, GapKind::Breakaway);
    }

    #[test]
    fn t_classify_gap_runaway_at_trend_middle() {
        // position = 50/100 = 0.50 ∈ [0.25, 0.80] → Runaway
        let k = classify_gap(50, 100, DowPhase::Uptrend, GapDir::Up);
        assert_eq!(k, GapKind::Runaway);
    }

    #[test]
    fn t_classify_gap_exhaustion_at_trend_end() {
        // position = 90/100 = 0.90 > 0.80 → Exhaustion
        let k = classify_gap(90, 100, DowPhase::Downtrend, GapDir::Down);
        assert_eq!(k, GapKind::Exhaustion);
    }

    // -------- detect --------

    #[test]
    fn t_detect_no_gap_when_ranges_overlap() {
        // bar0 range [99, 101], bar1 range [100, 102]（重叠）→ 无缺口
        let klines = vec![
            mk_kline(0, 100.0, 101.0, 99.0, 100.5),
            mk_kline(1, 100.5, 102.0, 100.0, 101.5),
        ];
        let gaps = detect(&klines, 0.001, DowPhase::Consolidation, 10);
        assert!(gaps.is_empty());
    }

    #[test]
    fn t_detect_up_gap_when_low_above_prev_high() {
        // bar0 high=100, bar1 low=105 → 向上缺口 [100, 105]
        let klines = vec![
            mk_kline(0, 99.0, 100.0, 98.0, 99.5),
            mk_kline(1, 106.0, 107.0, 105.0, 106.5),
        ];
        let gaps = detect(&klines, 0.001, DowPhase::Uptrend, 10);
        assert_eq!(gaps.len(), 1);
        assert_eq!(gaps[0].dir, GapDir::Up);
        assert!((gaps[0].top - 105.0).abs() < 1e-9);
        assert!((gaps[0].bottom - 100.0).abs() < 1e-9);
        // size_pct = (105-100) / 99.5 ≈ 0.0503
        assert!((gaps[0].size_pct - 5.0 / 99.5).abs() < 1e-9);
    }

    #[test]
    fn t_detect_down_gap_when_high_below_prev_low() {
        // bar0 low=100, bar1 high=95 → 向下缺口
        let klines = vec![
            mk_kline(0, 101.0, 102.0, 100.0, 101.5),
            mk_kline(1, 94.0, 95.0, 92.0, 93.5),
        ];
        let gaps = detect(&klines, 0.001, DowPhase::Downtrend, 10);
        assert_eq!(gaps.len(), 1);
        assert_eq!(gaps[0].dir, GapDir::Down);
    }

    #[test]
    fn t_detect_skips_gap_below_size_threshold() {
        // 缺口仅 0.01/99.5 ≈ 0.01% < 0.5% 阈值
        let klines = vec![
            mk_kline(0, 99.0, 100.0, 98.0, 99.5),
            mk_kline(1, 100.5, 101.0, 100.01, 100.5),
        ];
        let gaps = detect(&klines, 0.005, DowPhase::Uptrend, 10);
        assert!(gaps.is_empty(), "缺口小于阈值应被跳过");
    }

    #[test]
    fn t_detect_filled_when_later_bar_covers_gap() {
        // bar0 high=100, bar1 low=105（缺口 100-105）
        // bar2 low=99 → 回补（low 跌回缺口底 100 之下）
        let klines = vec![
            mk_kline(0, 99.0, 100.0, 98.0, 99.5),
            mk_kline(1, 106.0, 107.0, 105.0, 106.5),
            mk_kline(2, 105.0, 106.0, 99.0, 100.0),
        ];
        let gaps = detect(&klines, 0.001, DowPhase::Uptrend, 10);
        assert_eq!(gaps.len(), 1);
        assert!(gaps[0].filled, "后续 bar 应回补缺口");
        assert_eq!(gaps[0].filled_index, Some(2));
    }

    #[test]
    fn t_detect_not_filled_when_price_continues_holding() {
        // bar0 high=100, bar1 low=105；bar2/3 保持 > 100 → 未回补
        let klines = vec![
            mk_kline(0, 99.0, 100.0, 98.0, 99.5),
            mk_kline(1, 106.0, 107.0, 105.0, 106.5),
            mk_kline(2, 107.0, 108.0, 106.0, 107.5),
            mk_kline(3, 108.0, 109.0, 107.0, 108.5),
        ];
        let gaps = detect(&klines, 0.001, DowPhase::Uptrend, 10);
        assert_eq!(gaps.len(), 1);
        assert!(!gaps[0].filled);
        assert!(gaps[0].filled_index.is_none());
    }

    #[test]
    fn t_detect_empty_or_single_kline_no_gap() {
        assert!(detect(&[], 0.005, DowPhase::Uptrend, 10).is_empty());
        let klines = vec![mk_kline(0, 100.0, 101.0, 99.0, 100.5)];
        assert!(detect(&klines, 0.005, DowPhase::Uptrend, 10).is_empty());
    }
}
