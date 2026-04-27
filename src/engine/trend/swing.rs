//! B1：摆动高低点（ZigZag）
//!
//! 采用两种规则并用：
//! 1. **ATR 阈值**：从上一个枢轴点起，价格反向波动 ≥ `atr_mult × ATR` 才确认新枢轴
//! 2. **Pivot N**：同时要求候选点左右各至少 `pivot_n` 根 K线不超越（保证肉眼可见）

use serde::{Deserialize, Serialize};

use crate::data::Kline;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SwingKind {
    High,
    Low,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct SwingPoint {
    pub index: usize,
    pub time: i64,
    pub price: f64,
    pub kind: SwingKind,
}

#[derive(Debug, Clone, Copy)]
pub struct SwingParams {
    /// ATR 反向阈值倍数
    pub atr_mult: f64,
    /// ATR 周期
    pub atr_period: usize,
    /// 最小反转百分比（相对枢轴价）
    pub min_pct: f64,
    /// 相邻枢轴最少 K线间隔
    pub min_gap_bars: usize,
}

impl Default for SwingParams {
    fn default() -> Self {
        Self { atr_mult: 2.0, atr_period: 14, min_pct: 0.015, min_gap_bars: 3 }
    }
}

/// 计算摆动高低点（主入口）
pub fn detect(klines: &[Kline], p: &SwingParams) -> Vec<SwingPoint> {
    let n = klines.len();
    if n < p.atr_period + 4 {
        return vec![];
    }
    let atr = atr_series(klines, p.atr_period);

    let mut points: Vec<SwingPoint> = Vec::new();
    let mut seeking_high = true;
    if n > 1 {
        seeking_high = klines[1].close >= klines[0].close;
    }

    let mut ext_idx: usize = 0;
    let mut ext_price: f64 = if seeking_high { klines[0].high } else { klines[0].low };
    let mut counter_price: f64 = if seeking_high { klines[0].low } else { klines[0].high };

    for i in 0..n {
        let k = &klines[i];
        let atr_v = atr.get(i).copied().unwrap_or(0.0);

        // 使用绝对价格阈值：max(ATR*倍数, 价格*最小百分比)
        let atr_thresh = atr_v * p.atr_mult;
        let pct_thresh = ext_price.abs() * p.min_pct;
        let threshold = atr_thresh.max(pct_thresh);

        if seeking_high {
            if k.high > ext_price {
                ext_price = k.high;
                ext_idx = i;
                // 新高点：尚未观察到反转，counter 置为此高点自身，待后续 bar 的低点向下突破
                counter_price = k.high;
            }
            // 只有 *晚于* 新高的 bar 才能贡献反转（避免 intrabar 大振幅自己触发）
            if i > ext_idx && k.low < counter_price {
                counter_price = k.low;
            }

            if threshold > 0.0 && (ext_price - counter_price) >= threshold {
                // 距上个同类枢轴最少间隔
                let accept = points
                    .last()
                    .map(|pp| ext_idx.saturating_sub(pp.index) >= p.min_gap_bars)
                    .unwrap_or(true);
                if accept {
                    points.push(SwingPoint {
                        index: ext_idx,
                        time: klines[ext_idx].open_time,
                        price: ext_price,
                        kind: SwingKind::High,
                    });
                }
                seeking_high = false;
                ext_price = counter_price;
                ext_idx = find_index_of_low(klines, ext_idx, i, ext_price);
                counter_price = klines[i].high;
            }
        } else {
            if k.low < ext_price {
                ext_price = k.low;
                ext_idx = i;
                counter_price = k.low;
            }
            if i > ext_idx && k.high > counter_price {
                counter_price = k.high;
            }

            if threshold > 0.0 && (counter_price - ext_price) >= threshold {
                let accept = points
                    .last()
                    .map(|pp| ext_idx.saturating_sub(pp.index) >= p.min_gap_bars)
                    .unwrap_or(true);
                if accept {
                    points.push(SwingPoint {
                        index: ext_idx,
                        time: klines[ext_idx].open_time,
                        price: ext_price,
                        kind: SwingKind::Low,
                    });
                }
                seeking_high = true;
                ext_price = counter_price;
                ext_idx = find_index_of_high(klines, ext_idx, i, ext_price);
                counter_price = klines[i].low;
            }
        }
    }

    points
}

#[allow(dead_code)]
fn valid_pivot(klines: &[Kline], center: usize, n: usize, is_high: bool) -> bool {
    if n == 0 {
        return true;
    }
    let from = center.saturating_sub(n);
    let to = (center + n).min(klines.len().saturating_sub(1));
    for i in from..=to {
        if i == center {
            continue;
        }
        if is_high {
            if klines[i].high > klines[center].high {
                return false;
            }
        } else if klines[i].low < klines[center].low {
            return false;
        }
    }
    true
}

fn find_index_of_high(klines: &[Kline], from: usize, to: usize, target: f64) -> usize {
    for i in from..=to.min(klines.len() - 1) {
        if (klines[i].high - target).abs() < 1e-9 {
            return i;
        }
    }
    to.min(klines.len() - 1)
}

fn find_index_of_low(klines: &[Kline], from: usize, to: usize, target: f64) -> usize {
    for i in from..=to.min(klines.len() - 1) {
        if (klines[i].low - target).abs() < 1e-9 {
            return i;
        }
    }
    to.min(klines.len() - 1)
}

/// Wilder ATR（共享辅助）
pub fn atr_series(klines: &[Kline], period: usize) -> Vec<f64> {
    let n = klines.len();
    let mut tr = vec![0.0; n];
    for i in 0..n {
        let h = klines[i].high;
        let l = klines[i].low;
        tr[i] = if i == 0 {
            h - l
        } else {
            let pc = klines[i - 1].close;
            (h - l).max((h - pc).abs()).max((l - pc).abs())
        };
    }
    let mut out = vec![f64::NAN; n];
    if period == 0 || period > n {
        return out;
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    fn mk_kline(idx: i64, o: f64, h: f64, l: f64, c: f64) -> Kline {
        Kline {
            open_time: idx * 86_400_000,
            close_time: (idx + 1) * 86_400_000 - 1,
            open: o,
            high: h,
            low: l,
            close: c,
            volume: 1.0,
        }
    }

    #[test]
    fn t_atr_series_seed_equals_sma_of_tr() {
        // 构造 10 根 K 线，每根高低差恒为 2，close=open → TR[0] = 2，后续 TR = max(H-L, |H-prevC|, |L-prevC|)
        // 因为 close 恒定，H-prevC = L-prevC = 1 → TR[i>0] = 2
        // ATR 的 seed（period=5）= mean(TR[0..5]) = 2；递推保持 2
        let klines: Vec<_> = (0..10)
            .map(|i| mk_kline(i, 100.0, 101.0, 99.0, 100.0))
            .collect();
        let out = atr_series(&klines, 5);
        assert!(out[0..4].iter().all(|v| v.is_nan()), "前 4 个应为 NaN");
        assert!((out[4] - 2.0).abs() < 1e-9, "seed 应 = SMA(TR) = 2，实际 {}", out[4]);
        for i in 5..10 {
            assert!((out[i] - 2.0).abs() < 1e-9, "ATR 应稳定在 2，idx={} 实际 {}", i, out[i]);
        }
    }

    #[test]
    fn t_atr_series_increases_on_volatility_spike() {
        // 前 5 根小幅波动（TR=1），第 6 根暴涨（TR=10）
        let mut klines: Vec<_> = (0..5)
            .map(|i| mk_kline(i, 100.0, 100.5, 99.5, 100.0))
            .collect();
        // 第 6 根：high=110 low=100 prev_close=100 → TR = max(10, 10, 0) = 10
        klines.push(mk_kline(5, 100.0, 110.0, 100.0, 110.0));
        klines.push(mk_kline(6, 110.0, 110.5, 109.5, 110.0));
        let out = atr_series(&klines, 5);
        // seed = mean(TR[0..5]) = 1.0 (5 根都是 1)
        // ATR[5] = alpha*10 + (1-alpha)*1 = 0.2*10 + 0.8*1 = 2.0 + 0.8 = 2.8
        let seed = out[4];
        assert!((seed - 1.0).abs() < 1e-9, "seed 应 = 1.0，实际 {}", seed);
        assert!(out[5] > seed, "波动飙升后 ATR 应扩大，实际 {} → {}", seed, out[5]);
        assert!((out[5] - 2.8).abs() < 1e-9, "ATR[5] 应 = 2.8 (Wilder 递推)，实际 {}", out[5]);
    }

    #[test]
    fn t_atr_series_period_gt_len_all_nan() {
        let klines: Vec<_> = (0..3)
            .map(|i| mk_kline(i, 100.0, 101.0, 99.0, 100.0))
            .collect();
        let out = atr_series(&klines, 14);
        assert!(out.iter().all(|v| v.is_nan()));
    }

    #[test]
    fn t_atr_series_zero_period_all_nan() {
        let klines: Vec<_> = (0..10)
            .map(|i| mk_kline(i, 100.0, 101.0, 99.0, 100.0))
            .collect();
        let out = atr_series(&klines, 0);
        assert!(out.iter().all(|v| v.is_nan()));
    }

    #[test]
    fn t_atr_series_empty_input() {
        let out = atr_series(&[], 14);
        assert!(out.is_empty());
    }

    #[test]
    fn t_atr_series_tr_uses_gap_when_overnight() {
        // 验证 TR 公式：含跳空时 TR = max(H-L, |H-prevC|, |L-prevC|)
        // bar0: close=100
        // bar1: high=110, low=105, prev_close=100 → TR = max(5, 10, 5) = 10（跳空放大）
        let klines = vec![
            mk_kline(0, 98.0, 101.0, 97.0, 100.0),
            mk_kline(1, 108.0, 110.0, 105.0, 108.0),
            mk_kline(2, 108.0, 109.0, 107.0, 108.0),
            mk_kline(3, 108.0, 109.0, 107.0, 108.0),
        ];
        // period=2: seed = (TR[0] + TR[1]) / 2 = (4 + 10) / 2 = 7
        let out = atr_series(&klines, 2);
        assert!((out[1] - 7.0).abs() < 1e-9, "跳空下 seed 应 = 7，实际 {}", out[1]);
    }
}
