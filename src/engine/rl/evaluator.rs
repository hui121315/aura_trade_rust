//! 触发登记 + horizon 结算
//!
//! 工作流：
//! 1. 信号触发时调 [`register_trigger`] 把 arm + 触发时价 + 方向入队 pending
//! 2. 每根新 K 线调 [`settle_expired`] 处理已到 horizon 的 pending
//! 3. 统计胜/负 → [`ArmState::settle`]

use crate::data::Kline;

use super::types::{ArmCategory, BanditState, PendingEvaluation};

/// 最小有意义涨跌幅（小于此视为中性，不更新后验）
///
/// 建议默认 0.15%（覆盖一次往返手续费 + 滑点）
pub const NEUTRAL_THRESHOLD_PCT: f64 = 0.15;

/// 登记一次触发
///
/// - 若 arm 不存在则按参数创建
/// - 若 current bar 价格不合法则忽略
pub fn register_trigger(
    state: &mut BanditState,
    name: &str,
    label: &str,
    category: ArmCategory,
    book_source: Option<&str>,
    symbol: &str,
    interval: &str,
    current_bar: &Kline,
    direction: i8,
    horizon_bars: usize,
) {
    if !current_bar.close.is_finite() || current_bar.close.abs() < 1e-9 {
        return;
    }

    let arm = state.get_or_insert(name, label, category, book_source);
    arm.total_triggers += 1;

    state.total_plays += 1;
    state.pending.push(PendingEvaluation {
        arm_name: name.to_string(),
        symbol: symbol.to_string(),
        interval: interval.to_string(),
        triggered_at_ms: current_bar.open_time,
        trigger_price: current_bar.close,
        direction,
        horizon_bars: horizon_bars.max(1),
        bars_elapsed: 0,
    });
}

/// 在新一根 bar 收盘时调用
///
/// - 所有 pending 的 `bars_elapsed += 1`
/// - 若达到 horizon 则用当前 bar 的 close 作为 price_after 结算，结果更新到 arm
///
/// 返回：本次结算的条目数
pub fn on_new_bar(state: &mut BanditState, bar: &Kline) -> usize {
    // 先 tick 所有 pending，再筛选到期的
    let mut settled = 0usize;
    let mut keep: Vec<PendingEvaluation> = Vec::with_capacity(state.pending.len());

    // 取出 pending 以避免 borrow 冲突
    let pendings = std::mem::take(&mut state.pending);
    for mut p in pendings {
        p.bars_elapsed += 1;
        if p.bars_elapsed >= p.horizon_bars {
            apply_settlement(state, &p, bar.close, bar.open_time);
            settled += 1;
        } else {
            keep.push(p);
        }
    }
    state.pending = keep;
    settled
}

/// 强制以 `current_price` 结算所有未完成的 pending（批量回放结束时使用）
pub fn settle_all(state: &mut BanditState, current_price: f64, now_ms: i64) -> usize {
    let pendings = std::mem::take(&mut state.pending);
    let n = pendings.len();
    for p in pendings {
        apply_settlement(state, &p, current_price, now_ms);
    }
    n
}

fn apply_settlement(
    state: &mut BanditState,
    p: &PendingEvaluation,
    price_after: f64,
    now_ms: i64,
) {
    if !price_after.is_finite() || p.trigger_price.abs() < 1e-9 {
        return;
    }
    // directional return（百分比数字，如 1.5 = +1.5%）
    let raw = (price_after - p.trigger_price) / p.trigger_price * 100.0;
    let dir_pct = (p.direction as f64) * raw;

    let win = dir_pct > NEUTRAL_THRESHOLD_PCT;
    let loss = dir_pct < -NEUTRAL_THRESHOLD_PCT;

    // 更新 arm（如果 arm 不存在则忽略；register_trigger 时已经建）
    if let Some(arm) = state.arms.get_mut(&p.arm_name) {
        arm.settle(dir_pct, win, loss, now_ms);
        state.total_settled += 1;
    }
}

// ============================================================
// 测试
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::rl::types::ArmCategory;

    fn mk_kline(t: i64, price: f64) -> Kline {
        Kline {
            open_time: t,
            close_time: t + 60_000,
            open: price,
            high: price * 1.005,
            low: price * 0.995,
            close: price,
            volume: 1000.0,
        }
    }

    #[test]
    fn t_register_then_settle_win() {
        let mut state = BanditState::new();
        // 触发：price 100，direction=+1，horizon=2
        let bar0 = mk_kline(0, 100.0);
        register_trigger(
            &mut state,
            "x",
            "X",
            ArmCategory::Signal,
            None,
            "BTC",
            "1h",
            &bar0,
            1,
            2,
        );
        assert_eq!(state.arms["x"].total_triggers, 1);
        assert_eq!(state.pending.len(), 1);

        // 新 bar 1：未到 horizon
        let bar1 = mk_kline(60_000, 101.0);
        on_new_bar(&mut state, &bar1);
        assert_eq!(state.pending.len(), 1);
        assert_eq!(state.arms["x"].samples(), 0);

        // 新 bar 2：到 horizon，price 升到 102 → dir_pct = +2% → win
        let bar2 = mk_kline(120_000, 102.0);
        let n = on_new_bar(&mut state, &bar2);
        assert_eq!(n, 1);
        assert_eq!(state.pending.len(), 0);
        let arm = &state.arms["x"];
        assert_eq!(arm.total_wins, 1);
        assert_eq!(arm.total_losses, 0);
        assert!((arm.cumulative_return_pct - 2.0).abs() < 1e-9);
    }

    #[test]
    fn t_settle_loss() {
        let mut state = BanditState::new();
        register_trigger(
            &mut state,
            "x",
            "X",
            ArmCategory::Signal,
            None,
            "BTC",
            "1h",
            &mk_kline(0, 100.0),
            1, // +1 多头
            1,
        );
        // 跌到 98 → -2% → loss
        on_new_bar(&mut state, &mk_kline(60_000, 98.0));
        let arm = &state.arms["x"];
        assert_eq!(arm.total_losses, 1);
        assert!((arm.cumulative_return_pct - (-2.0)).abs() < 1e-9);
        assert_eq!(arm.alpha, 1.0);
        assert_eq!(arm.beta, 2.0);
    }

    #[test]
    fn t_neutral_does_not_update_posterior() {
        let mut state = BanditState::new();
        register_trigger(
            &mut state,
            "x",
            "X",
            ArmCategory::Signal,
            None,
            "BTC",
            "1h",
            &mk_kline(0, 100.0),
            1,
            1,
        );
        // 只涨 0.05%（< NEUTRAL_THRESHOLD_PCT=0.15）→ 中性
        on_new_bar(&mut state, &mk_kline(60_000, 100.05));
        let arm = &state.arms["x"];
        assert_eq!(arm.total_neutral, 1);
        assert_eq!(arm.total_wins, 0);
        assert_eq!(arm.total_losses, 0);
        assert_eq!(arm.alpha, 1.0);
        assert_eq!(arm.beta, 1.0);
    }

    #[test]
    fn t_settle_all_force() {
        let mut state = BanditState::new();
        for i in 0..3 {
            register_trigger(
                &mut state,
                "x",
                "X",
                ArmCategory::Signal,
                None,
                "BTC",
                "1h",
                &mk_kline(i * 60_000, 100.0),
                1,
                10,
            );
        }
        assert_eq!(state.pending.len(), 3);
        let n = settle_all(&mut state, 105.0, 999);
        assert_eq!(n, 3);
        assert_eq!(state.pending.len(), 0);
        assert_eq!(state.arms["x"].total_wins, 3); // 全部 +5% win
    }

    #[test]
    fn t_multiple_arms_isolated() {
        let mut state = BanditState::new();
        register_trigger(&mut state, "a", "A", ArmCategory::Signal, None, "BTC", "1h",
            &mk_kline(0, 100.0), 1, 1);
        register_trigger(&mut state, "b", "B", ArmCategory::Signal, None, "BTC", "1h",
            &mk_kline(0, 100.0), -1, 1);
        // price 上升，a 应 win、b 应 loss
        on_new_bar(&mut state, &mk_kline(60_000, 102.0));
        assert_eq!(state.arms["a"].total_wins, 1);
        assert_eq!(state.arms["b"].total_losses, 1);
    }

    #[test]
    fn t_short_direction_win_on_drop() {
        let mut state = BanditState::new();
        register_trigger(&mut state, "short", "S", ArmCategory::Signal, None, "BTC",
            "1h", &mk_kline(0, 100.0), -1, 1);
        on_new_bar(&mut state, &mk_kline(60_000, 97.0));
        // direction=-1, raw = -3, dir_pct = +3 → win
        let arm = &state.arms["short"];
        assert_eq!(arm.total_wins, 1);
        assert!((arm.cumulative_return_pct - 3.0).abs() < 1e-9);
    }
}
