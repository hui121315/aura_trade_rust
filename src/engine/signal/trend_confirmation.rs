//! F10：趋势确认组合判定器（R-P1-18 / R-P1-22，Sprint 16）
//!
//! # 原书铁证
//!
//! ## R-P1-18 L4 共振警告（ma p.100 + 轮次 14）
//!
//! 葛南维 L4（逆势反弹买入）本身就要求**轻仓**（≤30%）。
//! 如果 L4 与**其他危险信号**（断头铡刀 / 均线粘合 / 空头排列）共振，
//! 则应**升级为"不买反卖"警告**。
//!
//! ## R-P1-22 HH/HL + 3% 突破双重确认（trend p.203）
//!
//! 单纯 3% 突破易被假突破欺骗。真正的趋势反转需要：
//! 1. 有效突破（≥ 3% 原书阈值）
//! 2. 道氏 HH/HL 结构确认（最近 swing 创出新高 + 新高低）
//!
//! 两条件同时满足 = 强确认趋势反转

use serde::{Deserialize, Serialize};

use crate::engine::ma::advanced::MaAdvancedKind;
use crate::engine::ma::GranvilleRule;
use crate::engine::trend::{DowPhase, SwingKind, SwingPoint};

// ==================== R-P1-18 L4 共振警告 ====================

/// L4 共振警告级别
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum L4WarningLevel {
    /// 无警告（L4 单独出现，正常轻仓即可）
    None,
    /// 警告：L4 同时伴随均线粘合或毒蜘蛛 → 建议不买
    Caution,
    /// 严重：L4 + 断头铡刀 / 空头排列 → **反转为卖出信号**
    Critical,
}

impl L4WarningLevel {
    pub fn label(&self) -> &'static str {
        match self {
            L4WarningLevel::None => "无警告",
            L4WarningLevel::Caution => "警告（不宜买入）",
            L4WarningLevel::Critical => "严重（应反向卖出）",
        }
    }

    /// 推荐仓位上限（0.0 - 1.0）
    pub fn max_position(&self) -> f64 {
        match self {
            L4WarningLevel::None => 0.30,      // 原书 L4 默认
            L4WarningLevel::Caution => 0.10,   // 极轻仓
            L4WarningLevel::Critical => 0.0,   // 不买
        }
    }
}

/// 检测 L4 与其他危险信号的共振
///
/// # 参数
/// - `current_rule`：当前葛南维规则
/// - `ma_advanced`：可选当前 ma 高级形态
/// - `current_phase`：道氏当前趋势阶段
pub fn detect_l4_warning(
    current_rule: GranvilleRule,
    ma_advanced: Option<MaAdvancedKind>,
    current_phase: DowPhase,
) -> L4WarningLevel {
    // 仅处理 L4 买入信号
    if current_rule != GranvilleRule::B4DivergenceBuy {
        return L4WarningLevel::None;
    }

    // Critical：断头铡刀 或 确认的空头排列（Downtrend）
    if let Some(MaAdvancedKind::Guillotine) = ma_advanced {
        return L4WarningLevel::Critical;
    }
    if current_phase == DowPhase::Downtrend {
        // L4 本来就在下降趋势中才触发，但如果道氏也确认下降 → 危险
        return L4WarningLevel::Critical;
    }

    // Caution：毒蜘蛛
    if let Some(MaAdvancedKind::PoissonSpider) = ma_advanced {
        return L4WarningLevel::Caution;
    }

    L4WarningLevel::None
}

// ==================== R-P1-22 HH/HL + 3% 确认 ====================

/// 趋势反转确认结果
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ReversalConfirmation {
    /// 完整确认（3% 突破 + HH/HL）
    Confirmed,
    /// 部分确认（仅 3% 突破，无结构确认）
    PartialOnlyPriceBreak,
    /// 部分确认（仅 HH/HL，无 3% 突破）
    PartialOnlyStructure,
    /// 未确认
    NotConfirmed,
}

impl ReversalConfirmation {
    pub fn label(&self) -> &'static str {
        use ReversalConfirmation::*;
        match self {
            Confirmed => "双重确认（3%+HH/HL）",
            PartialOnlyPriceBreak => "仅价格突破",
            PartialOnlyStructure => "仅结构确认",
            NotConfirmed => "未确认",
        }
    }

    pub fn is_reliable(&self) -> bool {
        matches!(self, ReversalConfirmation::Confirmed)
    }
}

/// 检查看涨反转确认（从下降 → 上升）
///
/// # 参数
/// - `swings`：swing 点序列
/// - `price_now`：当前价
/// - `key_level`：关键位（如趋势线 / 均线 / 颈线）
/// - `effective_pct`：3% 有效阈值
pub fn confirm_bullish_reversal(
    swings: &[SwingPoint],
    price_now: f64,
    key_level: f64,
    effective_pct: f64,
) -> ReversalConfirmation {
    let price_break = if key_level.abs() < 1e-9 {
        false
    } else {
        (price_now - key_level) / key_level.abs() >= effective_pct
    };

    // 道氏 HH/HL 确认：最新高 > 前高 且 最新低 > 前低
    let highs: Vec<&SwingPoint> = swings
        .iter()
        .filter(|s| s.kind == SwingKind::High)
        .collect();
    let lows: Vec<&SwingPoint> = swings
        .iter()
        .filter(|s| s.kind == SwingKind::Low)
        .collect();

    let structure_ok = highs.len() >= 2
        && lows.len() >= 2
        && {
            let hh = highs[highs.len() - 1].price > highs[highs.len() - 2].price;
            let hl = lows[lows.len() - 1].price > lows[lows.len() - 2].price;
            hh && hl
        };

    match (price_break, structure_ok) {
        (true, true) => ReversalConfirmation::Confirmed,
        (true, false) => ReversalConfirmation::PartialOnlyPriceBreak,
        (false, true) => ReversalConfirmation::PartialOnlyStructure,
        (false, false) => ReversalConfirmation::NotConfirmed,
    }
}

/// 看跌反转确认（从上升 → 下降）
pub fn confirm_bearish_reversal(
    swings: &[SwingPoint],
    price_now: f64,
    key_level: f64,
    effective_pct: f64,
) -> ReversalConfirmation {
    let price_break = if key_level.abs() < 1e-9 {
        false
    } else {
        (key_level - price_now) / key_level.abs() >= effective_pct
    };

    // 道氏 LL/LH 确认：最新低 < 前低 且 最新高 < 前高
    let highs: Vec<&SwingPoint> = swings
        .iter()
        .filter(|s| s.kind == SwingKind::High)
        .collect();
    let lows: Vec<&SwingPoint> = swings
        .iter()
        .filter(|s| s.kind == SwingKind::Low)
        .collect();

    let structure_ok = highs.len() >= 2
        && lows.len() >= 2
        && {
            let ll = lows[lows.len() - 1].price < lows[lows.len() - 2].price;
            let lh = highs[highs.len() - 1].price < highs[highs.len() - 2].price;
            ll && lh
        };

    match (price_break, structure_ok) {
        (true, true) => ReversalConfirmation::Confirmed,
        (true, false) => ReversalConfirmation::PartialOnlyPriceBreak,
        (false, true) => ReversalConfirmation::PartialOnlyStructure,
        (false, false) => ReversalConfirmation::NotConfirmed,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sp(idx: usize, price: f64, kind: SwingKind) -> SwingPoint {
        SwingPoint {
            index: idx,
            time: (idx as i64) * 86_400_000,
            price,
            kind,
        }
    }

    // -------- R-P1-18 L4 共振警告 --------

    #[test]
    fn t_l4_alone_no_warning() {
        let w = detect_l4_warning(GranvilleRule::B4DivergenceBuy, None, DowPhase::Uptrend);
        assert_eq!(w, L4WarningLevel::None);
        assert_eq!(w.max_position(), 0.30);
    }

    #[test]
    fn t_l4_with_guillotine_critical() {
        let w = detect_l4_warning(
            GranvilleRule::B4DivergenceBuy,
            Some(MaAdvancedKind::Guillotine),
            DowPhase::Unknown,
        );
        assert_eq!(w, L4WarningLevel::Critical);
        assert_eq!(w.max_position(), 0.0);
    }

    #[test]
    fn t_l4_in_downtrend_critical() {
        let w = detect_l4_warning(
            GranvilleRule::B4DivergenceBuy,
            None,
            DowPhase::Downtrend,
        );
        assert_eq!(w, L4WarningLevel::Critical);
    }

    #[test]
    fn t_l4_with_poisson_spider_caution() {
        let w = detect_l4_warning(
            GranvilleRule::B4DivergenceBuy,
            Some(MaAdvancedKind::PoissonSpider),
            DowPhase::Uptrend,
        );
        assert_eq!(w, L4WarningLevel::Caution);
        assert!(w.max_position() < 0.30);
    }

    #[test]
    fn t_non_l4_no_warning() {
        // 不是 L4 → 不触发此警告
        let w = detect_l4_warning(
            GranvilleRule::B1BreakoutBuy,
            Some(MaAdvancedKind::Guillotine),
            DowPhase::Downtrend,
        );
        assert_eq!(w, L4WarningLevel::None);
    }

    // -------- R-P1-22 HH/HL + 3% 双重确认 --------

    #[test]
    fn t_bullish_confirmed_both_conditions() {
        // 3% 突破 + HH/HL 全满足
        let swings = vec![
            sp(0, 100.0, SwingKind::High),
            sp(5, 95.0, SwingKind::Low),
            sp(10, 105.0, SwingKind::High), // HH (105 > 100)
            sp(15, 98.0, SwingKind::Low),   // HL (98 > 95)
        ];
        let result = confirm_bullish_reversal(&swings, 105.0, 100.0, 0.03);
        assert_eq!(result, ReversalConfirmation::Confirmed);
        assert!(result.is_reliable());
    }

    #[test]
    fn t_bullish_only_price_break() {
        // 3% 突破但无 HH/HL
        let swings = vec![
            sp(0, 100.0, SwingKind::High),
            sp(5, 95.0, SwingKind::Low),
            sp(10, 98.0, SwingKind::High), // 未 HH
            sp(15, 94.0, SwingKind::Low),
        ];
        let result = confirm_bullish_reversal(&swings, 105.0, 100.0, 0.03);
        assert_eq!(result, ReversalConfirmation::PartialOnlyPriceBreak);
        assert!(!result.is_reliable());
    }

    #[test]
    fn t_bullish_only_structure() {
        // HH/HL 满足但未 3% 突破
        let swings = vec![
            sp(0, 100.0, SwingKind::High),
            sp(5, 95.0, SwingKind::Low),
            sp(10, 105.0, SwingKind::High),
            sp(15, 98.0, SwingKind::Low),
        ];
        // 只涨 1%
        let result = confirm_bullish_reversal(&swings, 101.0, 100.0, 0.03);
        assert_eq!(result, ReversalConfirmation::PartialOnlyStructure);
    }

    #[test]
    fn t_bullish_not_confirmed() {
        let swings = vec![
            sp(0, 100.0, SwingKind::High),
            sp(5, 95.0, SwingKind::Low),
        ];
        let result = confirm_bullish_reversal(&swings, 100.5, 100.0, 0.03);
        assert_eq!(result, ReversalConfirmation::NotConfirmed);
    }

    #[test]
    fn t_bearish_confirmed() {
        // 3% 跌破 + LL/LH
        let swings = vec![
            sp(0, 100.0, SwingKind::Low),
            sp(5, 105.0, SwingKind::High),
            sp(10, 95.0, SwingKind::Low),   // LL (95 < 100)
            sp(15, 102.0, SwingKind::High), // LH (102 < 105)
        ];
        let result = confirm_bearish_reversal(&swings, 95.0, 100.0, 0.03);
        assert_eq!(result, ReversalConfirmation::Confirmed);
    }

    #[test]
    fn t_reliable_flag_correct() {
        assert!(ReversalConfirmation::Confirmed.is_reliable());
        assert!(!ReversalConfirmation::PartialOnlyPriceBreak.is_reliable());
        assert!(!ReversalConfirmation::PartialOnlyStructure.is_reliable());
        assert!(!ReversalConfirmation::NotConfirmed.is_reliable());
    }
}
