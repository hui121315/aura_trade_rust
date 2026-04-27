//! `SystemDefinition` 及其周边配置类型
//!
//! 详见 `SYSTEM_LAB_DESIGN.md` §4。
//!
//! 本文件定义的所有结构都是**纯数据**，不含逻辑；组合/回测/发现逻辑在其他文件。

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use super::component::COMPONENT_MAX_K;
use crate::engine::backtest::{EquityPoint, Performance};

// ============================================================
// 聚合规则
// ============================================================

/// 当多组件同时触发时，如何得出最终的交易信号
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type")]
pub enum CombineRule {
    /// 所有组件必须同向触发（最严格）
    AllAligned,

    /// 至少 `k` 个组件同向触发
    MajorityK { k: usize },

    /// 加权分数超过阈值：Σ(weight_i × direction_i × confidence_i) ≥ threshold
    ///
    /// `weights` 在 `SystemDefinition.weights` 里
    WeightedScore { threshold: f64 },

    /// 级联：按 `components` 声明顺序依次触发，窗口内有效
    ///
    /// 备注：M1 暂不实现，定义保留
    SequentialCascade { window_bars: usize },
}

impl Default for CombineRule {
    fn default() -> Self {
        CombineRule::AllAligned
    }
}

// ============================================================
// 风控 / 回测 / 成本
// ============================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RiskParams {
    /// 止损 = entry ± ATR × mult
    pub stop_atr_mult: f64,
    /// 目标止盈 = R × target_r（R = |entry − stop|）
    pub target_r: f64,
    /// 最大持仓 K 线数（超过强制离场，以收盘价）
    pub max_hold_bars: usize,
    /// 最大仓位占资金比例（0.0-1.0）；M1 使用固定风险 1%，此字段 v2 生效
    pub max_position_pct: f64,
}

impl Default for RiskParams {
    fn default() -> Self {
        Self {
            stop_atr_mult: 2.0,
            target_r: 3.0,
            max_hold_bars: 30,
            max_position_pct: 0.5,
        }
    }
}

/// 成本模型（见 SYSTEM_LAB_DESIGN.md §16）
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(tag = "mode")]
pub enum CostModel {
    /// 零成本
    Zero,
    /// 固定双边：单边手续费 + 单边滑点（均为百分数，如 0.1 = 0.1%）
    Fixed { fee_pct: f64, slip_pct: f64 },
}

impl Default for CostModel {
    fn default() -> Self {
        CostModel::Fixed {
            fee_pct: 0.10, // 0.10% 单边
            slip_pct: 0.05, // 0.05% 单边
        }
    }
}

impl CostModel {
    /// 单边成本百分比（0.15 = 0.15%）
    pub fn one_way_pct(&self) -> f64 {
        match self {
            CostModel::Zero => 0.0,
            CostModel::Fixed { fee_pct, slip_pct } => fee_pct + slip_pct,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BacktestParams {
    /// 预热根数（前 N 根不开仓，让均线等指标稳定）
    pub warmup_bars: usize,
    /// 成本模型
    pub cost_model: CostModel,
}

impl Default for BacktestParams {
    fn default() -> Self {
        Self {
            warmup_bars: 60,
            cost_model: CostModel::default(),
        }
    }
}

// ============================================================
// 体系元数据 & 定义
// ============================================================

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum SystemOrigin {
    /// 种子体系（硬编码，来自原书）
    Seed,
    /// 用户自定义
    User,
    /// 自动发现器产出
    Discovered,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SystemMeta {
    pub created_at_ms: i64,
    pub last_backtested_ms: Option<i64>,
    pub last_backtest_symbol: Option<String>,
    pub last_backtest_interval: Option<String>,
    #[serde(default = "default_schema_version")]
    pub schema_version: u32,
    /// M10：入库时自动跑一次的跨市场 WF 快照（每 (symbol, interval) 一条）
    #[serde(default)]
    pub last_benchmark: Vec<BenchmarkSnapshot>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_benchmark_at_ms: Option<i64>,
}

/// M10：跨市场 WF 快照（嵌入 `SystemMeta`）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkSnapshot {
    pub symbol: String,
    pub interval: String,
    pub wf_consistency: f64,
    pub wf_avg_sharpe: f64,
    pub wf_avg_return_pct: f64,
    pub total_trades: usize,
}

fn default_schema_version() -> u32 {
    1
}

/// 交易体系的不可变描述（序列化/持久化根）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemDefinition {
    pub id: String,
    pub name: String,
    pub origin: SystemOrigin,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    /// 组件 ID 列表（顺序在 `SequentialCascade` 下有意义）
    pub components: Vec<String>,

    pub combine: CombineRule,

    /// 每个组件的权重（仅 `WeightedScore` 使用）
    #[serde(default)]
    pub weights: HashMap<String, f64>,

    pub risk: RiskParams,
    pub backtest: BacktestParams,

    #[serde(default)]
    pub meta: SystemMeta,
}

impl SystemDefinition {
    /// 基础健康检查：组件数不超 MAX_K、非空、聚合规则合法
    pub fn validate(&self) -> Result<(), String> {
        if self.components.is_empty() {
            return Err("components 不能为空".into());
        }
        if self.components.len() > COMPONENT_MAX_K {
            return Err(format!(
                "最多 {} 个组件，当前 {}",
                COMPONENT_MAX_K,
                self.components.len()
            ));
        }
        // 组件必须在注册表中存在
        for cid in &self.components {
            if super::component::find_component(cid).is_none() {
                return Err(format!("未知组件 ID: {}", cid));
            }
        }
        // MajorityK 的 k 合法性
        if let CombineRule::MajorityK { k } = &self.combine {
            if *k == 0 || *k > self.components.len() {
                return Err(format!(
                    "MajorityK.k={} 越界 (组件数 {})",
                    k,
                    self.components.len()
                ));
            }
        }
        if self.risk.stop_atr_mult <= 0.0 || self.risk.target_r <= 0.0 {
            return Err("风控参数必须为正".into());
        }
        if self.risk.max_hold_bars == 0 {
            return Err("max_hold_bars 必须 ≥ 1".into());
        }
        Ok(())
    }
}

// ============================================================
// 交易 & 回测结果
// ============================================================

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum TradeSide {
    Long,
    Short,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum TradeExitReason {
    StopLoss,
    TakeProfit,
    TimeExit,
    /// 被 **断头铡刀铁律** 强制清仓
    GuillotineOverride,
    /// 反向信号平仓
    ReverseSignal,
    /// 数据结束，强制以收盘价结算
    EndOfData,
}

/// 一笔系统产生的交易
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemTrade {
    pub id: usize,
    pub side: TradeSide,
    pub entry_bar: usize,
    pub entry_time_ms: i64,
    pub entry_price: f64,
    pub stop: f64,
    pub target: f64,
    pub exit_bar: usize,
    pub exit_time_ms: i64,
    pub exit_price: f64,
    pub exit_reason: TradeExitReason,
    /// 含成本后的净收益百分比（如 0.015 = +1.5%）
    pub pnl_pct: f64,
    /// R-multiple（以 entry - stop 为单位 R）
    pub r_multiple: f64,
    /// 促成本次开仓的组件 ID 列表（便于归因）
    pub triggered_components: Vec<String>,
    /// 持仓 K 线数
    pub hold_bars: usize,
}

/// 每个组件对交易触发的贡献统计
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComponentContrib {
    pub component_id: String,
    /// 组件在整段数据上被识别到的触发次数
    pub triggers: usize,
    /// 实际促成体系开仓的次数（即：组件触发 AND 聚合规则通过）
    pub matched_system_entries: usize,
}

/// 体系回测完整结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemBacktestResult {
    pub system_id: String,
    pub symbol: String,
    pub interval: String,
    pub bars: usize,
    pub cost_model: CostModel,
    pub performance: Performance,
    pub equity: Vec<EquityPoint>,
    pub trades: Vec<SystemTrade>,
    pub component_contribution: Vec<ComponentContrib>,
}

// ============================================================
// 测试
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn good_def() -> SystemDefinition {
        SystemDefinition {
            id: "test".into(),
            name: "Test".into(),
            origin: SystemOrigin::User,
            description: None,
            components: vec!["ma.granville.b2_pullback".into()],
            combine: CombineRule::AllAligned,
            weights: HashMap::new(),
            risk: RiskParams::default(),
            backtest: BacktestParams::default(),
            meta: SystemMeta::default(),
        }
    }

    #[test]
    fn t_validate_good() {
        assert!(good_def().validate().is_ok());
    }

    #[test]
    fn t_validate_empty_components_rejected() {
        let mut d = good_def();
        d.components.clear();
        assert!(d.validate().is_err());
    }

    #[test]
    fn t_validate_unknown_component_rejected() {
        let mut d = good_def();
        d.components = vec!["unknown.xyz".into()];
        assert!(d.validate().is_err());
    }

    #[test]
    fn t_validate_too_many_components_rejected() {
        let mut d = good_def();
        d.components = vec![
            "ma.granville.b1_breakout".into(),
            "ma.granville.b2_pullback".into(),
            "ma.granville.b3_false_break".into(),
            "ma.granville.s1_breakdown".into(),
            "ma.granville.s2_rebound".into(),
            "candle.morning_star".into(), // 第 6 个超限
        ];
        assert!(d.validate().is_err());
    }

    #[test]
    fn t_validate_majority_k_bounds() {
        let mut d = good_def();
        d.components = vec![
            "ma.granville.b1_breakout".into(),
            "ma.granville.b2_pullback".into(),
        ];
        d.combine = CombineRule::MajorityK { k: 3 };
        assert!(d.validate().is_err());

        d.combine = CombineRule::MajorityK { k: 0 };
        assert!(d.validate().is_err());

        d.combine = CombineRule::MajorityK { k: 2 };
        assert!(d.validate().is_ok());
    }

    #[test]
    fn t_cost_model_one_way() {
        assert_eq!(CostModel::Zero.one_way_pct(), 0.0);
        let m = CostModel::Fixed { fee_pct: 0.10, slip_pct: 0.05 };
        assert!((m.one_way_pct() - 0.15).abs() < 1e-9);
    }

    #[test]
    fn t_cost_model_default() {
        match CostModel::default() {
            CostModel::Fixed { fee_pct, slip_pct } => {
                assert_eq!(fee_pct, 0.10);
                assert_eq!(slip_pct, 0.05);
            }
            _ => panic!("默认应为 Fixed"),
        }
    }
}
