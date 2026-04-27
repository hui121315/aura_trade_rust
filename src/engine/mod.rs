//! 计算引擎（四维 + 回测 + 信号层）
//!
//! 按 PRD 严格分层：
//! - [`ma`]       模块 A：均线引擎（Phase 1.3 起实现）
//! - [`trend`]    模块 B：趋势引擎（Phase 3）
//! - [`candle`]   模块 C：K线形态引擎（Phase 1.4 起实现）
//! - [`chartpattern`] 模块 D：技术图形引擎（Phase 4）
//! - [`backtest`] 模块 E：回测引擎（Phase 2）
//! - [`signal`]   模块 F：**信号层**（Phase 5+，跨模块组合信号：多合一/衰减/陷阱）

pub mod ma;
pub mod trend;
pub mod candle;
pub mod chartpattern;
pub mod indicator;
pub mod resonance;
pub mod backtest;
pub mod signal;
pub mod effectiveness;
pub mod rl;
pub mod system;
