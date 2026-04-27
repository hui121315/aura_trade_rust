//! 均线基础算法：SMA / EMA / WMA
//!
//! 对应 PRD §A1：均线基础原理
//! 输出与输入对齐，前 N-1 位置为 NaN（未成熟）。

use serde::{Deserialize, Serialize};

/// 均线类型枚举
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MaKind {
    /// 简单移动平均（原书默认）
    Sma,
    /// 指数移动平均
    Ema,
    /// 加权移动平均（线性加权）
    Wma,
}

impl MaKind {
    pub fn parse(s: &str) -> Option<Self> {
        Some(match s.to_ascii_lowercase().as_str() {
            "sma" => MaKind::Sma,
            "ema" => MaKind::Ema,
            "wma" => MaKind::Wma,
            _ => return None,
        })
    }
}

/// 计算均线，返回与 `closes` 等长的序列，前 `period-1` 位为 NaN。
pub fn compute(kind: MaKind, closes: &[f64], period: usize) -> Vec<f64> {
    match kind {
        MaKind::Sma => sma(closes, period),
        MaKind::Ema => ema(closes, period),
        MaKind::Wma => wma(closes, period),
    }
}

pub fn sma(closes: &[f64], period: usize) -> Vec<f64> {
    let n = closes.len();
    let mut out = vec![f64::NAN; n];
    if period == 0 || period > n {
        return out;
    }
    let mut sum: f64 = closes[..period].iter().copied().sum();
    out[period - 1] = sum / period as f64;
    for i in period..n {
        sum += closes[i] - closes[i - period];
        out[i] = sum / period as f64;
    }
    out
}

pub fn ema(closes: &[f64], period: usize) -> Vec<f64> {
    let n = closes.len();
    let mut out = vec![f64::NAN; n];
    if period == 0 || period > n {
        return out;
    }
    let alpha = 2.0 / (period as f64 + 1.0);
    // 用前 period 根的简单平均作为种子（与多数教科书一致）
    let seed: f64 = closes[..period].iter().copied().sum::<f64>() / period as f64;
    out[period - 1] = seed;
    let mut prev = seed;
    for i in period..n {
        let cur = closes[i] * alpha + prev * (1.0 - alpha);
        out[i] = cur;
        prev = cur;
    }
    out
}

pub fn wma(closes: &[f64], period: usize) -> Vec<f64> {
    let n = closes.len();
    let mut out = vec![f64::NAN; n];
    if period == 0 || period > n {
        return out;
    }
    let denom: f64 = (1..=period).map(|x| x as f64).sum();
    for i in (period - 1)..n {
        let mut num = 0.0;
        for k in 0..period {
            let w = (period - k) as f64; // 最新给最大权重
            num += closes[i - k] * w;
        }
        // 上面权重是 period..1，总和 == denom
        out[i] = num / denom;
    }
    out
}

/// 计算 `period` 根均线的变化率（对数/线性无关，仅内部相对量）：
/// slope(t) = (ma[t] - ma[t-k]) / ma[t-k]
pub fn slope(ma: &[f64], lookback: usize) -> Vec<f64> {
    let n = ma.len();
    let mut out = vec![f64::NAN; n];
    if lookback == 0 || lookback >= n {
        return out;
    }
    for i in lookback..n {
        let prev = ma[i - lookback];
        let cur = ma[i];
        if prev.is_finite() && cur.is_finite() && prev != 0.0 {
            out[i] = (cur - prev) / prev.abs();
        }
    }
    out
}

/// BIAS = (price - ma) / ma
pub fn bias(closes: &[f64], ma: &[f64]) -> Vec<f64> {
    let n = closes.len().min(ma.len());
    let mut out = vec![f64::NAN; n];
    for i in 0..n {
        if ma[i].is_finite() && ma[i] != 0.0 {
            out[i] = (closes[i] - ma[i]) / ma[i];
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sma_basic() {
        let v = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        let ma = sma(&v, 3);
        assert!(ma[0].is_nan() && ma[1].is_nan());
        assert!((ma[2] - 2.0).abs() < 1e-9);
        assert!((ma[3] - 3.0).abs() < 1e-9);
        assert!((ma[4] - 4.0).abs() < 1e-9);
    }

    #[test]
    fn ema_monotonic_up() {
        let v = vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0];
        let ma = ema(&v, 3);
        for i in 3..7 {
            assert!(ma[i] > ma[i - 1], "ema should be increasing at {}", i);
        }
    }

    #[test]
    fn wma_basic() {
        // wma of [1,2,3] period 3 = (1*1 + 2*2 + 3*3)/6 = 14/6
        let v = vec![1.0, 2.0, 3.0];
        let ma = wma(&v, 3);
        assert!((ma[2] - 14.0 / 6.0).abs() < 1e-9);
    }

    // -------- slope / bias / MaKind / compute dispatch --------

    #[test]
    fn slope_basic_uptrend() {
        // ma 从 100 上行到 110，lookback=5：slope[5] = (110-100)/100 = 0.10
        let ma = vec![100.0, 102.0, 104.0, 106.0, 108.0, 110.0];
        let s = slope(&ma, 5);
        assert!(s[0..5].iter().all(|v| v.is_nan()), "前 5 个应为 NaN");
        assert!((s[5] - 0.10).abs() < 1e-9, "slope[5] 应 ≈ 0.10，实际 {}", s[5]);
    }

    #[test]
    fn slope_downtrend_negative() {
        // ma 从 100 下行到 90，lookback=5：slope[5] = (90-100)/100 = -0.10
        let ma = vec![100.0, 98.0, 96.0, 94.0, 92.0, 90.0];
        let s = slope(&ma, 5);
        assert!((s[5] - (-0.10)).abs() < 1e-9);
    }

    #[test]
    fn slope_zero_when_flat() {
        let ma = vec![100.0; 10];
        let s = slope(&ma, 5);
        assert!((s[5]).abs() < 1e-9);
        assert!((s[9]).abs() < 1e-9);
    }

    #[test]
    fn slope_edge_lookback_ge_len_all_nan() {
        let ma = vec![100.0, 101.0, 102.0];
        let s = slope(&ma, 5); // lookback >= len
        assert!(s.iter().all(|v| v.is_nan()));
    }

    #[test]
    fn bias_basic_above_below() {
        // close > ma → bias > 0；close < ma → bias < 0
        let closes = vec![105.0, 100.0, 95.0];
        let ma = vec![100.0, 100.0, 100.0];
        let b = bias(&closes, &ma);
        assert!((b[0] - 0.05).abs() < 1e-9);
        assert!(b[1].abs() < 1e-9);
        assert!((b[2] - (-0.05)).abs() < 1e-9);
    }

    #[test]
    fn bias_nan_safe_when_ma_nan_or_zero() {
        let closes = vec![100.0, 100.0, 100.0];
        let ma = vec![f64::NAN, 0.0, 100.0];
        let b = bias(&closes, &ma);
        assert!(b[0].is_nan(), "ma NaN → bias NaN");
        assert!(b[1].is_nan(), "ma=0 → bias NaN");
        assert!(b[2].is_finite());
    }

    #[test]
    fn makind_parse_all_variants_case_insensitive() {
        assert_eq!(MaKind::parse("sma"), Some(MaKind::Sma));
        assert_eq!(MaKind::parse("SMA"), Some(MaKind::Sma));
        assert_eq!(MaKind::parse("Ema"), Some(MaKind::Ema));
        assert_eq!(MaKind::parse("WMA"), Some(MaKind::Wma));
        assert_eq!(MaKind::parse("unknown"), None);
        assert_eq!(MaKind::parse(""), None);
    }

    #[test]
    fn compute_dispatch_matches_direct_functions() {
        // compute(kind, ...) 与直接调用 sma/ema/wma 应一致
        let v = vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0];
        for (kind, expected) in [
            (MaKind::Sma, sma(&v, 3)),
            (MaKind::Ema, ema(&v, 3)),
            (MaKind::Wma, wma(&v, 3)),
        ] {
            let got = compute(kind, &v, 3);
            assert_eq!(got.len(), expected.len());
            for i in 0..got.len() {
                if expected[i].is_nan() {
                    assert!(got[i].is_nan(), "kind={:?} idx={}", kind, i);
                } else {
                    assert!(
                        (got[i] - expected[i]).abs() < 1e-12,
                        "kind={:?} idx={} got={} expected={}",
                        kind, i, got[i], expected[i]
                    );
                }
            }
        }
    }

    #[test]
    fn compute_empty_or_oversize_period_all_nan() {
        let v = vec![1.0, 2.0, 3.0];
        assert!(sma(&v, 5).iter().all(|x| x.is_nan()), "period > len → 全 NaN");
        assert!(ema(&v, 0).iter().all(|x| x.is_nan()), "period=0 → 全 NaN");
        assert!(wma(&[], 3).is_empty());
    }
}
