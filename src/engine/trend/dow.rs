//! B2：道氏趋势分类
//!
//! 规则：基于最近 4 个摆动点（H-L-H-L 或 L-H-L-H）判断
//! - **Uptrend**（上升）：HH 且 HL（最新高 > 前高，最新低 > 前低）
//! - **Downtrend**（下降）：LH 且 LL
//! - **Consolidation**（整固/盘整）：高点或低点被破坏
//! - **Unknown**：摆动点数量不足

use serde::{Deserialize, Serialize};

use super::swing::{SwingKind, SwingPoint};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DowPhase {
    Uptrend,
    Downtrend,
    Consolidation,
    Unknown,
}

impl DowPhase {
    pub fn label(&self) -> &'static str {
        match self {
            DowPhase::Uptrend => "上升趋势",
            DowPhase::Downtrend => "下降趋势",
            DowPhase::Consolidation => "整固 / 盘整",
            DowPhase::Unknown => "样本不足",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DowState {
    pub phase: DowPhase,
    pub phase_label: String,
    /// 近 4 个摆动点的时间和价格（方便前端直接绘制）
    pub recent_swings: Vec<SwingPoint>,
    /// 最近两次高点和最近两次低点
    pub last_highs: Vec<SwingPoint>,
    pub last_lows: Vec<SwingPoint>,
    /// 可选的延伸计数（未破位已有多少根 K线）
    pub structure_age_bars: usize,
    pub last_bar_index: usize,
}

pub fn classify(swings: &[SwingPoint], last_bar_index: usize) -> DowState {
    let highs: Vec<SwingPoint> = swings.iter().filter(|s| s.kind == SwingKind::High).copied().collect();
    let lows: Vec<SwingPoint> = swings.iter().filter(|s| s.kind == SwingKind::Low).copied().collect();

    let nh = highs.len();
    let nl = lows.len();

    let phase = if nh >= 2 && nl >= 2 {
        let h1 = highs[nh - 1];
        let h0 = highs[nh - 2];
        let l1 = lows[nl - 1];
        let l0 = lows[nl - 2];
        if h1.price > h0.price && l1.price > l0.price {
            DowPhase::Uptrend
        } else if h1.price < h0.price && l1.price < l0.price {
            DowPhase::Downtrend
        } else {
            DowPhase::Consolidation
        }
    } else {
        DowPhase::Unknown
    };

    // 最后一个枢轴距今多少根 K线
    let age = swings
        .last()
        .map(|p| last_bar_index.saturating_sub(p.index))
        .unwrap_or(0);

    DowState {
        phase,
        phase_label: phase.label().to_string(),
        recent_swings: swings.iter().rev().take(6).rev().copied().collect(),
        last_highs: highs.into_iter().rev().take(2).rev().collect(),
        last_lows: lows.into_iter().rev().take(2).rev().collect(),
        structure_age_bars: age,
        last_bar_index,
    }
}
