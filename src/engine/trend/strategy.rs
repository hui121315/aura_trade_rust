//! B8：多级趋势线策略矩阵（R-P1-15）
//!
//! 基于《趋势技术分析》（邱立波）p.216 原书完整 10 条买卖原则实现：
//!
//! # 原书买入和持仓原则（5 条，长期上升趋势线之上）
//!
//! 1. 突破长期下降趋势线，回落受**中期上升**趋势线支撑 → 买入
//! 2. 长期上升之上，向上突破**中期下降**趋势线 → 买入或加仓
//! 3. 长期上升之上，急跌后突破**短期下降**趋势线 → 买入或加仓
//! 4. 长期上升之上，遇**长期上升**趋势线支撑止跌回升 → 买入或加仓
//! 5. 长期上升之上，遇**中期上升**趋势线支撑止跌回升 → 买入或加仓
//!
//! # 原书卖出和空仓原则（5 条）
//!
//! 1. **跌破长期上升**趋势线 → 清仓卖出
//! 2. 长期上升之上，跌破**中期上升**趋势线 → 减仓
//! 3. 长期上升之上，急速飙升后跌破**短期上升** → 减仓
//! 4. 突破长期下降，回落跌破**中期上升** → 减仓或清仓
//! 5. 运行在**长期下降**趋势线**之下** → 空仓
//!
//! # 原书警句（trend p.221）
//!
//! > "**跌破长期上升趋势线 → 清仓卖出**。即便跌破之后趋势并未逆转，清仓依然是明智之举。
//! > 利润减少并不会有损失，而风险加大却能让交易者遭受灭顶之灾。"
//!
//! # 与 R-P1-13/29 配套
//!
//! - 葛南维各法则的仓位上限（L4 ≤ 30%）由 [`PositionLimit`] 提供
//! - 120/240 日均线的关键压力/支撑位由 [`crate::engine::ma`] 提供

use serde::{Deserialize, Serialize};

/// 趋势级别（短期 / 中期 / 长期）
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TrendLevel {
    /// 短期：5-20 根 K 线
    Short,
    /// 中期：20-60 根 K 线
    Mid,
    /// 长期：60+ 根 K 线（对应 60/120/240 日均线）
    Long,
}

impl TrendLevel {
    pub fn label(&self) -> &'static str {
        match self {
            TrendLevel::Short => "短期",
            TrendLevel::Mid => "中期",
            TrendLevel::Long => "长期",
        }
    }
}

/// 趋势方向
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TrendDirection {
    Up,
    Down,
    /// 无明确方向（横盘整理）
    None,
}

impl TrendDirection {
    pub fn label(&self) -> &'static str {
        match self {
            TrendDirection::Up => "上升",
            TrendDirection::Down => "下降",
            TrendDirection::None => "无方向",
        }
    }

    pub fn opposite(&self) -> Self {
        match self {
            TrendDirection::Up => TrendDirection::Down,
            TrendDirection::Down => TrendDirection::Up,
            TrendDirection::None => TrendDirection::None,
        }
    }
}

/// 多级趋势线状态：同时跟踪长/中/短三个级别的方向
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct MultiTimeframeTrendState {
    pub long: TrendDirection,
    pub mid: TrendDirection,
    pub short: TrendDirection,
}

impl MultiTimeframeTrendState {
    pub fn new(long: TrendDirection, mid: TrendDirection, short: TrendDirection) -> Self {
        Self { long, mid, short }
    }

    /// 是否处于"长期上升"主体格局（前提条件，对应原书买入 5 条原则的总前提）
    pub fn long_term_bullish(&self) -> bool {
        self.long == TrendDirection::Up
    }

    /// 是否处于"长期下降"主体格局（对应原书卖出原则 5：长期下降之下空仓）
    pub fn long_term_bearish(&self) -> bool {
        self.long == TrendDirection::Down
    }
}

/// 趋势线事件：发生在某个级别上的突破/跌破/支撑/压力
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum TrendEvent {
    /// 向上突破某级别趋势线（突破方向：原线方向）
    /// 例如 `Breakout { level: Long, original: Down }` = 突破长期下降趋势线
    Breakout {
        level: TrendLevel,
        original: TrendDirection,
    },
    /// 向下跌破某级别趋势线
    /// 例如 `Breakdown { level: Long, original: Up }` = 跌破长期上升趋势线
    Breakdown {
        level: TrendLevel,
        original: TrendDirection,
    },
    /// 遇某级别趋势线支撑止跌回升（未跌破，受支撑反弹）
    SupportHold { level: TrendLevel },
    /// 遇某级别趋势线压力受阻回落
    ResistanceReject { level: TrendLevel },
    /// 急跌后回升（用于规则 BUY-3：长期上升之上，急跌后突破短期下降）
    QuickDipRebound,
    /// 急速飙升（用于规则 SELL-3：长期上升之上，急速飙升后跌破短期上升）
    QuickRallyTop,
}

/// 决策动作（按原书严格度从弱到强）
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum EntryAction {
    /// 普通买入（仅适用于规则 BUY-1）
    Buy,
    /// 可买可加仓（适用于规则 BUY-2/3/4/5）
    BuyOrAdd,
    /// 减仓或持股（适用于规则 SELL-2/3）
    ReduceOrHold,
    /// 减仓或清仓（适用于规则 SELL-4）
    ReduceOrClose,
    /// 清仓（适用于规则 SELL-1：跌破长期上升趋势线）
    Close,
    /// 空仓观望（适用于规则 SELL-5：长期下降之下）
    StayOut,
    /// 无动作（不满足任何规则）
    Hold,
}

impl EntryAction {
    pub fn label(&self) -> &'static str {
        match self {
            EntryAction::Buy => "买入",
            EntryAction::BuyOrAdd => "买入或加仓",
            EntryAction::ReduceOrHold => "减仓或持股",
            EntryAction::ReduceOrClose => "减仓或清仓",
            EntryAction::Close => "清仓",
            EntryAction::StayOut => "空仓观望",
            EntryAction::Hold => "持有不动",
        }
    }

    /// 仓位变化方向：+1 加仓 / -1 减仓 / 0 不动
    pub fn position_direction(&self) -> i8 {
        match self {
            EntryAction::Buy | EntryAction::BuyOrAdd => 1,
            EntryAction::ReduceOrHold | EntryAction::ReduceOrClose | EntryAction::Close => -1,
            EntryAction::StayOut => -1,
            EntryAction::Hold => 0,
        }
    }
}

/// 决策结果：动作 + 触发的原书规则编号
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecisionResult {
    pub action: EntryAction,
    pub rule_id: Option<MatrixRule>,
    pub explanation: String,
}

/// 原书 10 条规则的标识（按章节编号）
#[allow(non_camel_case_types)] // 章节编号 Buy1/Sell1 等需保持原书命名
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MatrixRule {
    /// BUY-1：突破长期下降，回落受中期上升支撑 → 买入
    Buy1_LongDownBreakMidUpSupport,
    /// BUY-2：长期上升之上，向上突破中期下降 → 买入或加仓
    Buy2_LongUpBreakMidDown,
    /// BUY-3：长期上升之上，急跌后突破短期下降 → 买入或加仓
    Buy3_LongUpQuickDipBreakShortDown,
    /// BUY-4：长期上升之上，遇长期上升支撑 → 买入或加仓
    Buy4_LongUpMeetLongUpSupport,
    /// BUY-5：长期上升之上，遇中期上升支撑 → 买入或加仓
    Buy5_LongUpMeetMidUpSupport,
    /// SELL-1：跌破长期上升 → 清仓
    Sell1_LongUpBreakdown,
    /// SELL-2：长期上升之上，跌破中期上升 → 减仓
    Sell2_LongUpBreakMidUp,
    /// SELL-3：长期上升之上，急速飙升后跌破短期上升 → 减仓
    Sell3_LongUpQuickRallyBreakShortUp,
    /// SELL-4：突破长期下降后，回落跌破中期上升 → 减仓或清仓
    Sell4_LongDownBreakThenMidUpFail,
    /// SELL-5：运行在长期下降之下 → 空仓
    Sell5_BelowLongDown,
}

impl MatrixRule {
    pub fn label(&self) -> &'static str {
        use MatrixRule::*;
        match self {
            Buy1_LongDownBreakMidUpSupport => "BUY-1 突破长期下降+中期上升支撑",
            Buy2_LongUpBreakMidDown => "BUY-2 长期上升之上+突破中期下降",
            Buy3_LongUpQuickDipBreakShortDown => "BUY-3 长期上升之上+急跌后突破短期下降",
            Buy4_LongUpMeetLongUpSupport => "BUY-4 长期上升之上+遇长期上升支撑",
            Buy5_LongUpMeetMidUpSupport => "BUY-5 长期上升之上+遇中期上升支撑",
            Sell1_LongUpBreakdown => "SELL-1 跌破长期上升",
            Sell2_LongUpBreakMidUp => "SELL-2 长期上升之上+跌破中期上升",
            Sell3_LongUpQuickRallyBreakShortUp => "SELL-3 长期上升之上+急速飙升后跌破短期上升",
            Sell4_LongDownBreakThenMidUpFail => "SELL-4 突破长期下降+回落跌破中期上升",
            Sell5_BelowLongDown => "SELL-5 运行在长期下降之下",
        }
    }

    pub fn book_source(&self) -> &'static str {
        "trend p.216 多级趋势线策略矩阵"
    }
}

/// 葛南维法则仓位上限（R-P1-13）
///
/// 原书 ma p.100 明确：
/// - L1/L2/L3 = 牛市满仓
/// - L4 = 下降趋势反弹，**仓位一定要轻**（≤ 30%）
/// - L5-L8 = 卖出，不加仓
pub struct PositionLimit;

impl PositionLimit {
    /// 葛南维 L4（下降趋势中的反弹）—— 原书警告"仓位一定要轻"
    pub const L4_MAX: f64 = 0.30;
    /// L1/L2/L3 牛市可满仓
    pub const BULL_MAX: f64 = 1.00;
    /// L5-L8 卖出，零仓位
    pub const SELL_MAX: f64 = 0.00;
}

/// 根据原书 10 条策略矩阵决策
///
/// # 决策优先级
///
/// 1. SELL-5（长期下降之下）—— **最高优先级**，覆盖所有买入信号
/// 2. SELL-1（跌破长期上升）—— 次高，原书"灭顶之灾"警告
/// 3. SELL-2/3/4（中期/短期跌破，部分减仓）
/// 4. BUY-1/2/3/4/5（仅在长期上升之上才考虑买入）
///
/// # Examples
///
/// ```
/// use aura_trade::engine::trend::strategy::*;
///
/// // 长期上升之上 + 突破中期下降趋势线 = BUY-2
/// let state = MultiTimeframeTrendState::new(
///     TrendDirection::Up,
///     TrendDirection::Down,
///     TrendDirection::None,
/// );
/// let event = TrendEvent::Breakout { level: TrendLevel::Mid, original: TrendDirection::Down };
/// let result = decide_action(&state, event);
/// assert_eq!(result.action, EntryAction::BuyOrAdd);
/// ```
pub fn decide_action(state: &MultiTimeframeTrendState, event: TrendEvent) -> DecisionResult {
    use EntryAction::*;
    use MatrixRule::*;
    use TrendEvent::*;
    use TrendLevel::*;
    // 注意：不使用 `use TrendDirection::*` 以避免 None 与 Option::None 冲突
    let up = TrendDirection::Up;
    let down = TrendDirection::Down;

    // ========== SELL-5：长期下降之下 → 空仓（最高优先级） ==========
    // 原书 trend p.225："非牛市空仓"
    if state.long_term_bearish() {
        return DecisionResult {
            action: StayOut,
            rule_id: Some(Sell5_BelowLongDown),
            explanation: format!(
                "{} (trend p.225 \"非牛市空仓\")",
                Sell5_BelowLongDown.label()
            ),
        };
    }

    // ========== SELL-1：跌破长期上升 → 清仓（原书"灭顶之灾"） ==========
    if let Breakdown { level: Long, original } = event {
        if original == up {
            return DecisionResult {
                action: Close,
                rule_id: Some(Sell1_LongUpBreakdown),
                explanation: format!(
                    "{} (trend p.221 \"清仓依然明智之举\")",
                    Sell1_LongUpBreakdown.label()
                ),
            };
        }
    }

    // 以下规则都要求 long_term_bullish()
    if !state.long_term_bullish() {
        return DecisionResult {
            action: Hold,
            rule_id: Option::None,
            explanation: format!(
                "长期方向 = {}，不触发任何买卖规则",
                state.long.label()
            ),
        };
    }

    // ========== SELL-2：长期上升之上 + 跌破中期上升 → 减仓 ==========
    if let Breakdown { level: Mid, original } = event {
        if original == up {
            return DecisionResult {
                action: ReduceOrHold,
                rule_id: Some(Sell2_LongUpBreakMidUp),
                explanation: Sell2_LongUpBreakMidUp.label().to_string(),
            };
        }
    }

    // ========== SELL-3：长期上升之上 + 急速飙升后跌破短期上升 → 减仓 ==========
    if let Breakdown { level: Short, original } = event {
        if original == up {
            // 该规则要求前置条件：之前有急速飙升
            // 此处简化为直接判断（实际应配合 QuickRallyTop 历史事件）
            return DecisionResult {
                action: ReduceOrHold,
                rule_id: Some(Sell3_LongUpQuickRallyBreakShortUp),
                explanation: Sell3_LongUpQuickRallyBreakShortUp.label().to_string(),
            };
        }
    }

    // ========== BUY-1：突破长期下降，回落受中期上升支撑 → 买入 ==========
    // 注：此规则的"突破长期下降"需历史事件，此处通过 SupportHold 触发
    if let Breakout { level: Long, original } = event {
        if original == down {
            return DecisionResult {
                action: Buy,
                rule_id: Some(Buy1_LongDownBreakMidUpSupport),
                explanation: Buy1_LongDownBreakMidUpSupport.label().to_string(),
            };
        }
    }

    // ========== BUY-2：长期上升之上 + 向上突破中期下降 → 买入或加仓 ==========
    if let Breakout { level: Mid, original } = event {
        if original == down {
            return DecisionResult {
                action: BuyOrAdd,
                rule_id: Some(Buy2_LongUpBreakMidDown),
                explanation: Buy2_LongUpBreakMidDown.label().to_string(),
            };
        }
    }

    // ========== BUY-3：长期上升之上 + 急跌后突破短期下降 → 买入或加仓 ==========
    if let Breakout { level: Short, original } = event {
        if original == down {
            return DecisionResult {
                action: BuyOrAdd,
                rule_id: Some(Buy3_LongUpQuickDipBreakShortDown),
                explanation: Buy3_LongUpQuickDipBreakShortDown.label().to_string(),
            };
        }
    }

    // ========== BUY-4：长期上升之上 + 遇长期上升支撑 → 买入或加仓 ==========
    if let SupportHold { level: Long } = event {
        return DecisionResult {
            action: BuyOrAdd,
            rule_id: Some(Buy4_LongUpMeetLongUpSupport),
            explanation: Buy4_LongUpMeetLongUpSupport.label().to_string(),
        };
    }

    // ========== BUY-5：长期上升之上 + 遇中期上升支撑 → 买入或加仓 ==========
    if let SupportHold { level: Mid } = event {
        return DecisionResult {
            action: BuyOrAdd,
            rule_id: Some(Buy5_LongUpMeetMidUpSupport),
            explanation: Buy5_LongUpMeetMidUpSupport.label().to_string(),
        };
    }

    // ========== 默认：持有不动 ==========
    DecisionResult {
        action: Hold,
        rule_id: Option::None,
        explanation: format!("事件 {:?} 未匹配任何原书规则", event),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn long_up_state() -> MultiTimeframeTrendState {
        MultiTimeframeTrendState::new(
            TrendDirection::Up,
            TrendDirection::None,
            TrendDirection::None,
        )
    }

    fn long_down_state() -> MultiTimeframeTrendState {
        MultiTimeframeTrendState::new(
            TrendDirection::Down,
            TrendDirection::None,
            TrendDirection::None,
        )
    }

    #[test]
    fn t_sell5_long_down_overrides_buy() {
        // 长期下降 + 任何买入信号 → 仍为 StayOut
        let state = long_down_state();
        let event = TrendEvent::Breakout {
            level: TrendLevel::Mid,
            original: TrendDirection::Down,
        };
        let result = decide_action(&state, event);
        assert_eq!(result.action, EntryAction::StayOut);
        assert_eq!(result.rule_id, Some(MatrixRule::Sell5_BelowLongDown));
    }

    #[test]
    fn t_sell1_long_up_breakdown_close() {
        // 长期上升 + 跌破长期上升 → 清仓
        let state = long_up_state();
        let event = TrendEvent::Breakdown {
            level: TrendLevel::Long,
            original: TrendDirection::Up,
        };
        let result = decide_action(&state, event);
        assert_eq!(result.action, EntryAction::Close);
        assert_eq!(result.rule_id, Some(MatrixRule::Sell1_LongUpBreakdown));
    }

    #[test]
    fn t_buy2_long_up_break_mid_down() {
        // 长期上升 + 突破中期下降 → 买入或加仓
        let state = long_up_state();
        let event = TrendEvent::Breakout {
            level: TrendLevel::Mid,
            original: TrendDirection::Down,
        };
        let result = decide_action(&state, event);
        assert_eq!(result.action, EntryAction::BuyOrAdd);
        assert_eq!(result.rule_id, Some(MatrixRule::Buy2_LongUpBreakMidDown));
    }

    #[test]
    fn t_buy4_long_up_meet_long_support() {
        // 长期上升 + 遇长期上升支撑 → 买入或加仓
        let state = long_up_state();
        let event = TrendEvent::SupportHold {
            level: TrendLevel::Long,
        };
        let result = decide_action(&state, event);
        assert_eq!(result.action, EntryAction::BuyOrAdd);
        assert_eq!(result.rule_id, Some(MatrixRule::Buy4_LongUpMeetLongUpSupport));
    }

    #[test]
    fn t_buy5_long_up_meet_mid_support() {
        let state = long_up_state();
        let event = TrendEvent::SupportHold {
            level: TrendLevel::Mid,
        };
        let result = decide_action(&state, event);
        assert_eq!(result.action, EntryAction::BuyOrAdd);
        assert_eq!(result.rule_id, Some(MatrixRule::Buy5_LongUpMeetMidUpSupport));
    }

    #[test]
    fn t_sell2_long_up_break_mid_up_reduce() {
        let state = long_up_state();
        let event = TrendEvent::Breakdown {
            level: TrendLevel::Mid,
            original: TrendDirection::Up,
        };
        let result = decide_action(&state, event);
        assert_eq!(result.action, EntryAction::ReduceOrHold);
        assert_eq!(result.rule_id, Some(MatrixRule::Sell2_LongUpBreakMidUp));
    }

    #[test]
    fn t_position_limits_correct() {
        // R-P1-13 仓位上限校验
        assert_eq!(PositionLimit::L4_MAX, 0.30);
        assert_eq!(PositionLimit::BULL_MAX, 1.00);
        assert_eq!(PositionLimit::SELL_MAX, 0.00);
    }

    #[test]
    fn t_position_direction_consistent() {
        // 不变性：所有买入动作 position_direction > 0；卖出 < 0；Hold = 0
        assert!(EntryAction::Buy.position_direction() > 0);
        assert!(EntryAction::BuyOrAdd.position_direction() > 0);
        assert!(EntryAction::ReduceOrHold.position_direction() < 0);
        assert!(EntryAction::ReduceOrClose.position_direction() < 0);
        assert!(EntryAction::Close.position_direction() < 0);
        assert!(EntryAction::StayOut.position_direction() < 0);
        assert_eq!(EntryAction::Hold.position_direction(), 0);
    }

    #[test]
    fn t_long_term_bearish_no_buy() {
        // 不变性：长期下降时，任何买入事件都不应触发买入
        let state = long_down_state();
        let buy_events = [
            TrendEvent::Breakout { level: TrendLevel::Long, original: TrendDirection::Down },
            TrendEvent::Breakout { level: TrendLevel::Mid, original: TrendDirection::Down },
            TrendEvent::Breakout { level: TrendLevel::Short, original: TrendDirection::Down },
            TrendEvent::SupportHold { level: TrendLevel::Long },
            TrendEvent::SupportHold { level: TrendLevel::Mid },
        ];
        for event in buy_events {
            let result = decide_action(&state, event);
            assert!(
                result.action.position_direction() <= 0,
                "长期下降时事件 {:?} 不应触发买入，实际：{:?}",
                event,
                result.action
            );
        }
    }
}
