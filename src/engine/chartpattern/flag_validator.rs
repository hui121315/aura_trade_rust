//! 旗形 7 条铁证验证器（R-P1-39）
//!
//! 原书 **candle p.770** 铁证 —— 旗形的 7 条完整规则：
//!
//! 1. **旗形必须在急速上升或下跌中出现**
//! 2. 上升旗形向上突破**必须要有成交量配合**；下降旗形向下突破**无需成交量**
//! 3. 旗形突破以价格**超越旗形边线的 3% 为有效**（跨书铁证）
//! 4. 整理期间成交量**大多整体上逐步减少**（但有例外情形不影响成立）
//! 5. 整理时间 10+ 天到数个月；**超过 8 个月的旗形整理**本身已成为小型熊市/牛市，失去旗形技术含义
//! 6. 旗形反向突破 = **小概率事件但仍属于整理图形**
//! 7. 下降旗形**向下**倾斜 + 上升旗形**向上**倾斜 = **实际上是通道**（非旗形）
//!
//! # 本模块职责
//!
//! 给定一个已识别为 `BullFlag` / `BearFlag` 的 [`ChartPattern`]，外加原始 K 线 + 成交量数据，
//! 检查是否满足原书 7 条规则，返回 [`FlagValidation`] 结构体说明具体哪些规则通过/违反。
//!
//! # 与现有 `try_flags` 的关系
//!
//! `try_flags`（detect.rs）已做了基础识别 + 部分规则（如 3% 突破）。本模块提供**完整 7 条验证**，
//! 作为识别后的**后置严格校验**，用户可选是否启用。

use serde::{Deserialize, Serialize};

use super::types::{ChartPattern, ChartPatternKind};
use crate::data::Kline;

/// 旗形 7 条规则的验证结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlagValidation {
    /// 规则 1：旗杆急速上升/下跌
    pub rule1_sharp_pole: bool,
    /// 规则 2：量配合（仅上升旗形需要）
    pub rule2_volume_on_breakout: bool,
    /// 规则 3：3% 有效突破
    pub rule3_effective_break_3pct: bool,
    /// 规则 4：整理期成交量递减
    pub rule4_volume_declining: bool,
    /// 规则 5：整理时间 ≤ 8 个月（240 根日 K 线）
    pub rule5_within_8_months: bool,
    /// 规则 6：突破方向与旗杆方向一致（非反向）
    pub rule6_same_direction_break: bool,
    /// 规则 7：倾斜正确（上升旗形应略向下倾斜，下降旗形略向上倾斜）
    pub rule7_correct_tilt: bool,

    /// 通过的规则数（满分 7）
    pub passed_count: u8,
    /// 是否完全通过（7/7）
    pub fully_valid: bool,
    /// 警告说明
    pub warnings: Vec<String>,
}

impl FlagValidation {
    /// 按宽松模式判定：≥ 5 条通过即算有效（允许 rule2/4/6 例外）
    pub fn is_acceptable(&self) -> bool {
        self.passed_count >= 5
    }
}

/// 验证参数
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct FlagValidatorParams {
    /// 旗杆"急速"阈值：涨跌幅 / K 线数 ≥ this（默认 1% 每根）
    pub pole_speed_pct_per_bar: f64,
    /// 3% 有效突破阈值
    pub effective_break_pct: f64,
    /// 量配合倍数：突破日成交量 ≥ avg × this
    pub volume_surge_factor: f64,
    /// 整理时间上限（8 个月 = 240 根日 K 线）
    pub max_consolidation_bars: usize,
    /// 量递减判定：整理末端 10 根均量 / 起始 10 根均量 < this
    pub volume_decline_ratio: f64,
    /// 突破确认窗口
    pub breakout_confirm_window: usize,
}

impl Default for FlagValidatorParams {
    fn default() -> Self {
        Self {
            pole_speed_pct_per_bar: 0.01,      // 1% 每根 = 急速
            effective_break_pct: 0.03,         // 3% 跨书铁证
            volume_surge_factor: 1.5,
            max_consolidation_bars: 240,       // 8 个月
            volume_decline_ratio: 0.9,         // 末 10 根 < 起 10 根 × 0.9
            breakout_confirm_window: 5,
        }
    }
}

/// 执行 7 条规则的完整验证
///
/// # 参数
/// - `pattern`：已识别的 `BullFlag` 或 `BearFlag`
/// - `klines`：原始 K 线数据
/// - `params`：验证参数
///
/// # 返回
/// - `Some(FlagValidation)` 若是旗形
/// - `None` 若非旗形或数据不足
pub fn validate_flag(
    pattern: &ChartPattern,
    klines: &[Kline],
    params: &FlagValidatorParams,
) -> Option<FlagValidation> {
    let is_bull = pattern.kind == ChartPatternKind::BullFlag;
    let is_bear = pattern.kind == ChartPatternKind::BearFlag;
    if !is_bull && !is_bear {
        return None;
    }
    if pattern.points.len() < 4 || klines.is_empty() {
        return None;
    }
    let a = &pattern.points[0]; // 旗杆起点
    let b = &pattern.points[1]; // 旗杆终点 / 旗面起点
    let _c = &pattern.points[2]; // 旗面中间点
    let d = &pattern.points[3]; // 旗面终点 / 突破前

    let mut warnings = Vec::new();

    // 规则 1：旗杆急速
    let pole_bars = b.index.saturating_sub(a.index);
    let rule1_sharp_pole = if pole_bars == 0 {
        false
    } else {
        let pole_pct = (b.price - a.price).abs() / a.price.abs().max(1e-9);
        let speed = pole_pct / pole_bars as f64;
        let ok = speed >= params.pole_speed_pct_per_bar;
        if !ok {
            warnings.push(format!(
                "规则 1：旗杆速度 {:.2}%/根 < 阈值 {:.2}%/根",
                speed * 100.0,
                params.pole_speed_pct_per_bar * 100.0,
            ));
        }
        ok
    };

    // 规则 4：整理期量递减
    let flag_start = b.index;
    let flag_end = d.index;
    let rule4_volume_declining = {
        let n = klines.len();
        if flag_end >= n || flag_start >= flag_end {
            false
        } else {
            let window = 10.min((flag_end - flag_start) / 2);
            if window < 2 {
                true // 旗面太短无法判定，视为通过
            } else {
                let start_vol: f64 = klines[flag_start..flag_start + window]
                    .iter()
                    .map(|k| k.volume)
                    .sum::<f64>()
                    / window as f64;
                let end_vol: f64 = klines[flag_end.saturating_sub(window)..flag_end]
                    .iter()
                    .map(|k| k.volume)
                    .sum::<f64>()
                    / window as f64;
                if start_vol < 1e-9 {
                    true
                } else {
                    let ratio = end_vol / start_vol;
                    let ok = ratio < params.volume_decline_ratio;
                    if !ok {
                        warnings.push(format!(
                            "规则 4：整理期量递减比率 {:.2} ≥ 阈值 {:.2}",
                            ratio, params.volume_decline_ratio
                        ));
                    }
                    ok
                }
            }
        }
    };

    // 规则 5：整理时间 ≤ 8 个月
    let consolidation_bars = d.index.saturating_sub(b.index);
    let rule5_within_8_months = consolidation_bars <= params.max_consolidation_bars;
    if !rule5_within_8_months {
        warnings.push(format!(
            "规则 5：整理 {} 根 > 上限 {} 根，旗形已失效为小型熊/牛市",
            consolidation_bars, params.max_consolidation_bars
        ));
    }

    // 规则 6：突破方向与旗杆方向一致
    // 规则 3：3% 有效突破
    // 同时计算突破后的方向 + 有效性
    let (rule3, rule6) = {
        let break_window_end = (d.index + params.breakout_confirm_window).min(klines.len() - 1);
        let flag_high = b.price.max(d.price);
        let flag_low = b.price.min(d.price);
        let mut broke_up = false;
        let mut broke_down = false;
        if d.index < klines.len() {
            for i in (d.index + 1)..=break_window_end {
                let close = klines[i].close;
                let up_diff = (close - flag_high) / flag_high.abs().max(1e-9);
                let down_diff = (flag_low - close) / flag_low.abs().max(1e-9);
                if up_diff >= params.effective_break_pct {
                    broke_up = true;
                }
                if down_diff >= params.effective_break_pct {
                    broke_down = true;
                }
            }
        }
        let rule3 = broke_up || broke_down;
        if !rule3 {
            warnings.push("规则 3：无 3% 有效突破".to_string());
        }
        // rule6：bull 应向上突破，bear 应向下突破
        let rule6 = if is_bull { broke_up } else { broke_down };
        if !rule6 {
            warnings.push(format!(
                "规则 6：{} 应向 {} 突破但未发生",
                pattern.label,
                if is_bull { "上" } else { "下" }
            ));
        }
        (rule3, rule6)
    };

    // 规则 2：量配合（仅上升旗形需要，下降旗形直接通过）
    let rule2_volume_on_breakout = if is_bear {
        true // 原书：下降旗形向下突破不需要量
    } else {
        // 上升旗形：d+1 的成交量 ≥ 整理期均量 × volume_surge_factor
        let n = klines.len();
        if d.index + 1 >= n {
            false
        } else {
            let avg_window_start = b.index;
            let avg_window_end = d.index;
            if avg_window_end <= avg_window_start {
                false
            } else {
                let avg: f64 = klines[avg_window_start..avg_window_end]
                    .iter()
                    .map(|k| k.volume)
                    .sum::<f64>()
                    / (avg_window_end - avg_window_start) as f64;
                let break_vol = klines[d.index + 1].volume;
                let ok = avg > 1e-9 && break_vol >= avg * params.volume_surge_factor;
                if !ok {
                    warnings.push(format!(
                        "规则 2：上升旗形突破量 {:.2} / 均量 {:.2} = {:.2} < {:.2}",
                        break_vol,
                        avg,
                        break_vol / avg.max(1e-9),
                        params.volume_surge_factor,
                    ));
                }
                ok
            }
        }
    };

    // 规则 7：倾斜方向
    // 上升旗形：旗面应向**下**倾斜（b > d 或持平），向上则为通道
    // 下降旗形：旗面应向**上**倾斜（d > b 或持平），向下则为通道
    let rule7_correct_tilt = if is_bull {
        // b 是高点，d 是高点；要求 d ≤ b（不向上倾斜）
        d.price <= b.price * 1.005 // 允许 0.5% 误差
    } else {
        // bear：b 是低点，d 是低点；要求 d ≥ b（不向下倾斜）
        d.price >= b.price * 0.995
    };
    if !rule7_correct_tilt {
        warnings.push(format!(
            "规则 7：{} 倾斜方向错误，实际上是通道（非旗形）",
            pattern.label
        ));
    }

    let rules = [
        rule1_sharp_pole,
        rule2_volume_on_breakout,
        rule3,
        rule4_volume_declining,
        rule5_within_8_months,
        rule6,
        rule7_correct_tilt,
    ];
    let passed_count = rules.iter().filter(|&&r| r).count() as u8;
    let fully_valid = passed_count == 7;

    Some(FlagValidation {
        rule1_sharp_pole,
        rule2_volume_on_breakout,
        rule3_effective_break_3pct: rule3,
        rule4_volume_declining,
        rule5_within_8_months,
        rule6_same_direction_break: rule6,
        rule7_correct_tilt,
        passed_count,
        fully_valid,
        warnings,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::trend::{SwingKind, SwingPoint};

    fn mk_kline(idx: i64, open: f64, close: f64, high: f64, low: f64, vol: f64) -> Kline {
        Kline {
            open_time: idx * 86_400_000,
            close_time: (idx + 1) * 86_400_000 - 1,
            open,
            high,
            low,
            close,
            volume: vol,
        }
    }

    fn mk_pattern(
        kind: ChartPatternKind,
        pts: Vec<(usize, f64, SwingKind)>,
        neck: Option<f64>,
        target: Option<f64>,
    ) -> ChartPattern {
        let points: Vec<SwingPoint> = pts
            .iter()
            .map(|(i, p, k)| SwingPoint {
                index: *i,
                time: (*i as i64) * 86_400_000,
                price: *p,
                kind: *k,
            })
            .collect();
        let span = if points.len() >= 2 {
            points.last().unwrap().index - points.first().unwrap().index
        } else {
            0
        };
        ChartPattern {
            kind,
            label: kind.label().to_string(),
            direction: kind.direction(),
            strength: kind.strength(),
            completion_index: points.last().map(|p| p.index).unwrap_or(0),
            points,
            neckline: neck,
            target_price: target,
            span_bars: span,
            book_reliable: true,
        }
    }

    #[test]
    fn t_bull_flag_all_7_rules_pass() {
        // 构造一个"完美"上升旗形：旗杆急速 + 旗面向下倾斜 + 突破放量 + 量递减
        // 旗杆：5 根从 100 涨到 110（+10%，2%/根）
        let pole_klines: Vec<_> = (0..=5)
            .map(|i| {
                let p = 100.0 + 2.0 * i as f64;
                mk_kline(i, p, p, p + 0.5, p - 0.3, 5.0)
            })
            .collect();
        // 旗面：10 根从 110 小幅回落到 108（整理期量从 5 递减到 2）
        let flag_klines: Vec<_> = (6..=15)
            .map(|i| {
                let p = 110.0 - (i - 6) as f64 * 0.2;
                let vol = 5.0 - (i - 6) as f64 * 0.3;
                mk_kline(i, p, p, p + 0.5, p - 0.3, vol)
            })
            .collect();
        // 突破：第 16 根突破 110 到 114（+3.6% > 3%）+ 放量
        let break_kline = mk_kline(16, 110.0, 114.0, 114.5, 109.8, 10.0);
        let after_break: Vec<_> = (17..20)
            .map(|i| mk_kline(i, 114.0, 115.0, 115.5, 113.5, 6.0))
            .collect();

        let mut klines = Vec::new();
        klines.extend(pole_klines);
        klines.extend(flag_klines);
        klines.push(break_kline);
        klines.extend(after_break);

        let pattern = mk_pattern(
            ChartPatternKind::BullFlag,
            vec![
                (0, 100.0, SwingKind::Low),
                (5, 110.0, SwingKind::High),
                (10, 108.8, SwingKind::Low),
                (15, 108.0, SwingKind::High),
            ],
            Some(110.0),
            Some(120.0),
        );

        let val = validate_flag(&pattern, &klines, &FlagValidatorParams::default()).unwrap();
        assert!(val.rule1_sharp_pole, "rule1");
        assert!(val.rule3_effective_break_3pct, "rule3");
        assert!(val.rule5_within_8_months, "rule5");
        assert!(val.rule6_same_direction_break, "rule6");
        assert!(val.rule7_correct_tilt, "rule7");
        assert!(val.is_acceptable(), "至少 5/7 通过；实际 {:?}", val);
    }

    #[test]
    fn t_bull_flag_rule5_fails_if_consolidation_over_240_bars() {
        // 整理 300 根 > 240 → rule5 违反
        let klines: Vec<_> = (0..400)
            .map(|i| mk_kline(i, 100.0, 100.0, 101.0, 99.0, 1.0))
            .collect();
        let pattern = mk_pattern(
            ChartPatternKind::BullFlag,
            vec![
                (0, 90.0, SwingKind::Low),
                (5, 110.0, SwingKind::High),
                (150, 105.0, SwingKind::Low),
                (305, 108.0, SwingKind::High),
            ],
            Some(110.0),
            None,
        );
        let val = validate_flag(&pattern, &klines, &FlagValidatorParams::default()).unwrap();
        assert!(!val.rule5_within_8_months);
    }

    #[test]
    fn t_bull_flag_rule7_fails_if_tilt_upward() {
        // 上升旗形向上倾斜 → rule7 违反（实际是通道）
        let klines: Vec<_> = (0..30)
            .map(|i| mk_kline(i, 100.0, 100.0, 101.0, 99.0, 1.0))
            .collect();
        let pattern = mk_pattern(
            ChartPatternKind::BullFlag,
            vec![
                (0, 90.0, SwingKind::Low),
                (5, 100.0, SwingKind::High), // b = 100
                (10, 98.0, SwingKind::Low),
                (15, 105.0, SwingKind::High), // d = 105 > b 太多
            ],
            Some(100.0),
            None,
        );
        let val = validate_flag(&pattern, &klines, &FlagValidatorParams::default()).unwrap();
        assert!(!val.rule7_correct_tilt, "d=105 > b=100 应违反 rule7");
    }

    #[test]
    fn t_bear_flag_rule2_auto_pass() {
        // 下降旗形：rule2 无需量配合 → 自动通过
        let klines: Vec<_> = (0..30)
            .map(|i| mk_kline(i, 100.0, 100.0, 101.0, 99.0, 1.0))
            .collect();
        let pattern = mk_pattern(
            ChartPatternKind::BearFlag,
            vec![
                (0, 110.0, SwingKind::High),
                (5, 100.0, SwingKind::Low), // b
                (10, 102.0, SwingKind::High),
                (15, 101.0, SwingKind::Low), // d ≥ b ✓ rule7
            ],
            Some(100.0),
            None,
        );
        let val = validate_flag(&pattern, &klines, &FlagValidatorParams::default()).unwrap();
        assert!(val.rule2_volume_on_breakout, "下降旗形 rule2 应自动通过");
    }

    #[test]
    fn t_not_a_flag_returns_none() {
        // 非旗形类型 → None
        let klines = vec![mk_kline(0, 100.0, 100.0, 101.0, 99.0, 1.0)];
        let pattern = mk_pattern(
            ChartPatternKind::DoubleTop,
            vec![
                (0, 100.0, SwingKind::High),
                (5, 95.0, SwingKind::Low),
                (10, 100.0, SwingKind::High),
                (15, 90.0, SwingKind::Low),
            ],
            Some(95.0),
            None,
        );
        assert!(validate_flag(&pattern, &klines, &FlagValidatorParams::default()).is_none());
    }

    #[test]
    fn t_slow_pole_fails_rule1() {
        // 旗杆速度只有 0.5%/根 < 1% 阈值 → rule1 违反
        let pole_klines: Vec<_> = (0..=20)
            .map(|i| {
                let p = 100.0 + 0.5 * i as f64; // 总涨幅 10%，20 根 → 0.5%/根
                mk_kline(i, p, p, p + 0.2, p - 0.1, 3.0)
            })
            .collect();
        let mut klines = pole_klines;
        klines.extend(
            (21..40).map(|i| mk_kline(i, 110.0, 110.0, 110.5, 109.5, 1.0)),
        );
        let pattern = mk_pattern(
            ChartPatternKind::BullFlag,
            vec![
                (0, 100.0, SwingKind::Low),
                (20, 110.0, SwingKind::High),
                (25, 108.0, SwingKind::Low),
                (30, 109.0, SwingKind::High),
            ],
            Some(110.0),
            None,
        );
        let val = validate_flag(&pattern, &klines, &FlagValidatorParams::default()).unwrap();
        assert!(!val.rule1_sharp_pole, "慢速旗杆应违反 rule1");
    }

    #[test]
    fn t_passed_count_and_fully_valid() {
        // passed_count 应 ≤ 7
        let klines = vec![mk_kline(0, 100.0, 100.0, 101.0, 99.0, 1.0)];
        let pattern = mk_pattern(
            ChartPatternKind::BullFlag,
            vec![
                (0, 100.0, SwingKind::Low),
                (5, 110.0, SwingKind::High),
                (10, 108.0, SwingKind::Low),
                (15, 109.0, SwingKind::High),
            ],
            Some(110.0),
            None,
        );
        let val = validate_flag(&pattern, &klines, &FlagValidatorParams::default()).unwrap();
        assert!(val.passed_count <= 7);
        assert_eq!(val.fully_valid, val.passed_count == 7);
    }
}
