//! Thompson Sampling / UCB 选择策略
//!
//! 给定一组"被触发"的 arm 名，根据它们各自的 Beta 后验采样 θ，选出最高者。

use super::rng::Xoshiro256;
use super::types::{ArmCategory, BanditState};

/// 选择策略
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectionPolicy {
    /// Thompson Sampling：对每个 arm 采样 θ ~ Beta(α, β)，选 argmax
    Thompson,
    /// UCB1：`mean + c · sqrt(ln(N)/n)`
    Ucb1 { c: u8 },
    /// Greedy：直接选 posterior_mean 最高
    Greedy,
}

impl Default for SelectionPolicy {
    fn default() -> Self {
        SelectionPolicy::Thompson
    }
}

/// 根据策略从 candidates 中选出执行者
///
/// # 冷启动保护
///
/// 如果被选中的 arm 触发总数 < `min_samples`，则回退到 fallback_order（按优先级取第一个存在于 candidates 的）。
pub fn choose<'a>(
    state: &'a BanditState,
    candidates: &[&'a str],
    policy: SelectionPolicy,
    rng: &mut Xoshiro256,
    min_samples: u64,
    fallback_order: &[&str],
) -> Option<String> {
    if candidates.is_empty() {
        return None;
    }

    // 查看候选中是否全部仍在冷启动
    let cold_start = candidates.iter().all(|n| {
        state
            .arms
            .get(*n)
            .map(|a| a.total_triggers < min_samples)
            .unwrap_or(true)
    });
    if cold_start {
        for pref in fallback_order {
            if candidates.iter().any(|c| c == pref) {
                return Some((*pref).to_string());
            }
        }
        // fallback 也不包含 → 直接取第一个 candidate
        return Some(candidates[0].to_string());
    }

    let best = match policy {
        SelectionPolicy::Thompson => select_thompson(state, candidates, rng),
        SelectionPolicy::Ucb1 { c } => select_ucb1(state, candidates, c as f64),
        SelectionPolicy::Greedy => select_greedy(state, candidates),
    };
    best.map(|s| s.to_string())
}

fn select_thompson<'a>(
    state: &'a BanditState,
    candidates: &[&'a str],
    rng: &mut Xoshiro256,
) -> Option<&'a str> {
    let mut best: Option<(&str, f64)> = None;
    for name in candidates {
        // 未在 state 中的 arm 用默认 Beta(1,1)，θ ~ Uniform
        let theta = match state.arms.get(*name) {
            Some(arm) => arm.sample_theta(rng),
            None => super::rng::beta_sample(1.0, 1.0, rng),
        };
        if best.map(|(_, t)| theta > t).unwrap_or(true) {
            best = Some((*name, theta));
        }
    }
    best.map(|(n, _)| n)
}

fn select_ucb1<'a>(state: &'a BanditState, candidates: &[&'a str], c: f64) -> Option<&'a str> {
    let total = state.total_plays.max(1);
    let mut best: Option<(&str, f64)> = None;
    for name in candidates {
        let score = match state.arms.get(*name) {
            Some(arm) => arm.ucb1(total, c),
            None => f64::INFINITY, // 未见过的 arm 应优先探索
        };
        if best.map(|(_, s)| score > s).unwrap_or(true) {
            best = Some((*name, score));
        }
    }
    best.map(|(n, _)| n)
}

fn select_greedy<'a>(state: &'a BanditState, candidates: &[&'a str]) -> Option<&'a str> {
    let mut best: Option<(&str, f64)> = None;
    for name in candidates {
        let score = state.arms.get(*name).map(|a| a.posterior_mean()).unwrap_or(0.5);
        if best.map(|(_, s)| score > s).unwrap_or(true) {
            best = Some((*name, score));
        }
    }
    best.map(|(n, _)| n)
}

/// 把一份 [`effectiveness::EffectivenessReport`] 批量合并到 Bandit state
///
/// 语义：对每个 `rankings[i]`，把 wins 加到 α、losses 加到 β，
/// 不丢弃已有先验，幂等合并（多次调用 = 多次增量）。
///
/// 返回：更新的 arm 数（包括全新插入）
pub fn merge_report(
    state: &mut BanditState,
    report: &crate::engine::effectiveness::EffectivenessReport,
    now_ms: i64,
) -> usize {
    use std::collections::HashMap;
    let category_map: HashMap<&str, ArmCategory> = [
        ("Signal", ArmCategory::Signal),
        ("Playbook", ArmCategory::Playbook),
        ("Pattern", ArmCategory::Pattern),
        ("Confluence", ArmCategory::Confluence),
    ]
    .into_iter()
    .collect();

    let mut updated = 0usize;
    for r in &report.rankings {
        let cat = category_map
            .get(r.category.as_str())
            .copied()
            .unwrap_or(ArmCategory::Signal);
        let arm = state.get_or_insert(&r.arm, &r.label, cat, r.book_source.as_deref());
        arm.alpha += r.wins as f64;
        arm.beta += r.losses as f64;
        arm.total_triggers += r.n as u64;
        arm.total_wins += r.wins as u64;
        arm.total_losses += r.losses as u64;
        arm.cumulative_return_pct += r.avg_return_pct * r.n as f64;
        if r.max_return > arm.max_return_pct || arm.samples() == 0 {
            arm.max_return_pct = r.max_return * 100.0;
        }
        if r.min_return < arm.min_return_pct || arm.samples() == 0 {
            arm.min_return_pct = r.min_return * 100.0;
        }
        arm.last_updated_ms = now_ms;
        state.total_plays += r.n as u64;
        state.total_settled += (r.wins + r.losses) as u64;
        updated += 1;
    }
    state.last_saved_ms = now_ms;
    updated
}

/// 按类别分组的排行榜快照（用于 API 输出）
pub fn rank_snapshot(state: &BanditState) -> Vec<super::types::ArmState> {
    let mut v: Vec<_> = state.arms.values().cloned().collect();
    v.sort_by(|a, b| {
        // 按 posterior_mean 降序，tie break 用 samples
        b.posterior_mean()
            .partial_cmp(&a.posterior_mean())
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| b.samples().cmp(&a.samples()))
    });
    v
}

#[allow(dead_code)]
fn _keepalive(_: ArmCategory) {}

// ============================================================
// 测试
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::rl::types::ArmCategory;

    #[test]
    fn t_empty_candidates_returns_none() {
        let state = BanditState::new();
        let mut rng = Xoshiro256::from_seed(1);
        let out = choose(&state, &[], SelectionPolicy::Thompson, &mut rng, 5, &[]);
        assert!(out.is_none());
    }

    #[test]
    fn t_cold_start_uses_fallback() {
        let state = BanditState::new();
        let mut rng = Xoshiro256::from_seed(1);
        let out = choose(
            &state,
            &["a", "b", "c"],
            SelectionPolicy::Thompson,
            &mut rng,
            5,
            &["b", "a"],
        );
        // min_samples=5，没有 arm 满足 → fallback "b"
        assert_eq!(out.as_deref(), Some("b"));
    }

    #[test]
    fn t_after_many_trials_prefers_winner() {
        // 构造 arm "winner" 胜率 90%；"loser" 胜率 20%
        let mut state = BanditState::new();
        let w = state.get_or_insert("winner", "W", ArmCategory::Signal, None);
        w.total_triggers = 100;
        w.alpha = 91.0; // 90 胜
        w.beta = 11.0; // 10 负
        let l = state.get_or_insert("loser", "L", ArmCategory::Signal, None);
        l.total_triggers = 100;
        l.alpha = 21.0;
        l.beta = 81.0;

        // 跑 1000 次 Thompson，看 winner 被选中的比例
        let mut rng = Xoshiro256::from_seed(42);
        let mut count_winner = 0;
        for _ in 0..1000 {
            let choice = choose(
                &state,
                &["winner", "loser"],
                SelectionPolicy::Thompson,
                &mut rng,
                5,
                &[],
            )
            .unwrap();
            if choice == "winner" {
                count_winner += 1;
            }
        }
        // 应该绝大多数情况选 winner
        assert!(count_winner > 950, "got {}", count_winner);
    }

    #[test]
    fn t_thompson_explores_when_uncertain() {
        // winner 和 explorer：winner 90% 胜率但样本 100；explorer 胜率未知但样本 5
        let mut state = BanditState::new();
        let w = state.get_or_insert("winner", "W", ArmCategory::Signal, None);
        w.total_triggers = 100;
        w.alpha = 91.0;
        w.beta = 11.0;
        let e = state.get_or_insert("explorer", "E", ArmCategory::Signal, None);
        e.total_triggers = 5;
        e.alpha = 3.0; // 2 胜
        e.beta = 4.0; // 3 负

        let mut rng = Xoshiro256::from_seed(42);
        let mut count_explorer = 0;
        for _ in 0..1000 {
            let choice = choose(
                &state,
                &["winner", "explorer"],
                SelectionPolicy::Thompson,
                &mut rng,
                3,
                &[],
            )
            .unwrap();
            if choice == "explorer" {
                count_explorer += 1;
            }
        }
        // Thompson 应该给不确定的 explorer 一定探索机会
        // 但由于 winner 已非常确定（α=91/β=11 方差极小），explorer 被选中次数应较少
        // 关键断言：不是完全确定性（有随机性 → 至少偶尔选 explorer），但不会频繁选
        assert!(
            count_explorer >= 1 && count_explorer < 300,
            "got {}",
            count_explorer
        );
    }

    #[test]
    fn t_greedy_always_picks_mean_max() {
        let mut state = BanditState::new();
        let a = state.get_or_insert("a", "A", ArmCategory::Signal, None);
        a.total_triggers = 10;
        a.alpha = 7.0;
        a.beta = 3.0; // mean 0.7
        let b = state.get_or_insert("b", "B", ArmCategory::Signal, None);
        b.total_triggers = 10;
        b.alpha = 4.0;
        b.beta = 6.0; // mean 0.4

        let mut rng = Xoshiro256::from_seed(1);
        for _ in 0..50 {
            let c = choose(
                &state,
                &["a", "b"],
                SelectionPolicy::Greedy,
                &mut rng,
                5,
                &[],
            )
            .unwrap();
            assert_eq!(c, "a");
        }
    }

    #[test]
    fn t_rank_snapshot_sorted() {
        let mut state = BanditState::new();
        state.get_or_insert("lo", "L", ArmCategory::Signal, None).alpha = 2.0;
        state.get_or_insert("hi", "H", ArmCategory::Signal, None).alpha = 9.0;
        let ranked = rank_snapshot(&state);
        assert_eq!(ranked.len(), 2);
        assert_eq!(ranked[0].name, "hi");
        assert_eq!(ranked[1].name, "lo");
    }
}
