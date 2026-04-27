//! C1：K线基础度量
//!
//! 对应 PRD §C1：实体/上影/下影/波幅/实体占比/阴阳。

use serde::{Deserialize, Serialize};

use crate::data::Kline;

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct CandleMetrics {
    pub open_time: i64,
    pub body: f64,
    pub upper: f64,
    pub lower: f64,
    pub range: f64,
    pub body_ratio: f64,  // body / range
    pub upper_ratio: f64, // upper / range
    pub lower_ratio: f64, // lower / range
    pub bullish: bool,
    /// 相对于前一根收盘的变化幅度（百分比），用来判定大/中/小阳/阴
    pub rel_change: f64,
}

pub fn metrics_for(k: &Kline, prev_close: Option<f64>) -> CandleMetrics {
    let body = k.body();
    let upper = k.upper_shadow();
    let lower = k.lower_shadow();
    let range = k.range();
    let ratio = |v: f64| if range > 0.0 { v / range } else { 0.0 };
    let rel = match prev_close {
        Some(pc) if pc > 0.0 => (k.close - k.open) / pc,
        _ => 0.0,
    };
    CandleMetrics {
        open_time: k.open_time,
        body,
        upper,
        lower,
        range,
        body_ratio: ratio(body),
        upper_ratio: ratio(upper),
        lower_ratio: ratio(lower),
        bullish: k.is_bullish(),
        rel_change: rel,
    }
}

pub fn metrics_series(klines: &[Kline]) -> Vec<CandleMetrics> {
    let mut out = Vec::with_capacity(klines.len());
    for (i, k) in klines.iter().enumerate() {
        let prev = if i == 0 { None } else { Some(klines[i - 1].close) };
        out.push(metrics_for(k, prev));
    }
    out
}

/// 粗分类：大阳/中阳/小阳/十字/小阴/中阴/大阴/一字
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CandleClass {
    DojiFlat,    // 一字线：开=收=高=低
    Doji,        // 十字线：body_ratio < 5%
    LongUpper,   // 长上影（倒锤/射击之星候选）
    LongLower,   // 长下影（锤头/吊颈候选）
    SpinningTop, // 螺旋桨：小实体 + 上下长影
    SmallBull,
    SmallBear,
    MediumBull,
    MediumBear,
    BigBull,     // 大阳线
    BigBear,     // 大阴线
    Marubozu,    // 光头光脚：实体 > 90%，无影
}

pub fn classify(m: &CandleMetrics) -> CandleClass {
    let abs_rel = m.rel_change.abs();
    // 一字线：开=收=高=低
    if m.range == 0.0 {
        return CandleClass::DojiFlat;
    }
    // 光头光脚
    if m.body_ratio > 0.9 {
        return CandleClass::Marubozu;
    }
    // 十字线
    if m.body_ratio < 0.05 {
        return CandleClass::Doji;
    }
    // 螺旋桨：小实体 + 上下都有显著影
    if m.body_ratio < 0.3 && m.upper_ratio > 0.3 && m.lower_ratio > 0.3 {
        return CandleClass::SpinningTop;
    }
    // 长上影 / 长下影
    if m.body_ratio < 0.35 && m.upper_ratio > 0.55 {
        return CandleClass::LongUpper;
    }
    if m.body_ratio < 0.35 && m.lower_ratio > 0.55 {
        return CandleClass::LongLower;
    }
    // 按幅度分大中小
    if m.bullish {
        if abs_rel > 0.04 {
            CandleClass::BigBull
        } else if abs_rel > 0.015 {
            CandleClass::MediumBull
        } else {
            CandleClass::SmallBull
        }
    } else if abs_rel > 0.04 {
        CandleClass::BigBear
    } else if abs_rel > 0.015 {
        CandleClass::MediumBear
    } else {
        CandleClass::SmallBear
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mk_kline(o: f64, h: f64, l: f64, c: f64) -> Kline {
        Kline {
            open_time: 0,
            close_time: 0,
            open: o,
            high: h,
            low: l,
            close: c,
            volume: 1.0,
        }
    }

    // -------- metrics_for --------

    #[test]
    fn t_metrics_for_bullish_candle_ratios_sum_to_one() {
        // open=100, close=105 (阳)，high=106, low=99
        // body=5, upper=1, lower=1, range=7
        let k = mk_kline(100.0, 106.0, 99.0, 105.0);
        let m = metrics_for(&k, None);
        assert!((m.body - 5.0).abs() < 1e-9);
        assert!((m.upper - 1.0).abs() < 1e-9);
        assert!((m.lower - 1.0).abs() < 1e-9);
        assert!((m.range - 7.0).abs() < 1e-9);
        assert!(m.bullish);
        // ratios 之和应 = 1.0
        let sum = m.body_ratio + m.upper_ratio + m.lower_ratio;
        assert!((sum - 1.0).abs() < 1e-9, "比率之和应 = 1，实际 {}", sum);
    }

    #[test]
    fn t_metrics_for_flat_candle_range_zero_ratios_zero() {
        // open=close=high=low=100 → range=0，所有 ratio 应 = 0
        let k = mk_kline(100.0, 100.0, 100.0, 100.0);
        let m = metrics_for(&k, None);
        assert_eq!(m.range, 0.0);
        assert_eq!(m.body_ratio, 0.0);
        assert_eq!(m.upper_ratio, 0.0);
        assert_eq!(m.lower_ratio, 0.0);
    }

    #[test]
    fn t_metrics_for_rel_change_uses_prev_close() {
        // open=100, close=105, prev_close=100 → rel_change = (105-100)/100 = 0.05
        let k = mk_kline(100.0, 106.0, 99.0, 105.0);
        let m = metrics_for(&k, Some(100.0));
        assert!((m.rel_change - 0.05).abs() < 1e-9);
    }

    #[test]
    fn t_metrics_for_rel_change_defaults_to_zero_when_no_prev() {
        let k = mk_kline(100.0, 106.0, 99.0, 105.0);
        let m = metrics_for(&k, None);
        assert_eq!(m.rel_change, 0.0);
    }

    #[test]
    fn t_metrics_series_length_matches_input() {
        let klines = vec![
            mk_kline(100.0, 101.0, 99.0, 100.5),
            mk_kline(100.5, 102.0, 100.0, 101.5),
            mk_kline(101.5, 103.0, 101.0, 102.5),
        ];
        let series = metrics_series(&klines);
        assert_eq!(series.len(), 3);
        // 第 0 根无 prev → rel_change = 0
        assert_eq!(series[0].rel_change, 0.0);
        // 第 1 根 rel_change = (101.5 - 100.5) / 100.5
        assert!((series[1].rel_change - (101.5 - 100.5) / 100.5).abs() < 1e-9);
    }

    // -------- classify --------

    #[test]
    fn t_classify_doji_flat_when_range_zero() {
        let k = mk_kline(100.0, 100.0, 100.0, 100.0);
        let m = metrics_for(&k, None);
        assert_eq!(classify(&m), CandleClass::DojiFlat);
    }

    #[test]
    fn t_classify_marubozu_when_body_ratio_above_90_pct() {
        // body=10, range=10 → body_ratio=1.0 > 0.9 → Marubozu
        let k = mk_kline(100.0, 110.0, 100.0, 110.0);
        let m = metrics_for(&k, None);
        assert_eq!(classify(&m), CandleClass::Marubozu);
    }

    #[test]
    fn t_classify_doji_when_body_under_5_pct() {
        // body=0.05, range=10 → body_ratio=0.005 < 0.05 → Doji
        let k = mk_kline(100.0, 105.0, 95.0, 100.05);
        let m = metrics_for(&k, None);
        assert_eq!(classify(&m), CandleClass::Doji);
    }

    #[test]
    fn t_classify_spinning_top_with_small_body_and_long_shadows() {
        // body=0.5 (小)，上影=4.5, 下影=5, range=10
        // body_ratio=0.05, upper_ratio=0.45, lower_ratio=0.5
        // body<0.3 ✓, upper>0.3 ✓, lower>0.3 ✓，但 body_ratio < 0.05 会先命中 Doji
        // 调大 body：body=1（调 open/close 差 1），range=10
        // open=100, close=101，high=105, low=95 → body=1, upper=4, lower=5, range=10
        // ratios: body=0.1, upper=0.4, lower=0.5 → Spinning ✓（body 不是 Doji）
        let k = mk_kline(100.0, 105.0, 95.0, 101.0);
        let m = metrics_for(&k, None);
        assert_eq!(classify(&m), CandleClass::SpinningTop);
    }

    #[test]
    fn t_classify_long_upper_shadow() {
        // 长上影：body_ratio<0.35 且 upper_ratio>0.55
        // open=100, close=100.5 (小阳), high=107, low=99.5
        // body=0.5, upper=6.5, lower=0.5, range=7.5
        // body_ratio=0.067, upper_ratio=0.867, lower_ratio=0.067 → LongUpper ✓
        let k = mk_kline(100.0, 107.0, 99.5, 100.5);
        let m = metrics_for(&k, None);
        assert_eq!(classify(&m), CandleClass::LongUpper);
    }

    #[test]
    fn t_classify_long_lower_shadow() {
        // 长下影：body_ratio<0.35 且 lower_ratio>0.55
        // open=100, close=100.5, high=101, low=93.5
        // body=0.5, upper=0.5, lower=6.5, range=7.5
        // lower_ratio=0.867 → LongLower ✓
        let k = mk_kline(100.0, 101.0, 93.5, 100.5);
        let m = metrics_for(&k, None);
        assert_eq!(classify(&m), CandleClass::LongLower);
    }

    #[test]
    fn t_classify_big_bull_on_large_rel_change() {
        // abs_rel > 4% → BigBull；需要非 Marubozu，body_ratio 在 [0.35, 0.9]
        // open=100, close=106 (阳, 6% rel vs prev=100)
        // high=107, low=99 → body=6, upper=1, lower=1, range=8, body_ratio=0.75
        let k = mk_kline(100.0, 107.0, 99.0, 106.0);
        let m = metrics_for(&k, Some(100.0));
        assert_eq!(classify(&m), CandleClass::BigBull);
    }

    #[test]
    fn t_classify_medium_bull() {
        // 1.5% < abs_rel <= 4%：open=100, close=103 (3%)
        // high=103.5, low=99.5 → body=3, upper=0.5, lower=0.5, range=4, body_ratio=0.75
        let k = mk_kline(100.0, 103.5, 99.5, 103.0);
        let m = metrics_for(&k, Some(100.0));
        assert_eq!(classify(&m), CandleClass::MediumBull);
    }

    #[test]
    fn t_classify_small_bull() {
        // abs_rel <= 1.5%：open=100, close=101 (1%)
        // high=101.3, low=99.7 → body=1, upper=0.3, lower=0.3, range=1.6, body_ratio=0.625
        let k = mk_kline(100.0, 101.3, 99.7, 101.0);
        let m = metrics_for(&k, Some(100.0));
        assert_eq!(classify(&m), CandleClass::SmallBull);
    }

    #[test]
    fn t_classify_big_bear_on_large_negative_rel() {
        // 阴线 abs_rel > 4%：open=100, close=94 (-6%)
        // high=101, low=93 → body=6, upper=1, lower=1, range=8, body_ratio=0.75
        let k = mk_kline(100.0, 101.0, 93.0, 94.0);
        let m = metrics_for(&k, Some(100.0));
        assert_eq!(classify(&m), CandleClass::BigBear);
    }

    #[test]
    fn t_classify_medium_bear() {
        let k = mk_kline(100.0, 100.5, 96.5, 97.0);
        let m = metrics_for(&k, Some(100.0));
        assert_eq!(classify(&m), CandleClass::MediumBear);
    }

    #[test]
    fn t_classify_small_bear() {
        let k = mk_kline(100.0, 100.3, 98.7, 99.0);
        let m = metrics_for(&k, Some(100.0));
        assert_eq!(classify(&m), CandleClass::SmallBear);
    }
}
