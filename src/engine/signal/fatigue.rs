//! F2：信号衰减框架（R-P1-52，反过度交易原则）
//!
//! # 原书铁证（ma p.360 完整原文）
//!
//! > "在长期下降趋势中，经常多次出现均线复合死亡走势。对交易而言，
//! > **最具有实战意义的只有前期的一两次**，越靠后的均线复合死亡
//! > 离底部越近，技术信号就越不可靠。"
//!
//! > "对于趋势交易者而言，下降趋势中的原则是空仓。如果严格执行纪律，
//! > 应当在**第一次或第二次发出卖出信号时就已空仓**，即使后面发出
//! > 十次、二十次卖出信号，其实都没有太大的意义。"
//!
//! # 工程实现
//!
//! - `SignalFatigue` 跟踪每种信号的连续出现次数
//! - `weight_decay(kind)` 返回 `0.5^(n-1)` 衰减因子（n = 当前次数）
//! - `register(kind)` 记录一次信号（计数 +1）
//! - `reset(kind)` 反向信号出现时重置
//!
//! # 衰减表
//!
//! | 第 n 次 | 衰减因子 |
//! |---|---|
//! | 1 | 1.0（完整权重）|
//! | 2 | 0.5（半数）|
//! | 3 | 0.25 |
//! | 4 | 0.125 |
//! | 5+ | ≤ 0.0625（几乎无效）|
//!
//! # 特别注意：断头铡刀反常规律
//!
//! 原书 ma p.310 反常铁证：**断头铡刀的第二次比第一次更凶狠**
//! （因 60 日均线已转熊）。这是**唯一例外**，应用时需在 `SignalKind` 内
//! 专门处理（见 `SignalKind::Guillotine` 的 `is_anti_fatigue()` 标志）。

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// 可被衰减跟踪的信号类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SignalKind {
    // --- 买入信号 ---
    /// 葛南维 B1-B4
    GranvilleBuy,
    /// 黄金交叉
    GoldenCross,
    /// 旱地拔葱
    HangingScallions,
    /// 再次粘合向上发散
    BondUpwardDiverge,

    // --- 卖出信号 ---
    /// 葛南维 S1-S4
    GranvilleSell,
    /// 死亡交叉
    DeathCross,
    /// 毒蜘蛛 / 首次交叉向下发散
    PoissonSpider,
    /// 断头铡刀（**反常：第二次更凶，见 is_anti_fatigue**）
    Guillotine,
    /// 均线复合死亡
    CompoundDeath,

    // --- 其他 ---
    /// 多头陷阱
    BullTrap,
    /// 空头陷阱
    BearTrap,
}

impl SignalKind {
    /// 是否为反疲劳信号（第 n 次权重**不应**递减）
    ///
    /// 原书 ma p.310 铁证：**断头铡刀的第二次比第一次更凶狠**
    pub fn is_anti_fatigue(&self) -> bool {
        matches!(self, SignalKind::Guillotine)
    }

    /// 是否为买入信号
    pub fn is_buy(&self) -> bool {
        matches!(
            self,
            SignalKind::GranvilleBuy
                | SignalKind::GoldenCross
                | SignalKind::HangingScallions
                | SignalKind::BondUpwardDiverge
                | SignalKind::BearTrap
        )
    }

    /// 是否为卖出信号
    pub fn is_sell(&self) -> bool {
        matches!(
            self,
            SignalKind::GranvilleSell
                | SignalKind::DeathCross
                | SignalKind::PoissonSpider
                | SignalKind::Guillotine
                | SignalKind::CompoundDeath
                | SignalKind::BullTrap
        )
    }

    /// 反向信号（用于 reset 时判断是否应重置）
    pub fn opposite_direction(&self) -> bool {
        // 简化：对于任何信号，另一方向的任何信号都应 reset 它
        // 更精细的实现可以查找对应 opposite kind
        false
    }
}

/// 信号衰减跟踪器
///
/// 持有一个 `HashMap<SignalKind, usize>` 记录每种信号的**连续出现次数**。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SignalFatigue {
    counts: HashMap<SignalKind, usize>,
}

impl SignalFatigue {
    pub fn new() -> Self {
        Self::default()
    }

    /// 注册一次信号（计数 +1）
    ///
    /// 返回注册后的总次数
    pub fn register(&mut self, kind: SignalKind) -> usize {
        let counter = self.counts.entry(kind).or_insert(0);
        *counter += 1;
        *counter
    }

    /// 重置某类信号的计数（用于方向反转等场景）
    pub fn reset(&mut self, kind: SignalKind) {
        self.counts.remove(&kind);
    }

    /// 重置所有反向信号（买入信号出现 → 重置所有卖出衰减，反之亦然）
    pub fn reset_opposite(&mut self, kind: SignalKind) {
        let sells = [
            SignalKind::GranvilleSell,
            SignalKind::DeathCross,
            SignalKind::PoissonSpider,
            SignalKind::Guillotine,
            SignalKind::CompoundDeath,
            SignalKind::BullTrap,
        ];
        let buys = [
            SignalKind::GranvilleBuy,
            SignalKind::GoldenCross,
            SignalKind::HangingScallions,
            SignalKind::BondUpwardDiverge,
            SignalKind::BearTrap,
        ];
        if kind.is_buy() {
            for s in sells {
                self.counts.remove(&s);
            }
        } else if kind.is_sell() {
            for s in buys {
                self.counts.remove(&s);
            }
        }
    }

    /// 获取某类信号当前计数
    pub fn count(&self, kind: SignalKind) -> usize {
        self.counts.get(&kind).copied().unwrap_or(0)
    }

    /// 计算当前信号的**衰减权重因子**（不含反疲劳信号的例外）
    ///
    /// - 第 1 次 → 1.0
    /// - 第 2 次 → 0.5
    /// - 第 n 次 → 0.5^(n-1)
    /// - **反疲劳信号**（断头铡刀）→ 永远 1.0，不衰减
    pub fn weight_decay(&self, kind: SignalKind) -> f64 {
        let n = self.count(kind);
        if n == 0 {
            return 1.0; // 尚未注册，按首次处理
        }
        if kind.is_anti_fatigue() {
            return 1.0;
        }
        let exp = (n.saturating_sub(1)) as i32;
        0.5f64.powi(exp)
    }

    /// 注册信号并返回**本次应用的权重**（=register + decay 的组合）
    ///
    /// # 逻辑
    /// 1. 先 register（计数 +1）
    /// 2. 返回此时的 `weight_decay(kind)`
    ///
    /// 即：第 1 次 register 后返回 1.0；第 2 次返回 0.5；...
    pub fn register_and_get_weight(&mut self, kind: SignalKind) -> f64 {
        self.register(kind);
        self.weight_decay(kind)
    }

    /// 清空所有计数
    pub fn clear(&mut self) {
        self.counts.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn t_first_signal_full_weight() {
        let mut fatigue = SignalFatigue::new();
        let weight = fatigue.register_and_get_weight(SignalKind::DeathCross);
        assert_eq!(weight, 1.0, "第 1 次应为完整权重");
    }

    #[test]
    fn t_decay_follows_power_of_half() {
        let mut fatigue = SignalFatigue::new();
        assert_eq!(
            fatigue.register_and_get_weight(SignalKind::DeathCross),
            1.0
        );
        assert_eq!(
            fatigue.register_and_get_weight(SignalKind::DeathCross),
            0.5
        );
        assert_eq!(
            fatigue.register_and_get_weight(SignalKind::DeathCross),
            0.25
        );
        assert_eq!(
            fatigue.register_and_get_weight(SignalKind::DeathCross),
            0.125
        );
    }

    #[test]
    fn t_guillotine_anti_fatigue() {
        // 原书 ma p.310 铁证：断头铡刀第二次比第一次更凶狠
        let mut fatigue = SignalFatigue::new();
        assert_eq!(
            fatigue.register_and_get_weight(SignalKind::Guillotine),
            1.0
        );
        // 第 2 次仍应为 1.0（反疲劳例外）
        assert_eq!(
            fatigue.register_and_get_weight(SignalKind::Guillotine),
            1.0
        );
        assert_eq!(
            fatigue.register_and_get_weight(SignalKind::Guillotine),
            1.0
        );
        assert!(SignalKind::Guillotine.is_anti_fatigue());
    }

    #[test]
    fn t_reset_clears_single_kind() {
        let mut fatigue = SignalFatigue::new();
        fatigue.register(SignalKind::DeathCross);
        fatigue.register(SignalKind::DeathCross);
        assert_eq!(fatigue.count(SignalKind::DeathCross), 2);
        fatigue.reset(SignalKind::DeathCross);
        assert_eq!(fatigue.count(SignalKind::DeathCross), 0);
    }

    #[test]
    fn t_reset_opposite_buy_clears_sells() {
        // 买入信号出现 → 所有卖出信号计数清零
        let mut fatigue = SignalFatigue::new();
        fatigue.register(SignalKind::DeathCross);
        fatigue.register(SignalKind::PoissonSpider);
        fatigue.register(SignalKind::GranvilleBuy);
        // 先验证卖出已计数
        assert_eq!(fatigue.count(SignalKind::DeathCross), 1);
        // 触发买入信号的 reset_opposite
        fatigue.reset_opposite(SignalKind::GoldenCross);
        assert_eq!(fatigue.count(SignalKind::DeathCross), 0);
        assert_eq!(fatigue.count(SignalKind::PoissonSpider), 0);
        // 买入信号不受影响
        assert_eq!(fatigue.count(SignalKind::GranvilleBuy), 1);
    }

    #[test]
    fn t_reset_opposite_sell_clears_buys() {
        let mut fatigue = SignalFatigue::new();
        fatigue.register(SignalKind::GranvilleBuy);
        fatigue.register(SignalKind::GoldenCross);
        fatigue.reset_opposite(SignalKind::DeathCross);
        assert_eq!(fatigue.count(SignalKind::GranvilleBuy), 0);
        assert_eq!(fatigue.count(SignalKind::GoldenCross), 0);
    }

    #[test]
    fn t_different_kinds_independent() {
        // 不同类型信号的计数独立
        let mut fatigue = SignalFatigue::new();
        fatigue.register(SignalKind::DeathCross);
        fatigue.register(SignalKind::PoissonSpider);
        assert_eq!(fatigue.count(SignalKind::DeathCross), 1);
        assert_eq!(fatigue.count(SignalKind::PoissonSpider), 1);
        // 独立衰减
        assert_eq!(fatigue.weight_decay(SignalKind::DeathCross), 1.0);
        fatigue.register(SignalKind::DeathCross);
        assert_eq!(fatigue.weight_decay(SignalKind::DeathCross), 0.5);
        // PoissonSpider 权重不受影响
        assert_eq!(fatigue.weight_decay(SignalKind::PoissonSpider), 1.0);
    }

    #[test]
    fn t_clear_resets_all() {
        let mut fatigue = SignalFatigue::new();
        fatigue.register(SignalKind::DeathCross);
        fatigue.register(SignalKind::GranvilleBuy);
        fatigue.clear();
        assert_eq!(fatigue.count(SignalKind::DeathCross), 0);
        assert_eq!(fatigue.count(SignalKind::GranvilleBuy), 0);
    }
}
