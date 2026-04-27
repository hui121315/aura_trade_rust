//! 模块 D：四维共振评分系统（Resonance）
//!
//! 将 A 均线 / B 趋势 / C K线形态 / D 技术图形 四个维度的信号
//! 统一计算成一个**方向一致性分数**：
//!
//! - 每个维度在当前 K 线产生的方向信号（+1 / -1 / 0）带权重
//! - 得分 = Σ (direction × strength_weight)
//! - 归一化到 [-100, +100] 区间
//! - 提供决策标签：强烈看涨 / 看涨 / 中性 / 看跌 / 强烈看跌

pub mod score;
pub mod suggestion;

pub use score::{compute_resonance, DimensionScore, ResonanceScore, Stance};
pub use suggestion::{compute_suggestion, TradeSuggestion};
