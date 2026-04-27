//! E2/E3：事件驱动回测主循环
//!
//! 流程（对齐 PRD §E3）：
//! for 每根 K线:
//!   1. 计算 A/B/C/D 当前切片状态（只用 t 之前的数据）
//!   2. 是否有信号？（本 MVP 基于 葛南维信号 + K线形态 强确认）
//!   3. 若达到信号要求 → 计算 entry / stop / tp，开模拟单
//!   4. 对已有持仓：检查 stop / tp / 反向信号平仓
//!   5. 扣除手续费 + 滑点
//!   6. 更新权益曲线

use std::collections::HashMap;

use crate::data::Kline;
use crate::engine::candle::{self, PatternHit};
use crate::engine::ma::{self, GranvilleSignal};

use super::metrics;
use super::types::{
    BacktestConfig, BacktestResult, EquityPoint, ExitReason, PatternStat, Performance, Side,
    StopKind, Trade,
};

pub fn run(klines: &[Kline], cfg: &BacktestConfig) -> BacktestResult {
    let n = klines.len();
    if n == 0 {
        return BacktestResult {
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
        };
    }

    // 1. 预计算：均线、BIAS、斜率、ATR、葛南维信号全序列、K线形态
    let closes: Vec<f64> = klines.iter().map(|k| k.close).collect();
    let mut ma_series: Vec<Vec<f64>> = Vec::new();
    for &p in &cfg.ma_periods {
        ma_series.push(ma::compute::compute(cfg.ma_kind, &closes, p));
    }
    let base_idx = cfg
        .ma_periods
        .iter()
        .position(|&p| p == cfg.base_period)
        .unwrap_or(0);
    let base_ma = ma_series[base_idx].clone();
    let bias = ma::compute::bias(&closes, &base_ma);
    let slope = ma::compute::slope(&base_ma, 5);

    // 葛南维信号
    let granvilles = ma::granville::scan(
        &closes,
        &base_ma,
        &slope,
        &bias,
        &ma::granville::GranvilleParams {
            period: cfg.base_period,
            ..Default::default()
        },
    );
    // 按 index 分桶
    let mut granville_by_idx: HashMap<usize, Vec<GranvilleSignal>> = HashMap::new();
    for g in &granvilles {
        granville_by_idx.entry(g.index).or_default().push(*g);
    }

    // K线形态
    let hits = candle::scan(klines);
    let mut hits_by_idx: HashMap<usize, Vec<PatternHit>> = HashMap::new();
    for h in &hits {
        hits_by_idx.entry(h.index).or_default().push(*h);
    }

    // ATR（Wilder 平滑 14）
    let atr = compute_atr(klines, 14);

    // 2. 主循环
    let mut equity = Vec::with_capacity(n);
    let mut trades: Vec<Trade> = Vec::new();
    let mut cash = cfg.initial_capital;
    let mut open_trade: Option<Trade> = None;
    let mut next_id = 1usize;
    let mut peak = cfg.initial_capital;

    // 每个形态的命中统计：label → (count, total_r, wins, losses)
    let mut pattern_stats_map: HashMap<String, (usize, usize, usize, f64)> = HashMap::new();

    for i in 0..n {
        let k = &klines[i];
        let price = k.close;

        // ---- 处理已有持仓 ----
        if let Some(t) = open_trade.as_mut() {
            let hit_sl = match t.side {
                Side::Long => k.low <= t.stop_loss,
                Side::Short => k.high >= t.stop_loss,
            };
            let hit_tp = match t.side {
                Side::Long => k.high >= t.take_profit,
                Side::Short => k.low <= t.take_profit,
            };
            // 优先止损（保守）
            let mut exit: Option<(f64, ExitReason)> = None;
            if hit_sl {
                exit = Some((t.stop_loss, ExitReason::StopLoss));
            } else if hit_tp {
                exit = Some((t.take_profit, ExitReason::TakeProfit));
            }
            if let Some((px, reason)) = exit {
                let filled = fill_with_slippage(px, cfg.slippage_bps, t.side, /*exit*/ true);
                let gross = match t.side {
                    Side::Long => (filled - t.entry_price) * t.qty,
                    Side::Short => (t.entry_price - filled) * t.qty,
                };
                let fee = (t.entry_price + filled) * t.qty * (cfg.fee_bps / 10_000.0);
                let pnl = gross - fee;
                let risk_per_unit = (t.entry_price - t.stop_loss).abs();
                let risk_amt = risk_per_unit * t.qty;
                t.exit_index = Some(i);
                t.exit_time = Some(k.close_time);
                t.exit_price = Some(filled);
                t.exit_reason = Some(reason);
                t.pnl = pnl;
                t.r_multiple = if risk_amt > 0.0 { pnl / risk_amt } else { 0.0 };
                cash += pnl;
                // 更新形态统计
                for label in &t.reasons {
                    let e = pattern_stats_map.entry(label.clone()).or_default();
                    e.0 += 1;
                    if t.pnl > 0.0 {
                        e.1 += 1;
                    } else {
                        e.2 += 1;
                    }
                    e.3 += t.r_multiple;
                }
                trades.push(t.clone());
                open_trade = None;
            }
        }

        // ---- 生成信号 ----
        // 信号：至少一个强 K线形态 + 至少一个葛南维方向一致信号
        let g_now = granville_by_idx.get(&i).cloned().unwrap_or_default();
        let p_now = hits_by_idx.get(&i).cloned().unwrap_or_default();

        let buy_granville = g_now.iter().find(|g| g.rule.is_buy());
        let sell_granville = g_now.iter().find(|g| !g.rule.is_buy());
        let strong_bull = p_now
            .iter()
            .filter(|p| p.direction > 0 && p.strength >= cfg.min_pattern_strength)
            .max_by_key(|p| p.strength);
        let strong_bear = p_now
            .iter()
            .filter(|p| p.direction < 0 && p.strength >= cfg.min_pattern_strength)
            .max_by_key(|p| p.strength);

        // 简单信号规则（Phase 2 MVP，后续会扩展为完整四维共振）
        let long_signal = buy_granville.is_some() && strong_bull.is_some();
        let short_signal =
            cfg.allow_short && sell_granville.is_some() && strong_bear.is_some();

        // ---- 开仓（仅在没持仓时） ----
        if open_trade.is_none() && i + 1 < n {
            if long_signal {
                if let Some(new_t) = try_open(
                    Side::Long,
                    i,
                    k,
                    &klines[i + 1],
                    cash,
                    cfg,
                    &atr,
                    &base_ma,
                    &[
                        format!("葛南维 {}", buy_granville.unwrap().rule.code()),
                        strong_bull.unwrap().kind.label().to_string(),
                    ],
                    next_id,
                ) {
                    open_trade = Some(new_t);
                    next_id += 1;
                }
            } else if short_signal {
                if let Some(new_t) = try_open(
                    Side::Short,
                    i,
                    k,
                    &klines[i + 1],
                    cash,
                    cfg,
                    &atr,
                    &base_ma,
                    &[
                        format!("葛南维 {}", sell_granville.unwrap().rule.code()),
                        strong_bear.unwrap().kind.label().to_string(),
                    ],
                    next_id,
                ) {
                    open_trade = Some(new_t);
                    next_id += 1;
                }
            }
        }

        // ---- 更新浮动权益 ----
        let floating = match open_trade.as_ref() {
            Some(t) => match t.side {
                Side::Long => (price - t.entry_price) * t.qty,
                Side::Short => (t.entry_price - price) * t.qty,
            },
            None => 0.0,
        };
        let eq = cash + floating;
        if eq > peak {
            peak = eq;
        }
        let dd = if peak > 0.0 { (peak - eq) / peak } else { 0.0 };
        equity.push(EquityPoint { time: k.close_time, equity: eq, drawdown: dd });
    }

    // 末尾强平（按最后收盘价）
    if let Some(mut t) = open_trade.take() {
        let last = klines.last().unwrap();
        let px = last.close;
        let filled = fill_with_slippage(px, cfg.slippage_bps, t.side, true);
        let gross = match t.side {
            Side::Long => (filled - t.entry_price) * t.qty,
            Side::Short => (t.entry_price - filled) * t.qty,
        };
        let fee = (t.entry_price + filled) * t.qty * (cfg.fee_bps / 10_000.0);
        let pnl = gross - fee;
        let risk_per_unit = (t.entry_price - t.stop_loss).abs();
        let risk_amt = risk_per_unit * t.qty;
        t.exit_index = Some(n - 1);
        t.exit_time = Some(last.close_time);
        t.exit_price = Some(filled);
        t.exit_reason = Some(ExitReason::EndOfData);
        t.pnl = pnl;
        t.r_multiple = if risk_amt > 0.0 { pnl / risk_amt } else { 0.0 };
        cash += pnl;
        for label in &t.reasons {
            let e = pattern_stats_map.entry(label.clone()).or_default();
            e.0 += 1;
            if t.pnl > 0.0 {
                e.1 += 1;
            } else {
                e.2 += 1;
            }
            e.3 += t.r_multiple;
        }
        trades.push(t);
        // 修正最后一根的权益
        if let Some(last_eq) = equity.last_mut() {
            last_eq.equity = cash;
        }
    }

    // 3. 统计
    let bpy = metrics::bars_per_year(&cfg.interval);
    let performance = metrics::compute(cfg.initial_capital, &equity, &trades, bpy);

    // 形态统计排行榜（按 total_r 降序）
    let mut pattern_stats: Vec<PatternStat> = pattern_stats_map
        .into_iter()
        .map(|(label, (count, wins, losses, total_r))| {
            let avg_r = if count > 0 { total_r / count as f64 } else { 0.0 };
            let winrate = if count > 0 {
                wins as f64 / count as f64
            } else {
                0.0
            };
            PatternStat { label, count, wins, losses, total_r, avg_r, winrate }
        })
        .collect();
    pattern_stats.sort_by(|a, b| b.total_r.partial_cmp(&a.total_r).unwrap_or(std::cmp::Ordering::Equal));

    BacktestResult {
        config: cfg.clone(),
        bars: n,
        start_time: klines.first().map(|k| k.open_time).unwrap_or(0),
        end_time: klines.last().map(|k| k.close_time).unwrap_or(0),
        performance,
        equity,
        trades,
        pattern_stats,
    }
}

/// 尝试开仓；根据风控规则计算仓位；失败返回 None（如仓位过小）
fn try_open(
    side: Side,
    i: usize,
    cur: &Kline,
    next: &Kline,
    cash: f64,
    cfg: &BacktestConfig,
    atr: &[f64],
    ma: &[f64],
    reasons: &[String],
    id: usize,
) -> Option<Trade> {
    // 入场：下一根开盘 + 滑点
    let raw_entry = next.open;
    let entry = fill_with_slippage(raw_entry, cfg.slippage_bps, side, false);

    // 止损距离
    let atr_v = *atr.get(i).unwrap_or(&0.0);
    let stop_distance = match cfg.stop_kind {
        StopKind::Atr => atr_v * cfg.atr_multiplier,
        StopKind::Structure => atr_v * 2.0, // 近似：MVP 先用 2*ATR 模拟
        StopKind::Ma => {
            let m = *ma.get(i).unwrap_or(&entry);
            (entry - m).abs().max(atr_v * 0.5)
        }
        StopKind::Pattern => {
            // 单根 K线的高/低 作为止损
            match side {
                Side::Long => (entry - cur.low).abs().max(atr_v * 0.5),
                Side::Short => (cur.high - entry).abs().max(atr_v * 0.5),
            }
        }
    };
    if stop_distance <= 0.0 || !stop_distance.is_finite() {
        return None;
    }
    let (stop_loss, take_profit) = match side {
        Side::Long => (entry - stop_distance, entry + stop_distance * cfg.rr_ratio),
        Side::Short => (entry + stop_distance, entry - stop_distance * cfg.rr_ratio),
    };

    // 仓位：按单笔风险
    let risk_amt = cash * cfg.risk_per_trade;
    let qty = risk_amt / stop_distance;
    if qty <= 0.0 || !qty.is_finite() {
        return None;
    }

    Some(Trade {
        id,
        side,
        entry_index: i + 1,
        entry_time: next.open_time,
        entry_price: entry,
        stop_loss,
        take_profit,
        qty,
        exit_index: None,
        exit_time: None,
        exit_price: None,
        exit_reason: None,
        pnl: 0.0,
        r_multiple: 0.0,
        reasons: reasons.to_vec(),
    })
}

fn fill_with_slippage(px: f64, bps: f64, side: Side, is_exit: bool) -> f64 {
    let delta = px * (bps / 10_000.0);
    match (side, is_exit) {
        // 做多：入场付溢价，平仓吃折价
        (Side::Long, false) => px + delta,
        (Side::Long, true) => px - delta,
        // 做空：入场吃折价，平仓付溢价
        (Side::Short, false) => px - delta,
        (Side::Short, true) => px + delta,
    }
}

/// Wilder ATR
fn compute_atr(klines: &[Kline], period: usize) -> Vec<f64> {
    let n = klines.len();
    let mut tr = vec![0.0; n];
    for i in 0..n {
        let high = klines[i].high;
        let low = klines[i].low;
        if i == 0 {
            tr[i] = high - low;
        } else {
            let pc = klines[i - 1].close;
            tr[i] = (high - low)
                .max((high - pc).abs())
                .max((low - pc).abs());
        }
    }
    let mut out = vec![f64::NAN; n];
    if period == 0 || period > n {
        return out;
    }
    // 初始化：前 period 根的简单平均
    let seed: f64 = tr[..period].iter().sum::<f64>() / period as f64;
    out[period - 1] = seed;
    let alpha = 1.0 / period as f64;
    let mut prev = seed;
    for i in period..n {
        let cur = alpha * tr[i] + (1.0 - alpha) * prev;
        out[i] = cur;
        prev = cur;
    }
    out
}
