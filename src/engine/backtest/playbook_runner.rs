//! E8：Playbook 驱动的回测（Sprint 11，R-P1-12 配套）
//!
//! 不同于 [`super::runner::run`] 基于 pattern 命中 + K 线形态的触发，
//! 本模块通过 [`Playbook`] 模板实现**原书策略驱动**的回测：
//!
//! 1. 预计算每根 K 线的 ma 高级形态 + 趋势方向
//! 2. 每根 K 线构造 [`PlaybookContext`] 并调用 `playbook.decide()`
//! 3. 根据 `PlaybookDecision` 开/平仓 + 仓位校验
//! 4. 输出 [`BacktestResult`]（复用现有数据结构）
//!
//! # 与原 `runner::run` 的区别
//!
//! | 维度 | run | run_with_playbook |
//! |---|---|---|
//! | 触发 | 葛南维 + K 线形态命中 | **Playbook 策略** |
//! | 方向决策 | 强度门槛 | **原书铁证规则** |
//! | 仓位 | 固定 risk_per_trade | **target_position 占比** |
//! | 可复用策略 | 否 | **是**（通过 `Playbook` trait）|
//!
//! # 使用
//!
//! ```no_run
//! use aura_trade::engine::backtest::{BacktestConfig, CompositePlaybook};
//! use aura_trade::engine::backtest::playbook_runner::run_with_playbook;
//!
//! let cfg = BacktestConfig::default();
//! let klines = vec![]; // 从 KlineCache 获取
//! let mut pb = CompositePlaybook::default_combo();
//! let result = run_with_playbook(&klines, &cfg, &mut pb);
//! println!("总收益率：{:.2}%", result.performance.total_return_pct);
//! ```

use crate::data::Kline;
use crate::engine::ma::{self, advanced::MaAdvancedKind, scan_advanced, MaAdvancedParams};
use crate::engine::signal::staged_exit::ToppingSignalSeverity;

use super::playbook::{Playbook, PlaybookContext, PlaybookDecision};
use super::types::{
    BacktestConfig, BacktestResult, EquityPoint, ExitReason, Performance, Side, Trade,
};

/// 用 Playbook 驱动回测
pub fn run_with_playbook(
    klines: &[Kline],
    cfg: &BacktestConfig,
    playbook: &mut dyn Playbook,
) -> BacktestResult {
    let n = klines.len();
    if n == 0 {
        return empty_result(cfg);
    }

    // 1. 预计算 ma + ma_advanced 事件
    let closes: Vec<f64> = klines.iter().map(|k| k.close).collect();
    let opens: Vec<f64> = klines.iter().map(|k| k.open).collect();
    let volumes: Vec<f64> = klines.iter().map(|k| k.volume).collect();

    let periods = [5usize, 10, 20, 60];
    let mas: Vec<Vec<f64>> = periods.iter().map(|&p| ma::sma(&closes, p)).collect();
    let ma60 = &mas[3];

    // 预扫 ma_advanced 所有事件 → 存入按索引查找的映射
    let adv_events = scan_advanced(
        &closes,
        &opens,
        &volumes,
        &mas,
        &[5usize, 10, 20, 60],
        &MaAdvancedParams::default(),
    );
    let mut adv_by_index: std::collections::HashMap<usize, MaAdvancedKind> =
        std::collections::HashMap::new();
    for e in &adv_events {
        adv_by_index.insert(e.index, e.kind);
    }

    // 2. 主循环
    let mut position: f64 = 0.0; // 当前仓位占比 0-1
    let mut equity: f64 = cfg.initial_capital;
    let mut cash: f64 = cfg.initial_capital;
    let mut shares: f64 = 0.0;
    let mut entry_price: f64 = 0.0;
    let mut current_trade: Option<Trade> = None;
    let mut trades: Vec<Trade> = Vec::new();
    let mut equity_curve: Vec<EquityPoint> = Vec::with_capacity(n);
    let mut peak_equity: f64 = cfg.initial_capital;

    let fee_rate = cfg.fee_bps / 10_000.0;
    let slip_rate = cfg.slippage_bps / 10_000.0;

    for i in 0..n {
        let price = closes[i];
        if !price.is_finite() {
            continue;
        }

        if i > 0 && klines[i].open.is_finite() {
            let signal_i = i - 1;
            let exec_price = klines[i].open;

            // 估算长期趋势（用 ma60 斜率）
            let long_trend: i8 = if signal_i >= 10
                && ma60[signal_i].is_finite()
                && ma60[signal_i - 10].is_finite()
            {
                if ma60[signal_i] > ma60[signal_i - 10] * 1.005 {
                    1
                } else if ma60[signal_i] < ma60[signal_i - 10] * 0.995 {
                    -1
                } else {
                    0
                }
            } else {
                0
            };

            // 构造 Playbook 上下文
            let ctx = PlaybookContext {
                klines,
                index: signal_i,
                current_position: position,
                ma_advanced_kind: adv_by_index.get(&signal_i).copied(),
                topping_severity: adv_by_index.get(&signal_i).and_then(|k| match k {
                    MaAdvancedKind::Guillotine => Some(ToppingSignalSeverity::Severe),
                    MaAdvancedKind::PoissonSpider => Some(ToppingSignalSeverity::Intermediate),
                    _ => None,
                }),
                long_trend,
            };
            let decision = playbook.decide(&ctx);

            // 根据决策执行
            match decision {
                PlaybookDecision::Buy {
                    target_position,
                    reason,
                } => {
                    if target_position > position {
                        let delta = target_position - position;
                        let entry_with_slip = exec_price * (1.0 + slip_rate);
                        let cost = equity * delta;
                        let fee = cost * fee_rate;
                        let new_shares = (cost - fee) / entry_with_slip;
                        shares += new_shares;
                        cash -= cost;
                        position = target_position;

                        if current_trade.is_none() {
                            // 新开仓
                            entry_price = entry_with_slip;
                            current_trade = Some(Trade {
                                id: trades.len(),
                                side: Side::Long,
                                entry_index: i,
                                entry_time: klines[i].open_time,
                                entry_price: entry_with_slip,
                                stop_loss: 0.0,
                                take_profit: 0.0,
                                qty: new_shares,
                                exit_index: None,
                                exit_time: None,
                                exit_price: None,
                                exit_reason: None,
                                pnl: 0.0,
                                r_multiple: 0.0,
                                reasons: vec![reason],
                            });
                        } else if let Some(t) = current_trade.as_mut() {
                            t.qty += new_shares;
                            t.reasons.push(format!("加仓：{}", reason));
                        }
                    }
                }
                PlaybookDecision::Sell {
                    target_position,
                    reason,
                } => {
                    if target_position < position {
                        let delta = position - target_position;
                        let exit_with_slip = exec_price * (1.0 - slip_rate);
                        let shares_to_sell = shares * (delta / position.max(1e-9));
                        let proceeds = shares_to_sell * exit_with_slip;
                        let fee = proceeds * fee_rate;
                        cash += proceeds - fee;
                        shares -= shares_to_sell;
                        position = target_position;

                        if let Some(mut t) = current_trade.take() {
                            // 部分平仓也算一次平仓（简化：按累计计算）
                            t.exit_index = Some(i);
                            t.exit_time = Some(klines[i].open_time);
                            t.exit_price = Some(exit_with_slip);
                            let pnl = (exit_with_slip - entry_price) * shares_to_sell - fee;
                            t.pnl = pnl;
                            t.exit_reason = Some(if target_position <= 1e-9 {
                                ExitReason::TakeProfit
                            } else {
                                ExitReason::Reverse
                            });
                            t.reasons.push(reason);
                            let risk = (t.entry_price - t.stop_loss).abs().max(1e-9) * t.qty;
                            t.r_multiple = if risk > 0.0 { pnl / risk } else { 0.0 };
                            trades.push(t);
                            if position > 1e-9 {
                                // 还有余仓：创建新 trade 续跟踪（简化：从头开始）
                                current_trade = Some(Trade {
                                    id: trades.len(),
                                    side: Side::Long,
                                    entry_index: i,
                                    entry_time: klines[i].open_time,
                                    entry_price: exit_with_slip,
                                    stop_loss: 0.0,
                                    take_profit: 0.0,
                                    qty: shares,
                                    exit_index: None,
                                    exit_time: None,
                                    exit_price: None,
                                    exit_reason: None,
                                    pnl: 0.0,
                                    r_multiple: 0.0,
                                    reasons: vec!["余仓续持".to_string()],
                                });
                            }
                        }
                    }
                }
                PlaybookDecision::StayOut { .. } | PlaybookDecision::Hold => {
                    // 持有不动
                }
            }
        }

        // 更新权益
        equity = cash + shares * price;
        if equity > peak_equity {
            peak_equity = equity;
        }
        let drawdown = if peak_equity > 1e-9 {
            (peak_equity - equity) / peak_equity
        } else {
            0.0
        };
        equity_curve.push(EquityPoint {
            time: klines[i].close_time,
            equity,
            drawdown,
        });
    }

    // 收盘平仓（如果还有持仓）
    if let Some(mut t) = current_trade.take() {
        let last_price = *closes.last().unwrap();
        t.exit_index = Some(n - 1);
        t.exit_time = Some(klines[n - 1].close_time);
        t.exit_price = Some(last_price);
        t.pnl = (last_price - t.entry_price) * t.qty;
        t.exit_reason = Some(ExitReason::EndOfData);
        t.r_multiple = 0.0;
        trades.push(t);
    }

    // 计算绩效
    let performance = compute_performance(&trades, &equity_curve, cfg);

    BacktestResult {
        config: cfg.clone(),
        bars: n,
        start_time: klines.first().map(|k| k.open_time).unwrap_or(0),
        end_time: klines.last().map(|k| k.close_time).unwrap_or(0),
        performance,
        equity: equity_curve,
        trades,
        pattern_stats: vec![], // Playbook 模式下无 pattern 统计
    }
}

fn empty_result(cfg: &BacktestConfig) -> BacktestResult {
    BacktestResult {
        config: cfg.clone(),
        bars: 0,
        start_time: 0,
        end_time: 0,
        performance: Performance {
            total_return_pct: 0.0,
            annualized_return_pct: 0.0,
            max_drawdown_pct: 0.0,
            max_drawdown_duration_bars: 0,
            win_rate: 0.0,
            profit_factor: 0.0,
            avg_win: 0.0,
            avg_loss: 0.0,
            expectancy_r: 0.0,
            sharpe: 0.0,
            sortino: 0.0,
            calmar: 0.0,
            total_trades: 0,
            wins: 0,
            losses: 0,
            max_consec_wins: 0,
            max_consec_losses: 0,
            avg_hold_bars: 0.0,
        },
        equity: vec![],
        trades: vec![],
        pattern_stats: vec![],
    }
}

fn compute_performance(
    trades: &[Trade],
    equity: &[EquityPoint],
    cfg: &BacktestConfig,
) -> Performance {
    let total = trades.len();
    let initial = cfg.initial_capital;
    let final_equity = equity.last().map(|p| p.equity).unwrap_or(initial);
    let total_return = (final_equity - initial) / initial * 100.0;

    let wins_list: Vec<&Trade> = trades.iter().filter(|t| t.pnl > 0.0).collect();
    let losses_list: Vec<&Trade> = trades.iter().filter(|t| t.pnl <= 0.0).collect();
    let wins = wins_list.len();
    let losses = losses_list.len();
    let win_rate = if total > 0 {
        wins as f64 / total as f64
    } else {
        0.0
    };

    let gross_profit: f64 = wins_list.iter().map(|t| t.pnl).sum();
    let gross_loss: f64 = losses_list.iter().map(|t| t.pnl.abs()).sum();
    let profit_factor = if gross_loss > 1e-9 {
        gross_profit / gross_loss
    } else {
        0.0
    };

    let avg_win = if wins > 0 {
        gross_profit / wins as f64
    } else {
        0.0
    };
    let avg_loss = if losses > 0 {
        -(gross_loss / losses as f64)
    } else {
        0.0
    };

    let max_drawdown = equity.iter().map(|p| p.drawdown).fold(0.0f64, f64::max) * 100.0;

    // 简化的 Sharpe：使用权益变化率
    let returns: Vec<f64> = equity
        .windows(2)
        .map(|w| {
            if w[0].equity.abs() > 1e-9 {
                (w[1].equity - w[0].equity) / w[0].equity
            } else {
                0.0
            }
        })
        .collect();
    let sharpe = if returns.is_empty() {
        0.0
    } else {
        let mean = returns.iter().sum::<f64>() / returns.len() as f64;
        let var = returns.iter().map(|r| (r - mean).powi(2)).sum::<f64>() / returns.len() as f64;
        let std = var.sqrt();
        if std > 1e-9 {
            mean / std
        } else {
            0.0
        }
    };

    let expectancy_r = if total > 0 {
        trades.iter().map(|t| t.r_multiple).sum::<f64>() / total as f64
    } else {
        0.0
    };

    Performance {
        total_return_pct: total_return,
        annualized_return_pct: 0.0,
        max_drawdown_pct: max_drawdown,
        max_drawdown_duration_bars: 0,
        win_rate,
        profit_factor,
        avg_win,
        avg_loss,
        expectancy_r,
        sharpe,
        sortino: 0.0,
        calmar: 0.0,
        total_trades: total,
        wins,
        losses,
        max_consec_wins: 0,
        max_consec_losses: 0,
        avg_hold_bars: 0.0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::backtest::playbook::HangingScallionsEntryPlaybook;

    struct BuyAtIndex {
        index: usize,
    }

    impl Playbook for BuyAtIndex {
        fn name(&self) -> &'static str {
            "test"
        }

        fn book_source(&self) -> &'static str {
            "test"
        }

        fn decide(&mut self, ctx: &PlaybookContext<'_>) -> PlaybookDecision {
            if ctx.index == self.index && ctx.current_position <= 0.0 {
                PlaybookDecision::Buy {
                    target_position: 0.5,
                    reason: "test buy".to_string(),
                }
            } else {
                PlaybookDecision::Hold
            }
        }
    }

    fn mk_kline(idx: i64, o: f64, c: f64, h: f64, l: f64, v: f64) -> Kline {
        Kline {
            open_time: idx * 86_400_000,
            close_time: (idx + 1) * 86_400_000 - 1,
            open: o,
            high: h,
            low: l,
            close: c,
            volume: v,
        }
    }

    #[test]
    fn t_empty_klines_returns_empty_result() {
        let cfg = BacktestConfig::default();
        let mut pb = HangingScallionsEntryPlaybook;
        let result = run_with_playbook(&[], &cfg, &mut pb);
        assert_eq!(result.bars, 0);
        assert!(result.trades.is_empty());
    }

    #[test]
    fn t_playbook_hold_produces_no_trades() {
        // HangingScallionsEntryPlaybook 只在 HangingScallions 事件触发
        // 构造不会触发的均价 K 线
        let klines: Vec<Kline> = (0..100)
            .map(|i| mk_kline(i, 100.0, 100.0, 101.0, 99.0, 1.0))
            .collect();
        let cfg = BacktestConfig::default();
        let mut pb = HangingScallionsEntryPlaybook;
        let result = run_with_playbook(&klines, &cfg, &mut pb);
        // 无事件触发 → 无交易
        assert_eq!(result.trades.len(), 0);
        assert_eq!(result.bars, 100);
    }

    #[test]
    fn t_performance_structure_correct() {
        let klines: Vec<Kline> = (0..100)
            .map(|i| mk_kline(i, 100.0, 100.0, 101.0, 99.0, 1.0))
            .collect();
        let cfg = BacktestConfig::default();
        let mut pb = HangingScallionsEntryPlaybook;
        let result = run_with_playbook(&klines, &cfg, &mut pb);
        // 基本字段存在
        assert_eq!(result.performance.total_trades, 0);
        assert_eq!(result.performance.win_rate, 0.0);
        assert_eq!(result.bars, 100);
        assert_eq!(result.equity.len(), 100);
        // 初始和末端权益一致（无交易）
        let initial = cfg.initial_capital;
        assert!((result.equity.last().unwrap().equity - initial).abs() < 1e-9);
    }

    #[test]
    fn t_equity_curve_starts_from_initial_capital() {
        let klines: Vec<Kline> = (0..20)
            .map(|i| mk_kline(i, 100.0, 100.0, 101.0, 99.0, 1.0))
            .collect();
        let cfg = BacktestConfig::default();
        let mut pb = HangingScallionsEntryPlaybook;
        let result = run_with_playbook(&klines, &cfg, &mut pb);
        let first_equity = result.equity.first().unwrap().equity;
        assert!((first_equity - cfg.initial_capital).abs() < 1e-9);
    }

    #[test]
    fn t_playbook_signal_executes_on_next_open() {
        let klines = vec![
            mk_kline(0, 100.0, 100.0, 101.0, 99.0, 1.0),
            mk_kline(1, 120.0, 121.0, 122.0, 119.0, 1.0),
            mk_kline(2, 130.0, 130.0, 131.0, 129.0, 1.0),
        ];
        let mut cfg = BacktestConfig::default();
        cfg.fee_bps = 0.0;
        cfg.slippage_bps = 0.0;
        let mut pb = BuyAtIndex { index: 0 };
        let result = run_with_playbook(&klines, &cfg, &mut pb);
        assert_eq!(result.trades.len(), 1);
        assert_eq!(result.trades[0].entry_index, 1);
        assert_eq!(result.trades[0].entry_time, klines[1].open_time);
        assert!((result.trades[0].entry_price - 120.0).abs() < 1e-9);
    }
}
