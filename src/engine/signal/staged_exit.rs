//! F4：分级减仓策略（R-P1-42 / R-P1-32，保本哲学的工程实现）
//!
//! # 原书铁证
//!
//! - **candle p.605**（倒置 V 形反转）武钢股份（600005）案例：
//!   > "K 线形态出现三次短线交易减仓信号。顶部收出下跌三连阴 K 线形态时，应果断离场。"
//!   > "减仓之后股价继续上涨，导致交易者部分资金踏空，但**这是保障资金安全必须付出的代价**。"
//!
//! - **candle p.540** 顶部多 K 线（吊颈线 + 黄昏之星 + 阴十字 + 长十字 + 连续跳空小阴）
//!   → **逐渐减仓直至清仓**
//!
//! # 工程实现
//!
//! [`StagedExitPlanner`] 跟踪顶部见顶信号序列，按原书 30% / 50% / 100% 三段式减仓。
//! 对应不变量 2.3（分级减仓/保本哲学）。

use serde::{Deserialize, Serialize};

/// 见顶信号的严重度（用于分级减仓）
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ToppingSignalSeverity {
    /// 初级见顶（如单个镊子顶、长十字）→ 减 30%
    Early,
    /// 中级见顶（吊颈线、小阴十字）→ 减 50%
    Intermediate,
    /// 严重见顶（下跌三连阴、黄昏星完成、倾盆大雨）→ 清仓
    Severe,
}

impl ToppingSignalSeverity {
    pub fn label(&self) -> &'static str {
        match self {
            ToppingSignalSeverity::Early => "初级见顶",
            ToppingSignalSeverity::Intermediate => "中级见顶",
            ToppingSignalSeverity::Severe => "严重见顶",
        }
    }
}

/// 减仓事件
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExitEvent {
    /// 见顶信号索引
    pub index: usize,
    /// 本次累计已减仓比例（0.0 - 1.0）
    pub cumulative_exit_fraction: f64,
    /// 本次减仓比例（相对建仓时的满仓）
    pub this_step_fraction: f64,
    /// 见顶严重度
    pub severity: ToppingSignalSeverity,
    /// 说明
    pub reason: String,
}

/// 参数
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct StagedExitParams {
    /// 第一次见顶信号减仓比例（原书 30%）
    pub first_exit_fraction: f64,
    /// 第二次见顶信号累计减仓至（原书 50%）
    pub second_cumulative_exit: f64,
    /// 严重见顶直接清仓至（100%）
    pub full_exit_cumulative: f64,
}

impl Default for StagedExitParams {
    fn default() -> Self {
        Self {
            first_exit_fraction: 0.30,   // 原书 p.605 三次减仓第一次
            second_cumulative_exit: 0.50,
            full_exit_cumulative: 1.00,
        }
    }
}

/// 分级减仓规划器
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StagedExitPlanner {
    params: StagedExitParams,
    /// 当前已累计减仓比例
    cumulative_exited: f64,
    /// 已触发的见顶信号数
    triggered_count: usize,
    /// 减仓历史
    history: Vec<ExitEvent>,
}

impl Default for StagedExitPlanner {
    fn default() -> Self {
        Self::new(StagedExitParams::default())
    }
}

impl StagedExitPlanner {
    pub fn new(params: StagedExitParams) -> Self {
        Self {
            params,
            cumulative_exited: 0.0,
            triggered_count: 0,
            history: Vec::new(),
        }
    }

    /// 注册一个见顶信号，返回本次减仓事件（若已清仓或信号不足则返回 None）
    pub fn on_topping_signal(
        &mut self,
        index: usize,
        severity: ToppingSignalSeverity,
        reason: impl Into<String>,
    ) -> Option<ExitEvent> {
        // 已清仓则不再处理
        if self.cumulative_exited >= self.params.full_exit_cumulative - 1e-9 {
            return None;
        }

        self.triggered_count += 1;

        // 根据严重度 + 第几次信号决定目标累计减仓
        let target_cumulative = match severity {
            ToppingSignalSeverity::Severe => self.params.full_exit_cumulative,
            ToppingSignalSeverity::Intermediate => {
                self.params.second_cumulative_exit.max(self.cumulative_exited)
            }
            ToppingSignalSeverity::Early => {
                if self.triggered_count == 1 {
                    self.params.first_exit_fraction
                } else if self.triggered_count == 2 {
                    self.params.second_cumulative_exit
                } else {
                    self.params.full_exit_cumulative
                }
            }
        };

        let step = (target_cumulative - self.cumulative_exited).max(0.0);
        if step < 1e-9 {
            return None;
        }
        self.cumulative_exited = target_cumulative;

        let ev = ExitEvent {
            index,
            cumulative_exit_fraction: self.cumulative_exited,
            this_step_fraction: step,
            severity,
            reason: reason.into(),
        };
        self.history.push(ev.clone());
        Some(ev)
    }

    /// 当前仍持仓比例（1.0 = 满仓，0.0 = 清仓）
    pub fn current_position_fraction(&self) -> f64 {
        (1.0 - self.cumulative_exited).max(0.0)
    }

    /// 是否已清仓
    pub fn is_fully_exited(&self) -> bool {
        self.cumulative_exited >= self.params.full_exit_cumulative - 1e-9
    }

    pub fn triggered_count(&self) -> usize {
        self.triggered_count
    }

    pub fn history(&self) -> &[ExitEvent] {
        &self.history
    }

    /// 重置（重新建仓后调用）
    pub fn reset(&mut self) {
        self.cumulative_exited = 0.0;
        self.triggered_count = 0;
        self.history.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn t_three_stage_exit_30_50_100() {
        // 原书铁证：三次减仓 30% / 50% / 100%
        let mut planner = StagedExitPlanner::default();
        // 第 1 次：镊子顶 → 减 30%
        let e1 = planner
            .on_topping_signal(10, ToppingSignalSeverity::Early, "镊子顶")
            .unwrap();
        assert_eq!(e1.cumulative_exit_fraction, 0.30);
        assert_eq!(planner.current_position_fraction(), 0.70);

        // 第 2 次：吊颈线 → 累计减至 50%
        let e2 = planner
            .on_topping_signal(15, ToppingSignalSeverity::Early, "吊颈线")
            .unwrap();
        assert_eq!(e2.cumulative_exit_fraction, 0.50);
        assert_eq!(planner.current_position_fraction(), 0.50);

        // 第 3 次：下跌三连阴 → 清仓
        let e3 = planner
            .on_topping_signal(20, ToppingSignalSeverity::Severe, "下跌三连阴")
            .unwrap();
        assert_eq!(e3.cumulative_exit_fraction, 1.00);
        assert!(planner.is_fully_exited());
        assert_eq!(planner.current_position_fraction(), 0.0);
    }

    #[test]
    fn t_severe_signal_direct_fully_exit() {
        // 单次严重信号（如断头铡刀）→ 直接清仓
        let mut planner = StagedExitPlanner::default();
        let e = planner
            .on_topping_signal(5, ToppingSignalSeverity::Severe, "断头铡刀")
            .unwrap();
        assert_eq!(e.cumulative_exit_fraction, 1.00);
        assert_eq!(e.this_step_fraction, 1.00);
        assert!(planner.is_fully_exited());
    }

    #[test]
    fn t_no_exit_after_fully_exited() {
        // 已清仓后新信号 → None
        let mut planner = StagedExitPlanner::default();
        planner.on_topping_signal(5, ToppingSignalSeverity::Severe, "初始清仓");
        let e = planner.on_topping_signal(10, ToppingSignalSeverity::Early, "晚来一步");
        assert!(e.is_none());
    }

    #[test]
    fn t_intermediate_skips_to_50_percent() {
        // 中级信号直接跳到 50%
        let mut planner = StagedExitPlanner::default();
        let e = planner
            .on_topping_signal(5, ToppingSignalSeverity::Intermediate, "黄昏十字")
            .unwrap();
        assert_eq!(e.cumulative_exit_fraction, 0.50);
        assert_eq!(e.this_step_fraction, 0.50);
    }

    #[test]
    fn t_reset_resets_state() {
        let mut planner = StagedExitPlanner::default();
        planner.on_topping_signal(5, ToppingSignalSeverity::Severe, "x");
        planner.reset();
        assert_eq!(planner.triggered_count(), 0);
        assert_eq!(planner.current_position_fraction(), 1.0);
        // 可以再次减仓
        let e = planner.on_topping_signal(10, ToppingSignalSeverity::Early, "重新减仓");
        assert!(e.is_some());
    }

    #[test]
    fn t_history_records_each_step() {
        let mut planner = StagedExitPlanner::default();
        planner.on_topping_signal(5, ToppingSignalSeverity::Early, "s1");
        planner.on_topping_signal(10, ToppingSignalSeverity::Early, "s2");
        planner.on_topping_signal(15, ToppingSignalSeverity::Severe, "s3");
        assert_eq!(planner.history().len(), 3);
        assert_eq!(planner.history()[0].reason, "s1");
        assert_eq!(planner.history()[2].severity, ToppingSignalSeverity::Severe);
    }

    #[test]
    fn t_step_fraction_reflects_delta() {
        // 每次 this_step_fraction = cumulative 的增量
        let mut planner = StagedExitPlanner::default();
        let e1 = planner
            .on_topping_signal(5, ToppingSignalSeverity::Early, "s1")
            .unwrap();
        assert_eq!(e1.this_step_fraction, 0.30);
        let e2 = planner
            .on_topping_signal(10, ToppingSignalSeverity::Early, "s2")
            .unwrap();
        assert_eq!(e2.this_step_fraction, 0.20); // 50% - 30% = 20%
        let e3 = planner
            .on_topping_signal(15, ToppingSignalSeverity::Severe, "s3")
            .unwrap();
        assert_eq!(e3.this_step_fraction, 0.50); // 100% - 50% = 50%
    }
}
