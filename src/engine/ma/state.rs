//! A1–A5 的汇总出口：`compute_ma_state`
//!
//! 对应 `/api/ma_state` 端点的返回体结构。

use serde::{Deserialize, Serialize};

use crate::data::Kline;

use super::alignment::{self, Alignment, Cross};
use super::compute::{self, MaKind};
use super::granville::{self, GranvilleParams, GranvilleSignal};

/// API 返回：完整的均线状态快照
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MaState {
    pub symbol: String,
    pub timeframe: String,
    pub kind: MaKind,
    /// 所有均线周期（如 [5, 10, 20, 60, 120, 250]）
    pub periods: Vec<usize>,
    /// 每条均线的末值
    pub last_values: Vec<f64>,
    /// 每条均线的完整序列（与 K线等长）
    pub series: Vec<Vec<f64>>,
    /// 当前排列状态
    pub alignment: Alignment,
    /// 排列状态别名（原书多重命名）
    pub alignment_aliases: Vec<String>,
    /// 是否处于粘合 / 收敛 / 发散
    pub spread_state: Option<Alignment>,
    /// 以基准周期的 BIAS
    pub bias_base: f64,
    pub bias_base_period: usize,
    /// 每条均线的斜率（最后一根）
    pub slopes: Vec<f64>,
    /// 交叉事件（全序列扫描）
    pub crosses: Vec<Cross>,
    /// 葛南维信号
    pub granville: Vec<GranvilleSignal>,
    /// 基准均线的最新价位置描述（above / below / near）
    pub price_vs_base: &'static str,
}

/// 计算入口
pub fn compute_ma_state(
    symbol: &str,
    timeframe: &str,
    kind: MaKind,
    klines: &[Kline],
    periods: &[usize],
) -> MaState {
    let closes: Vec<f64> = klines.iter().map(|k| k.close).collect();

    // 1. 算出每条均线序列
    let mut series: Vec<Vec<f64>> = Vec::with_capacity(periods.len());
    for &p in periods {
        series.push(compute::compute(kind, &closes, p));
    }

    let last = closes.len().saturating_sub(1);

    // 2. 末值 + 斜率
    let last_values: Vec<f64> = series.iter().map(|s| *s.last().unwrap_or(&f64::NAN)).collect();
    let slopes: Vec<f64> = series
        .iter()
        .map(|s| {
            let slope_series = compute::slope(s, 5);
            *slope_series.last().unwrap_or(&f64::NAN)
        })
        .collect();

    // 3. 排列状态
    let stack_refs: Vec<&[f64]> = series.iter().map(|v| v.as_slice()).collect();
    let alignment = alignment::classify(&stack_refs, last, 0.005);
    let spread_state = alignment::spread_trend(&stack_refs, last, 20);

    // 4. 选一条基准均线：优先 MA30（新 PRD 默认），其次 MA20，最后第一条
    let base_idx = periods.iter().position(|&p| p == 30)
        .or_else(|| periods.iter().position(|&p| p == 20))
        .unwrap_or(0);
    let base_series = &series[base_idx];
    let base_period = periods[base_idx];
    let bias_series = compute::bias(&closes, base_series);
    let bias_base = *bias_series.last().unwrap_or(&f64::NAN);
    let slope_series = compute::slope(base_series, 5);

    // 5. 交叉事件（遍历所有相邻周期对）
    let mut crosses = Vec::new();
    for i in 0..periods.len() {
        for j in (i + 1)..periods.len() {
            let cs = alignment::find_crosses(&series[i], &series[j], periods[i], periods[j]);
            crosses.extend(cs);
        }
    }
    // 只保留最近 50 根内的交叉，避免 API 过大
    let cutoff = last.saturating_sub(50);
    crosses.retain(|c| c.index >= cutoff);
    crosses.sort_by_key(|c| c.index);

    // 6. 葛南维信号
    let granville = granville::scan(
        &closes,
        base_series,
        &slope_series,
        &bias_series,
        &GranvilleParams { period: base_period, ..Default::default() },
    );
    let granville_recent: Vec<GranvilleSignal> = granville
        .into_iter()
        .filter(|g| g.index >= cutoff)
        .collect();

    // 7. 当前价格相对基准的位置
    let price_vs_base = {
        let c = closes.last().copied().unwrap_or(f64::NAN);
        let m = *base_series.last().unwrap_or(&f64::NAN);
        if !c.is_finite() || !m.is_finite() {
            "unknown"
        } else if c > m * 1.002 {
            "above"
        } else if c < m * 0.998 {
            "below"
        } else {
            "near"
        }
    };

    MaState {
        symbol: symbol.to_string(),
        timeframe: timeframe.to_string(),
        kind,
        periods: periods.to_vec(),
        last_values,
        series,
        alignment,
        alignment_aliases: alignment.aliases().iter().map(|s| s.to_string()).collect(),
        spread_state,
        bias_base,
        bias_base_period: base_period,
        slopes,
        crosses,
        granville: granville_recent,
        price_vs_base,
    }
}
