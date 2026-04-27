//! B9：趋势状态机（R-P1-08，Sprint 15）
//!
//! 在 [`super::dow::DowPhase`] 基础上添加**状态转移跟踪**：
//!
//! - 历史状态序列（按索引记录每次 phase 变化）
//! - 状态转移事件分类（启动上升 / 启动下降 / 进入整理 / 恢复原趋势）
//! - 便于策略层判断"刚进入新趋势"vs"趋势已持续很久"
//!
//! # 原书原则（trend Ch3）
//!
//! > "趋势改变的信号要谨慎识别 —— **第一次转折通常是假信号，第二次确认才可靠**。"
//!
//! 工程上：通过 `transition_count_since` API 跟踪每种转换的次数，
//! 供策略层做"n 次以上才行动"的判断。

use serde::{Deserialize, Serialize};

use super::dow::{self, DowPhase};
use super::swing::SwingPoint;

/// 状态转移事件
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TrendTransition {
    /// 启动上升（Consolidation/Downtrend → Uptrend）
    UptrendStarted,
    /// 启动下降（Uptrend/Consolidation → Downtrend）
    DowntrendStarted,
    /// 进入整理（Uptrend/Downtrend → Consolidation）
    ConsolidationEntered,
    /// 继续维持（相同 phase）
    NoChange,
}

impl TrendTransition {
    pub fn label(&self) -> &'static str {
        use TrendTransition::*;
        match self {
            UptrendStarted => "启动上升",
            DowntrendStarted => "启动下降",
            ConsolidationEntered => "进入整理",
            NoChange => "状态不变",
        }
    }

    pub fn is_change(&self) -> bool {
        !matches!(self, TrendTransition::NoChange)
    }
}

fn classify_transition(prev: DowPhase, curr: DowPhase) -> TrendTransition {
    use DowPhase::*;
    use TrendTransition::*;
    if prev == curr {
        return NoChange;
    }
    match curr {
        Uptrend => UptrendStarted,
        Downtrend => DowntrendStarted,
        Consolidation => ConsolidationEntered,
        Unknown => NoChange, // 不记录 Unknown 作为转移
    }
}

/// 单条转移记录
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct TransitionRecord {
    pub bar_index: usize,
    pub from: DowPhase,
    pub to: DowPhase,
    pub transition: TrendTransition,
}

/// 趋势状态机
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrendStateMachine {
    current: DowPhase,
    history: Vec<TransitionRecord>,
}

impl Default for TrendStateMachine {
    fn default() -> Self {
        Self::new()
    }
}

impl TrendStateMachine {
    pub fn new() -> Self {
        Self {
            current: DowPhase::Unknown,
            history: Vec::new(),
        }
    }

    /// 用新 swing 点集 + last_bar_index 更新状态
    ///
    /// 如果 phase 变化，记录到 history
    pub fn update(&mut self, swings: &[SwingPoint], last_bar_index: usize) -> TrendTransition {
        let new_state = dow::classify(swings, last_bar_index);
        let transition = classify_transition(self.current, new_state.phase);
        if transition.is_change() {
            self.history.push(TransitionRecord {
                bar_index: last_bar_index,
                from: self.current,
                to: new_state.phase,
                transition,
            });
        }
        self.current = new_state.phase;
        transition
    }

    pub fn current_phase(&self) -> DowPhase {
        self.current
    }

    pub fn history(&self) -> &[TransitionRecord] {
        &self.history
    }

    /// 自 `since_bar` 以来发生指定转移的次数（用于"n 次后才行动"）
    pub fn transition_count_since(&self, transition: TrendTransition, since_bar: usize) -> usize {
        self.history
            .iter()
            .filter(|r| r.transition == transition && r.bar_index >= since_bar)
            .count()
    }

    /// 最近一次转移
    pub fn last_transition(&self) -> Option<&TransitionRecord> {
        self.history.last()
    }

    /// 当前 phase 持续了多少根 K 线（根据最后一次转移计算）
    pub fn phase_duration(&self, current_bar: usize) -> usize {
        match self.history.last() {
            Some(r) => current_bar.saturating_sub(r.bar_index),
            None => current_bar,
        }
    }

    /// 清空历史
    pub fn clear(&mut self) {
        self.history.clear();
        self.current = DowPhase::Unknown;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::swing::SwingKind;

    fn sp(idx: usize, price: f64, kind: SwingKind) -> SwingPoint {
        SwingPoint {
            index: idx,
            time: (idx as i64) * 86_400_000,
            price,
            kind,
        }
    }

    #[test]
    fn t_initial_state_is_unknown() {
        let m = TrendStateMachine::new();
        assert_eq!(m.current_phase(), DowPhase::Unknown);
        assert!(m.history().is_empty());
    }

    #[test]
    fn t_uptrend_started_detected() {
        // HH + HL → Uptrend
        let swings = vec![
            sp(0, 90.0, SwingKind::Low),
            sp(5, 100.0, SwingKind::High),
            sp(10, 95.0, SwingKind::Low),
            sp(15, 105.0, SwingKind::High),
        ];
        let mut m = TrendStateMachine::new();
        let trans = m.update(&swings, 20);
        assert_eq!(trans, TrendTransition::UptrendStarted);
        assert_eq!(m.current_phase(), DowPhase::Uptrend);
        assert_eq!(m.history().len(), 1);
    }

    #[test]
    fn t_downtrend_started_detected() {
        let swings = vec![
            sp(0, 100.0, SwingKind::High),
            sp(5, 95.0, SwingKind::Low),
            sp(10, 98.0, SwingKind::High),
            sp(15, 90.0, SwingKind::Low),
        ];
        let mut m = TrendStateMachine::new();
        let trans = m.update(&swings, 20);
        assert_eq!(trans, TrendTransition::DowntrendStarted);
        assert_eq!(m.current_phase(), DowPhase::Downtrend);
    }

    #[test]
    fn t_no_change_when_phase_same() {
        let swings = vec![
            sp(0, 90.0, SwingKind::Low),
            sp(5, 100.0, SwingKind::High),
            sp(10, 95.0, SwingKind::Low),
            sp(15, 105.0, SwingKind::High),
        ];
        let mut m = TrendStateMachine::new();
        m.update(&swings, 20);
        // 再次 update 同样的 swings → NoChange
        let trans = m.update(&swings, 25);
        assert_eq!(trans, TrendTransition::NoChange);
        assert_eq!(m.history().len(), 1, "不变时不应记录新历史");
    }

    #[test]
    fn t_phase_duration_tracked() {
        let swings = vec![
            sp(0, 90.0, SwingKind::Low),
            sp(5, 100.0, SwingKind::High),
            sp(10, 95.0, SwingKind::Low),
            sp(15, 105.0, SwingKind::High),
        ];
        let mut m = TrendStateMachine::new();
        m.update(&swings, 20);
        assert_eq!(m.phase_duration(50), 30); // 50 - 20 = 30
    }

    #[test]
    fn t_transition_count_since() {
        let mut m = TrendStateMachine::new();
        // 上升
        m.update(
            &vec![
                sp(0, 90.0, SwingKind::Low),
                sp(5, 100.0, SwingKind::High),
                sp(10, 95.0, SwingKind::Low),
                sp(15, 105.0, SwingKind::High),
            ],
            20,
        );
        // 转整理
        m.update(
            &vec![
                sp(0, 90.0, SwingKind::Low),
                sp(5, 100.0, SwingKind::High),
                sp(10, 95.0, SwingKind::Low),
                sp(15, 98.0, SwingKind::High),
            ],
            30,
        );
        // 再上升
        m.update(
            &vec![
                sp(0, 90.0, SwingKind::Low),
                sp(5, 100.0, SwingKind::High),
                sp(10, 95.0, SwingKind::Low),
                sp(15, 108.0, SwingKind::High),
            ],
            40,
        );
        assert_eq!(
            m.transition_count_since(TrendTransition::UptrendStarted, 0),
            2
        );
        assert_eq!(
            m.transition_count_since(TrendTransition::UptrendStarted, 25),
            1
        );
    }

    #[test]
    fn t_last_transition_latest() {
        let mut m = TrendStateMachine::new();
        m.update(
            &vec![
                sp(0, 90.0, SwingKind::Low),
                sp(5, 100.0, SwingKind::High),
                sp(10, 95.0, SwingKind::Low),
                sp(15, 105.0, SwingKind::High),
            ],
            20,
        );
        let last = m.last_transition().unwrap();
        assert_eq!(last.transition, TrendTransition::UptrendStarted);
        assert_eq!(last.bar_index, 20);
    }

    #[test]
    fn t_clear_resets() {
        let mut m = TrendStateMachine::new();
        m.update(
            &vec![
                sp(0, 90.0, SwingKind::Low),
                sp(5, 100.0, SwingKind::High),
                sp(10, 95.0, SwingKind::Low),
                sp(15, 105.0, SwingKind::High),
            ],
            20,
        );
        m.clear();
        assert_eq!(m.current_phase(), DowPhase::Unknown);
        assert!(m.history().is_empty());
    }
}
