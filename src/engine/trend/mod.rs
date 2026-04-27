//! 模块 B：趋势引擎（Phase 3）
//!
//! - [`swing`]    B1 摆动高低点（ZigZag）
//! - [`dow`]      B2 道氏趋势分类（HH/HL 上升、LH/LL 下降、整固）
//! - [`lines`]    B3 趋势线自动拟合
//! - [`sr`]       B4 支撑/压力位聚类
//! - [`channel`]  B5 平行通道
//! - [`gap`]      B6 缺口识别与分类
//! - [`state`]    B1-B6 聚合 API 输出
//! - [`strategy`] **B8 多级趋势线策略矩阵**（R-P1-15，trend p.216 原书 10 条买卖原则）

pub mod channel;
pub mod dow;
pub mod gap;
pub mod lines;
pub mod sr;
pub mod state;
pub mod state_machine;
pub mod strategy;
pub mod swing;

pub use dow::{DowPhase, DowState};
pub use gap::{Gap, GapKind};
pub use lines::{CoordinateSystem, TrendLine, TrendLineKind};
pub use sr::{RoleFlip, RoleHistory, SrKind, SrLevel};
pub use state::{compute_trend_state, TrendState};
pub use state_machine::{TransitionRecord, TrendStateMachine, TrendTransition};
pub use strategy::{
    decide_action, DecisionResult, EntryAction, MatrixRule, MultiTimeframeTrendState,
    PositionLimit, TrendDirection, TrendEvent, TrendLevel,
};
pub use swing::{SwingKind, SwingPoint};
