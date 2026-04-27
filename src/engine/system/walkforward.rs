//! Walk-Forward / 滚动窗口回测（M7）
//!
//! # 目的
//!
//! 单次整段回测容易让人误以为"稳定盈利"——实际上可能只是某一段行情（如 2023-2024
//! 的大牛市）在拉高整体 KPI。**Walk-Forward** 通过把 K 线拆成 N 段**独立回测**，
//! 把每段的结果分别汇报出来，帮助我们回答：
//!
//! - 体系在不同的市场状态（牛/熊/震荡）下表现是否一致？
//! - 各段 Sharpe 的标准差有多大？
//! - 有几段实际盈利（`consistency_ratio`）？
//!
//! # 设计决策
//!
//! - **不做参数优化**（我们的体系是离散组件组合，没有连续超参数）
//! - 每个 fold 独立调用 [`super::runner::run`]，完整重新 warmup + 重新跑 scan
//! - 折数范围 `2..=10`，避免单折数据过少
//! - 返回的 `aggregate` 字段旨在一眼看出"这个体系到底稳不稳"

use serde::{Deserialize, Serialize};

use crate::data::Kline;
use crate::engine::backtest::Performance;

use super::definition::{SystemBacktestResult, SystemDefinition};
use super::runner;

// ============================================================
// 输入 / 输出类型
// ============================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WalkForwardConfig {
    /// 折数（2..=10）
    pub folds: usize,
    /// 可选：跳过起始 N 根（整段预热，避免第一折冷启动）
    #[serde(default)]
    pub prewarm_bars: usize,
}

impl Default for WalkForwardConfig {
    fn default() -> Self {
        Self { folds: 4, prewarm_bars: 0 }
    }
}

/// 单折结果（精简 `SystemBacktestResult` 保留关键 KPI）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WalkForwardFold {
    pub fold_index: usize,
    /// 本折在原始 K 线中的起止索引（左闭右开）
    pub start_bar: usize,
    pub end_bar: usize,
    pub start_time: i64,
    pub end_time: i64,
    pub performance: Performance,
}

/// 聚合指标：衡量"跨 fold 的一致性"
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WalkForwardAggregate {
    /// 各 fold 的平均 Sharpe
    pub avg_sharpe: f64,
    /// Sharpe 的样本标准差（越小越稳定）
    pub sharpe_std: f64,
    /// 平均总收益（pct）
    pub avg_return_pct: f64,
    /// 平均最大回撤（pct）
    pub avg_max_dd_pct: f64,
    /// 平均胜率
    pub avg_win_rate: f64,
    /// 盈利折比例：`#{fold | total_return > 0} / folds`
    ///
    /// - `1.0` = 每折都赚钱（稳健）
    /// - `0.5` = 一半赚一半亏（存疑）
    /// - `0.0` = 完全无效
    pub consistency_ratio: f64,
    /// 累计交易数
    pub total_trades: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WalkForwardReport {
    pub system_id: String,
    pub symbol: String,
    pub interval: String,
    pub config: WalkForwardConfig,
    pub folds: Vec<WalkForwardFold>,
    pub aggregate: WalkForwardAggregate,
}

// ============================================================
// 核心实现
// ============================================================

/// 运行 walk-forward：把 `klines` 均分为 N 折，每折独立跑体系回测
///
/// # 错误
///
/// - `folds < 2` 或 `folds > 10`
/// - `klines` 太短（每折至少要能容纳 `warmup_bars + 20` 根）
pub fn run_walkforward(
    def: &SystemDefinition,
    klines: &[Kline],
    symbol: &str,
    interval: &str,
    cfg: &WalkForwardConfig,
) -> Result<WalkForwardReport, String> {
    if !(2..=10).contains(&cfg.folds) {
        return Err(format!("folds 必须在 2..=10，实际 {}", cfg.folds));
    }
    let warmup = def.backtest.warmup_bars;
    let min_per_fold = warmup + 20;
    let usable = klines.len().saturating_sub(cfg.prewarm_bars);
    if usable < cfg.folds * min_per_fold {
        return Err(format!(
            "K 线太少：{} 根可用 / {} 折，每折至少需要 {} 根",
            usable,
            cfg.folds,
            min_per_fold,
        ));
    }

    let base_len = usable / cfg.folds;

    // M8：各 fold 独立，通过 rayon 并行化。保持结果按 fold_index 顺序。
    use rayon::prelude::*;
    let folds: Vec<WalkForwardFold> = (0..cfg.folds)
        .into_par_iter()
        .map(|i| -> Result<WalkForwardFold, String> {
            let start = cfg.prewarm_bars + i * base_len;
            let end = if i == cfg.folds - 1 {
                klines.len()
            } else {
                cfg.prewarm_bars + (i + 1) * base_len
            };
            let slice = &klines[start..end];
            let result = runner::run(def, slice, symbol, interval)
                .map_err(|e| format!("fold {} 回测失败: {}", i, e))?;
            Ok(WalkForwardFold {
                fold_index: i,
                start_bar: start,
                end_bar: end,
                start_time: slice.first().map(|k| k.open_time).unwrap_or(0),
                end_time: slice.last().map(|k| k.close_time).unwrap_or(0),
                performance: result.performance,
            })
        })
        .collect::<Result<Vec<_>, String>>()?;

    let aggregate = compute_aggregate(&folds);

    Ok(WalkForwardReport {
        system_id: def.id.clone(),
        symbol: symbol.to_string(),
        interval: interval.to_string(),
        config: cfg.clone(),
        folds,
        aggregate,
    })
}

fn compute_aggregate(folds: &[WalkForwardFold]) -> WalkForwardAggregate {
    let n = folds.len() as f64;
    if folds.is_empty() {
        return WalkForwardAggregate {
            avg_sharpe: 0.0,
            sharpe_std: 0.0,
            avg_return_pct: 0.0,
            avg_max_dd_pct: 0.0,
            avg_win_rate: 0.0,
            consistency_ratio: 0.0,
            total_trades: 0,
        };
    }
    let sharpes: Vec<f64> = folds.iter().map(|f| f.performance.sharpe).collect();
    let avg_sharpe: f64 = sharpes.iter().sum::<f64>() / n;
    let sharpe_std: f64 = if folds.len() > 1 {
        let var = sharpes.iter().map(|s| (s - avg_sharpe).powi(2)).sum::<f64>() / (n - 1.0);
        var.sqrt()
    } else {
        0.0
    };
    let avg_return_pct =
        folds.iter().map(|f| f.performance.total_return_pct).sum::<f64>() / n;
    let avg_max_dd_pct =
        folds.iter().map(|f| f.performance.max_drawdown_pct).sum::<f64>() / n;
    let avg_win_rate: f64 =
        folds.iter().map(|f| f.performance.win_rate).sum::<f64>() / n;
    let profitable =
        folds.iter().filter(|f| f.performance.total_return_pct > 0.0).count() as f64;
    let consistency_ratio = profitable / n;
    let total_trades = folds.iter().map(|f| f.performance.total_trades).sum();

    WalkForwardAggregate {
        avg_sharpe,
        sharpe_std,
        avg_return_pct,
        avg_max_dd_pct,
        avg_win_rate,
        consistency_ratio,
        total_trades,
    }
}

// 允许下游通过 `SystemBacktestResult` 间接访问（保留预留 API，防 dead_code）
#[allow(dead_code)]
fn _typecheck(r: SystemBacktestResult) -> Performance {
    r.performance
}

// ============================================================
// 测试
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::Kline;
    use crate::engine::system::{all_seeds, find_seed};

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

    /// 制造 800 根温和上涨数据
    fn uptrend(n: usize) -> Vec<Kline> {
        (0..n)
            .map(|i| {
                let p = 100.0 + i as f64 * 0.3;
                mk_kline(
                    i as i64 * 86_400_000,
                    p,
                    p + 0.5,
                    p - 0.2,
                    p + (i % 3) as f64 * 0.1,
                )
            })
            .collect()
    }

    #[test]
    fn t_walkforward_basic_structure() {
        let def = find_seed("seed.ma_skeleton").unwrap();
        let klines = uptrend(800);
        let cfg = WalkForwardConfig { folds: 4, prewarm_bars: 0 };
        let r = run_walkforward(&def, &klines, "TEST", "1d", &cfg).expect("should succeed");
        assert_eq!(r.folds.len(), 4);
        // 各 fold 索引区间单调递增且不重叠
        for i in 0..r.folds.len() - 1 {
            assert!(r.folds[i].end_bar <= r.folds[i + 1].start_bar);
            assert!(r.folds[i].start_bar < r.folds[i].end_bar);
        }
        // 最后一折 end == klines.len()
        assert_eq!(r.folds.last().unwrap().end_bar, 800);
    }

    #[test]
    fn t_walkforward_rejects_too_few_folds() {
        let def = find_seed("seed.ma_skeleton").unwrap();
        let klines = uptrend(800);
        let cfg = WalkForwardConfig { folds: 1, prewarm_bars: 0 };
        assert!(run_walkforward(&def, &klines, "TEST", "1d", &cfg).is_err());
    }

    #[test]
    fn t_walkforward_rejects_insufficient_bars() {
        let def = find_seed("seed.ma_skeleton").unwrap();
        let klines = uptrend(100); // 远低于 4 × 80
        let cfg = WalkForwardConfig { folds: 4, prewarm_bars: 0 };
        assert!(run_walkforward(&def, &klines, "TEST", "1d", &cfg).is_err());
    }

    #[test]
    fn t_aggregate_consistency_ratio() {
        let def = find_seed("seed.golden_dragon").unwrap();
        let klines = uptrend(1000);
        let cfg = WalkForwardConfig { folds: 4, prewarm_bars: 0 };
        let r = run_walkforward(&def, &klines, "TEST", "1d", &cfg).unwrap();
        // 单调上涨合成数据上，理论上几乎每折都能盈利
        assert!(r.aggregate.consistency_ratio >= 0.0);
        assert!(r.aggregate.consistency_ratio <= 1.0);
        // 总交易数 = 各折之和
        let sum: usize = r.folds.iter().map(|f| f.performance.total_trades).sum();
        assert_eq!(r.aggregate.total_trades, sum);
    }

    #[test]
    fn t_walkforward_all_seeds_smoke() {
        // 冒烟测试：8 个种子体系都能至少不崩溃地跑完 4 折
        let klines = uptrend(1000);
        let cfg = WalkForwardConfig { folds: 4, prewarm_bars: 0 };
        for def in all_seeds() {
            let r = run_walkforward(&def, &klines, "TEST", "1d", &cfg);
            assert!(r.is_ok(), "{} walk-forward 失败: {:?}", def.id, r.err());
        }
    }
}
