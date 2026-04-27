//! E4：回测绩效指标计算

use super::types::{EquityPoint, Performance, Trade};

/// 从权益曲线与交易清单计算绩效指标
pub fn compute(
    initial: f64,
    equity: &[EquityPoint],
    trades: &[Trade],
    bars_per_year: f64,
) -> Performance {
    let total_return_pct = if let Some(last) = equity.last() {
        (last.equity - initial) / initial
    } else {
        0.0
    };

    // 年化（按 bars 估算）
    let bars = equity.len() as f64;
    let years = if bars_per_year > 0.0 { bars / bars_per_year } else { 0.0 };
    let annualized_return_pct = if years > 0.01 {
        (1.0 + total_return_pct).powf(1.0 / years) - 1.0
    } else {
        total_return_pct
    };

    // 最大回撤
    let (max_dd, dd_duration) = max_drawdown(equity);

    // 交易统计
    let closed: Vec<&Trade> = trades.iter().filter(|t| t.exit_price.is_some()).collect();
    let total_trades = closed.len();
    let wins: Vec<&&Trade> = closed.iter().filter(|t| t.pnl > 0.0).collect();
    let losses: Vec<&&Trade> = closed.iter().filter(|t| t.pnl <= 0.0).collect();
    let win_rate = if total_trades > 0 {
        wins.len() as f64 / total_trades as f64
    } else {
        0.0
    };

    let profit_sum: f64 = wins.iter().map(|t| t.pnl).sum();
    let loss_sum: f64 = losses.iter().map(|t| -t.pnl).sum(); // 取正
    let profit_factor = if loss_sum > 0.0 {
        profit_sum / loss_sum
    } else if profit_sum > 0.0 {
        f64::INFINITY
    } else {
        0.0
    };
    let avg_win = if !wins.is_empty() {
        profit_sum / wins.len() as f64
    } else {
        0.0
    };
    let avg_loss = if !losses.is_empty() {
        loss_sum / losses.len() as f64
    } else {
        0.0
    };
    let expectancy_r = if total_trades > 0 {
        closed.iter().map(|t| t.r_multiple).sum::<f64>() / total_trades as f64
    } else {
        0.0
    };

    // 夏普 / 索提诺（基于权益序列的对数收益）
    let returns: Vec<f64> = equity
        .windows(2)
        .filter_map(|w| {
            if w[0].equity > 0.0 {
                Some((w[1].equity / w[0].equity).ln())
            } else {
                None
            }
        })
        .collect();
    let sharpe = sharpe_ratio(&returns, bars_per_year);
    let sortino = sortino_ratio(&returns, bars_per_year);
    let calmar = if max_dd > 1e-9 {
        annualized_return_pct / max_dd
    } else if annualized_return_pct > 0.0 {
        f64::INFINITY
    } else {
        0.0
    };

    // 连胜/连亏 + 平均持仓
    let (max_w, max_l) = consec_streaks(&closed);
    let avg_hold_bars = if !closed.is_empty() {
        closed
            .iter()
            .map(|t| t.exit_index.unwrap_or(t.entry_index) as i64 - t.entry_index as i64)
            .map(|x| x.max(0) as f64)
            .sum::<f64>()
            / closed.len() as f64
    } else {
        0.0
    };

    Performance {
        total_return_pct,
        annualized_return_pct,
        max_drawdown_pct: max_dd,
        max_drawdown_duration_bars: dd_duration,
        win_rate,
        profit_factor,
        avg_win,
        avg_loss,
        expectancy_r,
        sharpe,
        sortino,
        calmar,
        total_trades,
        wins: wins.len(),
        losses: losses.len(),
        max_consec_wins: max_w,
        max_consec_losses: max_l,
        avg_hold_bars,
    }
}

/// (max_dd_pct, duration_bars)
fn max_drawdown(equity: &[EquityPoint]) -> (f64, usize) {
    let mut peak = f64::MIN;
    let mut peak_idx = 0usize;
    let mut max_dd = 0.0;
    let mut max_dur = 0usize;
    for (i, p) in equity.iter().enumerate() {
        if p.equity > peak {
            peak = p.equity;
            peak_idx = i;
        }
        if peak > 0.0 {
            let dd = (peak - p.equity) / peak;
            if dd > max_dd {
                max_dd = dd;
                max_dur = i - peak_idx;
            }
        }
    }
    (max_dd, max_dur)
}

fn sharpe_ratio(returns: &[f64], bars_per_year: f64) -> f64 {
    if returns.len() < 2 {
        return 0.0;
    }
    let mean = returns.iter().sum::<f64>() / returns.len() as f64;
    let var = returns.iter().map(|r| (r - mean).powi(2)).sum::<f64>() / (returns.len() - 1) as f64;
    let std = var.sqrt();
    if std < 1e-12 {
        return 0.0;
    }
    (mean / std) * bars_per_year.sqrt()
}

fn sortino_ratio(returns: &[f64], bars_per_year: f64) -> f64 {
    if returns.len() < 2 {
        return 0.0;
    }
    let mean = returns.iter().sum::<f64>() / returns.len() as f64;
    let downs: Vec<f64> = returns.iter().copied().filter(|r| *r < 0.0).collect();
    if downs.is_empty() {
        return 0.0;
    }
    let dstd = (downs.iter().map(|r| r.powi(2)).sum::<f64>() / downs.len() as f64).sqrt();
    if dstd < 1e-12 {
        return 0.0;
    }
    (mean / dstd) * bars_per_year.sqrt()
}

fn consec_streaks(trades: &[&Trade]) -> (usize, usize) {
    let mut max_w = 0usize;
    let mut max_l = 0usize;
    let mut cur_w = 0usize;
    let mut cur_l = 0usize;
    for t in trades {
        if t.pnl > 0.0 {
            cur_w += 1;
            cur_l = 0;
            max_w = max_w.max(cur_w);
        } else {
            cur_l += 1;
            cur_w = 0;
            max_l = max_l.max(cur_l);
        }
    }
    (max_w, max_l)
}

/// 每个 timeframe 的年化 bars 数
pub fn bars_per_year(interval: &str) -> f64 {
    match interval {
        "1m" => 525_600.0,
        "5m" => 105_120.0,
        "15m" => 35_040.0,
        "30m" => 17_520.0,
        "1h" => 8_760.0,
        "4h" => 2_190.0,
        "1d" => 365.0,
        "1w" => 52.0,
        "1M" => 12.0,
        _ => 365.0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::backtest::types::{EquityPoint, ExitReason, Side, Trade};

    fn eq_pt(time: i64, equity: f64) -> EquityPoint {
        EquityPoint { time, equity, drawdown: 0.0 }
    }

    fn win_trade(id: usize, entry: usize, exit: usize, pnl: f64, r: f64) -> Trade {
        Trade {
            id,
            side: Side::Long,
            entry_index: entry,
            entry_time: entry as i64,
            entry_price: 100.0,
            stop_loss: 95.0,
            take_profit: 110.0,
            qty: 1.0,
            exit_index: Some(exit),
            exit_time: Some(exit as i64),
            exit_price: Some(100.0 + pnl),
            exit_reason: Some(ExitReason::TakeProfit),
            pnl,
            r_multiple: r,
            reasons: vec![],
        }
    }

    fn loss_trade(id: usize, entry: usize, exit: usize, pnl: f64, r: f64) -> Trade {
        Trade {
            id,
            side: Side::Long,
            entry_index: entry,
            entry_time: entry as i64,
            entry_price: 100.0,
            stop_loss: 95.0,
            take_profit: 110.0,
            qty: 1.0,
            exit_index: Some(exit),
            exit_time: Some(exit as i64),
            exit_price: Some(100.0 + pnl),
            exit_reason: Some(ExitReason::StopLoss),
            pnl,
            r_multiple: r,
            reasons: vec![],
        }
    }

    // -------- bars_per_year --------

    #[test]
    fn t_bars_per_year_known_intervals() {
        assert_eq!(bars_per_year("1h"), 8_760.0);
        assert_eq!(bars_per_year("4h"), 2_190.0);
        assert_eq!(bars_per_year("1d"), 365.0);
        assert_eq!(bars_per_year("1w"), 52.0);
        assert_eq!(bars_per_year("1M"), 12.0);
    }

    #[test]
    fn t_bars_per_year_unknown_falls_back_to_daily() {
        assert_eq!(bars_per_year("unknown"), 365.0);
        assert_eq!(bars_per_year(""), 365.0);
    }

    // -------- compute() --------

    #[test]
    fn t_compute_empty_equity_zero_return() {
        let perf = compute(10_000.0, &[], &[], 365.0);
        assert_eq!(perf.total_return_pct, 0.0);
        assert_eq!(perf.total_trades, 0);
        assert_eq!(perf.win_rate, 0.0);
        assert_eq!(perf.max_drawdown_pct, 0.0);
    }

    #[test]
    fn t_compute_total_return_pct_from_equity() {
        // initial=10000, final=11000 → total_return = 0.10
        let equity = vec![eq_pt(0, 10_000.0), eq_pt(1, 11_000.0)];
        let perf = compute(10_000.0, &equity, &[], 365.0);
        assert!((perf.total_return_pct - 0.10).abs() < 1e-9);
    }

    #[test]
    fn t_compute_max_drawdown_from_peak_to_trough() {
        // 10000 → 12000 → 9000 → 11000
        // 峰 12000, 谷 9000, dd = (12000-9000)/12000 = 0.25
        let equity = vec![
            eq_pt(0, 10_000.0),
            eq_pt(1, 12_000.0),
            eq_pt(2, 9_000.0),
            eq_pt(3, 11_000.0),
        ];
        let perf = compute(10_000.0, &equity, &[], 365.0);
        assert!((perf.max_drawdown_pct - 0.25).abs() < 1e-9, "实际 {}", perf.max_drawdown_pct);
    }

    #[test]
    fn t_compute_max_drawdown_zero_when_monotonic_up() {
        let equity = vec![
            eq_pt(0, 10_000.0),
            eq_pt(1, 11_000.0),
            eq_pt(2, 12_000.0),
        ];
        let perf = compute(10_000.0, &equity, &[], 365.0);
        assert!(perf.max_drawdown_pct < 1e-9);
    }

    #[test]
    fn t_compute_win_rate_and_counts() {
        let trades = vec![
            win_trade(1, 0, 5, 100.0, 1.0),
            win_trade(2, 6, 10, 200.0, 2.0),
            loss_trade(3, 11, 15, -100.0, -1.0),
            loss_trade(4, 16, 20, -50.0, -0.5),
        ];
        let perf = compute(10_000.0, &[eq_pt(0, 10_000.0)], &trades, 365.0);
        assert_eq!(perf.total_trades, 4);
        assert_eq!(perf.wins, 2);
        assert_eq!(perf.losses, 2);
        assert!((perf.win_rate - 0.5).abs() < 1e-9);
    }

    #[test]
    fn t_compute_profit_factor_and_avg_win_loss() {
        let trades = vec![
            win_trade(1, 0, 5, 300.0, 3.0),
            loss_trade(2, 6, 10, -100.0, -1.0),
        ];
        let perf = compute(10_000.0, &[], &trades, 365.0);
        // profit_sum=300, loss_sum=100 → PF=3.0
        assert!((perf.profit_factor - 3.0).abs() < 1e-9);
        assert!((perf.avg_win - 300.0).abs() < 1e-9);
        assert!((perf.avg_loss - 100.0).abs() < 1e-9);
    }

    #[test]
    fn t_compute_profit_factor_infinity_when_no_loss() {
        let trades = vec![win_trade(1, 0, 5, 100.0, 1.0)];
        let perf = compute(10_000.0, &[], &trades, 365.0);
        assert!(perf.profit_factor.is_infinite());
    }

    #[test]
    fn t_compute_expectancy_r_from_trade_r_multiples() {
        // r: 2.0, -1.0, 1.5 → mean = 0.833
        let trades = vec![
            win_trade(1, 0, 5, 200.0, 2.0),
            loss_trade(2, 6, 10, -100.0, -1.0),
            win_trade(3, 11, 15, 150.0, 1.5),
        ];
        let perf = compute(10_000.0, &[], &trades, 365.0);
        assert!((perf.expectancy_r - (2.0 + -1.0 + 1.5) / 3.0).abs() < 1e-9);
    }

    #[test]
    fn t_compute_max_consec_wins_and_losses() {
        // W W L L L W W W L → max_w=3, max_l=3
        let trades = vec![
            win_trade(1, 0, 1, 10.0, 0.1),
            win_trade(2, 2, 3, 10.0, 0.1),
            loss_trade(3, 4, 5, -5.0, -0.05),
            loss_trade(4, 6, 7, -5.0, -0.05),
            loss_trade(5, 8, 9, -5.0, -0.05),
            win_trade(6, 10, 11, 10.0, 0.1),
            win_trade(7, 12, 13, 10.0, 0.1),
            win_trade(8, 14, 15, 10.0, 0.1),
            loss_trade(9, 16, 17, -5.0, -0.05),
        ];
        let perf = compute(10_000.0, &[], &trades, 365.0);
        assert_eq!(perf.max_consec_wins, 3);
        assert_eq!(perf.max_consec_losses, 3);
    }

    #[test]
    fn t_compute_avg_hold_bars_correct() {
        // 持仓 5, 10, 3 → avg = 6
        let trades = vec![
            win_trade(1, 0, 5, 10.0, 0.1),
            win_trade(2, 10, 20, 10.0, 0.1),
            win_trade(3, 30, 33, 10.0, 0.1),
        ];
        let perf = compute(10_000.0, &[], &trades, 365.0);
        assert!((perf.avg_hold_bars - 6.0).abs() < 1e-9);
    }

    #[test]
    fn t_compute_sharpe_zero_on_flat_returns() {
        // 权益恒定 → returns 全 0 → std=0 → sharpe=0
        let equity = vec![
            eq_pt(0, 10_000.0),
            eq_pt(1, 10_000.0),
            eq_pt(2, 10_000.0),
        ];
        let perf = compute(10_000.0, &equity, &[], 365.0);
        assert_eq!(perf.sharpe, 0.0);
    }

    #[test]
    fn t_compute_sharpe_positive_on_rising_equity() {
        // 稳步上涨（小波动）→ 正 sharpe
        let equity: Vec<_> = (0..30)
            .map(|i| eq_pt(i as i64, 10_000.0 + i as f64 * 100.0))
            .collect();
        let perf = compute(10_000.0, &equity, &[], 365.0);
        assert!(perf.sharpe > 0.0, "稳步上涨 sharpe 应 > 0，实际 {}", perf.sharpe);
    }

    #[test]
    fn t_compute_sortino_zero_when_no_losses() {
        // 全部上涨 → 无负收益 → sortino=0（按定义返回 0）
        let equity: Vec<_> = (0..10)
            .map(|i| eq_pt(i as i64, 10_000.0 + i as f64 * 100.0))
            .collect();
        let perf = compute(10_000.0, &equity, &[], 365.0);
        assert_eq!(perf.sortino, 0.0);
    }

    #[test]
    fn t_compute_calmar_positive_on_positive_return_with_drawdown() {
        // 10000 → 11000 → 9000 → 12000
        let equity = vec![
            eq_pt(0, 10_000.0),
            eq_pt(1, 11_000.0),
            eq_pt(2, 9_000.0),
            eq_pt(3, 12_000.0),
        ];
        let perf = compute(10_000.0, &equity, &[], 365.0);
        // total_return = 0.20, max_dd ≈ (11000-9000)/11000 ≈ 0.1818
        // calmar ≈ annualized / max_dd，年化会趋近 0.20（bars 很少）
        assert!(perf.calmar > 0.0, "calmar 应 > 0，实际 {}", perf.calmar);
    }
}
