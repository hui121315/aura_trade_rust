//! RL 核心类型：ArmState / BanditState / PendingEvaluation
//!
//! 设计思想见 `RL_EFFECTIVENESS_DESIGN.md` §3。

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

/// Arm 分类
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ArmCategory {
    Signal,
    Playbook,
    Pattern,
    Confluence,
}

impl ArmCategory {
    pub fn as_str(&self) -> &'static str {
        match self {
            ArmCategory::Signal => "Signal",
            ArmCategory::Playbook => "Playbook",
            ArmCategory::Pattern => "Pattern",
            ArmCategory::Confluence => "Confluence",
        }
    }
}

/// 单个 arm 的 Beta-Bernoulli 后验 + 聚合统计
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArmState {
    /// 唯一标识（同 effectiveness 模块）
    pub name: String,
    /// 可选：人类可读标签 + 原书
    #[serde(default)]
    pub label: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub book_source: Option<String>,
    /// 分类（L1/L2/L3/L4）
    pub category: ArmCategory,

    /// Beta(α, β) 后验参数；先验 Beta(1, 1)
    pub alpha: f64,
    pub beta: f64,

    /// 全部触发次数（含未结算 pending）
    pub total_triggers: u64,
    /// 已结算累计胜 / 负 / 中性
    pub total_wins: u64,
    pub total_losses: u64,
    pub total_neutral: u64,

    /// 累计 R-multiple（方向修正后的 directional_return 百分比数字，如 +1.5 表示 +1.5%）
    pub cumulative_return_pct: f64,
    pub max_return_pct: f64,
    pub min_return_pct: f64,

    /// 最后一次结算时间（ms UTC）
    pub last_updated_ms: i64,
    /// 最近一次采样得到的 θ（用于观察）
    #[serde(default)]
    pub last_theta: f64,
}

impl ArmState {
    /// 新建一个 arm，默认 Beta(1, 1) 均匀先验
    pub fn new(
        name: impl Into<String>,
        label: impl Into<String>,
        category: ArmCategory,
        book_source: Option<impl Into<String>>,
    ) -> Self {
        Self {
            name: name.into(),
            label: label.into(),
            book_source: book_source.map(Into::into),
            category,
            alpha: 1.0,
            beta: 1.0,
            total_triggers: 0,
            total_wins: 0,
            total_losses: 0,
            total_neutral: 0,
            cumulative_return_pct: 0.0,
            max_return_pct: 0.0,
            min_return_pct: 0.0,
            last_updated_ms: 0,
            last_theta: 0.5,
        }
    }

    /// 已结算样本数
    pub fn samples(&self) -> u64 {
        self.total_wins + self.total_losses + self.total_neutral
    }

    /// 后验胜率估计 = α / (α + β)
    pub fn posterior_mean(&self) -> f64 {
        self.alpha / (self.alpha + self.beta)
    }

    /// 后验方差（不确定度）
    pub fn posterior_variance(&self) -> f64 {
        let s = self.alpha + self.beta;
        (self.alpha * self.beta) / (s * s * (s + 1.0))
    }

    /// 平均 R（百分比）
    pub fn avg_return_pct(&self) -> f64 {
        let n = self.samples();
        if n == 0 {
            0.0
        } else {
            self.cumulative_return_pct / n as f64
        }
    }

    /// Thompson 采样一个 θ（[0, 1]）
    pub fn sample_theta(&self, rng: &mut super::rng::Xoshiro256) -> f64 {
        super::rng::beta_sample(self.alpha, self.beta, rng)
    }

    /// UCB1 置信上界（备选选择策略）
    ///
    /// total_plays = 所有 arm 合计触发次数
    pub fn ucb1(&self, total_plays: u64, c: f64) -> f64 {
        let n = (self.total_triggers as f64).max(1.0);
        self.posterior_mean() + c * ((total_plays as f64).ln() / n).sqrt()
    }

    /// 结算一次观察
    ///
    /// - `win`：胜（directional_return > cost_threshold）
    /// - `loss`：负（directional_return < -cost_threshold）
    /// - 中性：|directional_return| ≤ cost_threshold，不更新后验，只计 total
    pub fn settle(&mut self, return_pct: f64, win: bool, loss: bool, now_ms: i64) {
        // 截断极值，防止单次异常主导
        let r = return_pct.clamp(-50.0, 50.0);
        self.cumulative_return_pct += r;
        if r > self.max_return_pct || self.samples() == 0 {
            self.max_return_pct = r;
        }
        if r < self.min_return_pct || self.samples() == 0 {
            self.min_return_pct = r;
        }
        if win {
            self.alpha += 1.0;
            self.total_wins += 1;
        } else if loss {
            self.beta += 1.0;
            self.total_losses += 1;
        } else {
            self.total_neutral += 1;
        }
        self.last_updated_ms = now_ms;
    }

    /// 指数衰减 α, β（保留先验不低于 1.0）
    pub fn decay(&mut self, gamma: f64) {
        self.alpha = 1.0 + (self.alpha - 1.0) * gamma;
        self.beta = 1.0 + (self.beta - 1.0) * gamma;
    }
}

/// 一次触发等待结算的记录
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PendingEvaluation {
    pub arm_name: String,
    pub symbol: String,
    pub interval: String,
    /// 触发时的 bar open_time (ms UTC)
    pub triggered_at_ms: i64,
    pub trigger_price: f64,
    /// +1 多 / -1 空 / 0 中性
    pub direction: i8,
    /// 固定 horizon（多少根 K 线后结算）
    pub horizon_bars: usize,
    /// 已消耗的 K 线数（每根新 bar +1），到达 horizon 时自动结算
    #[serde(default)]
    pub bars_elapsed: usize,
}

/// 所有 arm 的全局状态（持久化根）
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct BanditState {
    pub version: u32,
    pub arms: HashMap<String, ArmState>,
    pub pending: Vec<PendingEvaluation>,
    /// 元数据
    pub total_plays: u64,
    pub total_settled: u64,
    pub last_saved_ms: i64,
}

impl BanditState {
    pub const CURRENT_VERSION: u32 = 1;

    pub fn new() -> Self {
        Self {
            version: Self::CURRENT_VERSION,
            ..Default::default()
        }
    }

    /// 获取或创建一个 arm
    pub fn get_or_insert(
        &mut self,
        name: &str,
        label: &str,
        category: ArmCategory,
        book_source: Option<&str>,
    ) -> &mut ArmState {
        if !self.arms.contains_key(name) {
            self.arms.insert(
                name.to_string(),
                ArmState::new(name, label, category, book_source.map(|s| s.to_string())),
            );
        }
        self.arms.get_mut(name).expect("just inserted")
    }

    /// 批量应用衰减
    pub fn decay_all(&mut self, gamma: f64) {
        for arm in self.arms.values_mut() {
            arm.decay(gamma);
        }
    }
}

// ============================================================
// 测试
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn t_new_arm_uniform_prior() {
        let a = ArmState::new("x", "X", ArmCategory::Signal, None::<String>);
        assert_eq!(a.alpha, 1.0);
        assert_eq!(a.beta, 1.0);
        assert_eq!(a.samples(), 0);
        assert!((a.posterior_mean() - 0.5).abs() < 1e-9);
    }

    #[test]
    fn t_settle_updates_posterior() {
        let mut a = ArmState::new("x", "X", ArmCategory::Signal, None::<String>);
        // 3 胜
        a.settle(2.0, true, false, 1000);
        a.settle(1.5, true, false, 2000);
        a.settle(3.0, true, false, 3000);
        // 1 负
        a.settle(-2.0, false, true, 4000);

        assert_eq!(a.total_wins, 3);
        assert_eq!(a.total_losses, 1);
        assert_eq!(a.alpha, 4.0); // 1 + 3
        assert_eq!(a.beta, 2.0); // 1 + 1
        assert!((a.posterior_mean() - 4.0 / 6.0).abs() < 1e-9);
        assert!((a.avg_return_pct() - (2.0 + 1.5 + 3.0 - 2.0) / 4.0).abs() < 1e-9);
        assert_eq!(a.max_return_pct, 3.0);
        assert_eq!(a.min_return_pct, -2.0);
    }

    #[test]
    fn t_settle_neutral_keeps_posterior_but_counts() {
        let mut a = ArmState::new("x", "X", ArmCategory::Signal, None::<String>);
        a.settle(0.05, false, false, 100);
        a.settle(-0.05, false, false, 200);
        assert_eq!(a.alpha, 1.0);
        assert_eq!(a.beta, 1.0);
        assert_eq!(a.total_neutral, 2);
        assert_eq!(a.samples(), 2);
    }

    #[test]
    fn t_return_pct_is_clamped() {
        let mut a = ArmState::new("x", "X", ArmCategory::Signal, None::<String>);
        // 模拟 BTC 1d 在一根 bar 内 +80%（应被截到 +50）
        a.settle(80.0, true, false, 1);
        assert_eq!(a.cumulative_return_pct, 50.0);
        assert_eq!(a.max_return_pct, 50.0);
    }

    #[test]
    fn t_decay_preserves_prior_floor() {
        let mut a = ArmState::new("x", "X", ArmCategory::Signal, None::<String>);
        // 10 胜 10 负
        for _ in 0..10 {
            a.settle(1.0, true, false, 0);
        }
        for _ in 0..10 {
            a.settle(-1.0, false, true, 0);
        }
        assert_eq!(a.alpha, 11.0);
        assert_eq!(a.beta, 11.0);
        a.decay(0.5);
        // (11-1)*0.5 + 1 = 6
        assert!((a.alpha - 6.0).abs() < 1e-9);
        assert!((a.beta - 6.0).abs() < 1e-9);
    }

    #[test]
    fn t_state_get_or_insert() {
        let mut s = BanditState::new();
        let arm1 = s.get_or_insert("a", "A", ArmCategory::Signal, Some("p.1"));
        arm1.alpha = 2.0;
        // 第二次获取：保留修改
        let arm2 = s.get_or_insert("a", "A", ArmCategory::Signal, None);
        assert_eq!(arm2.alpha, 2.0);
        assert_eq!(s.arms.len(), 1);
    }
}
