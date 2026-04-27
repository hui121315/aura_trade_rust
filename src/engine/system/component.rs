//! 组件元数据 + 注册表（MVP 21 个组件）
//!
//! 每个 [`Component`] 代表一个可复用的"信号识别单元"，对应原书某页描述的
//! 一个具体形态/规则。本文件**不存放识别逻辑**——识别逻辑在各自的 engine 模块里
//! 已经实现，本注册表只做"元数据 + ID 映射"。
//!
//! 触发事件的产生在 [`crate::engine::system::scan`] 里，一次性扫描所有 bar。
//!
//! # 命名规范
//!
//! `<dimension>.<family>.<variant>` 全小写 snake_case，例如：
//! - `ma.granville.b2_pullback`
//! - `ma_special.golden_valley`
//! - `ma_advanced.guillotine`
//! - `candle.bullish_engulfing`
//! - `trend.dow_uptrend`

use serde::{Deserialize, Serialize};

/// 单个体系允许的最多组件数（复杂度上限，详见 SYSTEM_LAB_DESIGN.md §8.2）
pub const COMPONENT_MAX_K: usize = 5;

/// 组件维度分类
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ComponentDimension {
    /// 均线信号（葛南维八法）
    MaSignal,
    /// 均线特殊形态（17 大）
    MaSpecial,
    /// 均线高级形态（旱地拔葱/毒蜘蛛/断头铡刀/向上发散）
    MaAdvanced,
    /// K 线形态
    CandlePattern,
    /// 技术图形（头肩/三角/楔形）
    ChartPattern,
    /// 趋势结构（道氏 HH/HL）
    TrendStructure,
}

impl ComponentDimension {
    pub fn as_str(&self) -> &'static str {
        match self {
            ComponentDimension::MaSignal => "MaSignal",
            ComponentDimension::MaSpecial => "MaSpecial",
            ComponentDimension::MaAdvanced => "MaAdvanced",
            ComponentDimension::CandlePattern => "CandlePattern",
            ComponentDimension::ChartPattern => "ChartPattern",
            ComponentDimension::TrendStructure => "TrendStructure",
        }
    }
}

/// 组件元数据（不含识别逻辑）
#[derive(Debug, Clone, Serialize)]
pub struct Component {
    /// 唯一 ID，例如 `ma.granville.b2_pullback`
    pub id: &'static str,
    /// 人类可读标签
    pub label: &'static str,
    /// 原书出处，例如 `"均线 §2.1 p.48"`
    pub book_source: &'static str,
    /// 维度分类
    pub dimension: ComponentDimension,
    /// 方向偏好：+1 只看多 / -1 只看空 / 0 双向
    pub direction_bias: i8,
    /// 可选：历史 alpha 百分点（来自 PATTERN_EFFECTIVENESS_REPORT）
    pub historical_alpha_pct: Option<f64>,
    /// 可选：历史胜率 [0, 1]
    pub historical_winrate: Option<f64>,
}

// ============================================================
// 注册表（MVP 21 个组件）
// ============================================================

/// 全局只读组件注册表
///
/// 使用 `const` + `&[Component]` 模式；零运行时成本，也不需要 `once_cell`。
pub const COMPONENTS: &[Component] = &[
    // ========= MaSignal (5) — 葛南维八法的 5 个代表 =========
    Component {
        id: "ma.granville.b1_breakout",
        label: "葛南维 B1 突破买入",
        book_source: "均线 §2.1",
        dimension: ComponentDimension::MaSignal,
        direction_bias: 1,
        historical_alpha_pct: None,
        historical_winrate: None,
    },
    Component {
        id: "ma.granville.b2_pullback",
        label: "葛南维 B2 回踩不破",
        book_source: "均线 §2.1",
        dimension: ComponentDimension::MaSignal,
        direction_bias: 1,
        historical_alpha_pct: None,
        historical_winrate: None,
    },
    Component {
        id: "ma.granville.b3_false_break",
        label: "葛南维 B3 假跌破",
        book_source: "均线 §2.1",
        dimension: ComponentDimension::MaSignal,
        direction_bias: 1,
        historical_alpha_pct: None,
        historical_winrate: None,
    },
    Component {
        id: "ma.granville.s1_breakdown",
        label: "葛南维 S1 跌破卖出",
        book_source: "均线 §2.1",
        dimension: ComponentDimension::MaSignal,
        direction_bias: -1,
        historical_alpha_pct: None,
        historical_winrate: None,
    },
    Component {
        id: "ma.granville.s2_rebound",
        label: "葛南维 S2 反弹卖出",
        book_source: "均线 §2.1",
        dimension: ComponentDimension::MaSignal,
        direction_bias: -1,
        historical_alpha_pct: None,
        historical_winrate: None,
    },
    // ========= MaSpecial (6) — 17 大里的最强 6 个 =========
    Component {
        id: "ma_special.golden_valley",
        label: "金山谷",
        book_source: "均线 Ch4·1·8",
        dimension: ComponentDimension::MaSpecial,
        direction_bias: 1,
        historical_alpha_pct: Some(2.13),
        historical_winrate: Some(0.553),
    },
    Component {
        id: "ma_special.death_valley",
        label: "死亡谷",
        book_source: "均线 Ch4·1·9",
        dimension: ComponentDimension::MaSpecial,
        direction_bias: -1,
        historical_alpha_pct: Some(19.03), // 周线极强
        historical_winrate: Some(0.587),
    },
    Component {
        id: "ma_special.bull_arrangement",
        label: "多头排列",
        book_source: "均线 Ch3·3·1 p.204",
        dimension: ComponentDimension::MaSpecial,
        direction_bias: 1,
        historical_alpha_pct: Some(1.83),
        historical_winrate: Some(0.541),
    },
    Component {
        id: "ma_special.bear_arrangement",
        label: "空头排列",
        book_source: "均线 Ch3·3·2 p.204",
        dimension: ComponentDimension::MaSpecial,
        direction_bias: -1,
        historical_alpha_pct: None,
        historical_winrate: None,
    },
    Component {
        id: "ma_special.accelerating_up",
        label: "加速上行",
        book_source: "均线 Ch4·1·1",
        dimension: ComponentDimension::MaSpecial,
        direction_bias: 1,
        historical_alpha_pct: Some(1.77),
        historical_winrate: Some(0.537),
    },
    Component {
        id: "ma_special.accelerating_down",
        label: "加速下行",
        book_source: "均线 Ch4·1·2",
        dimension: ComponentDimension::MaSpecial,
        direction_bias: -1,
        historical_alpha_pct: None,
        historical_winrate: None,
    },
    // ========= MaAdvanced (2) — 高级派生形态 =========
    Component {
        id: "ma_advanced.hanging_scallions",
        label: "旱地拔葱（早期看涨）",
        book_source: "均线 §A9 p.340",
        dimension: ComponentDimension::MaAdvanced,
        direction_bias: 1,
        historical_alpha_pct: None,
        historical_winrate: None,
    },
    Component {
        id: "ma_advanced.guillotine",
        label: "断头铡刀（紧急清仓）",
        book_source: "均线 §A9 p.380",
        dimension: ComponentDimension::MaAdvanced,
        direction_bias: -1,
        historical_alpha_pct: None,
        historical_winrate: None,
    },
    // ========= CandlePattern (6) — 最稳定的 6 个 =========
    Component {
        id: "candle.bullish_engulfing",
        label: "看涨吞没",
        book_source: "K线 §2 吞没",
        dimension: ComponentDimension::CandlePattern,
        direction_bias: 1,
        historical_alpha_pct: None,
        historical_winrate: None,
    },
    Component {
        id: "candle.bearish_engulfing",
        label: "看跌吞没",
        book_source: "K线 §2 吞没",
        dimension: ComponentDimension::CandlePattern,
        direction_bias: -1,
        historical_alpha_pct: None,
        historical_winrate: None,
    },
    Component {
        id: "candle.morning_star",
        label: "早晨之星",
        book_source: "K线 §3 三根",
        dimension: ComponentDimension::CandlePattern,
        direction_bias: 1,
        historical_alpha_pct: None,
        historical_winrate: None,
    },
    Component {
        id: "candle.evening_star",
        label: "黄昏之星",
        book_source: "K线 §3 三根",
        dimension: ComponentDimension::CandlePattern,
        direction_bias: -1,
        historical_alpha_pct: None,
        historical_winrate: None,
    },
    Component {
        id: "candle.three_white_soldiers",
        label: "红三兵",
        book_source: "K线 §3 三根",
        dimension: ComponentDimension::CandlePattern,
        direction_bias: 1,
        historical_alpha_pct: None,
        historical_winrate: None,
    },
    Component {
        id: "candle.three_black_crows",
        label: "黑三兵",
        book_source: "K线 §3 三根",
        dimension: ComponentDimension::CandlePattern,
        direction_bias: -1,
        historical_alpha_pct: None,
        historical_winrate: None,
    },
    // ========= CandlePattern M3 扩展 (+6) =========
    Component {
        id: "candle.piercing_line",
        label: "曙光初现",
        book_source: "K线 §2 双根",
        dimension: ComponentDimension::CandlePattern,
        direction_bias: 1,
        historical_alpha_pct: None,
        historical_winrate: None,
    },
    Component {
        id: "candle.dark_cloud_cover",
        label: "乌云盖顶",
        book_source: "K线 §2 双根",
        dimension: ComponentDimension::CandlePattern,
        direction_bias: -1,
        historical_alpha_pct: None,
        historical_winrate: None,
    },
    Component {
        id: "candle.tweezers_bottom",
        label: "镊子底（平底）",
        book_source: "K线 §2 双根",
        dimension: ComponentDimension::CandlePattern,
        direction_bias: 1,
        historical_alpha_pct: Some(6.05), // 周线 3/3 正
        historical_winrate: Some(0.80),
    },
    Component {
        id: "candle.tweezers_top",
        label: "镊子顶（平顶）",
        book_source: "K线 §2 双根",
        dimension: ComponentDimension::CandlePattern,
        direction_bias: -1,
        historical_alpha_pct: None,
        historical_winrate: None,
    },
    Component {
        id: "candle.close_marubozu_bull",
        label: "光脚阳线",
        book_source: "K线 §1 单根",
        dimension: ComponentDimension::CandlePattern,
        direction_bias: 1,
        historical_alpha_pct: Some(14.65), // 周线 3/3 极强
        historical_winrate: Some(0.662),
    },
    Component {
        id: "candle.close_marubozu_bear",
        label: "光脚阴线",
        book_source: "K线 §1 单根",
        dimension: ComponentDimension::CandlePattern,
        direction_bias: -1,
        historical_alpha_pct: Some(12.54), // 周线 3/3 + 日线/4h 一致
        historical_winrate: Some(0.60),
    },
    // ========= ChartPattern M3 扩展 (+5) =========
    Component {
        id: "chart.head_and_shoulders_top",
        label: "头肩顶",
        book_source: "K线 §7 图形",
        dimension: ComponentDimension::ChartPattern,
        direction_bias: -1,
        historical_alpha_pct: None,
        historical_winrate: None,
    },
    Component {
        id: "chart.head_and_shoulders_bottom",
        label: "头肩底",
        book_source: "K线 §7 图形",
        dimension: ComponentDimension::ChartPattern,
        direction_bias: 1,
        historical_alpha_pct: None,
        historical_winrate: None,
    },
    Component {
        id: "chart.double_top",
        label: "双顶（M 形）",
        book_source: "K线 §7 图形",
        dimension: ComponentDimension::ChartPattern,
        direction_bias: -1,
        historical_alpha_pct: None,
        historical_winrate: None,
    },
    Component {
        id: "chart.double_bottom",
        label: "双底（W 形）",
        book_source: "K线 §7 图形",
        dimension: ComponentDimension::ChartPattern,
        direction_bias: 1,
        historical_alpha_pct: None,
        historical_winrate: None,
    },
    Component {
        id: "chart.diamond_top",
        label: "菱形顶",
        book_source: "K线 §7 图形",
        dimension: ComponentDimension::ChartPattern,
        direction_bias: -1,
        historical_alpha_pct: Some(11.87), // 日线 3/3 强
        historical_winrate: Some(0.857),
    },
    // ========= TrendStructure (2) — 道氏 =========
    Component {
        id: "trend.dow_uptrend",
        label: "道氏上升趋势（HH+HL）",
        book_source: "趋势 §1.2",
        dimension: ComponentDimension::TrendStructure,
        direction_bias: 1,
        historical_alpha_pct: None,
        historical_winrate: None,
    },
    Component {
        id: "trend.dow_downtrend",
        label: "道氏下降趋势（LH+LL）",
        book_source: "趋势 §1.2",
        dimension: ComponentDimension::TrendStructure,
        direction_bias: -1,
        historical_alpha_pct: None,
        historical_winrate: None,
    },
];

/// 按 ID 查找组件
pub fn find_component(id: &str) -> Option<&'static Component> {
    COMPONENTS.iter().find(|c| c.id == id)
}

/// 返回所有组件切片
pub fn all_components() -> &'static [Component] {
    COMPONENTS
}

/// 按维度过滤
pub fn components_by_dimension(dim: ComponentDimension) -> Vec<&'static Component> {
    COMPONENTS.iter().filter(|c| c.dimension == dim).collect()
}

// ============================================================
// 测试
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn t_registry_has_expected_count() {
        assert_eq!(COMPONENTS.len(), 32);
    }

    #[test]
    fn t_all_ids_are_unique() {
        let mut ids: Vec<&str> = COMPONENTS.iter().map(|c| c.id).collect();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), COMPONENTS.len(), "组件 ID 必须全部唯一");
    }

    #[test]
    fn t_all_ids_follow_naming_convention() {
        for c in COMPONENTS {
            assert!(
                c.id.contains('.'),
                "ID 必须带命名空间点号：{}",
                c.id
            );
            assert!(
                c.id.chars().all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '.' || ch == '_'),
                "ID 只允许小写 ASCII / 数字 / . / _：{}",
                c.id
            );
        }
    }

    #[test]
    fn t_direction_bias_valid() {
        for c in COMPONENTS {
            assert!([-1i8, 0, 1].contains(&c.direction_bias), "bias: {}", c.id);
        }
    }

    #[test]
    fn t_find_component() {
        assert!(find_component("ma.granville.b2_pullback").is_some());
        assert!(find_component("ma_special.golden_valley").is_some());
        assert!(find_component("__not_exist__").is_none());
    }

    #[test]
    fn t_components_by_dimension() {
        let ma_signals = components_by_dimension(ComponentDimension::MaSignal);
        assert_eq!(ma_signals.len(), 5, "应有 5 个葛南维组件");

        let ma_advanced = components_by_dimension(ComponentDimension::MaAdvanced);
        assert_eq!(ma_advanced.len(), 2);

        let candles = components_by_dimension(ComponentDimension::CandlePattern);
        assert_eq!(candles.len(), 12, "M3 新增 6 candle → 总 12");

        let charts = components_by_dimension(ComponentDimension::ChartPattern);
        assert_eq!(charts.len(), 5, "M3 新增 5 chart");

        let trends = components_by_dimension(ComponentDimension::TrendStructure);
        assert_eq!(trends.len(), 2);

        let ma_special = components_by_dimension(ComponentDimension::MaSpecial);
        assert_eq!(ma_special.len(), 6);
    }

    #[test]
    fn t_guillotine_component_exists() {
        // 断头铡刀必须存在（runner 的铁律依赖它）
        let c = find_component("ma_advanced.guillotine");
        assert!(c.is_some());
        assert_eq!(c.unwrap().direction_bias, -1);
    }
}
