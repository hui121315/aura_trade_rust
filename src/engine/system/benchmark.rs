//! 体系基准矩阵（M9）
//!
//! 对给定的一批体系 × 多 symbol × 多周期做完整 Walk-Forward 回测，
//! 返回"每个 cell 的 WF 聚合"。前端据此渲染热力图，一眼看出"哪个
//! 体系在什么市场/周期强"。
//!
//! # 性能
//!
//! 借助 M8 的 rayon 并行化：
//! - 外层 `(system × symbol × interval)` 矩阵并行展开
//! - 每个 cell 内部 WF folds 也并行（来自 `walkforward.rs`）
//!
//! 实测 ~15 cells × 4 folds ≈ 150-250ms。

use rayon::prelude::*;
use serde::{Deserialize, Serialize};

use crate::data::Kline;

use super::definition::SystemDefinition;
use super::walkforward::{run_walkforward, WalkForwardConfig};

// ============================================================
// 输入 / 输出
// ============================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkCell {
    pub system_id: String,
    pub system_name: String,
    pub symbol: String,
    pub interval: String,
    /// Walk-forward 指标（失败时全为 NaN）
    pub wf_consistency: f64,
    pub wf_avg_sharpe: f64,
    pub wf_sharpe_std: f64,
    pub wf_avg_return_pct: f64,
    pub total_trades: usize,
    /// 失败原因（成功时为 None）；例如数据不足
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkReport {
    pub cells: Vec<BenchmarkCell>,
    pub folds: usize,
    pub elapsed_ms: u128,
}

/// `klines_for(symbol, interval)` 提供每个 (symbol, interval) 的 K 线
pub fn run_benchmark_with<F>(
    systems: &[SystemDefinition],
    symbols: &[String],
    intervals: &[String],
    folds: usize,
    klines_for: F,
) -> BenchmarkReport
where
    F: Fn(&str, &str) -> Option<Vec<Kline>> + Sync,
{
    let t0 = std::time::Instant::now();

    // 先准备 (symbol, interval, klines) 映射，避免重复拉取
    let keys: Vec<(String, String)> = symbols
        .iter()
        .flat_map(|s| intervals.iter().map(move |tf| (s.clone(), tf.clone())))
        .collect();
    let prepared: Vec<(String, String, Option<Vec<Kline>>)> = keys
        .into_par_iter()
        .map(|(s, tf)| {
            let kl = klines_for(&s, &tf);
            (s, tf, kl)
        })
        .collect();

    // 展开矩阵：每个 system × 每个 (symbol, interval)
    let tasks: Vec<(usize, usize)> = (0..systems.len())
        .flat_map(|i| (0..prepared.len()).map(move |j| (i, j)))
        .collect();

    let cells: Vec<BenchmarkCell> = tasks
        .par_iter()
        .map(|&(i, j)| {
            let def = &systems[i];
            let (sym, tf, kl_opt) = &prepared[j];

            let Some(klines) = kl_opt.as_ref() else {
                return BenchmarkCell {
                    system_id: def.id.clone(),
                    system_name: def.name.clone(),
                    symbol: sym.clone(),
                    interval: tf.clone(),
                    wf_consistency: f64::NAN,
                    wf_avg_sharpe: f64::NAN,
                    wf_sharpe_std: f64::NAN,
                    wf_avg_return_pct: f64::NAN,
                    total_trades: 0,
                    error: Some("K 线拉取失败".into()),
                };
            };

            let wf_cfg = WalkForwardConfig { folds, prewarm_bars: 0 };
            match run_walkforward(def, klines, sym, tf, &wf_cfg) {
                Ok(r) => BenchmarkCell {
                    system_id: def.id.clone(),
                    system_name: def.name.clone(),
                    symbol: sym.clone(),
                    interval: tf.clone(),
                    wf_consistency: r.aggregate.consistency_ratio,
                    wf_avg_sharpe: r.aggregate.avg_sharpe,
                    wf_sharpe_std: r.aggregate.sharpe_std,
                    wf_avg_return_pct: r.aggregate.avg_return_pct,
                    total_trades: r.aggregate.total_trades,
                    error: None,
                },
                Err(e) => BenchmarkCell {
                    system_id: def.id.clone(),
                    system_name: def.name.clone(),
                    symbol: sym.clone(),
                    interval: tf.clone(),
                    wf_consistency: f64::NAN,
                    wf_avg_sharpe: f64::NAN,
                    wf_sharpe_std: f64::NAN,
                    wf_avg_return_pct: f64::NAN,
                    total_trades: 0,
                    error: Some(e),
                },
            }
        })
        .collect();

    BenchmarkReport {
        cells,
        folds,
        elapsed_ms: t0.elapsed().as_millis(),
    }
}

// ============================================================
// 测试
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::system::{all_seeds, find_seed};

    fn mk_kline(i: usize) -> Kline {
        let p = 100.0 + i as f64 * 0.2;
        Kline {
            open_time: i as i64 * 86_400_000,
            close_time: (i as i64 + 1) * 86_400_000,
            open: p,
            high: p + 0.5,
            low: p - 0.2,
            close: p + 0.1,
            volume: 1000.0,
        }
    }

    #[test]
    fn t_benchmark_structure() {
        let systems: Vec<SystemDefinition> = vec![find_seed("seed.main_surge").unwrap()];
        let symbols = vec!["BTC".to_string(), "ETH".to_string()];
        let intervals = vec!["1d".to_string()];

        let klines: Vec<Kline> = (0..1000).map(mk_kline).collect();
        let report = run_benchmark_with(&systems, &symbols, &intervals, 4, |_s, _tf| {
            Some(klines.clone())
        });
        // 1 system × 2 symbol × 1 interval = 2 cells
        assert_eq!(report.cells.len(), 2);
        assert!(report.cells.iter().all(|c| c.error.is_none()));
    }

    #[test]
    fn t_benchmark_handles_missing_klines() {
        let systems = vec![find_seed("seed.main_surge").unwrap()];
        let symbols = vec!["BAD".to_string()];
        let intervals = vec!["1d".to_string()];
        let report = run_benchmark_with(&systems, &symbols, &intervals, 4, |_s, _tf| None);
        assert_eq!(report.cells.len(), 1);
        assert!(report.cells[0].error.is_some());
        assert!(report.cells[0].wf_avg_sharpe.is_nan());
    }

    #[test]
    fn t_benchmark_all_seeds_smoke() {
        let systems = all_seeds();
        let klines: Vec<Kline> = (0..1200).map(mk_kline).collect();
        let report = run_benchmark_with(
            &systems,
            &["BTC".to_string()],
            &["1d".to_string()],
            4,
            |_, _| Some(klines.clone()),
        );
        assert_eq!(report.cells.len(), systems.len());
    }
}
