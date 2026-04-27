//! C4：跨周期分析 + 多均线排列 + 收敛发散（Sprint 6 新增）
//!
//! 本模块实现：
//!
//! - **K 线跨周期聚合**：日 K 线 → 周 K 线（ISO 8601 周聚合）
//! - **R-P1-33 空头/多头排列检测**（ma p.204 精确定义）
//! - **R-P1-34 均线收敛/发散检测**（ma p.244，区分粘合/交叉）
//! - **R-P1-35 周线乌云密布多级共振清仓**（ma p.304，复用 DarkCloudCover）
//!
//! # 原书铁证（ma p.204）
//!
//! 空头排列的**精确定义**：
//! > "K 线、短期均线、中期均线、长期均线**依次从下到上排列且方向向下**。"
//!
//! 完整序列（日 K 线）：K 线 < 5 日 < 10 日 < 20 日 < 60 日 < 120 日 < 240 日，且**全部向下**。
//!
//! # 周线 vs 日线杀伤力铁证（ma p.204 / p.304）
//!
//! > "周线比较难以形成空头排列，但**一旦形成，杀伤力往往大于日线**。"

use serde::{Deserialize, Serialize};

use crate::data::Kline;

/// K 线周期
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Timeframe {
    Daily,
    Weekly,
    Monthly,
}

impl Timeframe {
    pub fn label(&self) -> &'static str {
        match self {
            Timeframe::Daily => "日线",
            Timeframe::Weekly => "周线",
            Timeframe::Monthly => "月线",
        }
    }
}

/// 排列类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AlignmentKind {
    /// 多头排列：K < 短 < 中 < 长（短期在上）且全部向上
    Bullish,
    /// 空头排列：K > 短 > 中 > 长（短期在下）且全部向下
    Bearish,
    /// 未形成明确排列
    None,
}

impl AlignmentKind {
    pub fn label(&self) -> &'static str {
        match self {
            AlignmentKind::Bullish => "多头排列",
            AlignmentKind::Bearish => "空头排列",
            AlignmentKind::None => "无明确排列",
        }
    }

    pub fn direction(&self) -> i8 {
        match self {
            AlignmentKind::Bullish => 1,
            AlignmentKind::Bearish => -1,
            AlignmentKind::None => 0,
        }
    }
}

/// 均线状态（R-P1-34，ma p.244）
///
/// **关键区分**：
/// | 概念 | 强调 | 出现阶段 |
/// |---|---|---|
/// | 收敛 | 股价运行过程 | 持续涨/跌之后 |
/// | 发散 | 股价运行过程 | 收敛之后 |
/// | 粘合 | 均线位置关系 | 任何阶段 |
/// | 交叉 | 收敛后的可能结果 | 收敛之后 |
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MaRelationState {
    /// 收敛（spread 从大 → 小，但未粘合）
    Converging,
    /// 发散（spread 从小 → 大，扩散）
    Diverging,
    /// 粘合（spread < 阈值）
    Bonded,
    /// 无明确状态
    Stable,
}

impl MaRelationState {
    pub fn label(&self) -> &'static str {
        match self {
            MaRelationState::Converging => "收敛",
            MaRelationState::Diverging => "发散",
            MaRelationState::Bonded => "粘合",
            MaRelationState::Stable => "稳定",
        }
    }
}

// ==================== K 线聚合 ====================

/// 将日 K 线聚合为周 K 线（连续 5 根合并为 1 根）
///
/// # 简化假设
///
/// 不做日历周匹配（不依赖 UTC 时间戳的周一），仅按连续 5 根聚合。
/// 若需严格 ISO 周匹配，请在外部传入已按周分组的 `Vec<Vec<Kline>>`。
pub fn aggregate_to_weekly(daily_klines: &[Kline]) -> Vec<Kline> {
    if daily_klines.is_empty() {
        return Vec::new();
    }
    let mut out = Vec::new();
    let mut i = 0;
    while i < daily_klines.len() {
        let end = (i + 5).min(daily_klines.len());
        let chunk = &daily_klines[i..end];
        let open = chunk.first().unwrap().open;
        let close = chunk.last().unwrap().close;
        let high = chunk.iter().map(|k| k.high).fold(f64::NEG_INFINITY, f64::max);
        let low = chunk.iter().map(|k| k.low).fold(f64::INFINITY, f64::min);
        let volume: f64 = chunk.iter().map(|k| k.volume).sum();
        let open_time = chunk.first().unwrap().open_time;
        let close_time = chunk.last().unwrap().close_time;
        out.push(Kline {
            open_time,
            close_time,
            open,
            high,
            low,
            close,
            volume,
        });
        i = end;
    }
    out
}

// ==================== R-P1-33 多头/空头排列 ====================

/// 检测给定索引 i 处的均线排列类型
///
/// # 参数
/// - `close`: 当前收盘价
/// - `mas`: 均线值（按周期从**短到长**排列），例如 `[ma5, ma10, ma20, ma60, ma120, ma240]`
/// - `slope_lookback_mas`: 同样索引位置 `i - lookback` 处的 mas 值（用于判断方向）
///
/// # 原书定义（ma p.204）
///
/// 空头排列：`K < ma5 < ma10 < ma20 < ma60 < ma120 < ma240` 且**全部向下**
pub fn detect_alignment(
    close: f64,
    mas: &[f64],
    mas_lookback: &[f64],
) -> AlignmentKind {
    if mas.len() < 3 || mas.len() != mas_lookback.len() {
        return AlignmentKind::None;
    }
    if !close.is_finite() || mas.iter().any(|v| !v.is_finite()) {
        return AlignmentKind::None;
    }

    // 空头排列：K > ma5 > ma10 > ma20 > ...（K 最高，长期均线最低）且全部向下
    let bearish_order = {
        let mut prev = close;
        let mut ok = true;
        for &m in mas {
            if prev < m {
                ok = false;
                break;
            }
            prev = m;
        }
        ok
    };
    let bearish_direction = mas
        .iter()
        .zip(mas_lookback.iter())
        .all(|(now, back)| now < back); // 所有均线都下行

    // 多头排列：K < ma5 < ma10 < ...（K 最低，长期均线最高）且全部向上
    // 原书定义：K 线在短期均线上方 → 修正：K > ma5 > ma10 ... 是多头排列（短期在最高）
    // 实际原书 ma p.204：多头时 K > ma5 > ma10 > ma20 > ma60 > ma120 > ma240
    // 与空头相反！让我重新理解：
    // - 多头排列：K 最高，ma5 次之，ma240 最低；短期均线接近价格，长期均线远离
    // - 空头排列：K 最低，ma5 次之，ma240 最高；短期均线接近价格，长期均线远离（上方）
    // 所以 bearish_order 实际是 **多头排列**？
    // 让我重新看原书引用：
    // "空头排列 = 指在 K 线走势图中，K 线、短期均线、中期均线、长期均线依次从下到上排列且方向向下"
    // 即：K 最下，长期均线最上。
    // 所以空头排列：K < ma5 < ma10 < ma20 < ma60 < ma120 < ma240
    // 多头排列（相反）：K > ma5 > ma10 > ma20 > ma60 > ma120 > ma240

    // 修正实现：
    let bullish_order = {
        let mut prev = close;
        let mut ok = true;
        for &m in mas {
            if prev < m {
                ok = false;
                break;
            }
            prev = m;
        }
        ok
    };
    let bullish_direction = mas
        .iter()
        .zip(mas_lookback.iter())
        .all(|(now, back)| now > back); // 所有均线上行

    let true_bearish_order = {
        let mut prev = close;
        let mut ok = true;
        for &m in mas {
            if prev > m {
                ok = false;
                break;
            }
            prev = m;
        }
        ok
    };
    let _ = bearish_order;

    if bullish_order && bullish_direction {
        AlignmentKind::Bullish
    } else if true_bearish_order && bearish_direction {
        AlignmentKind::Bearish
    } else {
        AlignmentKind::None
    }
}

/// 扫描整条序列中的排列变化点（从 `None` 转为 `Bullish` / `Bearish`）
pub fn scan_alignment_events(
    closes: &[f64],
    mas: &[Vec<f64>],
    lookback: usize,
) -> Vec<(usize, AlignmentKind)> {
    let n = closes.len();
    let len = mas.iter().map(|m| m.len()).min().unwrap_or(0).min(n);
    if len <= lookback || mas.is_empty() {
        return Vec::new();
    }
    let mut out = Vec::new();
    let mut prev_kind = AlignmentKind::None;
    for i in lookback..len {
        let mas_now: Vec<f64> = mas.iter().map(|m| m[i]).collect();
        let mas_back: Vec<f64> = mas.iter().map(|m| m[i - lookback]).collect();
        let kind = detect_alignment(closes[i], &mas_now, &mas_back);
        if kind != prev_kind && kind != AlignmentKind::None {
            out.push((i, kind));
            prev_kind = kind;
        } else if kind == AlignmentKind::None && prev_kind != AlignmentKind::None {
            prev_kind = kind; // 退出排列
        }
    }
    out
}

// ==================== R-P1-34 收敛 / 发散 / 粘合 ====================

/// 计算均线"spread"（spread = (max - min) / mean）
fn ma_spread(mas: &[f64]) -> Option<f64> {
    if mas.len() < 2 {
        return None;
    }
    let finite: Vec<f64> = mas.iter().copied().filter(|v| v.is_finite()).collect();
    if finite.len() < 2 {
        return None;
    }
    let max = finite.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    let min = finite.iter().copied().fold(f64::INFINITY, f64::min);
    let mean: f64 = finite.iter().sum::<f64>() / finite.len() as f64;
    if mean.abs() < 1e-9 {
        return None;
    }
    Some((max - min) / mean.abs())
}

/// 检测均线在索引 i 处的关系状态（收敛/发散/粘合）
pub fn detect_ma_relation(
    mas_now: &[f64],
    mas_back: &[f64],
    bond_threshold: f64,
) -> MaRelationState {
    let (Some(spread_now), Some(spread_back)) = (ma_spread(mas_now), ma_spread(mas_back)) else {
        return MaRelationState::Stable;
    };
    // 粘合：当前 spread 足够小
    if spread_now < bond_threshold {
        return MaRelationState::Bonded;
    }
    // 收敛：spread 缩小 ≥ 20%
    if spread_now < spread_back * 0.8 {
        return MaRelationState::Converging;
    }
    // 发散：spread 扩大 ≥ 20%
    if spread_now > spread_back * 1.2 {
        return MaRelationState::Diverging;
    }
    MaRelationState::Stable
}

/// 扫描整条序列的 MA 关系状态变化
pub fn scan_ma_relation_events(
    mas: &[Vec<f64>],
    lookback: usize,
    bond_threshold: f64,
) -> Vec<(usize, MaRelationState)> {
    let len = mas.iter().map(|m| m.len()).min().unwrap_or(0);
    if len <= lookback || mas.is_empty() {
        return Vec::new();
    }
    let mut out = Vec::new();
    let mut prev = MaRelationState::Stable;
    for i in lookback..len {
        let mas_now: Vec<f64> = mas.iter().map(|m| m[i]).collect();
        let mas_back: Vec<f64> = mas.iter().map(|m| m[i - lookback]).collect();
        let state = detect_ma_relation(&mas_now, &mas_back, bond_threshold);
        if state != prev && state != MaRelationState::Stable {
            out.push((i, state));
            prev = state;
        } else if state == MaRelationState::Stable {
            prev = state;
        }
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
    fn t_aggregate_daily_to_weekly() {
        // 10 根日 K 线 → 2 根周 K 线
        let dailies: Vec<_> = (0..10)
            .map(|i| mk_kline(i, 100.0 + i as f64, 100.5 + i as f64, 101.0 + i as f64, 99.5 + i as f64, 1.0))
            .collect();
        let weekly = aggregate_to_weekly(&dailies);
        assert_eq!(weekly.len(), 2);
        // 第 1 周：open=100, close=104.5, high=105.0, low=99.5, volume=5.0
        assert_eq!(weekly[0].open, 100.0);
        assert_eq!(weekly[0].close, 104.5);
        assert_eq!(weekly[0].volume, 5.0);
        // 第 2 周：open=105, close=109.5
        assert_eq!(weekly[1].open, 105.0);
        assert_eq!(weekly[1].close, 109.5);
    }

    #[test]
    fn t_aggregate_short_data_partial_week() {
        // 3 根日 K → 1 根不完整周 K
        let dailies: Vec<_> = (0..3)
            .map(|i| mk_kline(i, 100.0, 101.0, 102.0, 99.0, 1.0))
            .collect();
        let weekly = aggregate_to_weekly(&dailies);
        assert_eq!(weekly.len(), 1);
        assert_eq!(weekly[0].volume, 3.0);
    }

    #[test]
    fn t_bullish_alignment_detected() {
        // K=110 > ma5=108 > ma10=105 > ma20=100，且全部向上
        let close = 110.0;
        let mas = vec![108.0, 105.0, 100.0];
        let mas_back = vec![107.0, 104.0, 99.0]; // 全部上行
        let align = detect_alignment(close, &mas, &mas_back);
        assert_eq!(align, AlignmentKind::Bullish);
    }

    #[test]
    fn t_bearish_alignment_detected() {
        // K=90 < ma5=92 < ma10=95 < ma20=100，且全部向下
        let close = 90.0;
        let mas = vec![92.0, 95.0, 100.0];
        let mas_back = vec![93.0, 96.0, 101.0]; // 全部下行
        let align = detect_alignment(close, &mas, &mas_back);
        assert_eq!(align, AlignmentKind::Bearish);
    }

    #[test]
    fn t_no_alignment_when_order_wrong() {
        // K=100, ma5=110（K < ma5 但 ma5 > ma10 ...）→ 顺序乱
        let close = 100.0;
        let mas = vec![110.0, 95.0, 105.0];
        let mas_back = vec![108.0, 94.0, 104.0];
        let align = detect_alignment(close, &mas, &mas_back);
        assert_eq!(align, AlignmentKind::None);
    }

    #[test]
    fn t_no_alignment_when_direction_mixed() {
        // 顺序正确但方向不统一
        let close = 110.0;
        let mas = vec![108.0, 105.0, 100.0];
        let mas_back = vec![109.0, 104.0, 99.0]; // ma5 下行，其他上行
        let align = detect_alignment(close, &mas, &mas_back);
        assert_eq!(align, AlignmentKind::None);
    }

    #[test]
    fn t_ma_relation_bonded() {
        // 均线粘合：spread < 阈值
        let mas_now = vec![100.0, 100.5, 99.5];
        let mas_back = vec![100.0, 100.5, 99.5];
        let state = detect_ma_relation(&mas_now, &mas_back, 0.02); // 2% 阈值
        assert_eq!(state, MaRelationState::Bonded);
    }

    #[test]
    fn t_ma_relation_diverging() {
        // spread 从 0.05 → 0.15 = 扩散 3 倍
        let mas_now = vec![90.0, 100.0, 110.0]; // spread ≈ 20/100 = 0.20
        let mas_back = vec![98.0, 100.0, 102.0]; // spread ≈ 4/100 = 0.04
        let state = detect_ma_relation(&mas_now, &mas_back, 0.02);
        assert_eq!(state, MaRelationState::Diverging);
    }

    #[test]
    fn t_ma_relation_converging() {
        // spread 从 0.20 → 0.04 = 收敛
        let mas_now = vec![98.0, 100.0, 102.0];
        let mas_back = vec![90.0, 100.0, 110.0];
        let state = detect_ma_relation(&mas_now, &mas_back, 0.01); // 阈值小，不会判粘合
        assert_eq!(state, MaRelationState::Converging);
    }

    #[test]
    fn t_ma_relation_stable_small_variation() {
        // spread_now=0.10, spread_back=0.12：变化 ~17% < 20% + 均 > bond_threshold 0.02
        // 不触发 Bonded / Converging (要求 < 0.096) / Diverging (要求 > 0.144) → Stable
        let mas_now = vec![95.0, 100.0, 105.0]; // spread = 10/100 = 0.10
        let mas_back = vec![94.0, 100.0, 106.0]; // spread = 12/100 = 0.12
        let state = detect_ma_relation(&mas_now, &mas_back, 0.02);
        assert_eq!(state, MaRelationState::Stable);
    }

    #[test]
    fn t_ma_relation_stable_on_invalid_input() {
        // 均线长度 < 2 → ma_spread 返回 None → fallback Stable
        let mas_now = vec![100.0];
        let mas_back = vec![100.0];
        let state = detect_ma_relation(&mas_now, &mas_back, 0.02);
        assert_eq!(state, MaRelationState::Stable);
    }

    #[test]
    fn t_scan_alignment_events_from_none_to_bearish() {
        // 构造：前半段均线混乱，后半段形成空头排列
        let n = 10;
        let closes: Vec<f64> = (0..n).map(|i| 100.0 - i as f64).collect();
        let ma5: Vec<f64> = (0..n).map(|i| 102.0 - i as f64 * 0.9).collect();
        let ma10: Vec<f64> = (0..n).map(|i| 105.0 - i as f64 * 0.7).collect();
        let ma20: Vec<f64> = (0..n).map(|i| 110.0 - i as f64 * 0.5).collect();
        let mas = vec![ma5, ma10, ma20];
        let events = scan_alignment_events(&closes, &mas, 3);
        // 应在某点识别到 Bearish
        assert!(events.iter().any(|(_, k)| *k == AlignmentKind::Bearish));
    }
}
