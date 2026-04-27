//! F1：多合一现象识别器（R-P1-16）
//!
//! 原书跨章节铁证：当**均线 + 趋势线 + 支撑/压力位**在同一价格带（±3%）重叠时，
//! 该价位成为"兵家必争之地"，信号强度**倍增**。
//!
//! # 原书引用
//!
//! - **trend p.216** 多合一现象（10 条买卖矩阵的实战基础）
//! - **candle p.520** 圆底颈线 ∩ 下降三角形下边线 → 大阳线同时突破 = 多方获胜
//! - **ma p.310** 断头铡刀 + 倾盆大雨 + 60 日 S6 + 死亡谷 同区域共振
//!
//! # 工程规则
//!
//! 1. 同价格带（默认 ±3%）内出现 **≥2 种不同类型**的组件 → 1 个 Confluence
//! 2. `strength_multiplier` = 1.0 + 0.5 × (unique_component_kinds - 1)（clamp [1.0, 3.0]）
//! 3. 3% 来自跨全书铁证不变量（见 `AURA_BOOK_HANDBOOK.md` §2.1）
//!
//! # 使用
//!
//! ```
//! use aura_trade::engine::signal::confluence::*;
//! use aura_trade::engine::trend::TrendLevel;
//!
//! let components = vec![
//!     ConfluenceComponent::MovingAverage { period: 60, price: 100.0 },
//!     ConfluenceComponent::TrendLine { level: TrendLevel::Mid, price: 101.0 },
//!     ConfluenceComponent::SupportResistance { strength: 0.8, price: 99.5 },
//! ];
//! let params = ConfluenceParams::default();
//! let confluences = detect_confluences(&components, &params);
//! assert!(!confluences.is_empty());
//! ```

use serde::{Deserialize, Serialize};

use crate::engine::trend::TrendLevel;

/// 多合一组件类型（哪些来源的支撑/压力位重叠）
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum ConfluenceComponent {
    /// 均线（如 60 日均线当前价位）
    MovingAverage { period: usize, price: f64 },
    /// 趋势线（当前投影价）
    TrendLine { level: TrendLevel, price: f64 },
    /// 支撑/压力位
    SupportResistance { strength: f64, price: f64 },
    /// 斐波那契回撤位
    Fibonacci { ratio: f64, price: f64 },
    /// 整数心理价位（100/1000 等）
    PsychologicalPrice { price: f64 },
    /// 前高/前低
    PriorSwingPoint { is_high: bool, price: f64 },
}

impl ConfluenceComponent {
    /// 获取组件的价格
    pub fn price(&self) -> f64 {
        match self {
            ConfluenceComponent::MovingAverage { price, .. }
            | ConfluenceComponent::TrendLine { price, .. }
            | ConfluenceComponent::SupportResistance { price, .. }
            | ConfluenceComponent::Fibonacci { price, .. }
            | ConfluenceComponent::PsychologicalPrice { price, .. }
            | ConfluenceComponent::PriorSwingPoint { price, .. } => *price,
        }
    }

    /// 获取组件类型的判别（用于统计"不同类型"数量）
    pub fn kind_id(&self) -> u8 {
        match self {
            ConfluenceComponent::MovingAverage { .. } => 0,
            ConfluenceComponent::TrendLine { .. } => 1,
            ConfluenceComponent::SupportResistance { .. } => 2,
            ConfluenceComponent::Fibonacci { .. } => 3,
            ConfluenceComponent::PsychologicalPrice { .. } => 4,
            ConfluenceComponent::PriorSwingPoint { .. } => 5,
        }
    }

    pub fn kind_label(&self) -> &'static str {
        match self {
            ConfluenceComponent::MovingAverage { .. } => "均线",
            ConfluenceComponent::TrendLine { .. } => "趋势线",
            ConfluenceComponent::SupportResistance { .. } => "支撑压力位",
            ConfluenceComponent::Fibonacci { .. } => "斐波那契",
            ConfluenceComponent::PsychologicalPrice { .. } => "整数心理价",
            ConfluenceComponent::PriorSwingPoint { .. } => "前高/前低",
        }
    }
}

/// 多合一现象（R-P1-16 核心结构）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Confluence {
    /// 合流中心价（所有组件价格的算术平均）
    pub center_price: f64,
    /// 价格带上界
    pub price_upper: f64,
    /// 价格带下界
    pub price_lower: f64,
    /// 参与合流的组件列表
    pub components: Vec<ConfluenceComponent>,
    /// **强度倍增器**（R-P1-16 核心：1.5 × n）
    pub strength_multiplier: f64,
    /// 不同类型组件数量（≥ 2 才成立）
    pub unique_kinds: usize,
}

impl Confluence {
    /// 是否为"强合流"（≥3 种不同类型组件）
    pub fn is_strong(&self) -> bool {
        self.unique_kinds >= 3
    }

    /// 信号标签（用于 UI 展示）
    pub fn label(&self) -> String {
        let kinds: Vec<&str> = self.components.iter().map(|c| c.kind_label()).collect();
        let mut unique_kinds: Vec<&str> = Vec::new();
        for k in &kinds {
            if !unique_kinds.contains(k) {
                unique_kinds.push(k);
            }
        }
        format!(
            "多合一 @ {:.2}（{}）× {:.1}",
            self.center_price,
            unique_kinds.join("+"),
            self.strength_multiplier,
        )
    }
}

/// 参数
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct ConfluenceParams {
    /// 价格带容差（默认 3%，原书跨全书铁证）
    pub tolerance_pct: f64,
    /// 每多一种不同类型组件 → 额外 +0.5 倍强度
    pub per_kind_boost: f64,
    /// 最大倍增（避免单个合流权重失控）
    pub max_multiplier: f64,
}

impl Default for ConfluenceParams {
    fn default() -> Self {
        Self {
            tolerance_pct: 0.03, // 跨全书铁证
            per_kind_boost: 0.5, // 1.0 + 0.5 × (n-1)
            max_multiplier: 3.0,
        }
    }
}

/// 检测多合一现象
///
/// # 算法
/// 1. 按价格排序所有组件
/// 2. 贪心聚类：当前价格与聚类中心相差 ≤ tolerance_pct → 加入；否则新建聚类
/// 3. 每个聚类统计 **unique_kinds**（不同组件类型数）
/// 4. `unique_kinds >= 2` 才视为一个合流
/// 5. 强度倍增 = `1.0 + per_kind_boost × (unique_kinds - 1)`（clamp 到 max_multiplier）
pub fn detect_confluences(
    components: &[ConfluenceComponent],
    params: &ConfluenceParams,
) -> Vec<Confluence> {
    if components.len() < 2 {
        return Vec::new();
    }
    // 过滤无效价格
    let mut sorted: Vec<ConfluenceComponent> = components
        .iter()
        .filter(|c| c.price().is_finite() && c.price() > 0.0)
        .copied()
        .collect();
    sorted.sort_by(|a, b| {
        a.price()
            .partial_cmp(&b.price())
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    let mut clusters: Vec<Vec<ConfluenceComponent>> = Vec::new();
    for c in sorted {
        let added = if let Some(last) = clusters.last_mut() {
            let centroid = last.iter().map(|x| x.price()).sum::<f64>() / last.len() as f64;
            let diff = (c.price() - centroid).abs() / centroid.abs().max(1e-9);
            if diff <= params.tolerance_pct {
                last.push(c);
                true
            } else {
                false
            }
        } else {
            false
        };
        if !added {
            clusters.push(vec![c]);
        }
    }

    clusters
        .into_iter()
        .filter_map(|cluster| {
            if cluster.len() < 2 {
                return None;
            }
            // 统计 unique_kinds
            let mut kinds_seen: Vec<u8> = Vec::new();
            for c in &cluster {
                let k = c.kind_id();
                if !kinds_seen.contains(&k) {
                    kinds_seen.push(k);
                }
            }
            let unique_kinds = kinds_seen.len();
            if unique_kinds < 2 {
                return None; // 必须 ≥2 种**不同类型**
            }
            let center_price =
                cluster.iter().map(|c| c.price()).sum::<f64>() / cluster.len() as f64;
            let price_lower = center_price * (1.0 - params.tolerance_pct);
            let price_upper = center_price * (1.0 + params.tolerance_pct);
            let multiplier = (1.0 + params.per_kind_boost * (unique_kinds as f64 - 1.0))
                .min(params.max_multiplier);
            Some(Confluence {
                center_price,
                price_upper,
                price_lower,
                components: cluster,
                strength_multiplier: multiplier,
                unique_kinds,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn t_two_component_confluence_detected() {
        // 均线 + 趋势线 ≈ 同一价位 → 1 个合流
        let components = vec![
            ConfluenceComponent::MovingAverage {
                period: 60,
                price: 100.0,
            },
            ConfluenceComponent::TrendLine {
                level: TrendLevel::Mid,
                price: 101.0,
            },
        ];
        let confs = detect_confluences(&components, &ConfluenceParams::default());
        assert_eq!(confs.len(), 1);
        assert_eq!(confs[0].unique_kinds, 2);
        // 1 + 0.5 × (2-1) = 1.5
        assert!((confs[0].strength_multiplier - 1.5).abs() < 1e-9);
    }

    #[test]
    fn t_same_kind_no_confluence() {
        // 两条均线在同价位 → 不算合流（同类型不计）
        let components = vec![
            ConfluenceComponent::MovingAverage {
                period: 20,
                price: 100.0,
            },
            ConfluenceComponent::MovingAverage {
                period: 60,
                price: 101.0,
            },
        ];
        let confs = detect_confluences(&components, &ConfluenceParams::default());
        assert_eq!(confs.len(), 0, "同类型组件不构成多合一");
    }

    #[test]
    fn t_triple_confluence_strong() {
        // 均线 + 趋势线 + 支撑位 → 强合流
        let components = vec![
            ConfluenceComponent::MovingAverage {
                period: 60,
                price: 100.0,
            },
            ConfluenceComponent::TrendLine {
                level: TrendLevel::Long,
                price: 100.5,
            },
            ConfluenceComponent::SupportResistance {
                strength: 0.8,
                price: 99.5,
            },
        ];
        let confs = detect_confluences(&components, &ConfluenceParams::default());
        assert_eq!(confs.len(), 1);
        assert_eq!(confs[0].unique_kinds, 3);
        assert!(confs[0].is_strong());
        // 1 + 0.5 × 2 = 2.0
        assert!((confs[0].strength_multiplier - 2.0).abs() < 1e-9);
    }

    #[test]
    fn t_far_apart_prices_no_confluence() {
        // 价格相差 > 3% → 不聚类
        let components = vec![
            ConfluenceComponent::MovingAverage {
                period: 60,
                price: 100.0,
            },
            ConfluenceComponent::TrendLine {
                level: TrendLevel::Mid,
                price: 110.0, // 10% 差
            },
        ];
        let confs = detect_confluences(&components, &ConfluenceParams::default());
        assert_eq!(confs.len(), 0);
    }

    #[test]
    fn t_tolerance_boundary_3pct() {
        // 恰好 3% 差 → 允许
        let components = vec![
            ConfluenceComponent::MovingAverage {
                period: 60,
                price: 100.0,
            },
            ConfluenceComponent::TrendLine {
                level: TrendLevel::Mid,
                price: 103.0, // 3.0% 差
            },
        ];
        let confs = detect_confluences(&components, &ConfluenceParams::default());
        assert_eq!(confs.len(), 1, "3% 边界应被纳入");
    }

    #[test]
    fn t_multi_cluster_separated() {
        // 两组分别形成合流（不同价位）
        let components = vec![
            // 价位 100 组
            ConfluenceComponent::MovingAverage {
                period: 60,
                price: 100.0,
            },
            ConfluenceComponent::TrendLine {
                level: TrendLevel::Mid,
                price: 100.5,
            },
            // 价位 150 组
            ConfluenceComponent::MovingAverage {
                period: 20,
                price: 150.0,
            },
            ConfluenceComponent::SupportResistance {
                strength: 0.5,
                price: 149.0,
            },
        ];
        let confs = detect_confluences(&components, &ConfluenceParams::default());
        assert_eq!(confs.len(), 2);
    }

    #[test]
    fn t_max_multiplier_clamped() {
        // 6 种不同组件 → 理论上 1 + 0.5 × 5 = 3.5；应 clamp 到 3.0
        let components = vec![
            ConfluenceComponent::MovingAverage {
                period: 60,
                price: 100.0,
            },
            ConfluenceComponent::TrendLine {
                level: TrendLevel::Long,
                price: 100.1,
            },
            ConfluenceComponent::SupportResistance {
                strength: 0.8,
                price: 100.2,
            },
            ConfluenceComponent::Fibonacci {
                ratio: 0.618,
                price: 100.3,
            },
            ConfluenceComponent::PsychologicalPrice { price: 100.4 },
            ConfluenceComponent::PriorSwingPoint {
                is_high: true,
                price: 100.5,
            },
        ];
        let confs = detect_confluences(&components, &ConfluenceParams::default());
        assert_eq!(confs.len(), 1);
        assert_eq!(confs[0].unique_kinds, 6);
        assert!(
            (confs[0].strength_multiplier - 3.0).abs() < 1e-9,
            "应 clamp 到 max 3.0；实际 {}",
            confs[0].strength_multiplier
        );
    }

    #[test]
    fn t_empty_input_returns_empty() {
        let confs = detect_confluences(&[], &ConfluenceParams::default());
        assert!(confs.is_empty());
        let confs = detect_confluences(
            &[ConfluenceComponent::MovingAverage {
                period: 20,
                price: 100.0,
            }],
            &ConfluenceParams::default(),
        );
        assert!(confs.is_empty());
    }

    #[test]
    fn t_invalid_prices_filtered() {
        let components = vec![
            ConfluenceComponent::MovingAverage {
                period: 60,
                price: f64::NAN,
            },
            ConfluenceComponent::TrendLine {
                level: TrendLevel::Mid,
                price: 100.0,
            },
            ConfluenceComponent::SupportResistance {
                strength: 0.5,
                price: 100.5,
            },
        ];
        let confs = detect_confluences(&components, &ConfluenceParams::default());
        // NaN 被过滤，剩余 2 个有效组件形成 1 个合流
        assert_eq!(confs.len(), 1);
    }

    #[test]
    fn t_confluence_label_formats_correctly() {
        let components = vec![
            ConfluenceComponent::MovingAverage {
                period: 60,
                price: 100.0,
            },
            ConfluenceComponent::TrendLine {
                level: TrendLevel::Mid,
                price: 100.5,
            },
        ];
        let confs = detect_confluences(&components, &ConfluenceParams::default());
        let label = confs[0].label();
        assert!(label.contains("均线"));
        assert!(label.contains("趋势线"));
        assert!(label.contains("× 1.5"));
    }
}
