//! B7：聚合的 TrendState 输出

use serde::{Deserialize, Serialize};

use crate::data::Kline;

use super::channel::Channel;
use super::dow::{self, DowState};
use super::gap::{self, Gap};
use super::lines::{self, TrendLine};
use super::sr::{self, SrLevel};
use super::swing::{self, SwingParams, SwingPoint};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrendState {
    pub swings: Vec<SwingPoint>,
    pub dow: DowState,
    pub trend_lines: Vec<TrendLine>,
    pub channel: Option<Channel>,
    pub sr_levels: Vec<SrLevel>,
    pub gaps: Vec<Gap>,
    /// 统计
    pub bars: usize,
    /// 当前是否在通道内（百分位）
    pub channel_position: Option<f64>,
}

pub fn compute_trend_state(klines: &[Kline]) -> TrendState {
    let n = klines.len();
    if n == 0 {
        return TrendState {
            swings: vec![],
            dow: DowState {
                phase: dow::DowPhase::Unknown,
                phase_label: dow::DowPhase::Unknown.label().to_string(),
                recent_swings: vec![],
                last_highs: vec![],
                last_lows: vec![],
                structure_age_bars: 0,
                last_bar_index: 0,
            },
            trend_lines: vec![],
            channel: None,
            sr_levels: vec![],
            gaps: vec![],
            bars: 0,
            channel_position: None,
        };
    }

    let swings = swing::detect(klines, &SwingParams::default());
    let dow_state = dow::classify(&swings, n - 1);
    let trend_lines = lines::fit_lines(&swings, klines, 0.015); // 1.5% tol + E31 3% 画法校验
    let channel = super::channel::detect(&trend_lines, n - 1);
    let sr_levels = sr::cluster_levels(&swings, 0.008, n - 1); // 0.8% tolerance
    let gaps = gap::detect(klines, 0.003, dow_state.phase, n);

    // 计算当前价格在通道内的位置（0 = 下轨，1 = 上轨）
    let channel_position = channel.as_ref().and_then(|c| {
        let last = klines.last()?;
        let upper = c.upper.p1_price
            + c.upper.slope_per_bar * ((n - 1) as f64 - c.upper.p1_index as f64);
        let lower = c.lower.p1_price
            + c.lower.slope_per_bar * ((n - 1) as f64 - c.lower.p1_index as f64);
        let w = upper - lower;
        if w.abs() < 1e-9 {
            None
        } else {
            Some((last.close - lower) / w)
        }
    });

    TrendState {
        swings,
        dow: dow_state,
        trend_lines,
        channel,
        sr_levels,
        gaps,
        bars: n,
        channel_position,
    }
}
