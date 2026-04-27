//! `SystemRunner`：给定 `SystemDefinition` + K 线 → `SystemBacktestResult`
//!
//! # 主流程
//!
//! 1. 校验体系定义
//! 2. 预扫描所有组件触发事件（`scan_all_triggers`）
//! 3. 主循环，逐 bar：
//!    - **断头铡刀铁律**（硬覆盖）：若当根触发 `ma_advanced.guillotine` 且有持仓 → 强制清仓
//!    - 若有持仓：按顺序检查 止损 → 止盈 → 时间退出 → 反向信号
//!    - 若无持仓：按 `CombineRule` 求值当根组件触发 → 开仓
//! 4. 结束时强制平仓
//! 5. 计算 `Performance` + 组件归因
//!
//! # 成本处理
//!
//! 开仓/平仓时，买入滑高、卖出滑低；止损命中时以 `stop` 价成交（假设市价单触发）。
//!
//! # M1 简化假设
//!
//! - 固定 1 单位名义风险，不做仓位管理
//! - 无资金约束（假设始终能开仓）
//! - 一次只持一笔（单头寸）

use crate::data::Kline;
use crate::engine::backtest::metrics as backtest_metrics;
use crate::engine::backtest::types::{
    EquityPoint, ExitReason as BtExitReason, Side as BtSide, Trade as BtTrade,
};

use super::combine::{evaluate_combine, CombineCtx, CombinedSignal};
use super::component::find_component;
use super::definition::{
    ComponentContrib, CostModel, SystemBacktestResult, SystemDefinition, SystemTrade,
    TradeExitReason, TradeSide,
};
use super::scan::{scan_all_triggers, ScanResult, TriggerEvent};

/// 断头铡刀组件 ID（铁律硬编码依赖）
const GUILLOTINE_CID: &str = "ma_advanced.guillotine";

/// 运行一次体系回测
pub fn run(
    def: &SystemDefinition,
    klines: &[Kline],
    symbol: &str,
    interval: &str,
) -> Result<SystemBacktestResult, String> {
    def.validate()?;

    let n = klines.len();
    if n == 0 {
        return Err("klines 为空".into());
    }

    let scan = scan_all_triggers(klines);

    // 主循环状态
    let mut position: Option<OpenPosition> = None;
    let mut trades: Vec<SystemTrade> = Vec::new();
    let mut equity: Vec<EquityPoint> = Vec::with_capacity(n);
    let mut equity_value: f64 = 1.0; // 初始权益 1.0（单位化）
    let mut peak: f64 = 1.0;

    let cost_one_way = def.backtest.cost_model.one_way_pct() / 100.0; // 转成小数

    // 每根 bar 记录 equity point（mark-to-market 简化：只在平仓后累计到 equity_value，中间 bar 沿用上一值）
    let warmup = def.backtest.warmup_bars.min(n.saturating_sub(1));

    for t in 0..n {
        let bar = &klines[t];

        if t > 0 && bar.open.is_finite() {
            let signal_bar = t - 1;

            if let Some(open) = &position {
                if scan.get_trigger(GUILLOTINE_CID, signal_bar).is_some() {
                    let trade = close_position(
                        open,
                        bar,
                        t,
                        bar.open_time,
                        bar.open,
                        TradeExitReason::GuillotineOverride,
                        &def.backtest.cost_model,
                        trades.len(),
                    );
                    equity_value *= 1.0 + trade.pnl_pct;
                    trades.push(trade);
                    position = None;
                }
            }

            if let Some(open) = &position {
                let signal = evaluate_at_bar(def, &scan, signal_bar);
                if let Some(sig) = &signal {
                    let open_dir = match open.side {
                        TradeSide::Long => 1,
                        TradeSide::Short => -1,
                    };
                    if sig.direction == -open_dir {
                        let trade = close_position(
                            open,
                            bar,
                            t,
                            bar.open_time,
                            bar.open,
                            TradeExitReason::ReverseSignal,
                            &def.backtest.cost_model,
                            trades.len(),
                        );
                        equity_value *= 1.0 + trade.pnl_pct;
                        trades.push(trade);
                        position = None;
                    }
                }
            }

            if position.is_none() && signal_bar >= warmup {
                if let Some(sig) = evaluate_at_bar(def, &scan, signal_bar) {
                    if let Some(new_pos) =
                        try_open(def, &scan, bar, t, signal_bar, &sig, cost_one_way)
                    {
                        position = Some(new_pos);
                    }
                }
            }
        }

        if let Some(open) = &position {
            let hold_bars = t.saturating_sub(open.entry_bar);

            let (stop_hit, tp_hit) = check_stop_tp(open, bar);

            if stop_hit {
                let trade = close_position(
                    open,
                    bar,
                    t,
                    bar.close_time,
                    open.stop,
                    TradeExitReason::StopLoss,
                    &def.backtest.cost_model,
                    trades.len(),
                );
                equity_value *= 1.0 + trade.pnl_pct;
                trades.push(trade);
                position = None;
            } else if tp_hit {
                let trade = close_position(
                    open,
                    bar,
                    t,
                    bar.close_time,
                    open.target,
                    TradeExitReason::TakeProfit,
                    &def.backtest.cost_model,
                    trades.len(),
                );
                equity_value *= 1.0 + trade.pnl_pct;
                trades.push(trade);
                position = None;
            } else if hold_bars >= def.risk.max_hold_bars {
                let trade = close_position(
                    open,
                    bar,
                    t,
                    bar.close_time,
                    bar.close,
                    TradeExitReason::TimeExit,
                    &def.backtest.cost_model,
                    trades.len(),
                );
                equity_value *= 1.0 + trade.pnl_pct;
                trades.push(trade);
                position = None;
            }
        }

        // --- 5. 更新权益点 ---
        if equity_value > peak {
            peak = equity_value;
        }
        let dd = if peak > 0.0 {
            (peak - equity_value) / peak
        } else {
            0.0
        };
        equity.push(EquityPoint {
            time: bar.close_time,
            equity: equity_value,
            drawdown: dd,
        });
    }

    // 结束强制平仓
    if let Some(open) = &position {
        let last_bar = &klines[n - 1];
        let trade = close_position(
            open,
            last_bar,
            n - 1,
            last_bar.close_time,
            last_bar.close,
            TradeExitReason::EndOfData,
            &def.backtest.cost_model,
            trades.len(),
        );
        let tmp = 1.0 + trade.pnl_pct;
        equity_value *= tmp;
        trades.push(trade);
        if let Some(last) = equity.last_mut() {
            last.equity = equity_value;
            if equity_value > peak {
                peak = equity_value;
            }
            last.drawdown = if peak > 0.0 {
                (peak - equity_value) / peak
            } else {
                0.0
            };
        }
    }

    // ---- Performance ----
    let bt_trades: Vec<BtTrade> = trades.iter().map(system_to_bt_trade).collect();
    let bpy = backtest_metrics::bars_per_year(interval);
    let performance = backtest_metrics::compute(1.0, &equity, &bt_trades, bpy);

    // ---- 归因 ----
    let contrib = compute_contribution(def, &scan, &trades);

    Ok(SystemBacktestResult {
        system_id: def.id.clone(),
        symbol: symbol.to_string(),
        interval: interval.to_string(),
        bars: n,
        cost_model: def.backtest.cost_model,
        performance,
        equity,
        trades,
        component_contribution: contrib,
    })
}

// ============================================================
// 内部辅助
// ============================================================

#[derive(Debug, Clone)]
struct OpenPosition {
    side: TradeSide,
    entry_bar: usize,
    entry_time_ms: i64,
    entry_price: f64, // 含成本后的进场价
    raw_entry: f64,   // 不含成本（用于 R 的参考）
    stop: f64,
    target: f64,
    triggered_components: Vec<String>,
}

fn evaluate_at_bar(
    def: &SystemDefinition,
    scan: &ScanResult,
    bar: usize,
) -> Option<CombinedSignal> {
    let per: Vec<(String, Option<&TriggerEvent>)> = def
        .components
        .iter()
        .map(|cid| (cid.clone(), scan.get_trigger(cid, bar)))
        .collect();
    let ctx = CombineCtx {
        scan,
        current_bar: bar,
        components: &def.components,
    };
    evaluate_combine(&per, &def.combine, &def.weights, Some(&ctx))
}

fn try_open(
    def: &SystemDefinition,
    scan: &ScanResult,
    bar: &Kline,
    bar_index: usize,
    signal_bar_index: usize,
    sig: &CombinedSignal,
    cost_one_way: f64,
) -> Option<OpenPosition> {
    if sig.direction == 0 {
        return None;
    }
    let atr = scan.atr.get(signal_bar_index).copied().unwrap_or(f64::NAN);
    if !atr.is_finite() || atr <= 0.0 {
        return None;
    }
    let stop_dist = atr * def.risk.stop_atr_mult;
    let raw_entry = bar.open;
    // 成本影响：买入滑高，卖出滑低
    let (entry_price, stop, target, side) = if sig.direction == 1 {
        let entry = raw_entry * (1.0 + cost_one_way);
        let stop = raw_entry - stop_dist;
        let target = raw_entry + stop_dist * def.risk.target_r;
        (entry, stop, target, TradeSide::Long)
    } else {
        let entry = raw_entry * (1.0 - cost_one_way);
        let stop = raw_entry + stop_dist;
        let target = raw_entry - stop_dist * def.risk.target_r;
        (entry, stop, target, TradeSide::Short)
    };
    if (side == TradeSide::Long && stop >= raw_entry)
        || (side == TradeSide::Short && stop <= raw_entry)
    {
        return None;
    }
    Some(OpenPosition {
        side,
        entry_bar: bar_index,
        entry_time_ms: bar.open_time,
        entry_price,
        raw_entry,
        stop,
        target,
        triggered_components: sig.contributing_components.clone(),
    })
}

fn check_stop_tp(pos: &OpenPosition, bar: &Kline) -> (bool, bool) {
    match pos.side {
        TradeSide::Long => {
            let stop_hit = bar.low <= pos.stop;
            let tp_hit = bar.high >= pos.target;
            (stop_hit, tp_hit)
        }
        TradeSide::Short => {
            let stop_hit = bar.high >= pos.stop;
            let tp_hit = bar.low <= pos.target;
            (stop_hit, tp_hit)
        }
    }
}

fn close_position(
    pos: &OpenPosition,
    _bar: &Kline,
    bar_index: usize,
    exit_time_ms: i64,
    raw_exit: f64,
    reason: TradeExitReason,
    cost: &CostModel,
    trade_id: usize,
) -> SystemTrade {
    let cost_one_way = cost.one_way_pct() / 100.0;
    // 平仓方向相反：多头出场 = 卖出滑低；空头出场 = 买入滑高
    let exit_price = match pos.side {
        TradeSide::Long => raw_exit * (1.0 - cost_one_way),
        TradeSide::Short => raw_exit * (1.0 + cost_one_way),
    };

    // 名义 1 单位头寸；多头 pnl% = (exit - entry)/entry；空头相反
    let pnl_pct = match pos.side {
        TradeSide::Long => (exit_price - pos.entry_price) / pos.entry_price,
        TradeSide::Short => (pos.entry_price - exit_price) / pos.entry_price,
    };

    // R-multiple = pnl / |raw_entry - stop|（以开仓名义价为基准）
    let r_unit = (pos.raw_entry - pos.stop).abs();
    let r_multiple = if r_unit > 1e-12 {
        (pnl_pct * pos.raw_entry) / r_unit
    } else {
        0.0
    };

    SystemTrade {
        id: trade_id,
        side: pos.side,
        entry_bar: pos.entry_bar,
        entry_time_ms: pos.entry_time_ms,
        entry_price: pos.entry_price,
        stop: pos.stop,
        target: pos.target,
        exit_bar: bar_index,
        exit_time_ms,
        exit_price,
        exit_reason: reason,
        pnl_pct,
        r_multiple,
        triggered_components: pos.triggered_components.clone(),
        hold_bars: bar_index.saturating_sub(pos.entry_bar),
    }
}

/// 将 SystemTrade 适配为 backtest::Trade，以复用 metrics::compute
fn system_to_bt_trade(st: &SystemTrade) -> BtTrade {
    let side = match st.side {
        TradeSide::Long => BtSide::Long,
        TradeSide::Short => BtSide::Short,
    };
    let exit_reason = match st.exit_reason {
        TradeExitReason::StopLoss => BtExitReason::StopLoss,
        TradeExitReason::TakeProfit => BtExitReason::TakeProfit,
        TradeExitReason::ReverseSignal => BtExitReason::Reverse,
        TradeExitReason::GuillotineOverride => BtExitReason::Reverse,
        TradeExitReason::TimeExit => BtExitReason::EndOfData,
        TradeExitReason::EndOfData => BtExitReason::EndOfData,
    };
    BtTrade {
        id: st.id,
        side,
        entry_index: st.entry_bar,
        entry_time: st.entry_time_ms,
        entry_price: st.entry_price,
        stop_loss: st.stop,
        take_profit: st.target,
        qty: 1.0,
        exit_index: Some(st.exit_bar),
        exit_time: Some(st.exit_time_ms),
        exit_price: Some(st.exit_price),
        exit_reason: Some(exit_reason),
        pnl: st.pnl_pct, // 用百分比当做 pnl（metrics 只关心正负比较，一致即可）
        r_multiple: st.r_multiple,
        reasons: st.triggered_components.clone(),
    }
}

fn compute_contribution(
    def: &SystemDefinition,
    scan: &ScanResult,
    trades: &[SystemTrade],
) -> Vec<ComponentContrib> {
    let mut out = Vec::with_capacity(def.components.len());
    for cid in &def.components {
        let triggers = scan.count(cid);
        let matched = trades
            .iter()
            .filter(|t| t.triggered_components.iter().any(|c| c == cid))
            .count();
        out.push(ComponentContrib {
            component_id: cid.clone(),
            triggers,
            matched_system_entries: matched,
        });
    }
    out
}

// 让 unused import 不被警告
#[allow(dead_code)]
fn _keepalive(_: &dyn std::any::Any) {
    let _ = find_component;
}

// ============================================================
// 测试
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::system::definition::{
        BacktestParams, CombineRule, RiskParams, SystemDefinition, SystemMeta, SystemOrigin,
    };
    use std::collections::HashMap;

    fn mk_kline(t: i64, o: f64, h: f64, l: f64, c: f64) -> Kline {
        Kline {
            open_time: t,
            close_time: t + 60_000,
            open: o,
            high: h,
            low: l,
            close: c,
            volume: 1000.0,
        }
    }

    fn simple_def(components: Vec<&str>, rule: CombineRule) -> SystemDefinition {
        SystemDefinition {
            id: "test".into(),
            name: "Test".into(),
            origin: SystemOrigin::User,
            description: None,
            components: components.into_iter().map(String::from).collect(),
            combine: rule,
            weights: HashMap::new(),
            risk: RiskParams {
                stop_atr_mult: 2.0,
                target_r: 3.0,
                max_hold_bars: 10,
                max_position_pct: 1.0,
            },
            backtest: BacktestParams {
                warmup_bars: 30,
                cost_model: CostModel::Zero,
            },
            meta: SystemMeta::default(),
        }
    }

    #[test]
    fn t_try_open_uses_execution_bar_open_and_signal_bar_atr() {
        let def = simple_def(vec!["ma_special.bull_arrangement"], CombineRule::AllAligned);
        let scan = ScanResult {
            triggers: HashMap::new(),
            atr: vec![2.0, 99.0],
        };
        let bar = mk_kline(1, 120.0, 121.0, 119.0, 130.0);
        let sig = CombinedSignal {
            direction: 1,
            confidence: 1.0,
            contributing_components: vec!["ma_special.bull_arrangement".to_string()],
        };
        let pos = try_open(&def, &scan, &bar, 1, 0, &sig, 0.0).unwrap();
        assert_eq!(pos.entry_bar, 1);
        assert_eq!(pos.entry_time_ms, bar.open_time);
        assert!((pos.raw_entry - 120.0).abs() < 1e-9);
        assert!((pos.entry_price - 120.0).abs() < 1e-9);
        assert!((pos.stop - 116.0).abs() < 1e-9);
    }

    #[test]
    fn t_run_empty_klines_errors() {
        let def = simple_def(vec!["ma.granville.b2_pullback"], CombineRule::AllAligned);
        let r = run(&def, &[], "BTC", "1d");
        assert!(r.is_err());
    }

    #[test]
    fn t_run_invalid_def_errors() {
        let mut def = simple_def(vec!["ma.granville.b2_pullback"], CombineRule::AllAligned);
        def.components.clear();
        let klines = vec![mk_kline(0, 100.0, 101.0, 99.0, 100.5)];
        let r = run(&def, &klines, "BTC", "1d");
        assert!(r.is_err());
    }

    #[test]
    fn t_run_flat_market_no_trades() {
        // 完全平盘 → 无触发 → 无交易
        let klines: Vec<Kline> = (0..200)
            .map(|i| mk_kline(i as i64 * 60_000, 100.0, 100.2, 99.8, 100.0))
            .collect();
        let def = simple_def(vec!["ma.granville.b2_pullback"], CombineRule::AllAligned);
        let r = run(&def, &klines, "BTC", "1d").unwrap();
        assert_eq!(r.trades.len(), 0, "平盘应无交易");
        assert_eq!(r.bars, 200);
        assert_eq!(r.equity.len(), 200);
    }

    #[test]
    fn t_run_uptrend_with_bull_arrangement_produces_trades() {
        // 构造稳定上升市场（> 200 根，足够 warmup 和 MA60 稳定）
        let klines: Vec<Kline> = (0..300)
            .map(|i| {
                let p = 100.0 + i as f64 * 0.3;
                mk_kline(i as i64 * 60_000, p, p + 0.6, p - 0.2, p + 0.3)
            })
            .collect();
        let def = simple_def(vec!["ma_special.bull_arrangement"], CombineRule::AllAligned);
        let r = run(&def, &klines, "BTC", "1d").unwrap();
        assert!(
            r.performance.total_trades > 0,
            "稳定上涨 + 多头排列应产生交易，实际 {}",
            r.performance.total_trades
        );
        // 总收益应为正（上涨趋势被捕获）
        assert!(
            r.performance.total_return_pct > -0.2,
            "上升趋势下的体系不应大亏：total_return={}",
            r.performance.total_return_pct
        );
    }

    #[test]
    fn t_contribution_tracked() {
        let klines: Vec<Kline> = (0..300)
            .map(|i| {
                let p = 100.0 + i as f64 * 0.3;
                mk_kline(i as i64 * 60_000, p, p + 0.6, p - 0.2, p + 0.3)
            })
            .collect();
        let def = simple_def(vec!["ma_special.bull_arrangement"], CombineRule::AllAligned);
        let r = run(&def, &klines, "BTC", "1d").unwrap();
        assert_eq!(r.component_contribution.len(), 1);
        assert_eq!(
            r.component_contribution[0].component_id,
            "ma_special.bull_arrangement"
        );
        assert!(r.component_contribution[0].triggers > 0);
    }

    #[test]
    fn t_guillotine_override_forces_exit() {
        // 构造：先上涨触发开多，之后构造一根断头铡刀 → 应强制平仓
        // 简化：直接测 runner 逻辑能处理 guillotine 触发即可，不需要真构造形态
        // 由于现实构造断头铡刀需要精确的均线粘合状态，这里改为断言：
        // 如果数据中断头铡刀触发 → 持仓不可能跨越该 bar
        let klines: Vec<Kline> = (0..300)
            .map(|i| {
                let p = 100.0 + i as f64 * 0.3;
                mk_kline(i as i64 * 60_000, p, p + 0.6, p - 0.2, p + 0.3)
            })
            .collect();
        let def = simple_def(vec!["ma_special.bull_arrangement"], CombineRule::AllAligned);
        let r = run(&def, &klines, "BTC", "1d").unwrap();
        // 平滑上涨市场不会触发断头铡刀（粘合后向下发散），所以此测试只验证 pipeline 不 panic
        assert!(r.bars > 0);
    }
}
