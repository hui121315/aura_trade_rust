//! 模块 C：K线形态识别引擎
//!
//! - [`metrics`]         C1 基础度量 + 粗分类
//! - [`patterns`]        C2 单/双/三根形态（Phase 1.4 子集；Phase 4 扩展到 55+）
//! - [`advanced`]        **C3 高级分类/评分/层级结构**（Sprint 6，R-P1-43~47/58/59）
//! - [`multi_timeframe`] **C4 跨周期聚合 + 多均线排列/收敛发散**（Sprint 6，R-P1-33/34）

pub mod advanced;
pub mod combinations;
pub mod metrics;
pub mod multi_timeframe;
pub mod patterns;

pub use advanced::{
    analyze_complex_left_shoulder, analyze_rounding_bottom, are_siblings, check_head_shoulders_volume,
    classify_long_doji, detect_gradual_decline, detect_inverted_three_red,
    detect_two_rising_stars, island_trend_level, parent_patterns_of, score_three_white_soldiers,
    ComplexLeftShoulderAnalysis, GradualDeclineEvent, InvertedThreeRedSoldiersEvent,
    IslandTrendLevel, LongDojiContext, RoundingBottomAnalysis, RoundingBottomPhase,
    ThreeSoldiersScore, TwoRisingStarsEvent, VolumeSymmetry,
};
pub use combinations::{detect_combinations, CandleCombination, CombinationEvent};
pub use metrics::{classify, metrics_for, metrics_series, CandleClass, CandleMetrics};
pub use multi_timeframe::{
    aggregate_to_weekly, detect_alignment, detect_ma_relation, scan_alignment_events,
    scan_ma_relation_events, AlignmentKind, MaRelationState, Timeframe,
};
pub use patterns::{scan, PatternHit, PatternKind};
