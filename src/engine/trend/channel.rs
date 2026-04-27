//! B5：平行通道
//!
//! 策略：若支撑趋势线与阻力趋势线斜率差 ≤ 30% 则视为"近似平行通道"，
//! 取两条线作为通道上下轨。

use serde::{Deserialize, Serialize};

use super::lines::{TrendLine, TrendLineKind};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Channel {
    pub upper: TrendLine,
    pub lower: TrendLine,
    /// 两条线当前平均宽度（价格单位）
    pub width: f64,
    /// 平行度：1 - |slope_diff|/|avg_slope|
    pub parallelism: f64,
}

pub fn detect(lines: &[TrendLine], last_index: usize) -> Option<Channel> {
    let sup = lines.iter().find(|l| l.kind == TrendLineKind::Support)?;
    let res = lines.iter().find(|l| l.kind == TrendLineKind::Resistance)?;
    let avg = (sup.slope_per_bar.abs() + res.slope_per_bar.abs()) / 2.0;
    if avg < 1e-9 {
        // 两条线都几乎水平，仍然视作平行通道
        let upper_v = project(res, last_index);
        let lower_v = project(sup, last_index);
        return Some(Channel {
            upper: res.clone(),
            lower: sup.clone(),
            width: (upper_v - lower_v).abs(),
            parallelism: 1.0,
        });
    }
    let diff = (sup.slope_per_bar - res.slope_per_bar).abs();
    let parallelism = (1.0 - diff / avg).max(0.0);
    if parallelism < 0.7 {
        return None; // 不够平行
    }
    let upper_v = project(res, last_index);
    let lower_v = project(sup, last_index);
    Some(Channel {
        upper: res.clone(),
        lower: sup.clone(),
        width: (upper_v - lower_v).abs(),
        parallelism,
    })
}

fn project(line: &TrendLine, idx: usize) -> f64 {
    line.p1_price + line.slope_per_bar * ((idx as f64) - (line.p1_index as f64))
}
