//! 模块 C（扩展）：技术图形形态（Chart Patterns）
//!
//! 区分于单 K 线形态，本模块识别由多个摆动点构成的"图形"，
//! 基于 Phase 3 产出的 swing 点做几何匹配：
//!
//! - 反转：头肩顶/底、双顶/双底、三重顶/底、圆弧底、V 形反转、菱形顶/底
//! - 持续：三角形（上升/下降/对称）、旗形（多/空）、三角旗、楔形（上升/下降）、
//!         矩形、杯柄、头肩连续（看涨/看跌）
//! - 其它：扩散三角、扇形、岛形反转（在 trend/gap 已有）

pub mod detect;
pub mod flag_validator;
pub mod types;

pub use detect::detect_all;
pub use flag_validator::{validate_flag, FlagValidation, FlagValidatorParams};
pub use types::{
    ChartPattern, ChartPatternKind, HeadShouldersMeasure, MarketMakerBehavior, RectangleRole,
};
