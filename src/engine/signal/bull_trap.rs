//! F3：多头陷阱 / 空头陷阱识别器（R-P1-17）
//!
//! 原书跨书铁证：
//! - **ma p.155** 附近：假突破均线后迅速跌回 = 多头陷阱
//! - **candle p.700** "向下跌破做空头陷阱的底部三角形"
//! - **trend p.203** 3% 阈值：未达 3% = 伪突破
//!
//! # 工程规则
//!
//! 1. 价格**有效突破**（≥ 3%）某关键位（均线 / 趋势线 / 颈线 / 前高前低）
//! 2. 在 **N 根 K 线内**（默认 5 根）价格**重新回落**至关键位下方
//! 3. 回落必须**有效**（回落 ≥ 3% 或 收盘破位）
//! 4. 满足以上 3 条 → 标记为多头陷阱
//!
//! # 对称
//!
//! 空头陷阱（BearTrap）= 向下破位后迅速反弹 = 多头陷阱的镜像
//!
//! # 使用
//!
//! ```
//! use aura_trade::engine::signal::bull_trap::*;
//!
//! //  K 线 price 序列（收盘价）
//! let closes = vec![100.0, 101.0, 103.5, 104.0, 102.5, 99.5, 98.0];
//! //  关键位 = 100
//! let params = TrapParams::default();
//! let traps = detect_traps(&closes, 100.0, &params);
//! // 在 i=2 突破（>3%），i=5 跌回（<3% 以下）→ 1 个多头陷阱
//! ```

use serde::{Deserialize, Serialize};

/// 陷阱类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TrapKind {
    /// 多头陷阱：假突破向上后跌回 → 看空信号
    Bull,
    /// 空头陷阱：假跌破向下后反弹 → 看多信号
    Bear,
}

impl TrapKind {
    pub fn label(&self) -> &'static str {
        match self {
            TrapKind::Bull => "多头陷阱",
            TrapKind::Bear => "空头陷阱",
        }
    }

    /// 陷阱触发后的反向信号方向（+1 看多 / -1 看空）
    pub fn reverse_signal_direction(&self) -> i8 {
        match self {
            TrapKind::Bull => -1, // 多头陷阱 → 看空
            TrapKind::Bear => 1,  // 空头陷阱 → 看多
        }
    }
}

/// 陷阱事件
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrapEvent {
    pub kind: TrapKind,
    /// 突破/跌破发生的 K 线索引
    pub breakout_index: usize,
    /// 回落/反弹确认的 K 线索引
    pub reversal_index: usize,
    /// 关键位价格
    pub key_price: f64,
    /// 突破时的极值价
    pub extreme_price: f64,
}

/// 参数
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct TrapParams {
    /// 有效突破/跌破阈值（默认 3%，原书铁证）
    pub effective_break_pct: f64,
    /// 最大回落/反弹窗口（N 根 K 线内）
    pub reversal_window: usize,
    /// 回落/反弹有效阈值（默认 3%）
    pub reversal_threshold_pct: f64,
}

impl Default for TrapParams {
    fn default() -> Self {
        Self {
            effective_break_pct: 0.03,    // 原书 3% 铁证
            reversal_window: 5,           // 5 根 K 线内
            reversal_threshold_pct: 0.03, // 回落也需 3%
        }
    }
}

/// 扫描多头/空头陷阱
///
/// # 参数
/// - `closes`：收盘价序列
/// - `key_price`：关键位（均线值 / 趋势线投影价 / 颈线等）
/// - `params`：参数
///
/// # 返回
/// - 按时间顺序排列的陷阱事件列表
pub fn detect_traps(
    closes: &[f64],
    key_price: f64,
    params: &TrapParams,
) -> Vec<TrapEvent> {
    if closes.len() < 2 || !key_price.is_finite() || key_price.abs() < 1e-9 {
        return Vec::new();
    }

    let mut out = Vec::new();

    for (i, &c) in closes.iter().enumerate() {
        if !c.is_finite() {
            continue;
        }
        let diff_pct = (c - key_price) / key_price.abs();

        // 检测向上有效突破
        if diff_pct > params.effective_break_pct {
            // 向后扫描 reversal_window 根，查找是否回落破位
            let end = (i + params.reversal_window).min(closes.len() - 1);
            for j in (i + 1)..=end {
                let cj = closes[j];
                if !cj.is_finite() {
                    continue;
                }
                let diff_j = (cj - key_price) / key_price.abs();
                // 回落至关键位下方（超过阈值）
                if diff_j < -params.reversal_threshold_pct {
                    out.push(TrapEvent {
                        kind: TrapKind::Bull,
                        breakout_index: i,
                        reversal_index: j,
                        key_price,
                        extreme_price: c,
                    });
                    break;
                }
            }
        }

        // 检测向下有效跌破
        if diff_pct < -params.effective_break_pct {
            let end = (i + params.reversal_window).min(closes.len() - 1);
            for j in (i + 1)..=end {
                let cj = closes[j];
                if !cj.is_finite() {
                    continue;
                }
                let diff_j = (cj - key_price) / key_price.abs();
                // 反弹至关键位上方
                if diff_j > params.reversal_threshold_pct {
                    out.push(TrapEvent {
                        kind: TrapKind::Bear,
                        breakout_index: i,
                        reversal_index: j,
                        key_price,
                        extreme_price: c,
                    });
                    break;
                }
            }
        }
    }

    out
}

pub fn detect_traps_with_key_series(
    closes: &[f64],
    key_prices: &[f64],
    params: &TrapParams,
) -> Vec<TrapEvent> {
    let n = closes.len().min(key_prices.len());
    if n < 2 {
        return Vec::new();
    }

    let mut out = Vec::new();

    for i in 0..n {
        let c = closes[i];
        let key_price = key_prices[i];
        if !c.is_finite() || !key_price.is_finite() || key_price.abs() < 1e-9 {
            continue;
        }
        let diff_pct = (c - key_price) / key_price.abs();

        if diff_pct > params.effective_break_pct {
            let end = (i + params.reversal_window).min(n - 1);
            for j in (i + 1)..=end {
                let cj = closes[j];
                let kj = key_prices[j];
                if !cj.is_finite() || !kj.is_finite() || kj.abs() < 1e-9 {
                    continue;
                }
                let diff_j = (cj - kj) / kj.abs();
                if diff_j < -params.reversal_threshold_pct {
                    out.push(TrapEvent {
                        kind: TrapKind::Bull,
                        breakout_index: i,
                        reversal_index: j,
                        key_price,
                        extreme_price: c,
                    });
                    break;
                }
            }
        }

        if diff_pct < -params.effective_break_pct {
            let end = (i + params.reversal_window).min(n - 1);
            for j in (i + 1)..=end {
                let cj = closes[j];
                let kj = key_prices[j];
                if !cj.is_finite() || !kj.is_finite() || kj.abs() < 1e-9 {
                    continue;
                }
                let diff_j = (cj - kj) / kj.abs();
                if diff_j > params.reversal_threshold_pct {
                    out.push(TrapEvent {
                        kind: TrapKind::Bear,
                        breakout_index: i,
                        reversal_index: j,
                        key_price,
                        extreme_price: c,
                    });
                    break;
                }
            }
        }
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn t_bull_trap_detected() {
        // 突破 100 → 103.5（+3.5% > 3%）→ 跌回 96（-4% < -3%）
        let closes = vec![100.0, 101.0, 103.5, 104.0, 102.5, 99.5, 96.0];
        let traps = detect_traps(&closes, 100.0, &TrapParams::default());
        assert!(!traps.is_empty(), "应识别多头陷阱");
        assert_eq!(traps[0].kind, TrapKind::Bull);
        assert!(traps[0].breakout_index <= 3);
    }

    #[test]
    fn t_bear_trap_detected() {
        // 跌破 100 → 96（-4% < -3%）→ 反弹至 104（+4% > 3%）
        let closes = vec![100.0, 99.0, 96.5, 96.0, 98.0, 104.0];
        let traps = detect_traps(&closes, 100.0, &TrapParams::default());
        assert!(!traps.is_empty(), "应识别空头陷阱");
        assert_eq!(traps[0].kind, TrapKind::Bear);
    }

    #[test]
    fn t_no_trap_if_no_reversal_in_window() {
        // 突破后持续上涨（未回落）→ 无陷阱
        let closes = vec![100.0, 103.5, 105.0, 107.0, 109.0, 110.0, 112.0];
        let traps = detect_traps(&closes, 100.0, &TrapParams::default());
        assert_eq!(traps.len(), 0);
    }

    #[test]
    fn t_no_trap_if_breakout_insufficient() {
        // 仅突破 2%（< 3% 阈值）→ 不算有效突破，不产生陷阱
        let closes = vec![100.0, 101.0, 101.5, 102.0, 98.0];
        let traps = detect_traps(&closes, 100.0, &TrapParams::default());
        assert_eq!(traps.len(), 0);
    }

    #[test]
    fn t_reverse_signal_direction_correct() {
        assert_eq!(TrapKind::Bull.reverse_signal_direction(), -1);
        assert_eq!(TrapKind::Bear.reverse_signal_direction(), 1);
    }

    #[test]
    fn t_empty_input_returns_empty() {
        assert!(detect_traps(&[], 100.0, &TrapParams::default()).is_empty());
        assert!(detect_traps(&[100.0], 100.0, &TrapParams::default()).is_empty());
    }

    #[test]
    fn t_invalid_key_price_returns_empty() {
        let closes = vec![100.0, 103.5, 96.0];
        assert!(detect_traps(&closes, 0.0, &TrapParams::default()).is_empty());
        assert!(detect_traps(&closes, f64::NAN, &TrapParams::default()).is_empty());
    }

    #[test]
    fn t_dynamic_key_series_uses_bar_aligned_key_price() {
        let closes = vec![100.0, 103.5, 96.0];
        let keys = vec![100.0, 100.0, 100.0];
        let traps = detect_traps_with_key_series(&closes, &keys, &TrapParams::default());
        assert_eq!(traps.len(), 1);
        assert_eq!(traps[0].kind, TrapKind::Bull);
        assert_eq!(traps[0].key_price, 100.0);
    }

    #[test]
    fn t_dynamic_key_series_does_not_use_latest_key_for_history() {
        let closes = vec![100.0, 103.5, 96.0];
        let keys = vec![100.0, 120.0, 120.0];
        let traps = detect_traps_with_key_series(&closes, &keys, &TrapParams::default());
        assert!(traps.is_empty());
    }
}
