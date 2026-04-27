//! E6：葛南维仓位校验器（E16 修复 / R-P1-13）
//!
//! 原书 **ma p.100** 铁证：
//! > "L4 为均线下行中的超跌反弹，**仓位一定要轻**。"
//!
//! 原书明确对不同葛南维法则设定不同的仓位上限：
//!
//! | 法则 | 原书含义 | 最大仓位 |
//! |---|---|---|
//! | L1（B1 突破）| 均线由降→平 + 价格突破 → 牛市启动 | 100% |
//! | L2（B2 回踩）| 牛市中途回踩未破均线 → 加仓 | 100% |
//! | L3（B3 假跌）| 短暂跌破迅速收回 → 牛市加仓 | 100% |
//! | **L4（B4 乖离买入）** | **均线下行 + 深度负乖离 → 反弹** | **30% 轻仓** |
//! | L5（S1 跌破）| 均线由升→平 + 价格跌破 → 卖出 | 0% |
//! | L6（S2 反弹）| 熊市中反弹未破均线 → 卖出 | 0% |
//! | L7（S3 假涨）| 短暂突破迅速跌回 → 卖出 | 0% |
//! | L8（S4 乖离卖出）| 均线上行 + 深度正乖离 → 回落 | 0%（或轻仓卖出）|
//!
//! # 使用
//!
//! ```
//! use aura_trade::engine::backtest::position_limit::{
//!     PositionLimitChecker, OrderCheckResult,
//! };
//! use aura_trade::engine::ma::GranvilleRule;
//!
//! let checker = PositionLimitChecker::default();
//!
//! // L4 信号：仓位 50% 超限（应为 ≤ 30%）
//! let result = checker.check_order(
//!     Some(GranvilleRule::B4DivergenceBuy),
//!     0.0,  // 当前仓位
//!     0.5,  // 目标仓位
//! );
//! assert!(matches!(result, OrderCheckResult::Rejected { .. }));
//! ```

use serde::{Deserialize, Serialize};

use crate::engine::ma::GranvilleRule;

/// 葛南维各法则对应的仓位上限（占总资金比例）
///
/// 原书 ma p.100 铁证：L4 必须轻仓（≤ 30%），其他买入法则可满仓。
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct PositionLimit {
    /// L1-L3（B1/B2/B3）买入法则仓位上限（牛市可满仓）
    pub bull_buy_max: f64,
    /// **L4（B4）乖离买入仓位上限**（原书警告"一定要轻"）
    pub l4_buy_max: f64,
    /// L5-L8（S1-S4）卖出法则 → 应空仓
    pub sell_max_position: f64,
}

impl Default for PositionLimit {
    fn default() -> Self {
        Self {
            bull_buy_max: 1.00,       // 100% 满仓
            l4_buy_max: 0.30,         // **30% 轻仓**（ma p.100 铁证）
            sell_max_position: 0.00,  // 0% 空仓
        }
    }
}

impl PositionLimit {
    /// 保守配置：所有买入法则都限仓 50%
    pub fn conservative() -> Self {
        Self {
            bull_buy_max: 0.50,
            l4_buy_max: 0.20,
            sell_max_position: 0.00,
        }
    }

    /// 激进配置：除 L4 外都可满仓
    pub fn aggressive() -> Self {
        Self::default()
    }

    /// 返回指定葛南维规则的仓位上限
    pub fn max_for_rule(&self, rule: GranvilleRule) -> f64 {
        use GranvilleRule::*;
        match rule {
            B1BreakoutBuy | B2PullbackBuy | B3FalseBreakBuy => self.bull_buy_max,
            B4DivergenceBuy => self.l4_buy_max,
            S1BreakdownSell | S2ReboundSell | S3FalseBreakSell | S4DivergenceSell => {
                self.sell_max_position
            }
        }
    }
}

/// 订单校验结果
#[derive(Debug, Clone, PartialEq)]
pub enum OrderCheckResult {
    /// 通过（目标仓位在上限内）
    Approved { target_position: f64 },
    /// 拒绝（超上限），附调整建议
    Rejected {
        original_target: f64,
        max_allowed: f64,
        rule: GranvilleRule,
        reason: String,
    },
    /// 无葛南维信号上下文，放行（由其他规则管理）
    NoContext,
}

/// 葛南维仓位校验器
pub struct PositionLimitChecker {
    pub limits: PositionLimit,
}

impl Default for PositionLimitChecker {
    fn default() -> Self {
        Self {
            limits: PositionLimit::default(),
        }
    }
}

impl PositionLimitChecker {
    pub fn new(limits: PositionLimit) -> Self {
        Self { limits }
    }

    /// 校验订单是否合规
    ///
    /// # 参数
    /// - `rule`：当前触发的葛南维规则（若无则返回 `NoContext`）
    /// - `current_position`：当前持仓比例（0-1）
    /// - `target_position`：目标持仓比例（0-1）
    ///
    /// # 返回
    /// - `Approved` — 目标仓位在上限内
    /// - `Rejected` — 超上限，包含建议上限
    /// - `NoContext` — 无葛南维上下文（跳过校验）
    pub fn check_order(
        &self,
        rule: Option<GranvilleRule>,
        current_position: f64,
        target_position: f64,
    ) -> OrderCheckResult {
        let Some(r) = rule else {
            return OrderCheckResult::NoContext;
        };
        let max = self.limits.max_for_rule(r);
        if target_position <= max + 1e-9 {
            OrderCheckResult::Approved {
                target_position,
            }
        } else {
            OrderCheckResult::Rejected {
                original_target: target_position,
                max_allowed: max,
                rule: r,
                reason: format!(
                    "葛南维 {} 规则仓位上限 {:.0}%，目标 {:.0}% 超限（当前 {:.0}%）",
                    r.code(),
                    max * 100.0,
                    target_position * 100.0,
                    current_position * 100.0,
                ),
            }
        }
    }

    /// 返回符合规则的安全目标仓位（不超过上限）
    pub fn clamp_position(&self, rule: Option<GranvilleRule>, target: f64) -> f64 {
        match rule {
            Some(r) => target.min(self.limits.max_for_rule(r)),
            None => target,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn t_l4_max_is_30_percent() {
        // R-P1-13 核心铁证：L4 = 30%
        let limits = PositionLimit::default();
        assert_eq!(limits.l4_buy_max, 0.30);
        assert_eq!(limits.bull_buy_max, 1.00);
        assert_eq!(limits.sell_max_position, 0.00);
    }

    #[test]
    fn t_l4_order_rejected_if_over_30pct() {
        // L4 信号 + 目标 50% → 拒绝
        let checker = PositionLimitChecker::default();
        let result = checker.check_order(Some(GranvilleRule::B4DivergenceBuy), 0.0, 0.5);
        match result {
            OrderCheckResult::Rejected {
                original_target,
                max_allowed,
                rule,
                ..
            } => {
                assert_eq!(original_target, 0.5);
                assert_eq!(max_allowed, 0.30);
                assert_eq!(rule, GranvilleRule::B4DivergenceBuy);
            }
            _ => panic!("应拒绝 L4 50% 订单"),
        }
    }

    #[test]
    fn t_l4_order_approved_if_under_30pct() {
        // L4 信号 + 目标 30% → 通过（边界）
        let checker = PositionLimitChecker::default();
        let result = checker.check_order(Some(GranvilleRule::B4DivergenceBuy), 0.0, 0.30);
        assert!(matches!(
            result,
            OrderCheckResult::Approved { target_position } if (target_position - 0.30).abs() < 1e-9
        ));
    }

    #[test]
    fn t_l1_l2_l3_can_full_position() {
        // L1/L2/L3 允许满仓
        let checker = PositionLimitChecker::default();
        for rule in [
            GranvilleRule::B1BreakoutBuy,
            GranvilleRule::B2PullbackBuy,
            GranvilleRule::B3FalseBreakBuy,
        ] {
            let result = checker.check_order(Some(rule), 0.0, 1.0);
            assert!(
                matches!(result, OrderCheckResult::Approved { .. }),
                "{:?} 应允许满仓",
                rule
            );
        }
    }

    #[test]
    fn t_sell_rules_zero_position() {
        // 所有卖出规则 → 必须 0% 仓位
        let checker = PositionLimitChecker::default();
        for rule in [
            GranvilleRule::S1BreakdownSell,
            GranvilleRule::S2ReboundSell,
            GranvilleRule::S3FalseBreakSell,
            GranvilleRule::S4DivergenceSell,
        ] {
            // 0% 通过
            let ok = checker.check_order(Some(rule), 0.5, 0.0);
            assert!(matches!(ok, OrderCheckResult::Approved { .. }));
            // 任何 > 0% 都拒绝
            let rejected = checker.check_order(Some(rule), 0.5, 0.01);
            assert!(matches!(rejected, OrderCheckResult::Rejected { .. }));
        }
    }

    #[test]
    fn t_no_rule_returns_no_context() {
        let checker = PositionLimitChecker::default();
        let result = checker.check_order(None, 0.0, 1.0);
        assert_eq!(result, OrderCheckResult::NoContext);
    }

    #[test]
    fn t_clamp_position_enforces_limit() {
        let checker = PositionLimitChecker::default();
        // L4 + 50% 目标 → 被 clamp 到 30%
        let clamped = checker.clamp_position(Some(GranvilleRule::B4DivergenceBuy), 0.5);
        assert_eq!(clamped, 0.30);
        // L1 + 50% 目标 → 不变
        let no_clamp = checker.clamp_position(Some(GranvilleRule::B1BreakoutBuy), 0.5);
        assert_eq!(no_clamp, 0.5);
        // 无规则 → 不变
        let no_rule = checker.clamp_position(None, 0.8);
        assert_eq!(no_rule, 0.8);
    }

    #[test]
    fn t_conservative_and_aggressive_presets() {
        let cons = PositionLimit::conservative();
        assert!(cons.bull_buy_max < PositionLimit::default().bull_buy_max);
        assert!(cons.l4_buy_max < PositionLimit::default().l4_buy_max);

        let agg = PositionLimit::aggressive();
        assert_eq!(agg.bull_buy_max, 1.00);
        assert_eq!(agg.l4_buy_max, 0.30); // 即使激进也不违反 L4 铁证
    }
}
