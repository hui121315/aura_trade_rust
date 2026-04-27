//! E7：回测策略 PRD 模板（R-P1-12，Sprint 10）
//!
//! 把原书的操盘策略（"依样画葫芦"）**工程化为可回测的模板**。
//!
//! # 原书铁证（三书封底）
//!
//! > "**可模仿性** —— 大部分都指明了进场、离场的位置和区域，
//! > 交易者完全可以**依样画葫芦**地进行模仿操作。"
//!
//! 本模块把这些"可模仿"的具体操作转为 [`Playbook`] trait 接口，
//! 供外部回测引擎统一调用。
//!
//! # 内置模板
//!
//! - [`TrendMatrixPlaybook`]（R-P1-15 trend p.216 10 条买卖矩阵）
//! - [`GuillotineExitPlaybook`]（R-P1-53 ma p.380 断头铡刀清仓）
//! - [`StagedExitPlaybook`]（R-P1-42 candle p.605 三次减仓）
//! - [`HangingScallionsEntryPlaybook`]（R-P1-50 ma p.340 旱地拔葱轻仓入场）

use serde::{Deserialize, Serialize};

use crate::data::Kline;
use crate::engine::backtest::position_limit::PositionLimit;
use crate::engine::ma::MaAdvancedKind;
use crate::engine::signal::staged_exit::{StagedExitPlanner, ToppingSignalSeverity};

/// 策略决策
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum PlaybookDecision {
    /// 建仓 / 加仓至 target（0.0 - 1.0）
    Buy {
        target_position: f64,
        reason: String,
    },
    /// 减仓至 target（0.0 - 1.0）
    Sell {
        target_position: f64,
        reason: String,
    },
    /// 持有不动
    Hold,
    /// 空仓观望
    StayOut { reason: String },
}

impl PlaybookDecision {
    pub fn label(&self) -> &'static str {
        match self {
            PlaybookDecision::Buy { .. } => "买入/加仓",
            PlaybookDecision::Sell { .. } => "减仓/清仓",
            PlaybookDecision::Hold => "持有",
            PlaybookDecision::StayOut { .. } => "空仓",
        }
    }

    /// 目标仓位（Hold/StayOut 返回当前仓位占位 0.0）
    pub fn target_position(&self) -> f64 {
        match self {
            PlaybookDecision::Buy { target_position, .. } => *target_position,
            PlaybookDecision::Sell { target_position, .. } => *target_position,
            _ => 0.0,
        }
    }
}

/// 策略上下文（供 Playbook 访问）
pub struct PlaybookContext<'a> {
    pub klines: &'a [Kline],
    pub index: usize,
    pub current_position: f64,
    /// 可选：当前已识别的 ma 高级形态
    pub ma_advanced_kind: Option<MaAdvancedKind>,
    /// 可选：见顶信号严重度（用于 StagedExitPlaybook）
    pub topping_severity: Option<ToppingSignalSeverity>,
    /// 可选：长期趋势方向（+1/-1/0，用于 TrendMatrixPlaybook）
    pub long_trend: i8,
}

/// 策略模板 trait
pub trait Playbook {
    /// 策略名称
    fn name(&self) -> &'static str;
    /// 原书出处
    fn book_source(&self) -> &'static str;
    /// 在上下文中作出决策
    fn decide(&mut self, ctx: &PlaybookContext<'_>) -> PlaybookDecision;
}

// ==================== R-P1-53 断头铡刀清仓策略 ====================

/// **断头铡刀 = 清仓**（ma p.380）
///
/// 原书警语：
/// > "只有清仓才是解脱的好办法。"
pub struct GuillotineExitPlaybook;

impl Playbook for GuillotineExitPlaybook {
    fn name(&self) -> &'static str {
        "断头铡刀清仓"
    }
    fn book_source(&self) -> &'static str {
        "ma p.380"
    }
    fn decide(&mut self, ctx: &PlaybookContext<'_>) -> PlaybookDecision {
        if matches!(ctx.ma_advanced_kind, Some(MaAdvancedKind::Guillotine)) {
            return PlaybookDecision::Sell {
                target_position: 0.0,
                reason: "断头铡刀触发 → 清仓（ma p.380）".to_string(),
            };
        }
        PlaybookDecision::Hold
    }
}

// ==================== R-P1-50 旱地拔葱轻仓入场 ====================

/// **旱地拔葱 = 轻仓入场**（ma p.340）
///
/// 原书：下降楔形末期的旱地拔葱是最早期的看涨信号，但此时整体趋势尚未转多，
/// 所以入场仓位应"轻"（默认 30%，与 L4 仓位上限一致）。
pub struct HangingScallionsEntryPlaybook;

impl Playbook for HangingScallionsEntryPlaybook {
    fn name(&self) -> &'static str {
        "旱地拔葱轻仓入场"
    }
    fn book_source(&self) -> &'static str {
        "ma p.340"
    }
    fn decide(&mut self, ctx: &PlaybookContext<'_>) -> PlaybookDecision {
        if matches!(ctx.ma_advanced_kind, Some(MaAdvancedKind::HangingScallions)) {
            // 与 L4 仓位一致：最多 30%
            return PlaybookDecision::Buy {
                target_position: PositionLimit::default().l4_buy_max,
                reason: "旱地拔葱（最早期看涨）→ 轻仓入场 30%（ma p.340）"
                    .to_string(),
            };
        }
        PlaybookDecision::Hold
    }
}

// ==================== R-P1-42 三次减仓策略 ====================

/// **倒 V 三次减仓**（candle p.605）
///
/// 原书：顶部出现 3 次短线见顶信号时，分 3 段减仓（30%/50%/100%）
pub struct StagedExitPlaybook {
    planner: StagedExitPlanner,
}

impl StagedExitPlaybook {
    pub fn new() -> Self {
        Self {
            planner: StagedExitPlanner::default(),
        }
    }
}

impl Default for StagedExitPlaybook {
    fn default() -> Self {
        Self::new()
    }
}

impl Playbook for StagedExitPlaybook {
    fn name(&self) -> &'static str {
        "三次减仓"
    }
    fn book_source(&self) -> &'static str {
        "candle p.605"
    }
    fn decide(&mut self, ctx: &PlaybookContext<'_>) -> PlaybookDecision {
        if let Some(sev) = ctx.topping_severity {
            if let Some(ev) = self.planner.on_topping_signal(
                ctx.index,
                sev,
                format!("{:?}", sev),
            ) {
                let target = self.planner.current_position_fraction();
                return PlaybookDecision::Sell {
                    target_position: target,
                    reason: format!(
                        "三次减仓第 {} 次（{}）→ 减至 {:.0}%（candle p.605）",
                        self.planner.triggered_count(),
                        ev.reason,
                        target * 100.0,
                    ),
                };
            }
        }
        PlaybookDecision::Hold
    }
}

// ==================== R-P1-15 多级趋势线策略矩阵 ====================

/// **多级趋势线矩阵**（trend p.216）
///
/// 简化实现：
/// - 长期下降趋势 → 空仓
/// - 长期上升趋势 + 向上突破 → 买入
/// - 长期上升趋势 + 跌破长期线 → 清仓
pub struct TrendMatrixPlaybook;

impl Playbook for TrendMatrixPlaybook {
    fn name(&self) -> &'static str {
        "多级趋势线矩阵"
    }
    fn book_source(&self) -> &'static str {
        "trend p.216"
    }
    fn decide(&mut self, ctx: &PlaybookContext<'_>) -> PlaybookDecision {
        // SELL-5：长期下降 → 空仓
        if ctx.long_trend < 0 {
            return PlaybookDecision::StayOut {
                reason: "SELL-5 长期下降 → 非牛市空仓（trend p.225）".to_string(),
            };
        }
        // 长期上升趋势下，看 ma 高级信号决策
        if ctx.long_trend > 0 {
            if let Some(kind) = ctx.ma_advanced_kind {
                match kind {
                    MaAdvancedKind::Guillotine => {
                        return PlaybookDecision::Sell {
                            target_position: 0.0,
                            reason: "SELL-1 跌破长期上升 → 清仓（trend p.221）".to_string(),
                        };
                    }
                    MaAdvancedKind::BondUpwardDiverge => {
                        return PlaybookDecision::Buy {
                            target_position: 1.0,
                            reason: "BUY-2 长期上升 + 再次粘合向上 → 满仓（第三浪主升）"
                                .to_string(),
                        };
                    }
                    MaAdvancedKind::HangingScallions => {
                        return PlaybookDecision::Buy {
                            target_position: 0.3,
                            reason: "BUY-3 长期上升 + 旱地拔葱 → 轻仓".to_string(),
                        };
                    }
                    _ => {}
                }
            }
        }
        PlaybookDecision::Hold
    }
}

// ==================== 组合策略（多个 Playbook 联动）====================

/// 多 Playbook 组合：按优先级路由，取第一个非 Hold 决策
pub struct CompositePlaybook {
    playbooks: Vec<Box<dyn Playbook>>,
}

impl CompositePlaybook {
    pub fn new() -> Self {
        Self {
            playbooks: Vec::new(),
        }
    }

    /// 添加策略（顺序 = 优先级，前者优先）
    pub fn with(mut self, pb: Box<dyn Playbook>) -> Self {
        self.playbooks.push(pb);
        self
    }

    /// 默认组合：断头铡刀清仓 > 三次减仓 > 趋势矩阵 > 旱地拔葱
    pub fn default_combo() -> Self {
        Self::new()
            .with(Box::new(GuillotineExitPlaybook))
            .with(Box::new(StagedExitPlaybook::new()))
            .with(Box::new(TrendMatrixPlaybook))
            .with(Box::new(HangingScallionsEntryPlaybook))
    }
}

impl Default for CompositePlaybook {
    fn default() -> Self {
        Self::default_combo()
    }
}

impl Playbook for CompositePlaybook {
    fn name(&self) -> &'static str {
        "组合策略"
    }
    fn book_source(&self) -> &'static str {
        "三书综合"
    }
    fn decide(&mut self, ctx: &PlaybookContext<'_>) -> PlaybookDecision {
        for pb in &mut self.playbooks {
            let decision = pb.decide(ctx);
            if !matches!(decision, PlaybookDecision::Hold) {
                return decision;
            }
        }
        PlaybookDecision::Hold
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mk_ctx(advanced: Option<MaAdvancedKind>, trend: i8, sev: Option<ToppingSignalSeverity>) -> PlaybookContext<'static> {
        // 使用 Box::leak 构造静态生命周期的 slice，仅用于测试
        let klines: &'static [Kline] = Box::leak(Box::new([]));
        PlaybookContext {
            klines,
            index: 100,
            current_position: 0.5,
            ma_advanced_kind: advanced,
            topping_severity: sev,
            long_trend: trend,
        }
    }

    #[test]
    fn t_guillotine_exit_triggers_sell() {
        let mut pb = GuillotineExitPlaybook;
        let ctx = mk_ctx(Some(MaAdvancedKind::Guillotine), 1, None);
        let d = pb.decide(&ctx);
        match d {
            PlaybookDecision::Sell { target_position, .. } => {
                assert_eq!(target_position, 0.0);
            }
            _ => panic!("应触发 Sell"),
        }
    }

    #[test]
    fn t_guillotine_no_signal_holds() {
        let mut pb = GuillotineExitPlaybook;
        let ctx = mk_ctx(None, 1, None);
        assert_eq!(pb.decide(&ctx), PlaybookDecision::Hold);
    }

    #[test]
    fn t_hanging_scallions_light_position() {
        let mut pb = HangingScallionsEntryPlaybook;
        let ctx = mk_ctx(Some(MaAdvancedKind::HangingScallions), 1, None);
        let d = pb.decide(&ctx);
        match d {
            PlaybookDecision::Buy { target_position, .. } => {
                assert!((target_position - 0.30).abs() < 1e-9, "应为 30% 轻仓");
            }
            _ => panic!("应触发 Buy"),
        }
    }

    #[test]
    fn t_staged_exit_progresses_through_stages() {
        let mut pb = StagedExitPlaybook::new();
        // 第 1 次 Early → 30%
        let ctx = mk_ctx(None, 1, Some(ToppingSignalSeverity::Early));
        let d = pb.decide(&ctx);
        match d {
            PlaybookDecision::Sell { target_position, .. } => {
                // 剩余 70%
                assert!((target_position - 0.70).abs() < 1e-9);
            }
            _ => panic!("第 1 次应触发 Sell"),
        }
        // 第 2 次 Early → 50%（累计）
        let d2 = pb.decide(&ctx);
        match d2 {
            PlaybookDecision::Sell { target_position, .. } => {
                assert!((target_position - 0.50).abs() < 1e-9);
            }
            _ => panic!("第 2 次应触发 Sell"),
        }
        // 第 3 次 Severe → 清仓
        let ctx3 = mk_ctx(None, 1, Some(ToppingSignalSeverity::Severe));
        let d3 = pb.decide(&ctx3);
        match d3 {
            PlaybookDecision::Sell { target_position, .. } => {
                assert_eq!(target_position, 0.0);
            }
            _ => panic!("第 3 次应清仓"),
        }
    }

    #[test]
    fn t_trend_matrix_long_down_stay_out() {
        let mut pb = TrendMatrixPlaybook;
        let ctx = mk_ctx(None, -1, None);
        let d = pb.decide(&ctx);
        assert!(matches!(d, PlaybookDecision::StayOut { .. }));
    }

    #[test]
    fn t_trend_matrix_long_up_bond_diverge_full() {
        let mut pb = TrendMatrixPlaybook;
        let ctx = mk_ctx(Some(MaAdvancedKind::BondUpwardDiverge), 1, None);
        let d = pb.decide(&ctx);
        match d {
            PlaybookDecision::Buy { target_position, .. } => {
                assert_eq!(target_position, 1.0, "主升浪满仓");
            }
            _ => panic!("应满仓买入"),
        }
    }

    #[test]
    fn t_trend_matrix_long_up_guillotine_full_exit() {
        let mut pb = TrendMatrixPlaybook;
        let ctx = mk_ctx(Some(MaAdvancedKind::Guillotine), 1, None);
        let d = pb.decide(&ctx);
        match d {
            PlaybookDecision::Sell { target_position, .. } => {
                assert_eq!(target_position, 0.0);
            }
            _ => panic!("应清仓"),
        }
    }

    #[test]
    fn t_composite_priority_guillotine_over_scallions() {
        // 同时触发 Guillotine + HangingScallions（不可能但测试优先级）
        // 由于上下文只支持一个 advanced 字段，测试两个独立场景

        // 仅断头铡刀
        let mut comp = CompositePlaybook::default_combo();
        let ctx = mk_ctx(Some(MaAdvancedKind::Guillotine), 1, None);
        let d = comp.decide(&ctx);
        assert!(matches!(d, PlaybookDecision::Sell { .. }));

        // 仅旱地拔葱
        let mut comp2 = CompositePlaybook::default_combo();
        let ctx2 = mk_ctx(Some(MaAdvancedKind::HangingScallions), 1, None);
        let d2 = comp2.decide(&ctx2);
        assert!(matches!(d2, PlaybookDecision::Buy { .. }));
    }

    #[test]
    fn t_composite_all_hold_returns_hold() {
        let mut comp = CompositePlaybook::default_combo();
        let ctx = mk_ctx(None, 1, None);
        assert_eq!(comp.decide(&ctx), PlaybookDecision::Hold);
    }

    #[test]
    fn t_playbook_labels_and_book_sources() {
        let pb1 = GuillotineExitPlaybook;
        assert_eq!(pb1.name(), "断头铡刀清仓");
        assert_eq!(pb1.book_source(), "ma p.380");

        let pb2 = HangingScallionsEntryPlaybook;
        assert_eq!(pb2.name(), "旱地拔葱轻仓入场");
        assert_eq!(pb2.book_source(), "ma p.340");

        let pb3 = StagedExitPlaybook::new();
        assert_eq!(pb3.name(), "三次减仓");

        let pb4 = TrendMatrixPlaybook;
        assert_eq!(pb4.book_source(), "trend p.216");
    }

    #[test]
    fn t_decision_target_position() {
        let buy = PlaybookDecision::Buy {
            target_position: 0.5,
            reason: "test".to_string(),
        };
        assert_eq!(buy.target_position(), 0.5);

        let sell = PlaybookDecision::Sell {
            target_position: 0.0,
            reason: "test".to_string(),
        };
        assert_eq!(sell.target_position(), 0.0);

        assert_eq!(PlaybookDecision::Hold.target_position(), 0.0);
    }
}
