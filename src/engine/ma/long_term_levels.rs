//! A11：120 / 240 日长期压力位（R-P1-29，Sprint 14）
//!
//! 原书 **ma p.290+** 铁证：
//! > "120 日均线是中长期趋势分水岭；240 日均线是长期年线，**跨越 240 日 = 长期趋势改变**"
//!
//! 本模块专门跟踪 120 / 240 日均线作为**关键压力/支撑位**：
//!
//! - 价格从下方接近 240 日 → **长期压力**
//! - 价格突破 240 日 → **长期牛市确认**（需要成交量配合）
//! - 价格从上方跌破 240 日 → **长期熊市确认**（无需成交量）
//!
//! # 与 sr.rs 的区别
//!
//! - `trend/sr.rs`：通用支撑/压力（摆动高低点）
//! - **本模块**：专注于 120/240 日均线的**长期角色**

use serde::{Deserialize, Serialize};

use super::compute;

/// 长期均线关键事件类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum LongTermLevelEvent {
    /// 价格从下方触及 240 日（长期压力测试）
    TouchResistance240,
    /// 价格从上方触及 240 日（长期支撑测试）
    TouchSupport240,
    /// 价格有效突破 240 日（长期牛市确认，R-P1-29 核心）
    BreakAbove240,
    /// 价格有效跌破 240 日（长期熊市确认）
    BreakBelow240,
    /// 价格触及 120 日（中长期压力/支撑）
    Touch120,
    /// 价格有效突破 120 日
    BreakAbove120,
    /// 价格有效跌破 120 日
    BreakBelow120,
}

impl LongTermLevelEvent {
    pub fn label(&self) -> &'static str {
        use LongTermLevelEvent::*;
        match self {
            TouchResistance240 => "触及 240 日压力",
            TouchSupport240 => "触及 240 日支撑",
            BreakAbove240 => "突破 240 日（长期牛市确认）",
            BreakBelow240 => "跌破 240 日（长期熊市确认）",
            Touch120 => "触及 120 日",
            BreakAbove120 => "突破 120 日",
            BreakBelow120 => "跌破 120 日",
        }
    }

    pub fn direction(&self) -> i8 {
        use LongTermLevelEvent::*;
        match self {
            BreakAbove240 | BreakAbove120 | TouchSupport240 => 1,
            BreakBelow240 | BreakBelow120 | TouchResistance240 => -1,
            Touch120 => 0,
        }
    }

    /// 是否为长期级别（240 日）—— 长期信号权重更高
    pub fn is_long_term(&self) -> bool {
        use LongTermLevelEvent::*;
        matches!(
            self,
            TouchResistance240 | TouchSupport240 | BreakAbove240 | BreakBelow240
        )
    }
}

/// 事件记录
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LongTermLevelHit {
    pub index: usize,
    pub event: LongTermLevelEvent,
    /// 事件发生时价格
    pub price: f64,
    /// 所触及 ma 的值
    pub ma_value: f64,
}

/// 参数
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct LongTermParams {
    /// "触及"容忍（默认 1%：|价格 - ma| / ma < this）
    pub touch_tolerance: f64,
    /// "有效突破"阈值（默认 3%：跨书铁证）
    pub effective_break_pct: f64,
}

impl Default for LongTermParams {
    fn default() -> Self {
        Self {
            touch_tolerance: 0.01,
            effective_break_pct: 0.03,
        }
    }
}

/// 扫描 120 / 240 日均线的关键事件
pub fn scan_long_term_levels(
    closes: &[f64],
    params: &LongTermParams,
) -> Vec<LongTermLevelHit> {
    let ma120 = compute::sma(closes, 120);
    let ma240 = compute::sma(closes, 240);
    let n = closes.len();
    if n < 241 {
        return Vec::new();
    }
    let mut out = Vec::new();

    for i in 1..n {
        let c = closes[i];
        let c_prev = closes[i - 1];
        if !c.is_finite() || !c_prev.is_finite() {
            continue;
        }

        // --- 240 日 ---
        let m240 = ma240[i];
        let m240_prev = ma240[i - 1];
        if m240.is_finite() && m240_prev.is_finite() {
            let diff = (c - m240) / m240.abs().max(1e-9);
            let diff_prev = (c_prev - m240_prev) / m240_prev.abs().max(1e-9);

            // 突破/跌破（有效）
            if diff_prev <= 0.0 && diff > params.effective_break_pct {
                out.push(LongTermLevelHit {
                    index: i,
                    event: LongTermLevelEvent::BreakAbove240,
                    price: c,
                    ma_value: m240,
                });
                continue;
            }
            if diff_prev >= 0.0 && diff < -params.effective_break_pct {
                out.push(LongTermLevelHit {
                    index: i,
                    event: LongTermLevelEvent::BreakBelow240,
                    price: c,
                    ma_value: m240,
                });
                continue;
            }
            // 触及（未突破）
            if diff.abs() < params.touch_tolerance && diff_prev.abs() > params.touch_tolerance {
                let event = if diff_prev < 0.0 {
                    LongTermLevelEvent::TouchResistance240
                } else {
                    LongTermLevelEvent::TouchSupport240
                };
                out.push(LongTermLevelHit {
                    index: i,
                    event,
                    price: c,
                    ma_value: m240,
                });
                continue;
            }
        }

        // --- 120 日 ---
        let m120 = ma120[i];
        let m120_prev = ma120[i - 1];
        if m120.is_finite() && m120_prev.is_finite() {
            let diff = (c - m120) / m120.abs().max(1e-9);
            let diff_prev = (c_prev - m120_prev) / m120_prev.abs().max(1e-9);

            if diff_prev <= 0.0 && diff > params.effective_break_pct {
                out.push(LongTermLevelHit {
                    index: i,
                    event: LongTermLevelEvent::BreakAbove120,
                    price: c,
                    ma_value: m120,
                });
                continue;
            }
            if diff_prev >= 0.0 && diff < -params.effective_break_pct {
                out.push(LongTermLevelHit {
                    index: i,
                    event: LongTermLevelEvent::BreakBelow120,
                    price: c,
                    ma_value: m120,
                });
                continue;
            }
            if diff.abs() < params.touch_tolerance && diff_prev.abs() > params.touch_tolerance {
                out.push(LongTermLevelHit {
                    index: i,
                    event: LongTermLevelEvent::Touch120,
                    price: c,
                    ma_value: m120,
                });
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn t_break_above_240_detected() {
        // 构造：前 240 根横盘 100，第 241 根涨到 104（+4%，超过 3% 有效突破）
        let mut closes = vec![100.0; 241];
        closes[240] = 104.0;
        let hits = scan_long_term_levels(&closes, &LongTermParams::default());
        let has_break_above = hits.iter().any(|h| {
            h.index == 240 && h.event == LongTermLevelEvent::BreakAbove240
        });
        assert!(has_break_above, "应识别 240 日突破；实际：{:?}", hits);
    }

    #[test]
    fn t_break_below_240_detected() {
        // 构造：前 240 根 100，最后一根 95.5（跌破 3%+）
        let mut closes = vec![100.0; 241];
        closes[240] = 95.5;
        let hits = scan_long_term_levels(&closes, &LongTermParams::default());
        let has = hits
            .iter()
            .any(|h| h.index == 240 && h.event == LongTermLevelEvent::BreakBelow240);
        assert!(has);
    }

    #[test]
    fn t_too_short_data_returns_empty() {
        let closes = vec![100.0; 100]; // < 241
        let hits = scan_long_term_levels(&closes, &LongTermParams::default());
        assert!(hits.is_empty());
    }

    #[test]
    fn t_event_metadata_correct() {
        assert_eq!(
            LongTermLevelEvent::BreakAbove240.direction(),
            1,
        );
        assert_eq!(
            LongTermLevelEvent::BreakBelow240.direction(),
            -1,
        );
        assert!(LongTermLevelEvent::BreakAbove240.is_long_term());
        assert!(!LongTermLevelEvent::BreakAbove120.is_long_term());
    }

    #[test]
    fn t_small_touch_not_break() {
        // 价格触及 240 日但未达 3% 阈值 → 不应判定为突破
        let mut closes = vec![100.0; 241];
        closes[240] = 100.5; // 只 +0.5%
        let hits = scan_long_term_levels(&closes, &LongTermParams::default());
        // 应不包含 BreakAbove
        let has_break = hits
            .iter()
            .any(|h| h.event == LongTermLevelEvent::BreakAbove240);
        assert!(!has_break, "未达 3% 阈值不应判为突破");
    }
}
