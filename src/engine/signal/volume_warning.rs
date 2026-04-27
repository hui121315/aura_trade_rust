//! F9：成交量异常警告（R-P1-26，Sprint 14）
//!
//! 原书 **trend p.200 附近**铁证：
//! > "趋势线被一字跌停击破时（无量），杀伤力最强，是最凶险的信号。"
//!
//! 本模块检测两类成交量异常：
//!
//! 1. **无量跌停**：单根 K 线跌幅 ≥ 跌停阈值（默认 -9%）+ 成交量接近 0（<历史均量 × 某比例）
//! 2. **无量涨停**：涨幅 ≥ 涨停阈值（默认 +9%）+ 成交量萎缩
//!
//! # 工程含义
//!
//! - 无量跌停 = 主力弃盘 / 黑天鹅 → **不可抗力风险**，需**立即清仓**
//! - 无量涨停 = 主力封死 / 筹码高度集中 → 短期顶部预警

use serde::{Deserialize, Serialize};

use crate::data::Kline;

/// 成交量异常类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum VolumeAnomalyKind {
    /// 无量跌停（最凶险，原书警语）
    LimitDownNoVolume,
    /// 无量涨停
    LimitUpNoVolume,
}

impl VolumeAnomalyKind {
    pub fn label(&self) -> &'static str {
        match self {
            VolumeAnomalyKind::LimitDownNoVolume => "无量跌停（最凶险）",
            VolumeAnomalyKind::LimitUpNoVolume => "无量涨停",
        }
    }

    pub fn direction(&self) -> i8 {
        match self {
            VolumeAnomalyKind::LimitDownNoVolume => -1,
            VolumeAnomalyKind::LimitUpNoVolume => 1,
        }
    }
}

/// 成交量异常事件
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VolumeAnomalyEvent {
    pub index: usize,
    pub kind: VolumeAnomalyKind,
    /// 当根涨跌幅
    pub pct_change: f64,
    /// 成交量缩减比例（当量 / 历史均量）
    pub volume_ratio: f64,
}

/// 参数
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct VolumeWarningParams {
    /// 跌停阈值（默认 -0.09，即 -9%）
    pub limit_down_pct: f64,
    /// 涨停阈值（默认 +0.09）
    pub limit_up_pct: f64,
    /// "无量"定义：当根量 / 近 N 根均量 < this（默认 0.3）
    pub shrink_ratio: f64,
    /// 历史成交量回看窗口（默认 20）
    pub lookback_window: usize,
}

impl Default for VolumeWarningParams {
    fn default() -> Self {
        Self {
            limit_down_pct: -0.09,
            limit_up_pct: 0.09,
            shrink_ratio: 0.3,
            lookback_window: 20,
        }
    }
}

/// 检测无量涨停/跌停
pub fn detect_volume_anomalies(
    klines: &[Kline],
    params: &VolumeWarningParams,
) -> Vec<VolumeAnomalyEvent> {
    let n = klines.len();
    if n <= params.lookback_window {
        return Vec::new();
    }

    let mut out = Vec::new();
    for i in params.lookback_window..n {
        let k = &klines[i];
        if !k.close.is_finite() || !k.open.is_finite() || k.open.abs() < 1e-9 {
            continue;
        }
        // 计算涨跌幅：用（收盘 - 前收）/ 前收
        let prev_close = klines[i - 1].close;
        if prev_close.abs() < 1e-9 {
            continue;
        }
        let pct = (k.close - prev_close) / prev_close;
        // 历史均量
        let lo = i - params.lookback_window;
        let avg_vol = klines[lo..i]
            .iter()
            .map(|k| k.volume)
            .sum::<f64>()
            / params.lookback_window as f64;
        if avg_vol < 1e-9 {
            continue;
        }
        let ratio = k.volume / avg_vol;
        if ratio > params.shrink_ratio {
            continue;
        }
        // 判定
        let kind = if pct <= params.limit_down_pct {
            Some(VolumeAnomalyKind::LimitDownNoVolume)
        } else if pct >= params.limit_up_pct {
            Some(VolumeAnomalyKind::LimitUpNoVolume)
        } else {
            None
        };
        if let Some(k) = kind {
            out.push(VolumeAnomalyEvent {
                index: i,
                kind: k,
                pct_change: pct,
                volume_ratio: ratio,
            });
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn t_limit_down_no_volume_detected() {
        // 前 20 根 close=100 volume=10，第 21 根 close=90（-10%）volume=1（缩量 10%）
        let mut klines: Vec<_> = (0..20)
            .map(|i| mk_kline(i, 100.0, 100.0, 101.0, 99.0, 10.0))
            .collect();
        klines.push(mk_kline(20, 100.0, 90.0, 100.5, 89.5, 1.0));
        let events = detect_volume_anomalies(&klines, &VolumeWarningParams::default());
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].kind, VolumeAnomalyKind::LimitDownNoVolume);
    }

    #[test]
    fn t_limit_up_no_volume_detected() {
        // 前 20 根 volume=10，第 21 根 close=110（+10%）volume=2
        let mut klines: Vec<_> = (0..20)
            .map(|i| mk_kline(i, 100.0, 100.0, 101.0, 99.0, 10.0))
            .collect();
        klines.push(mk_kline(20, 100.0, 110.0, 110.5, 99.5, 2.0));
        let events = detect_volume_anomalies(&klines, &VolumeWarningParams::default());
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].kind, VolumeAnomalyKind::LimitUpNoVolume);
    }

    #[test]
    fn t_normal_limit_down_with_volume_not_anomaly() {
        // 跌停但量很大（没有无量）→ 不算 anomaly
        let mut klines: Vec<_> = (0..20)
            .map(|i| mk_kline(i, 100.0, 100.0, 101.0, 99.0, 10.0))
            .collect();
        klines.push(mk_kline(20, 100.0, 90.0, 100.5, 89.5, 15.0)); // 放量跌停
        let events = detect_volume_anomalies(&klines, &VolumeWarningParams::default());
        assert_eq!(events.len(), 0);
    }

    #[test]
    fn t_small_drop_no_anomaly() {
        // 只跌 2%，非跌停
        let mut klines: Vec<_> = (0..20)
            .map(|i| mk_kline(i, 100.0, 100.0, 101.0, 99.0, 10.0))
            .collect();
        klines.push(mk_kline(20, 100.0, 98.0, 100.5, 97.5, 1.0));
        let events = detect_volume_anomalies(&klines, &VolumeWarningParams::default());
        assert_eq!(events.len(), 0);
    }

    #[test]
    fn t_empty_input_returns_empty() {
        assert!(detect_volume_anomalies(&[], &VolumeWarningParams::default()).is_empty());
    }

    #[test]
    fn t_direction_labels() {
        assert_eq!(VolumeAnomalyKind::LimitDownNoVolume.direction(), -1);
        assert_eq!(VolumeAnomalyKind::LimitUpNoVolume.direction(), 1);
    }
}
