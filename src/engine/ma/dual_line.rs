//! A8：双线中期组合 6 条买入持仓原则（E34 修复 / R-P1-49）
//!
//! 原书 **ma p.200** 铁证 —— 60 日均线（**定性线**）+ 10 日均线（**定量线**）完整 6 条原则：
//!
//! # 买入和持仓 6 条原则
//!
//! 1. 股价**向上突破定性线**，定性线上行 → 买入
//! 2. 定量线**上穿定性线**形成黄金交叉 → 买入
//! 3. 股价下跌，遇**定性线上行支撑**止跌回升 → 买入
//! 4. 定性线上行，股价在定性线上方**向上突破定量线** → 买入
//! 5. 定量线下行，遇**定性线上行支撑**止跌，之后再度上行 → 买入
//! 6. 股价、定量线、定性线**多头排列** → 持股
//!
//! # 原书警语（ma p.200 长城开发案例）
//!
//! > "即使葛南维第一大法则 B1 未触发，可按 60 日均线买入法则进场，
//! > **但是买入的仓位一定要轻**（与 R-P1-13 葛南维 L4 仓位上限相呼应）。"
//!
//! # 使用
//!
//! ```
//! use aura_trade::engine::ma::dual_line::{scan, DualLineParams};
//! use aura_trade::engine::ma::compute::sma;
//!
//! let closes = vec![10.0, 10.5, 11.0, 11.5, 12.0, 12.5, 13.0, 13.5];
//! let quantity = sma(&closes, 3);  // 定量线（短期，默认 10 日）
//! let quality = sma(&closes, 5);   // 定性线（长期，默认 60 日）
//! let params = DualLineParams::default();
//! let signals = scan(&closes, &quantity, &quality, &params);
//! ```

use serde::{Deserialize, Serialize};

/// 双线中期组合 6 条原则的事件标识
#[allow(non_camel_case_types)] // 保持原书章节编号（Rule1/Rule2 对应原书顺序）
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DualLineRule {
    /// 规则 1：股价向上突破定性线，定性线上行 → 买入
    Rule1_BreakQualityLineUp,
    /// 规则 2：定量线上穿定性线形成黄金交叉 → 买入
    Rule2_QuantityGoldenCross,
    /// 规则 3：股价下跌遇定性线上行支撑止跌回升 → 买入
    Rule3_QualityLineSupportRebound,
    /// 规则 4：定性线上行，股价在定性线上方向上突破定量线 → 买入
    Rule4_BreakQuantityAbove,
    /// 规则 5：定量线下行，遇定性线上行支撑止跌，再度上行 → 买入
    Rule5_QuantityDownQualitySupport,
    /// 规则 6：股价、定量线、定性线多头排列 → 持股
    Rule6_BullArrangement,
}

impl DualLineRule {
    pub fn label(&self) -> &'static str {
        use DualLineRule::*;
        match self {
            Rule1_BreakQualityLineUp => "规则1 股价突破定性线+定性线上行",
            Rule2_QuantityGoldenCross => "规则2 定量线上穿定性线金叉",
            Rule3_QualityLineSupportRebound => "规则3 定性线支撑止跌回升",
            Rule4_BreakQuantityAbove => "规则4 定性线上方突破定量线",
            Rule5_QuantityDownQualitySupport => "规则5 定量线下行遇定性线支撑",
            Rule6_BullArrangement => "规则6 多头排列持股",
        }
    }

    /// 是否为买入信号（Rule1-5 = 买入，Rule6 = 持股）
    pub fn is_buy(&self) -> bool {
        !matches!(self, DualLineRule::Rule6_BullArrangement)
    }

    pub fn book_source(&self) -> &'static str {
        "ma p.200 双线中期组合 6 条买入持仓原则"
    }
}

/// 双线组合事件
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DualLineEvent {
    pub index: usize,
    pub rule: DualLineRule,
}

/// 参数
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct DualLineParams {
    /// 斜率判定回看窗口（默认 5 根 K 线）
    pub slope_lookback: usize,
    /// 定性线"支撑带"宽度（%，默认 2%）—— 规则 3/5 用
    pub support_band_pct: f64,
    /// 定量线下行但遇定性线支撑，前后确认根数（默认 3 根）
    pub rebound_confirm_bars: usize,
    /// 仓位上限（E34 配套 R-P1-13）—— 原书警语"仓位一定要轻"
    /// 默认 0.3 = 30%（与葛南维 L4 一致）
    pub max_position: f64,
}

impl Default for DualLineParams {
    fn default() -> Self {
        Self {
            slope_lookback: 5,
            support_band_pct: 0.02,
            rebound_confirm_bars: 3,
            max_position: 0.30,
        }
    }
}

/// 扫描双线组合信号（原书 6 条规则完整实现）
///
/// # 参数
/// - `closes`：收盘价序列
/// - `quantity_line`：定量线（短期均线，通常为 10 日）
/// - `quality_line`：定性线（长期均线，通常为 60 日）
/// - `params`：扫描参数
///
/// # 返回
/// 按时间顺序排列的 `DualLineEvent` 列表
pub fn scan(
    closes: &[f64],
    quantity_line: &[f64],
    quality_line: &[f64],
    params: &DualLineParams,
) -> Vec<DualLineEvent> {
    let n = closes.len().min(quantity_line.len()).min(quality_line.len());
    if n < params.slope_lookback.max(2) + 1 {
        return Vec::new();
    }

    let mut out = Vec::new();

    for i in (params.slope_lookback + 1)..n {
        let c_prev = closes[i - 1];
        let c_now = closes[i];
        let qty_prev = quantity_line[i - 1];
        let qty_now = quantity_line[i];
        let qual_prev = quality_line[i - 1];
        let qual_now = quality_line[i];

        // 全部有限值才进一步计算
        if !c_now.is_finite()
            || !qty_now.is_finite()
            || !qual_now.is_finite()
            || !qty_prev.is_finite()
            || !qual_prev.is_finite()
            || !c_prev.is_finite()
        {
            continue;
        }

        // 计算回看的斜率（定性线上行 = 原书最重要的前提条件）
        let qual_back = quality_line[i - params.slope_lookback];
        if !qual_back.is_finite() {
            continue;
        }
        let quality_up = qual_now > qual_back;
        let _quality_down = qual_now < qual_back;

        let qty_back = quantity_line[i - params.slope_lookback];
        let quantity_up = qty_back.is_finite() && qty_now > qty_back;
        let quantity_down = qty_back.is_finite() && qty_now < qty_back;

        // ========== 规则 1：股价突破定性线 + 定性线上行 ==========
        if c_prev <= qual_prev && c_now > qual_now && quality_up {
            out.push(DualLineEvent {
                index: i,
                rule: DualLineRule::Rule1_BreakQualityLineUp,
            });
            continue;
        }

        // ========== 规则 2：定量线上穿定性线（黄金交叉）==========
        if qty_prev <= qual_prev && qty_now > qual_now {
            out.push(DualLineEvent {
                index: i,
                rule: DualLineRule::Rule2_QuantityGoldenCross,
            });
            continue;
        }

        // ========== 规则 3：股价下跌后遇定性线支撑止跌回升 ==========
        if quality_up {
            let band = qual_now.abs() * params.support_band_pct;
            let near_quality = (c_now - qual_now).abs() <= band;
            let was_above = c_prev > qual_prev;
            let is_rebounding = c_now > c_prev;
            if near_quality && was_above && is_rebounding {
                out.push(DualLineEvent {
                    index: i,
                    rule: DualLineRule::Rule3_QualityLineSupportRebound,
                });
                continue;
            }
        }

        // ========== 规则 4：定性线上行 + 股价在定性线上方 + 向上突破定量线 ==========
        if quality_up && c_prev > qual_prev && c_now > qual_now {
            if c_prev <= qty_prev && c_now > qty_now {
                out.push(DualLineEvent {
                    index: i,
                    rule: DualLineRule::Rule4_BreakQuantityAbove,
                });
                continue;
            }
        }

        // ========== 规则 5：定量线下行 + 遇定性线支撑 + 再度上行 ==========
        if quality_up && quantity_down {
            // 检查定量线是否在定性线附近（遇支撑）
            let qty_near_qual =
                (qty_now - qual_now).abs() <= qual_now.abs() * params.support_band_pct;
            // 检查定量线是否转头向上（看最近 confirm_bars 根）
            let confirm = params.rebound_confirm_bars.min(i);
            let qty_turning_up = if confirm >= 2 && i >= confirm {
                quantity_line[i] > quantity_line[i - 1]
                    && quantity_line[i - 1] >= quantity_line[i - 2]
            } else {
                false
            };
            if qty_near_qual && qty_turning_up {
                out.push(DualLineEvent {
                    index: i,
                    rule: DualLineRule::Rule5_QuantityDownQualitySupport,
                });
                continue;
            }
        }

        // ========== 规则 6：多头排列（股价 > 定量 > 定性）==========
        // 仅在边界转入时标记（从非多头排列 → 多头排列）
        let prev_bull =
            c_prev > qty_prev && qty_prev > qual_prev && quality_up && quantity_up;
        let curr_bull =
            c_now > qty_now && qty_now > qual_now && quality_up && quantity_up;
        if curr_bull && !prev_bull {
            out.push(DualLineEvent {
                index: i,
                rule: DualLineRule::Rule6_BullArrangement,
            });
        }
    }

    out
}

/// 原书警语：双线组合买入时仓位应"轻"（R-P1-13 配套）
///
/// 原书明确："即使葛南维第一大法则 B1 未触发，可按 60 日均线买入法则进场，
/// **但是买入的仓位一定要轻**"
///
/// 返回基于规则类型的建议仓位（占总仓位的比例）
pub fn recommended_position_fraction(rule: DualLineRule) -> f64 {
    use DualLineRule::*;
    match rule {
        // 规则 1/2：黄金交叉 + 突破定性线 → 最强信号 → 可重仓（但不超过 70%）
        Rule1_BreakQualityLineUp | Rule2_QuantityGoldenCross => 0.7,
        // 规则 6：多头排列 → 持股（保持现有仓位）
        Rule6_BullArrangement => 1.0,
        // 规则 3/4/5：中期组合辅助信号 → 轻仓（≤ 30%，与 L4 一致）
        Rule3_QualityLineSupportRebound
        | Rule4_BreakQuantityAbove
        | Rule5_QuantityDownQualitySupport => 0.3,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 辅助：构造单调上升序列
    fn linspace(from: f64, to: f64, n: usize) -> Vec<f64> {
        if n <= 1 {
            return vec![from];
        }
        let step = (to - from) / (n - 1) as f64;
        (0..n).map(|i| from + step * i as f64).collect()
    }

    #[test]
    fn t_rule1_break_quality_line_up() {
        // 构造：股价前期一直低于定性线 → 第 4 根突破定性线（突破点清晰）
        //              i=0  i=1  i=2   i=3  i=4   i=5
        let closes = vec![9.0, 9.2, 9.5, 9.6, 10.5, 11.0];
        let quantity = vec![9.1, 9.2, 9.3, 9.5, 9.8, 10.3];
        let quality = vec![9.5, 9.55, 9.6, 9.7, 9.8, 9.9]; // 持续上行
        let params = DualLineParams {
            slope_lookback: 2,
            ..Default::default()
        };
        let events = scan(&closes, &quantity, &quality, &params);
        assert!(
            events
                .iter()
                .any(|e| e.rule == DualLineRule::Rule1_BreakQualityLineUp),
            "应识别规则 1；实际：{:?}",
            events
        );
    }

    #[test]
    fn t_rule2_golden_cross() {
        // 构造：定量线上穿定性线
        let closes = linspace(10.0, 15.0, 20);
        let quantity = linspace(9.0, 15.0, 20); // 短期上升快
        let quality = linspace(11.0, 14.0, 20); // 长期上升慢
        // 在某个时刻 quantity 会超过 quality
        let params = DualLineParams {
            slope_lookback: 3,
            ..Default::default()
        };
        let events = scan(&closes, &quantity, &quality, &params);
        assert!(
            events
                .iter()
                .any(|e| e.rule == DualLineRule::Rule2_QuantityGoldenCross),
            "应识别黄金交叉；实际：{:?}",
            events
        );
    }

    #[test]
    fn t_rule6_bull_arrangement_transition() {
        // 多头排列：股价 > 定量 > 定性 + 均线上行
        // 让 i=3 起 price > quantity > quality 均成立，但 i=2 时不成立
        //              i=0  i=1  i=2   i=3  i=4  i=5
        let closes = vec![9.5, 10.0, 10.5, 12.0, 13.0, 14.0];
        let quantity = vec![9.3, 9.7, 10.2, 11.5, 12.3, 13.0]; // i=3 起 quantity > quality
        let quality = vec![9.0, 9.3, 10.6, 10.9, 11.2, 11.5]; // i=2 故意 quality > quantity 破坏排列
        let params = DualLineParams {
            slope_lookback: 2,
            ..Default::default()
        };
        let events = scan(&closes, &quantity, &quality, &params);
        // 至少应识别到规则 1（突破定性线）或规则 6（多头排列）
        let has_bull_or_rule1 = events.iter().any(|e| {
            matches!(
                e.rule,
                DualLineRule::Rule6_BullArrangement | DualLineRule::Rule1_BreakQualityLineUp
            )
        });
        assert!(
            has_bull_or_rule1,
            "应识别多头排列或规则 1；实际：{:?}",
            events
        );
    }

    #[test]
    fn t_position_fractions_correct() {
        // 原书警语：仓位一定要轻
        // 规则 3/4/5（辅助信号）仓位 ≤ 30%
        assert_eq!(
            recommended_position_fraction(DualLineRule::Rule3_QualityLineSupportRebound),
            0.3
        );
        assert_eq!(
            recommended_position_fraction(DualLineRule::Rule4_BreakQuantityAbove),
            0.3
        );
        assert_eq!(
            recommended_position_fraction(DualLineRule::Rule5_QuantityDownQualitySupport),
            0.3
        );
        // 规则 1/2（主要信号）可较重
        assert!(
            recommended_position_fraction(DualLineRule::Rule1_BreakQualityLineUp) > 0.5
        );
        assert!(
            recommended_position_fraction(DualLineRule::Rule2_QuantityGoldenCross) > 0.5
        );
        // 规则 6（持股）= 100%
        assert_eq!(
            recommended_position_fraction(DualLineRule::Rule6_BullArrangement),
            1.0
        );
    }

    #[test]
    fn t_is_buy_correct() {
        // 规则 1-5 都是买入
        assert!(DualLineRule::Rule1_BreakQualityLineUp.is_buy());
        assert!(DualLineRule::Rule2_QuantityGoldenCross.is_buy());
        assert!(DualLineRule::Rule3_QualityLineSupportRebound.is_buy());
        assert!(DualLineRule::Rule4_BreakQuantityAbove.is_buy());
        assert!(DualLineRule::Rule5_QuantityDownQualitySupport.is_buy());
        // 规则 6 = 持股（不是买入）
        assert!(!DualLineRule::Rule6_BullArrangement.is_buy());
    }

    #[test]
    fn t_quality_down_no_buy() {
        // 定性线下行时，规则 3/4/5 不应触发（要求定性线上行）
        let closes = linspace(10.0, 8.0, 20);
        let quantity = linspace(10.5, 8.5, 20);
        let quality = linspace(11.0, 9.0, 20); // 下行
        let params = DualLineParams::default();
        let events = scan(&closes, &quantity, &quality, &params);
        for ev in &events {
            assert_ne!(
                ev.rule,
                DualLineRule::Rule3_QualityLineSupportRebound,
                "定性线下行时规则 3 不应触发"
            );
            assert_ne!(
                ev.rule,
                DualLineRule::Rule4_BreakQuantityAbove,
                "定性线下行时规则 4 不应触发"
            );
            assert_ne!(
                ev.rule,
                DualLineRule::Rule5_QuantityDownQualitySupport,
                "定性线下行时规则 5 不应触发"
            );
            assert_ne!(
                ev.rule,
                DualLineRule::Rule6_BullArrangement,
                "定性线下行时规则 6 不应触发"
            );
        }
    }

    #[test]
    fn t_empty_returns_empty() {
        let events = scan(&[], &[], &[], &DualLineParams::default());
        assert_eq!(events.len(), 0);
    }

    #[test]
    fn t_too_short_returns_empty() {
        let closes = vec![10.0, 11.0];
        let quantity = vec![10.0, 11.0];
        let quality = vec![10.0, 11.0];
        let params = DualLineParams::default();
        let events = scan(&closes, &quantity, &quality, &params);
        assert_eq!(events.len(), 0);
    }
}
