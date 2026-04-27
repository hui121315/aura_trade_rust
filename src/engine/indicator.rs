//! 辅助指标：RSI / MACD / Volume 均值
//!
//! 虽然 PRD 的四维体系不强依赖这些指标，但在 UI 上展示它们有助于辅助决策。

use serde::{Deserialize, Serialize};

use crate::data::Kline;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndicatorSnapshot {
    pub rsi_14: f64,
    pub macd: f64,
    pub macd_signal: f64,
    pub macd_histogram: f64,
    pub volume_ma20: f64,
    pub volume_current: f64,
    pub volume_ratio: f64,
}

/// 计算最后一根 K 线的指标快照（不保留序列以节省 bytes）
pub fn compute_snapshot(klines: &[Kline]) -> IndicatorSnapshot {
    let closes: Vec<f64> = klines.iter().map(|k| k.close).collect();
    let volumes: Vec<f64> = klines.iter().map(|k| k.volume).collect();

    let rsi = rsi(&closes, 14);
    let (macd_s, signal_s) = macd(&closes, 12, 26, 9);
    let last_idx = closes.len().saturating_sub(1);
    let rsi_v = *rsi.get(last_idx).unwrap_or(&f64::NAN);
    let macd_v = *macd_s.get(last_idx).unwrap_or(&f64::NAN);
    let sig_v = *signal_s.get(last_idx).unwrap_or(&f64::NAN);
    let hist = macd_v - sig_v;

    let vol_ma20 = if volumes.len() >= 20 {
        volumes[volumes.len() - 20..].iter().sum::<f64>() / 20.0
    } else if !volumes.is_empty() {
        volumes.iter().sum::<f64>() / volumes.len() as f64
    } else {
        0.0
    };
    let vol_cur = *volumes.last().unwrap_or(&0.0);
    let ratio = if vol_ma20 > 0.0 { vol_cur / vol_ma20 } else { 1.0 };

    IndicatorSnapshot {
        rsi_14: rsi_v,
        macd: macd_v,
        macd_signal: sig_v,
        macd_histogram: hist,
        volume_ma20: vol_ma20,
        volume_current: vol_cur,
        volume_ratio: ratio,
    }
}

/// Wilder RSI
pub fn rsi(closes: &[f64], period: usize) -> Vec<f64> {
    let n = closes.len();
    let mut out = vec![f64::NAN; n];
    if period == 0 || n <= period {
        return out;
    }
    let mut gain = 0.0;
    let mut loss = 0.0;
    for i in 1..=period {
        let diff = closes[i] - closes[i - 1];
        if diff > 0.0 { gain += diff; } else { loss += -diff; }
    }
    gain /= period as f64;
    loss /= period as f64;
    out[period] = if gain < 1e-12 && loss < 1e-12 { 50.0 } else if loss < 1e-12 { 100.0 } else {
        let rs = gain / loss;
        100.0 - (100.0 / (1.0 + rs))
    };
    let alpha = 1.0 / period as f64;
    for i in (period + 1)..n {
        let diff = closes[i] - closes[i - 1];
        let g = diff.max(0.0);
        let l = (-diff).max(0.0);
        gain = g * alpha + gain * (1.0 - alpha);
        loss = l * alpha + loss * (1.0 - alpha);
        out[i] = if gain < 1e-12 && loss < 1e-12 { 50.0 } else if loss < 1e-12 { 100.0 } else {
            let rs = gain / loss;
            100.0 - (100.0 / (1.0 + rs))
        };
    }
    out
}

/// MACD: (macd_line, signal_line)
pub fn macd(closes: &[f64], fast: usize, slow: usize, signal: usize) -> (Vec<f64>, Vec<f64>) {
    let ema_fast = ema(closes, fast);
    let ema_slow = ema(closes, slow);
    let n = closes.len();
    let macd_line: Vec<f64> = (0..n).map(|i| {
        let f = ema_fast.get(i).copied().unwrap_or(f64::NAN);
        let s = ema_slow.get(i).copied().unwrap_or(f64::NAN);
        if f.is_finite() && s.is_finite() { f - s } else { f64::NAN }
    }).collect();
    let mut signal_line = vec![f64::NAN; n];
    if signal > 0 {
        if let Some(first_valid) = macd_line.iter().position(|v| v.is_finite()) {
            let finite_macd: Vec<f64> = macd_line[first_valid..]
                .iter()
                .copied()
                .take_while(|v| v.is_finite())
                .collect();
            let finite_signal = ema(&finite_macd, signal);
            for (offset, v) in finite_signal.into_iter().enumerate() {
                if v.is_finite() {
                    signal_line[first_valid + offset] = v;
                }
            }
        }
    }
    (macd_line, signal_line)
}

/// StochRSI: 对 RSI 做随机指标归一化
///
/// 参数：
///   rsi_period      RSI 计算周期（默认 14）
///   stoch_period    stochastic 回看窗口（默认 14）
///   k_smooth        %K 的 SMA 平滑周期（默认 3）
///   d_smooth        %D 在 %K 上再做 SMA 平滑（默认 3）
///
/// 返回：(%K, %D)，两条都是 0–100；前 rsi_period+stoch_period 位为 NaN
///
/// 使用建议：K/D 上穿 20 → 底部反转；下穿 80 → 顶部反转（比 RSI 更灵敏）
pub fn stoch_rsi(
    closes: &[f64],
    rsi_period: usize,
    stoch_period: usize,
    k_smooth: usize,
    d_smooth: usize,
) -> (Vec<f64>, Vec<f64>) {
    let n = closes.len();
    let rsi_vals = rsi(closes, rsi_period);
    let mut raw = vec![f64::NAN; n];
    if stoch_period > 0 && n > stoch_period {
        for i in stoch_period..n {
            let window = &rsi_vals[i + 1 - stoch_period..=i];
            let mut lo = f64::INFINITY;
            let mut hi = f64::NEG_INFINITY;
            let mut ok = true;
            for v in window {
                if !v.is_finite() {
                    ok = false;
                    break;
                }
                if *v < lo {
                    lo = *v;
                }
                if *v > hi {
                    hi = *v;
                }
            }
            if !ok {
                continue;
            }
            if (hi - lo).abs() < 1e-12 {
                // 完全平坦（窗口内 RSI 无波动），退回 RSI 自身，避免强上涨被误标为超卖
                raw[i] = rsi_vals[i].clamp(0.0, 100.0);
            } else {
                raw[i] = (rsi_vals[i] - lo) / (hi - lo) * 100.0;
            }
        }
    }
    let k = sma(&raw, k_smooth);
    let d = sma(&k, d_smooth);
    (k, d)
}

/// 本文件专用的 NaN-safe SMA（不同于 ma::compute::sma：这里跳过 NaN 作为窗口内"未定义"）
fn sma(values: &[f64], period: usize) -> Vec<f64> {
    let n = values.len();
    let mut out = vec![f64::NAN; n];
    if period == 0 || period > n {
        return out;
    }
    for i in (period - 1)..n {
        let mut sum = 0.0;
        let mut ok = true;
        for j in (i + 1 - period)..=i {
            if !values[j].is_finite() {
                ok = false;
                break;
            }
            sum += values[j];
        }
        if ok {
            out[i] = sum / period as f64;
        }
    }
    out
}

fn ema(values: &[f64], period: usize) -> Vec<f64> {
    let n = values.len();
    let mut out = vec![f64::NAN; n];
    if period == 0 || n < period {
        return out;
    }
    let seed: f64 = values[..period].iter().sum::<f64>() / period as f64;
    out[period - 1] = seed;
    let k = 2.0 / (period as f64 + 1.0);
    let mut prev = seed;
    for i in period..n {
        let cur = values[i] * k + prev * (1.0 - k);
        out[i] = cur;
        prev = cur;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mk_kline(idx: i64, o: f64, h: f64, l: f64, c: f64, v: f64) -> Kline {
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

    // ==================== RSI ====================

    #[test]
    fn t_rsi_all_gains_yields_100() {
        // 连续上涨 → loss=0 → RSI=100（Wilder 惯例）
        let closes: Vec<f64> = (0..20).map(|i| 100.0 + i as f64).collect();
        let out = rsi(&closes, 14);
        assert!((out[14] - 100.0).abs() < 1e-6, "全涨 RSI 应 =100，实际 {}", out[14]);
        assert!((out[19] - 100.0).abs() < 1e-6);
    }

    #[test]
    fn t_rsi_all_losses_yields_0() {
        // 连续下跌 → gain=0 → RSI=0
        let closes: Vec<f64> = (0..20).map(|i| 100.0 - i as f64).collect();
        let out = rsi(&closes, 14);
        assert!(out[14].abs() < 1e-6, "全跌 RSI 应 =0，实际 {}", out[14]);
        assert!(out[19].abs() < 1e-6);
    }

    #[test]
    fn t_rsi_flat_sequence_returns_50() {
        // 持平 → gain=0, loss=0，应视作中性
        let closes: Vec<f64> = vec![100.0; 30];
        let out = rsi(&closes, 14);
        assert!((out[14] - 50.0).abs() < 1e-6);
    }

    #[test]
    fn t_rsi_too_short_all_nan() {
        let closes = vec![100.0, 101.0, 102.0];
        let out = rsi(&closes, 14);
        assert!(out.iter().all(|v| v.is_nan()));
    }

    #[test]
    fn t_rsi_mid_value_in_mixed_sequence() {
        // 混合涨跌：RSI 应落在 (0, 100) 区间
        let closes: Vec<f64> = vec![
            100.0, 102.0, 98.0, 105.0, 99.0, 108.0, 96.0, 110.0, 97.0, 112.0,
            95.0, 115.0, 92.0, 118.0, 90.0, 120.0, 88.0, 122.0,
        ];
        let out = rsi(&closes, 14);
        let v = out[17];
        assert!(v.is_finite(), "RSI 应有效");
        assert!(v > 0.0 && v < 100.0, "RSI 应 ∈ (0,100)，实际 {}", v);
    }

    // ==================== MACD ====================

    #[test]
    fn t_macd_positive_in_strong_uptrend() {
        // 单调强上涨：短 EMA > 长 EMA → MACD > 0
        let closes: Vec<f64> = (0..60).map(|i| 100.0 + i as f64).collect();
        let (m, s) = macd(&closes, 12, 26, 9);
        let last = closes.len() - 1;
        assert!(m[last] > 0.0, "强上涨 MACD 应 > 0，实际 {}", m[last]);
        assert!(s[last].is_finite());
    }

    #[test]
    fn t_macd_negative_in_strong_downtrend() {
        let closes: Vec<f64> = (0..60).map(|i| 200.0 - i as f64).collect();
        let (m, _) = macd(&closes, 12, 26, 9);
        let last = closes.len() - 1;
        assert!(m[last] < 0.0, "强下跌 MACD 应 < 0，实际 {}", m[last]);
    }

    #[test]
    fn t_macd_sign_flip_on_reversal() {
        // 前半段下跌，后半段上涨 → MACD 应从负翻正
        let mut closes = Vec::new();
        for i in 0..30 {
            closes.push(150.0 - i as f64);
        }
        for i in 0..30 {
            closes.push(120.0 + i as f64);
        }
        let (m, _) = macd(&closes, 12, 26, 9);
        let mid = m[29];
        let end = m[59];
        assert!(mid < 0.0, "下跌段末尾 MACD 应为负，实际 {}", mid);
        assert!(end > mid, "反转后 MACD 应回升，实际 mid={} end={}", mid, end);
    }

    #[test]
    fn t_macd_too_short_nan() {
        let closes = vec![100.0, 101.0, 102.0];
        let (m, s) = macd(&closes, 12, 26, 9);
        // slow=26 > n=3，所有 ema_slow 都 NaN → m 全 NaN
        assert!(m.iter().all(|v| v.is_nan() || *v == 0.0 || v.abs() < 1e-9));
        let _ = s;
    }

    #[test]
    fn t_macd_signal_starts_after_first_signal_window() {
        let closes: Vec<f64> = (0..60).map(|i| 100.0 + i as f64).collect();
        let (m, s) = macd(&closes, 12, 26, 9);
        assert!(m[..25].iter().all(|v| v.is_nan()));
        assert!(m[25].is_finite());
        assert!(s[..33].iter().all(|v| v.is_nan()));
        assert!(s[33].is_finite());
    }

    // ==================== StochRSI ====================

    #[test]
    fn t_stoch_rsi_bounded_in_0_to_100() {
        let closes: Vec<f64> = (0..60)
            .map(|i| 100.0 + ((i as f64) * 0.5).sin() * 5.0)
            .collect();
        let (k, d) = stoch_rsi(&closes, 14, 14, 3, 3);
        for v in k.iter().chain(d.iter()) {
            if v.is_finite() {
                assert!(*v >= -1e-6 && *v <= 100.0 + 1e-6, "StochRSI 值应 ∈ [0,100]，实际 {}", v);
            }
        }
    }

    #[test]
    fn t_stoch_rsi_near_100_on_monotonic_up() {
        // 单调上涨序列 → RSI 单调 → 最新 RSI = max → %K 应接近 100
        let closes: Vec<f64> = (0..50).map(|i| 100.0 + i as f64).collect();
        let (k, _) = stoch_rsi(&closes, 14, 14, 3, 3);
        let last = k.iter().rposition(|v| v.is_finite()).unwrap();
        assert!((k[last] - 100.0).abs() < 1e-6,
            "单调上涨 StochRSI %K 应 = 100，实际 {}", k[last]);
    }

    #[test]
    fn t_stoch_rsi_flat_sequence_is_neutral() {
        let closes: Vec<f64> = vec![100.0; 50];
        let (k, d) = stoch_rsi(&closes, 14, 14, 3, 3);
        let last = k.iter().rposition(|v| v.is_finite()).unwrap();
        assert!((k[last] - 50.0).abs() < 1e-6);
        assert!((d[last] - 50.0).abs() < 1e-6);
    }

    // ==================== Volume / Snapshot ====================

    #[test]
    fn t_compute_snapshot_empty_klines_no_panic() {
        let snap = compute_snapshot(&[]);
        assert!(snap.rsi_14.is_nan());
        assert!(snap.volume_ma20 >= 0.0);
        assert_eq!(snap.volume_current, 0.0);
    }

    #[test]
    fn t_compute_snapshot_volume_ratio_correct() {
        // 20 根 volume=1, 最后一根 volume=3 → ma20=1.1, ratio=3/1.1≈2.727
        let mut klines: Vec<_> = (0..20)
            .map(|i| mk_kline(i, 100.0, 101.0, 99.0, 100.5, 1.0))
            .collect();
        klines.push(mk_kline(20, 100.0, 101.0, 99.0, 100.5, 3.0));
        let snap = compute_snapshot(&klines);
        // ma20 覆盖最后 20 根：1..20 都是 1.0，最后一根 3.0，ma20=(19*1 + 3)/20=1.1
        assert!((snap.volume_ma20 - 1.1).abs() < 1e-9);
        assert!((snap.volume_current - 3.0).abs() < 1e-9);
        assert!((snap.volume_ratio - 3.0 / 1.1).abs() < 1e-6);
    }

    #[test]
    fn t_compute_snapshot_matches_independent_computations() {
        // snapshot 的 rsi/macd/macd_signal 字段应与独立调用结果一致（最后一根）
        let closes: Vec<f64> = (0..60).map(|i| 100.0 + (i as f64 * 0.3).cos() * 8.0).collect();
        let klines: Vec<_> = closes
            .iter()
            .enumerate()
            .map(|(i, &c)| mk_kline(i as i64, c, c + 1.0, c - 1.0, c, 1.0))
            .collect();
        let snap = compute_snapshot(&klines);
        let rsi_series = rsi(&closes, 14);
        let (macd_series, sig_series) = macd(&closes, 12, 26, 9);
        let last = closes.len() - 1;
        assert!((snap.rsi_14 - rsi_series[last]).abs() < 1e-9);
        assert!((snap.macd - macd_series[last]).abs() < 1e-9);
        assert!((snap.macd_signal - sig_series[last]).abs() < 1e-9);
        assert!((snap.macd_histogram - (snap.macd - snap.macd_signal)).abs() < 1e-9);
    }

    // ==================== 交叉验证 ====================

    #[test]
    fn it_rsi_below_30_and_macd_negative_coexist_in_downtrend() {
        // R-P1 铁证：强下跌环境下 RSI 进入超卖 (<30) 与 MACD 转负应**同期出现**
        let closes: Vec<f64> = (0..60).map(|i| 200.0 - i as f64 * 1.5).collect();
        let rsi_series = rsi(&closes, 14);
        let (macd_series, _) = macd(&closes, 12, 26, 9);
        let last = closes.len() - 1;
        assert!(rsi_series[last] < 30.0, "强下跌末尾 RSI 应 <30，实际 {}", rsi_series[last]);
        assert!(macd_series[last] < 0.0, "强下跌末尾 MACD 应 <0，实际 {}", macd_series[last]);
    }

    #[test]
    fn it_rsi_above_70_and_macd_positive_coexist_in_uptrend() {
        let closes: Vec<f64> = (0..60).map(|i| 100.0 + i as f64 * 1.5).collect();
        let rsi_series = rsi(&closes, 14);
        let (macd_series, _) = macd(&closes, 12, 26, 9);
        let last = closes.len() - 1;
        assert!(rsi_series[last] > 70.0, "强上涨末尾 RSI 应 >70，实际 {}", rsi_series[last]);
        assert!(macd_series[last] > 0.0, "强上涨末尾 MACD 应 >0，实际 {}", macd_series[last]);
    }
}
