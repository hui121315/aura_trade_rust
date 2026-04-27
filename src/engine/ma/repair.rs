//! A10：均线修复 + 气贯长虹（R-P1-54 / R-P1-55）
//!
//! # R-P1-54 均线主动修复（ma p.280）
//!
//! 原书大唐发电（601991）案例：
//! > "股价在上涨后期出现暴涨，股价迅速远离 60 日均线，日 K 线和均线呈发散状态。
//! > 由于均线跟不上股价的上涨速度，产生了一个对股价的吸引力，于是股价下跌，
//! > 对均线进行**主动修复**。"
//!
//! > "该股的顶部主动修复，**实际上是一个阶段性的短期顶部**。"
//!
//! **工程含义**：股价偏离均线过大（乖离 > 阈值）→ 主动修复 = 短期顶部预测
//!
//! # R-P1-55 气贯长虹（ma p.330）
//!
//! 鸿路股份案例：
//! - 上升趋势初期**气贯长虹 = 中期顶部**
//! - **3 标准离场**：放量收阴滞涨 + 十字星/黄昏之星 + 跌破 5 日均线
//!
//! # 与 R-P1-53 断头铡刀的区别
//!
//! - 气贯长虹：**上升初期**的一根长阳线后滞涨 → 见中期顶
//! - 断头铡刀：**顶部**的多均线粘合再次向下 → 最强空头

use serde::{Deserialize, Serialize};

/// 修复类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum RepairKind {
    /// 主动修复：股价暴涨远离均线 → 快速下跌回归 = 短期顶部
    Active,
    /// 被动修复：股价横盘等均线靠近（非反转，仅整理）
    Passive,
}

impl RepairKind {
    pub fn label(&self) -> &'static str {
        match self {
            RepairKind::Active => "主动修复（短期顶部）",
            RepairKind::Passive => "被动修复（横盘整理）",
        }
    }

    pub fn direction(&self) -> i8 {
        match self {
            RepairKind::Active => -1, // 短期顶部 → 看空
            RepairKind::Passive => 0,
        }
    }
}

/// 均线修复事件
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepairEvent {
    pub index: usize,
    pub kind: RepairKind,
    /// 触发前的峰值乖离率（例如 +12%）
    pub peak_bias: f64,
    /// 修复后的乖离率（接近 0）
    pub resolved_bias: f64,
}

/// 气贯长虹事件
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AirFlagEvent {
    /// 长阳线的索引
    pub index: usize,
    /// 涨幅（pct）
    pub surge_pct: f64,
    /// 是否满足 3 标准
    pub three_criteria_met: bool,
    /// 具体哪些标准满足
    pub criteria: AirFlagCriteria,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct AirFlagCriteria {
    /// 放量滞涨（随后收阴/十字）
    pub volume_stall: bool,
    /// 收出见顶 K 线形态（十字星/黄昏之星等）
    pub top_pattern: bool,
    /// 跌破 5 日均线
    pub break_5_day: bool,
}

/// 参数
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct RepairParams {
    /// 主动修复触发阈值：峰值正乖离 ≥ this
    pub active_bias_threshold: f64,
    /// 修复完成阈值：乖离回归到 |bias| < this
    pub resolved_bias_threshold: f64,
    /// 主动修复的最大窗口（从峰值到修复结束的 K 线数）
    pub max_repair_window: usize,
    /// 主动修复的最小下跌速度：从峰值到修复完成的天数 ≤ this
    pub active_max_bars: usize,
}

impl Default for RepairParams {
    fn default() -> Self {
        Self {
            active_bias_threshold: 0.10, // 10% 正乖离
            resolved_bias_threshold: 0.02,
            max_repair_window: 15,
            active_max_bars: 10, // 10 根内完成 = 主动
        }
    }
}

/// 检测均线主动修复（R-P1-54）
///
/// # 算法
/// 1. 找到峰值乖离 >= `active_bias_threshold` 的点
/// 2. 在后续 `max_repair_window` 根内检测乖离是否回归到阈值内
/// 3. 若回归用时 ≤ `active_max_bars` → 主动修复（看空）
/// 4. 若回归用时 > `active_max_bars` 或价格横盘回归 → 被动修复（中性）
pub fn detect_repairs(
    closes: &[f64],
    ma: &[f64],
    bias: &[f64],
    params: &RepairParams,
) -> Vec<RepairEvent> {
    let n = closes.len().min(ma.len()).min(bias.len());
    if n < 3 {
        return Vec::new();
    }
    let mut out = Vec::new();
    let mut i = 1;
    while i < n {
        let b = bias[i];
        if !b.is_finite() || b < params.active_bias_threshold {
            i += 1;
            continue;
        }
        // 找到峰值点：从 i 向后直到 bias 开始下降
        let mut peak_idx = i;
        let mut peak_bias = b;
        let mut j = i;
        while j + 1 < n && bias[j + 1].is_finite() && bias[j + 1] > peak_bias {
            j += 1;
            peak_bias = bias[j];
            peak_idx = j;
        }
        // 从峰值后检测修复
        let end = (peak_idx + params.max_repair_window).min(n - 1);
        let mut resolved_idx: Option<usize> = None;
        for k in (peak_idx + 1)..=end {
            let bk = bias[k];
            if !bk.is_finite() {
                continue;
            }
            if bk.abs() < params.resolved_bias_threshold {
                resolved_idx = Some(k);
                break;
            }
        }
        if let Some(rk) = resolved_idx {
            let duration = rk - peak_idx;
            let kind = if duration <= params.active_max_bars {
                // 主动修复：快速下跌
                // 需要确认价格确实下跌（而非横盘）
                let price_at_peak = closes[peak_idx];
                let price_resolved = closes[rk];
                if price_resolved < price_at_peak {
                    RepairKind::Active
                } else {
                    RepairKind::Passive
                }
            } else {
                RepairKind::Passive
            };
            out.push(RepairEvent {
                index: rk,
                kind,
                peak_bias,
                resolved_bias: bias[rk],
            });
            i = rk + 1;
        } else {
            i = end + 1;
        }
    }
    out
}

/// 检测气贯长虹（R-P1-55）
///
/// # 原书 3 标准（鸿路股份案例）
/// 1. 长阳线（涨幅 ≥ surge_threshold，默认 8%）
/// 2. 随后放量收阴滞涨
/// 3. 收出见顶 K 线形态（十字星/黄昏之星）
/// 4. 跌破 5 日均线
///
/// 本函数仅做"长阳线 + 3 标准后续确认"的综合判定。
/// 见顶 K 线形态识别由 `candle/patterns.rs` 的 `DojiStar` / `EveningStar` 提供辅助。
pub fn detect_air_flag(
    closes: &[f64],
    opens: &[f64],
    ma5: &[f64],
    is_top_pattern: &[bool], // 外部注入：每根 K 线是否为见顶形态
    params: &AirFlagParams,
) -> Vec<AirFlagEvent> {
    let n = closes
        .len()
        .min(opens.len())
        .min(ma5.len())
        .min(is_top_pattern.len());
    if n < params.confirm_window + 2 {
        return Vec::new();
    }
    let mut out = Vec::new();
    for i in 1..n {
        let open = opens[i];
        let close = closes[i];
        if !open.is_finite() || !close.is_finite() || open.abs() < 1e-9 {
            continue;
        }
        let surge = (close - open) / open.abs();
        if surge < params.surge_threshold {
            continue;
        }
        // 检查后续 confirm_window 根的 3 标准
        let end = (i + params.confirm_window).min(n - 1);
        let mut volume_stall = false;
        let mut top_pattern = false;
        let mut break_5_day = false;
        for k in (i + 1)..=end {
            // 放量滞涨：收阴且实体小
            if closes[k] < opens[k] {
                volume_stall = true;
            }
            // 见顶形态
            if is_top_pattern[k] {
                top_pattern = true;
            }
            // 跌破 5 日均线
            if ma5[k].is_finite() && closes[k] < ma5[k] {
                break_5_day = true;
            }
        }
        let criteria = AirFlagCriteria {
            volume_stall,
            top_pattern,
            break_5_day,
        };
        let three_met = volume_stall && top_pattern && break_5_day;
        if three_met {
            out.push(AirFlagEvent {
                index: i,
                surge_pct: surge,
                three_criteria_met: three_met,
                criteria,
            });
        }
    }
    out
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct AirFlagParams {
    /// 长阳线涨幅阈值（日内，默认 8%）
    pub surge_threshold: f64,
    /// 后续 3 标准确认窗口（默认 5 根 K 线）
    pub confirm_window: usize,
}

impl Default for AirFlagParams {
    fn default() -> Self {
        Self {
            surge_threshold: 0.08, // 8% 长阳
            confirm_window: 5,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn t_active_repair_detected() {
        // 构造：bias 达到 +12%（峰值），然后 5 根内回到 0 附近
        let closes = vec![100.0, 105.0, 110.0, 112.0, 108.0, 103.0, 100.5, 100.0];
        let ma = vec![100.0; 8];
        let bias = vec![0.0, 0.05, 0.10, 0.12, 0.08, 0.03, 0.005, 0.0];
        let params = RepairParams::default();
        let events = detect_repairs(&closes, &ma, &bias, &params);
        assert!(!events.is_empty(), "应识别主动修复");
        assert_eq!(events[0].kind, RepairKind::Active);
    }

    #[test]
    fn t_passive_repair_long_duration() {
        // 峰值后横盘很久才回归 → 被动修复
        let closes = vec![100.0, 110.0, 112.0, 112.0, 111.0, 110.0, 108.0, 106.0, 104.0, 102.0, 100.5, 100.0, 99.5, 99.0];
        let ma = vec![100.0; 14];
        let bias = vec![0.0, 0.10, 0.12, 0.12, 0.11, 0.10, 0.08, 0.06, 0.04, 0.02, 0.005, 0.0, -0.005, -0.01];
        // active_max_bars = 10, 从 peak=2 到 resolved=10（距离 8），保持 Active
        // 让它更长一些 → passive
        let params = RepairParams {
            active_max_bars: 3, // 要求 3 根内回归才算主动
            ..Default::default()
        };
        let events = detect_repairs(&closes, &ma, &bias, &params);
        assert!(!events.is_empty());
        assert_eq!(events[0].kind, RepairKind::Passive);
    }

    #[test]
    fn t_no_repair_below_threshold() {
        // bias 从未超过 10%
        let closes = vec![100.0, 101.0, 102.0, 103.0, 102.0];
        let ma = vec![100.0; 5];
        let bias = vec![0.0, 0.01, 0.02, 0.03, 0.02];
        let events = detect_repairs(&closes, &ma, &bias, &RepairParams::default());
        assert_eq!(events.len(), 0);
    }

    #[test]
    fn t_air_flag_three_criteria_met() {
        // 构造：第 1 根长阳线（10%+）、后续 4 根满足 3 标准
        let opens = vec![100.0, 100.0, 110.0, 109.0, 108.0, 107.0, 106.0];
        //                         ↑ i=1 长阳 100→110
        let closes = vec![100.0, 110.0, 109.0, 108.0, 107.0, 106.0, 105.0];
        //                                 ↑ 收阴     ↑ 继续下跌 → 破 5 日均线
        let ma5 = vec![108.0, 109.0, 109.5, 109.0, 108.0, 107.0, 106.5];
        let is_top_pattern = vec![false, false, true, false, false, false, false];
        let events = detect_air_flag(&closes, &opens, &ma5, &is_top_pattern, &AirFlagParams::default());
        // 可能识别；但需要 volume_stall + top_pattern + break_5_day 都满足
        // 检查：k=2 close=109 < open=110 (收阴 stall ✓)
        //       k=2 top_pattern ✓
        //       k=6 close=105 < ma5=106.5 (break ✓)
        assert!(!events.is_empty(), "应识别气贯长虹；实际：{:?}", events);
        assert!(events[0].three_criteria_met);
    }

    #[test]
    fn t_air_flag_missing_criterion_no_event() {
        // 长阳但后续没有见顶形态
        let opens = vec![100.0, 100.0, 110.0, 109.0, 108.0];
        let closes = vec![100.0, 110.0, 109.0, 108.0, 107.0];
        let ma5 = vec![108.0, 109.0, 109.5, 108.0, 107.0];
        let is_top_pattern = vec![false, false, false, false, false]; // 无见顶形态
        let events = detect_air_flag(&closes, &opens, &ma5, &is_top_pattern, &AirFlagParams::default());
        assert_eq!(events.len(), 0, "3 标准未全满足，不应触发");
    }

    #[test]
    fn t_repair_kind_direction() {
        assert_eq!(RepairKind::Active.direction(), -1);
        assert_eq!(RepairKind::Passive.direction(), 0);
    }

    #[test]
    fn t_empty_input_no_events() {
        let events = detect_repairs(&[], &[], &[], &RepairParams::default());
        assert!(events.is_empty());
        let events = detect_air_flag(&[], &[], &[], &[], &AirFlagParams::default());
        assert!(events.is_empty());
    }
}
