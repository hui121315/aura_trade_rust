//! 模块 A：均线引擎
//!
//! - [`compute`]     A1 基础算法 SMA / EMA / WMA + 斜率 + BIAS
//! - [`alignment`]   A5 排列与交叉识别
//! - [`granville`]   A4 葛南维八大法则
//! - [`special`]     A6 17 大特殊形态
//! - [`dual_line`]   **A8 双线中期组合 6 条买入持仓原则**（E34 / R-P1-49，ma p.200）
//! - [`advanced`]    **A9 高级形态**（R-P1-50~56：旱地拔葱/毒蜘蛛/断头铡刀/向上发散）
//! - [`repair`]      **A10 均线修复 + 气贯长虹**（R-P1-54 / R-P1-55）
//! - [`long_term_levels`] **A11 120/240 日长期压力位**（R-P1-29，Sprint 14）
//! - [`state`]       A1-A5 聚合的 API 输出结构

pub mod advanced;
pub mod alignment;
pub mod compute;
pub mod dual_line;
pub mod granville;
pub mod long_term_levels;
pub mod repair;
pub mod special;
pub mod state;

pub use advanced::{
    detect_bond_divergence, detect_hanging_scallions, scan_advanced, MaAdvancedEvent,
    MaAdvancedKind, MaAdvancedParams,
};
pub use long_term_levels::{
    scan_long_term_levels, LongTermLevelEvent, LongTermLevelHit, LongTermParams,
};
pub use repair::{
    detect_air_flag, detect_repairs, AirFlagCriteria, AirFlagEvent, AirFlagParams, RepairEvent,
    RepairKind, RepairParams,
};
pub use alignment::{Alignment, Cross, CrossKind};
pub use compute::{bias, ema, sma, slope, wma, MaKind};
pub use dual_line::{
    recommended_position_fraction as dual_line_position, scan as scan_dual_line,
    DualLineEvent, DualLineParams, DualLineRule,
};
pub use granville::{GranvilleRule, GranvilleSignal};
pub use special::{scan_at as scan_ma_special, MaSpecialHit, MaSpecialKind, SpecialParams};
pub use state::{compute_ma_state, MaState};
