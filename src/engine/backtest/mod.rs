//! 模块 E：回测引擎（Phase 2 MVP）
//!
//! - [`types`]           E1 配置/交易/结果/绩效 类型
//! - [`runner`]          E2/E3 事件驱动回测主循环
//! - [`metrics`]         E4 绩效指标
//! - [`position_limit`]  **E6 葛南维仓位校验器**（R-P1-13，ma p.100）
//! - [`playbook`]        **E7 回测策略 PRD 模板**（R-P1-12，Sprint 10）
//! - [`playbook_runner`] **E8 Playbook 驱动回测**（Sprint 11，R-P1-12 集成）

pub mod metrics;
pub mod playbook;
pub mod playbook_runner;
pub mod position_limit;
pub mod runner;
pub mod types;

pub use playbook::{
    CompositePlaybook, GuillotineExitPlaybook, HangingScallionsEntryPlaybook, Playbook,
    PlaybookContext, PlaybookDecision, StagedExitPlaybook, TrendMatrixPlaybook,
};
pub use playbook_runner::run_with_playbook;
pub use position_limit::{OrderCheckResult, PositionLimit, PositionLimitChecker};
pub use runner::run;
pub use types::{
    BacktestConfig, BacktestResult, EquityPoint, ExitReason, PatternStat, Performance, Side,
    StopKind, Trade,
};
