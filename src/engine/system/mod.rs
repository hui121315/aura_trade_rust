//! 模块 G：体系实验室（System Lab）
//!
//! 详见 `SYSTEM_LAB_DESIGN.md`。
//!
//! # 职责分层
//!
//! - [`component`]   组件元数据 + 注册表（21 个 MVP 组件）
//! - [`scan`]        预扫描：一次性把所有组件在所有 bar 上的触发事件索引好
//! - [`definition`]  `SystemDefinition` + `CombineRule` + `RiskParams` 等配置类型
//! - [`combine`]     聚合规则求值（`AllAligned` / `MajorityK`）
//! - [`runner`]      给定 `(SystemDefinition, klines)` → `SystemBacktestResult`
//! - [`registry`]    种子体系（`seed.ma_skeleton` 等）
//!
//! # 设计哲学
//!
//! - **可解释优先**：每个组件都带 `book_source`，追溯原书某页
//! - **组件不固定**：用户/探索器可以任意组合（受 MAX_K=5 约束）
//! - **断头铡刀铁律**：无论体系怎么配，检测到断头铡刀一律清仓（见 `runner`）
//!
//! # 与现有模块关系
//!
//! - 不重写识别器，只**引用**：`ma::granville` / `ma::scan_advanced` / `ma::scan_ma_special`
//!   / `candle::scan` / `trend::compute_trend_state`
//! - 复用 `backtest::types::{Performance, EquityPoint}` 作为结果字段

pub mod combine;
pub mod component;
pub mod definition;
pub mod registry;
pub mod benchmark;
pub mod discovery;
pub mod runner;
pub mod scan;
pub mod vault;
pub mod walkforward;

pub use combine::{evaluate_combine, CombinedSignal};
pub use component::{
    all_components, find_component, Component, ComponentDimension, COMPONENT_MAX_K,
};
pub use definition::{
    BacktestParams, BenchmarkSnapshot, CombineRule, CostModel, RiskParams, SystemBacktestResult,
    SystemDefinition, SystemMeta, SystemOrigin, SystemTrade, TradeExitReason, TradeSide,
};
pub use registry::{all_seeds, find_seed};
pub use runner::run;
pub use scan::{scan_all_triggers, ScanResult, TriggerEvent};
pub use discovery::{
    discover, CrossValidationResult, DiscoveryCandidate, DiscoveryConfig, DiscoveryReport,
};
pub use benchmark::{run_benchmark_with, BenchmarkCell, BenchmarkReport};
pub use vault::{add_promoted, load_promoted, remove_promoted, PROMOTED_ID_PREFIX};
pub use walkforward::{
    run_walkforward, WalkForwardAggregate, WalkForwardConfig, WalkForwardFold, WalkForwardReport,
};
