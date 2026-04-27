//! F6：信号级别 + 阶段标签 + 置信度（R-P1-11 / R-P1-03 / R-P1-02 / R-P1-10）
//!
//! 原书 E20 铁证："**谨慎买入，果断卖出**" —— 信号应有明确的强度级别，
//! 卖出权重应高于买入（默认 1.3×，见 HANDBOOK §2.2）。
//!
//! 本模块提供：
//! - `SignalLevel`：4 级信号强度（Strong/Medium/Weak/Noise）
//! - `Stage`：信号阶段（Entry/Hold/Exit/Watch）
//! - `SignalMetadata`：完整信号元数据（级别 + 阶段 + 置信度 + 消亡条件）
//!
//! # R-P1-11 信号级别
//!
//! | 级别 | 原书权重倍率 | 典型来源 |
//! |---|---|---|
//! | **Strong** | 1.5× | 断头铡刀 / 多合一 3+ 种共振 / SELL-1 跌破长期上升 |
//! | **Medium** | 1.0× | 单一葛南维 B1/S1 / 旗形完整 7 条 |
//! | **Weak** | 0.5× | 葛南维 B4 逆势反弹（仓位 ≤ 30%）/ 单一小阳线 |
//! | **Noise** | 0.1× | 首次信号后连续第 3+ 次（信号衰减） |
//!
//! # R-P1-03 阶段标签
//!
//! 明确信号在交易生命周期中的位置：
//!
//! - `Entry`：进场（建仓 / 加仓）
//! - `Hold`：持有（观察 / 持股）
//! - `Exit`：离场（减仓 / 清仓）
//! - `Watch`：观望（空仓等待）

use serde::{Deserialize, Serialize};

/// R-P1-11 信号强度级别
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Ord, PartialOrd, Serialize, Deserialize)]
pub enum SignalLevel {
    /// 噪声（权重极低，可过滤）
    Noise,
    /// 弱信号（辅助参考）
    Weak,
    /// 中等信号（常规交易）
    Medium,
    /// 强信号（重仓 / 清仓的主要依据）
    Strong,
}

impl SignalLevel {
    /// 权重倍率
    pub fn weight_multiplier(&self) -> f64 {
        match self {
            SignalLevel::Strong => 1.5,
            SignalLevel::Medium => 1.0,
            SignalLevel::Weak => 0.5,
            SignalLevel::Noise => 0.1,
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            SignalLevel::Strong => "强",
            SignalLevel::Medium => "中",
            SignalLevel::Weak => "弱",
            SignalLevel::Noise => "噪声",
        }
    }

    /// 应用"谨慎买入，果断卖出"哲学（E20）
    ///
    /// 对卖出信号自动提升权重 × 1.3（跨书铁证不变量 2.2）
    pub fn adjusted_for_direction(&self, is_sell: bool) -> f64 {
        let base = self.weight_multiplier();
        if is_sell { base * 1.3 } else { base }
    }
}

/// R-P1-03 信号阶段
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Stage {
    /// 进场（建仓 / 加仓）
    Entry,
    /// 持有（观察 / 持股不动）
    Hold,
    /// 离场（减仓 / 清仓）
    Exit,
    /// 观望（空仓等待）
    Watch,
}

impl Stage {
    pub fn label(&self) -> &'static str {
        match self {
            Stage::Entry => "进场",
            Stage::Hold => "持股",
            Stage::Exit => "离场",
            Stage::Watch => "观望",
        }
    }

    /// 阶段对应的仓位动作（+1 加仓 / 0 不动 / -1 减仓）
    pub fn position_action(&self) -> i8 {
        match self {
            Stage::Entry => 1,
            Stage::Hold => 0,
            Stage::Exit => -1,
            Stage::Watch => 0,
        }
    }
}

/// R-P1-10 形态消亡条件
///
/// 明确一个信号在何种情况下**失效**（应停止行动）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InvalidationCondition {
    /// 人类可读的失效描述
    pub description: String,
    /// 触发失效的价格阈值（可选）
    pub price_trigger: Option<f64>,
    /// 触发失效的 K 线索引上限（可选，超过此索引信号过期）
    pub expire_at_index: Option<usize>,
}

impl InvalidationCondition {
    pub fn new(description: impl Into<String>) -> Self {
        Self {
            description: description.into(),
            price_trigger: None,
            expire_at_index: None,
        }
    }

    pub fn with_price(mut self, price: f64) -> Self {
        self.price_trigger = Some(price);
        self
    }

    pub fn with_expiry(mut self, index: usize) -> Self {
        self.expire_at_index = Some(index);
        self
    }

    /// 检查当前是否已触发失效
    pub fn is_triggered(&self, current_price: f64, current_index: usize, is_buy: bool) -> bool {
        if let Some(exp) = self.expire_at_index {
            if current_index > exp {
                return true;
            }
        }
        if let Some(trig) = self.price_trigger {
            // 买入信号失效：价格跌破触发位；卖出信号失效：价格涨过触发位
            if is_buy && current_price < trig {
                return true;
            }
            if !is_buy && current_price > trig {
                return true;
            }
        }
        false
    }
}

/// R-P1-02 完整信号元数据
///
/// 每个识别出的信号都应附加此结构，便于策略层和 UI 统一处理
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignalMetadata {
    /// 信号级别
    pub level: SignalLevel,
    /// 阶段
    pub stage: Stage,
    /// 置信度 [0.0, 1.0]
    pub confidence: f64,
    /// 方向：+1 看多 / -1 看空 / 0 中性
    pub direction: i8,
    /// 消亡条件（可选，不设则永不过期）
    pub invalidation: Option<InvalidationCondition>,
    /// 原书出处（如 "ma p.200"）
    pub book_source: Option<String>,
    /// 人类可读描述
    pub explanation: String,
}

impl SignalMetadata {
    pub fn new(level: SignalLevel, stage: Stage, direction: i8) -> Self {
        Self {
            level,
            stage,
            confidence: 1.0,
            direction,
            invalidation: None,
            book_source: None,
            explanation: String::new(),
        }
    }

    pub fn with_confidence(mut self, c: f64) -> Self {
        self.confidence = c.clamp(0.0, 1.0);
        self
    }

    pub fn with_invalidation(mut self, inv: InvalidationCondition) -> Self {
        self.invalidation = Some(inv);
        self
    }

    pub fn with_book_source(mut self, src: impl Into<String>) -> Self {
        self.book_source = Some(src.into());
        self
    }

    pub fn with_explanation(mut self, exp: impl Into<String>) -> Self {
        self.explanation = exp.into();
        self
    }

    /// 最终权重 = level × direction_boost × confidence
    pub fn final_weight(&self) -> f64 {
        let is_sell = self.direction < 0;
        self.level.adjusted_for_direction(is_sell) * self.confidence
    }

    /// 检查是否已失效
    pub fn is_invalidated(&self, current_price: f64, current_index: usize) -> bool {
        let is_buy = self.direction > 0;
        self.invalidation
            .as_ref()
            .map(|inv| inv.is_triggered(current_price, current_index, is_buy))
            .unwrap_or(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn t_signal_level_weights() {
        assert_eq!(SignalLevel::Strong.weight_multiplier(), 1.5);
        assert_eq!(SignalLevel::Medium.weight_multiplier(), 1.0);
        assert_eq!(SignalLevel::Weak.weight_multiplier(), 0.5);
        assert_eq!(SignalLevel::Noise.weight_multiplier(), 0.1);
    }

    #[test]
    fn t_signal_level_ordering() {
        // Ord 允许比较：Strong > Medium > Weak > Noise
        assert!(SignalLevel::Strong > SignalLevel::Medium);
        assert!(SignalLevel::Medium > SignalLevel::Weak);
        assert!(SignalLevel::Weak > SignalLevel::Noise);
    }

    #[test]
    fn t_sell_signal_boosted_by_1_3x() {
        // E20 "果断卖出"：卖出权重 × 1.3
        let strong = SignalLevel::Strong;
        let buy = strong.adjusted_for_direction(false);
        let sell = strong.adjusted_for_direction(true);
        assert!(
            (sell - buy * 1.3).abs() < 1e-9,
            "卖出应 × 1.3；buy={} sell={}",
            buy,
            sell
        );
    }

    #[test]
    fn t_stage_position_action() {
        assert_eq!(Stage::Entry.position_action(), 1);
        assert_eq!(Stage::Exit.position_action(), -1);
        assert_eq!(Stage::Hold.position_action(), 0);
        assert_eq!(Stage::Watch.position_action(), 0);
    }

    #[test]
    fn t_invalidation_price_trigger_buy() {
        // 买入信号：价格跌破触发位 → 失效
        let inv = InvalidationCondition::new("跌破止损位").with_price(95.0);
        assert!(inv.is_triggered(90.0, 10, true));
        assert!(!inv.is_triggered(100.0, 10, true));
    }

    #[test]
    fn t_invalidation_price_trigger_sell() {
        // 卖出信号：价格涨过触发位 → 失效
        let inv = InvalidationCondition::new("涨过止盈位").with_price(105.0);
        assert!(inv.is_triggered(110.0, 10, false));
        assert!(!inv.is_triggered(100.0, 10, false));
    }

    #[test]
    fn t_invalidation_expiry_index() {
        // 超过过期索引 → 失效
        let inv = InvalidationCondition::new("5 日内有效").with_expiry(15);
        assert!(inv.is_triggered(100.0, 20, true));
        assert!(!inv.is_triggered(100.0, 10, true));
    }

    #[test]
    fn t_signal_metadata_final_weight() {
        // Strong 看涨 + confidence=0.8 → 1.5 × 1.0 × 0.8 = 1.2
        let meta = SignalMetadata::new(SignalLevel::Strong, Stage::Entry, 1).with_confidence(0.8);
        assert!((meta.final_weight() - 1.2).abs() < 1e-9);

        // Strong 看跌 + confidence=1.0 → 1.5 × 1.3 × 1.0 = 1.95
        let sell = SignalMetadata::new(SignalLevel::Strong, Stage::Exit, -1);
        assert!((sell.final_weight() - 1.95).abs() < 1e-9);
    }

    #[test]
    fn t_signal_metadata_invalidation_integration() {
        let meta = SignalMetadata::new(SignalLevel::Medium, Stage::Entry, 1)
            .with_invalidation(InvalidationCondition::new("止损").with_price(95.0));
        assert!(meta.is_invalidated(90.0, 5));
        assert!(!meta.is_invalidated(100.0, 5));
    }

    #[test]
    fn t_confidence_clamped_to_0_1() {
        let meta = SignalMetadata::new(SignalLevel::Strong, Stage::Entry, 1).with_confidence(1.5);
        assert_eq!(meta.confidence, 1.0);
        let meta = SignalMetadata::new(SignalLevel::Strong, Stage::Entry, 1).with_confidence(-0.5);
        assert_eq!(meta.confidence, 0.0);
    }
}
