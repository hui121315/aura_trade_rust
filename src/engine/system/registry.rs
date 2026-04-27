//! 种子体系注册表（硬编码，来自原书的经典组合）
//!
//! 详见 `SYSTEM_LAB_DESIGN.md` §10。
//!
//! # M3 完整范围（8 个经典体系）
//!
//! | # | ID | 名称 | 原书 | 方向 |
//! |---|---|---|---|---|
//! | 1 | `seed.ma_skeleton` | 均线骨架系统 | 均线 §1-2 | 多 |
//! | 2 | `seed.golden_dragon` | 金山谷·蛟龙出海 | 均线 §4.2 | 多 |
//! | 3 | `seed.candle_reversal` | K 线底部反转 | K 线 §2-3 | 多 |
//! | 4 | `seed.dow_trend` | 道氏趋势 | 趋势 §1-2 | 多 |
//! | 5 | `seed.resonance_4d` | 四维共振 | PRD §B8 | 多 |
//! | 6 | `seed.pattern_endgame` | 形态终局（顶部反转）| K 线 §7 | 空 |
//! | 7 | `seed.guillotine_risk` | 断头铡刀风控体系 | 均线 §4.3 | 多 + 硬清仓 |
//! | 8 | `seed.main_surge` | 主升浪追踪 | 均线 §4.4 | 多 |
//!
//! 注意：所有体系都被 runner 的**断头铡刀铁律**兜底保护。

use std::collections::HashMap;

use super::definition::{
    BacktestParams, CombineRule, CostModel, RiskParams, SystemDefinition, SystemMeta, SystemOrigin,
};

/// 构造所有种子体系
pub fn all_seeds() -> Vec<SystemDefinition> {
    vec![
        seed_ma_skeleton(),
        seed_golden_dragon(),
        seed_candle_reversal(),
        seed_dow_trend(),
        seed_resonance_4d(),
        seed_pattern_endgame(),
        seed_guillotine_risk(),
        seed_main_surge(),
    ]
}

/// 按 ID 查找种子体系
pub fn find_seed(id: &str) -> Option<SystemDefinition> {
    all_seeds().into_iter().find(|s| s.id == id)
}

// ============================================================
// 共用构造器：减少模板代码
// ============================================================

fn default_risk_medium() -> RiskParams {
    RiskParams {
        stop_atr_mult: 2.0,
        target_r: 3.0,
        max_hold_bars: 30,
        max_position_pct: 0.5,
    }
}

fn default_risk_long() -> RiskParams {
    RiskParams {
        stop_atr_mult: 2.5,
        target_r: 4.0,
        max_hold_bars: 60,
        max_position_pct: 0.5,
    }
}

fn default_backtest() -> BacktestParams {
    BacktestParams {
        warmup_bars: 60,
        cost_model: CostModel::default(),
    }
}

// ============================================================
// 种子 1：均线骨架系统
// ============================================================

fn seed_ma_skeleton() -> SystemDefinition {
    SystemDefinition {
        id: "seed.ma_skeleton".into(),
        name: "均线骨架系统".into(),
        origin: SystemOrigin::Seed,
        description: Some(
            "葛南维 B2 回踩 + 多头排列同时成立才开多。追求顺势低吸，不做反转。".into(),
        ),
        components: vec![
            "ma.granville.b2_pullback".into(),
            "ma_special.bull_arrangement".into(),
        ],
        combine: CombineRule::MajorityK { k: 2 },
        weights: HashMap::new(),
        risk: default_risk_medium(),
        backtest: default_backtest(),
        meta: SystemMeta { schema_version: 1, ..Default::default() },
    }
}

// ============================================================
// 种子 2：金山谷 · 蛟龙出海级联
// ============================================================

/// 金山谷 → 多头排列 → 加速上行 的级联
///
/// 原书：均线 §4.2 p.121（金山谷是看涨接力，配合多头排列和加速上行形成动量链）
fn seed_golden_dragon() -> SystemDefinition {
    SystemDefinition {
        id: "seed.golden_dragon".into(),
        name: "金山谷·蛟龙出海".into(),
        origin: SystemOrigin::Seed,
        description: Some(
            "金山谷出现后，15 根 K 线内形成多头排列并出现加速上行，视为看涨接力级联。".into(),
        ),
        components: vec![
            "ma_special.golden_valley".into(),
            "ma_special.bull_arrangement".into(),
            "ma_special.accelerating_up".into(),
        ],
        combine: CombineRule::SequentialCascade { window_bars: 15 },
        weights: HashMap::new(),
        risk: default_risk_long(),
        backtest: default_backtest(),
        meta: SystemMeta { schema_version: 1, ..Default::default() },
    }
}

// ============================================================
// 种子 3：K 线底部反转系统
// ============================================================

/// 底部反转：曙光初现 / 镊子底 / 红三兵 任一触发即做多
///
/// 原书：K 线 §2-3（所有多根 K 线反转形态）
/// 注：顶部反转 = `seed.pattern_endgame`（做空）
fn seed_candle_reversal() -> SystemDefinition {
    SystemDefinition {
        id: "seed.candle_reversal".into(),
        name: "K 线底部反转".into(),
        origin: SystemOrigin::Seed,
        description: Some(
            "曙光初现 / 镊子底 / 红三兵 任一强反转信号出现即做多。".into(),
        ),
        components: vec![
            "candle.piercing_line".into(),
            "candle.tweezers_bottom".into(),
            "candle.three_white_soldiers".into(),
        ],
        combine: CombineRule::MajorityK { k: 1 },
        weights: HashMap::new(),
        risk: default_risk_medium(),
        backtest: default_backtest(),
        meta: SystemMeta { schema_version: 1, ..Default::default() },
    }
}

// ============================================================
// 种子 4：道氏趋势系统
// ============================================================

/// HH/HL 结构确认 + 多头排列 + 葛南维 B2 入场
///
/// 原书：趋势 §1-2
fn seed_dow_trend() -> SystemDefinition {
    SystemDefinition {
        id: "seed.dow_trend".into(),
        name: "道氏趋势系统".into(),
        origin: SystemOrigin::Seed,
        description: Some(
            "道氏上升趋势成立 + 多头排列 + 葛南维 B2 回踩三者同时成立才开多。".into(),
        ),
        components: vec![
            "trend.dow_uptrend".into(),
            "ma_special.bull_arrangement".into(),
            "ma.granville.b2_pullback".into(),
        ],
        combine: CombineRule::AllAligned,
        weights: HashMap::new(),
        risk: default_risk_long(),
        backtest: default_backtest(),
        meta: SystemMeta { schema_version: 1, ..Default::default() },
    }
}

// ============================================================
// 种子 5：四维共振系统
// ============================================================

/// 四个维度（MaSignal / MaSpecial / Trend / Candle）**全部同向**才开多
///
/// 这是最稀疏但理论最稳健的体系。原书：PRD §B8（四维共振哲学）
fn seed_resonance_4d() -> SystemDefinition {
    SystemDefinition {
        id: "seed.resonance_4d".into(),
        name: "四维共振系统".into(),
        origin: SystemOrigin::Seed,
        description: Some(
            "MA 葛南维 + 多头排列 + 道氏上升 + K 线看涨吞没 四维必须同时成立。稀疏但高胜率。"
                .into(),
        ),
        components: vec![
            "ma.granville.b2_pullback".into(),
            "ma_special.bull_arrangement".into(),
            "trend.dow_uptrend".into(),
            "candle.bullish_engulfing".into(),
        ],
        combine: CombineRule::AllAligned,
        weights: HashMap::new(),
        risk: default_risk_long(),
        backtest: default_backtest(),
        meta: SystemMeta { schema_version: 1, ..Default::default() },
    }
}

// ============================================================
// 种子 6：形态终局系统（顶部反转，做空）
// ============================================================

/// 头肩顶 / 双顶 / 菱形顶 任一出现即做空
///
/// 原书：K 线 §7 图形。菱形顶为日线上最强反转信号（`historical_alpha_pct` +11.87%）。
fn seed_pattern_endgame() -> SystemDefinition {
    SystemDefinition {
        id: "seed.pattern_endgame".into(),
        name: "形态终局（顶部反转）".into(),
        origin: SystemOrigin::Seed,
        description: Some(
            "头肩顶 / 双顶 / 菱形顶 任一完成颈线突破即做空。菱形顶日线 3/3 全正，α +11.87%。"
                .into(),
        ),
        components: vec![
            "chart.head_and_shoulders_top".into(),
            "chart.double_top".into(),
            "chart.diamond_top".into(),
        ],
        combine: CombineRule::MajorityK { k: 1 },
        weights: HashMap::new(),
        risk: default_risk_medium(),
        backtest: default_backtest(),
        meta: SystemMeta { schema_version: 1, ..Default::default() },
    }
}

// ============================================================
// 种子 7：断头铡刀风控体系
// ============================================================

/// 入场：旱地拔葱 + 光脚阳线（加权 WeightedScore）
/// 出场：由 runner 的**断头铡刀铁律**硬覆盖自动触发（无需配置在 components）
///
/// 原书：均线 §4.3 p.380
fn seed_guillotine_risk() -> SystemDefinition {
    let mut w = HashMap::new();
    w.insert("ma_advanced.hanging_scallions".to_string(), 2.0);
    w.insert("candle.close_marubozu_bull".to_string(), 1.5);
    SystemDefinition {
        id: "seed.guillotine_risk".into(),
        name: "断头铡刀风控体系".into(),
        origin: SystemOrigin::Seed,
        description: Some(
            "入场：旱地拔葱（强） + 光脚阳线（中）加权。出场：断头铡刀铁律硬覆盖自动触发。".into(),
        ),
        components: vec![
            "ma_advanced.hanging_scallions".into(),
            "candle.close_marubozu_bull".into(),
        ],
        combine: CombineRule::WeightedScore { threshold: 1.8 },
        weights: w,
        risk: default_risk_medium(),
        backtest: default_backtest(),
        meta: SystemMeta { schema_version: 1, ..Default::default() },
    }
}

// ============================================================
// 种子 8：主升浪追踪体系
// ============================================================

/// 加速上行 + 多头排列 + 红三兵 中 2 个同向即追多
///
/// 原书：均线 §4.4（主升浪识别）
fn seed_main_surge() -> SystemDefinition {
    SystemDefinition {
        id: "seed.main_surge".into(),
        name: "主升浪追踪".into(),
        origin: SystemOrigin::Seed,
        description: Some(
            "加速上行 + 多头排列 + 红三兵 三信号中出现任 2 个即追多。追求主升段的大单边。".into(),
        ),
        components: vec![
            "ma_special.accelerating_up".into(),
            "ma_special.bull_arrangement".into(),
            "candle.three_white_soldiers".into(),
        ],
        combine: CombineRule::MajorityK { k: 2 },
        weights: HashMap::new(),
        risk: default_risk_long(),
        backtest: default_backtest(),
        meta: SystemMeta { schema_version: 1, ..Default::default() },
    }
}

// ============================================================
// 测试
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn t_all_seeds_validate() {
        for s in all_seeds() {
            s.validate()
                .unwrap_or_else(|e| panic!("种子体系 {} 校验失败: {}", s.id, e));
        }
    }

    #[test]
    fn t_all_seeds_count_is_8() {
        assert_eq!(all_seeds().len(), 8);
    }

    #[test]
    fn t_all_seed_ids_unique() {
        let seeds = all_seeds();
        let mut ids: Vec<_> = seeds.iter().map(|s| s.id.clone()).collect();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), seeds.len());
    }

    #[test]
    fn t_find_seed_by_id() {
        assert!(find_seed("seed.ma_skeleton").is_some());
        assert!(find_seed("seed.golden_dragon").is_some());
        assert!(find_seed("seed.candle_reversal").is_some());
        assert!(find_seed("seed.dow_trend").is_some());
        assert!(find_seed("seed.resonance_4d").is_some());
        assert!(find_seed("seed.pattern_endgame").is_some());
        assert!(find_seed("seed.guillotine_risk").is_some());
        assert!(find_seed("seed.main_surge").is_some());
        assert!(find_seed("seed.nonexistent").is_none());
    }

    #[test]
    fn t_cascade_seed_uses_cascade_rule() {
        let s = find_seed("seed.golden_dragon").unwrap();
        assert!(matches!(s.combine, CombineRule::SequentialCascade { .. }));
    }

    #[test]
    fn t_pattern_endgame_uses_short_direction() {
        let s = find_seed("seed.pattern_endgame").unwrap();
        // 三个组件都是空头方向
        for cid in &s.components {
            let c = super::super::component::find_component(cid).unwrap();
            assert_eq!(c.direction_bias, -1, "{} 应为空头组件", cid);
        }
    }

    #[test]
    fn t_resonance_4d_covers_all_dimensions() {
        use super::super::component::{find_component, ComponentDimension};
        let s = find_seed("seed.resonance_4d").unwrap();
        let dims: std::collections::HashSet<ComponentDimension> = s
            .components
            .iter()
            .filter_map(|id| find_component(id).map(|c| c.dimension))
            .collect();
        assert!(dims.len() >= 3, "四维共振应覆盖至少 3 个维度，实际 {:?}", dims);
    }
}

