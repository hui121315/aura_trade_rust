//! 模块 F：信号层（Phase 5+ 新增）
//!
//! 在 engine 的基础识别器（ma / trend / chart / candle）之上，
//! 提供**跨模块组合信号**：
//!
//! - [`confluence`]  **F1 多合一现象识别器**（R-P1-16，±3% 合流 → 强度 ×1.5）
//! - [`fatigue`]     **F2 信号衰减框架**（R-P1-52，反过度交易）
//! - [`bull_trap`]   **F3 多头/空头陷阱识别器**（R-P1-17，假突破迅速反向）
//! - [`staged_exit`] **F4 分级减仓策略**（R-P1-42/32，保本哲学）
//! - [`stealth`]     **F5 主力潜伏突破 + 通道穿头破脚**（R-P1-30 / R-P1-31）
//! - [`level`]       **F6 信号级别 + 阶段 + 消亡条件**（R-P1-02/03/10/11）
//! - [`router`]      **F7 模块 Priority 路由**（R-P1-05，Sprint 10）
//! - [`replay`]      **F8 历史再现验证框架**（R-P1-06，Sprint 10）
//! - [`volume_warning`] **F9 无量涨停/跌停警告**（R-P1-26，Sprint 14）
//! - [`trend_confirmation`] **F10 趋势确认**（R-P1-18/22，Sprint 16）

pub mod bull_trap;
pub mod confluence;
pub mod fatigue;
pub mod level;
pub mod replay;
pub mod router;
pub mod staged_exit;
pub mod stealth;
pub mod trend_confirmation;
pub mod volume_warning;

pub use bull_trap::{detect_traps, detect_traps_with_key_series, TrapEvent, TrapKind, TrapParams};
pub use confluence::{
    detect_confluences, Confluence, ConfluenceComponent, ConfluenceParams,
};
pub use fatigue::{SignalFatigue, SignalKind};
pub use level::{InvalidationCondition, SignalLevel, SignalMetadata, Stage};
pub use replay::{HistoricalReplay, ReplayRecord, ReplayStats};
pub use router::{RoutedSignal, SignalRouter};
pub use staged_exit::{ExitEvent, StagedExitParams, StagedExitPlanner, ToppingSignalSeverity};
pub use stealth::{
    detect_channel_piercing, detect_panic_capitulation, detect_stealth_breakouts,
    ChannelPiercingEvent, PanicCapitulationEvent, PanicParams, StealthBreakoutEvent,
    StealthParams,
};
pub use trend_confirmation::{
    confirm_bearish_reversal, confirm_bullish_reversal, detect_l4_warning, L4WarningLevel,
    ReversalConfirmation,
};
pub use volume_warning::{
    detect_volume_anomalies, VolumeAnomalyEvent, VolumeAnomalyKind, VolumeWarningParams,
};
