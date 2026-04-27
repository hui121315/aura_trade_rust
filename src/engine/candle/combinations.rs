//! C5：K 线组合映射（R-P1-09，Sprint 15）
//!
//! 原书 **candle 全书**铁证：单 K 线信号弱，**多 K 线组合**才是强信号。
//!
//! 本模块检测常见的"K 线组合加强"场景：
//!
//! - 锤头 + 次日看涨吞没 = **强烈底反转**
//! - 射击之星 + 次日看跌吞没 = **强烈顶反转**
//! - 长十字 + 次日大阳 = 转势确认
//! - 红三兵 + 均线粘合末端 = 旱地拔葱强化
//!
//! # 与 `advanced.rs::parent_patterns_of` 区别
//!
//! - `parent_patterns_of`：**静态映射**（子形态 ⊂ 父形态）
//! - 本模块：**动态组合**（两个形态在**连续**K 线上出现 → 复合信号）

use serde::{Deserialize, Serialize};

use super::patterns::{PatternHit, PatternKind};

/// K 线组合类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CandleCombination {
    /// 锤头 + 次日看涨吞没 = 强烈底反转
    HammerThenBullishEngulfing,
    /// 射击之星 + 次日看跌吞没 = 强烈顶反转
    ShootingStarThenBearishEngulfing,
    /// 长十字 + 次日大阳 = 转势确认（看涨）
    LongDojiThenBigBull,
    /// 长十字 + 次日大阴 = 转势确认（看跌）
    LongDojiThenBigBear,
    /// 看涨吞没 + 次日继续收阳 = 反转确认
    BullishEngulfingConfirmed,
    /// 看跌吞没 + 次日继续收阴
    BearishEngulfingConfirmed,
    /// 早晨之星 + 后续阳线（3 根组合的延伸确认）
    MorningStarConfirmed,
    /// 黄昏之星 + 后续阴线
    EveningStarConfirmed,
}

impl CandleCombination {
    pub fn label(&self) -> &'static str {
        use CandleCombination::*;
        match self {
            HammerThenBullishEngulfing => "锤头+看涨吞没（强烈底反转）",
            ShootingStarThenBearishEngulfing => "射击之星+看跌吞没（强烈顶反转）",
            LongDojiThenBigBull => "长十字+大阳（看涨转势确认）",
            LongDojiThenBigBear => "长十字+大阴（看跌转势确认）",
            BullishEngulfingConfirmed => "看涨吞没+续涨（反转确认）",
            BearishEngulfingConfirmed => "看跌吞没+续跌（反转确认）",
            MorningStarConfirmed => "早晨之星+续涨",
            EveningStarConfirmed => "黄昏之星+续跌",
        }
    }

    pub fn direction(&self) -> i8 {
        use CandleCombination::*;
        match self {
            HammerThenBullishEngulfing
            | LongDojiThenBigBull
            | BullishEngulfingConfirmed
            | MorningStarConfirmed => 1,
            ShootingStarThenBearishEngulfing
            | LongDojiThenBigBear
            | BearishEngulfingConfirmed
            | EveningStarConfirmed => -1,
        }
    }

    /// 权重倍率（相对单 K 线信号）—— 组合信号通常 ×1.5
    pub fn strength_multiplier(&self) -> f64 {
        1.5
    }
}

/// 组合事件
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CombinationEvent {
    /// 组合完成的 K 线索引（第二个 / 确认 K 线）
    pub confirm_index: usize,
    /// 首根 K 线索引
    pub first_index: usize,
    pub kind: CandleCombination,
}

/// 检测所有 K 线组合
///
/// # 参数
/// - `hits`：K 线识别器的输出（按 `index` 顺序）
///
/// # 算法
/// 按 index 排序 hits，两两相邻检查组合规则
pub fn detect_combinations(hits: &[PatternHit]) -> Vec<CombinationEvent> {
    if hits.len() < 2 {
        return Vec::new();
    }
    let mut sorted: Vec<&PatternHit> = hits.iter().collect();
    sorted.sort_by_key(|h| h.index);

    let mut out = Vec::new();
    for w in sorted.windows(2) {
        let a = w[0];
        let b = w[1];
        // 两根必须相邻（index 相差 ≤ 2，考虑可能跳过 1 根）
        if b.index.saturating_sub(a.index) > 2 || b.index == a.index {
            continue;
        }

        if let Some(kind) = classify_pair(a.kind, b.kind) {
            out.push(CombinationEvent {
                first_index: a.index,
                confirm_index: b.index,
                kind,
            });
        }
    }
    out
}

fn classify_pair(first: PatternKind, second: PatternKind) -> Option<CandleCombination> {
    use CandleCombination::*;
    use PatternKind::*;
    match (first, second) {
        (Hammer, BullishEngulfing) => Some(HammerThenBullishEngulfing),
        (ShootingStar, BearishEngulfing) => Some(ShootingStarThenBearishEngulfing),
        (LongDoji, BigBullCandle) => Some(LongDojiThenBigBull),
        (LongDoji, BigBearCandle) => Some(LongDojiThenBigBear),
        (BullishEngulfing, BigBullCandle) => Some(BullishEngulfingConfirmed),
        (BullishEngulfing, MarubozuBull) => Some(BullishEngulfingConfirmed),
        (BearishEngulfing, BigBearCandle) => Some(BearishEngulfingConfirmed),
        (BearishEngulfing, MarubozuBear) => Some(BearishEngulfingConfirmed),
        (MorningStar, BigBullCandle) => Some(MorningStarConfirmed),
        (MorningStar, MarubozuBull) => Some(MorningStarConfirmed),
        (EveningStar, BigBearCandle) => Some(EveningStarConfirmed),
        (EveningStar, MarubozuBear) => Some(EveningStarConfirmed),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hit(index: usize, kind: PatternKind) -> PatternHit {
        PatternHit {
            index,
            kind,
            direction: kind.direction(),
            strength: 3,
        }
    }

    #[test]
    fn t_hammer_then_bullish_engulfing_detected() {
        let hits = vec![
            hit(5, PatternKind::Hammer),
            hit(6, PatternKind::BullishEngulfing),
        ];
        let combos = detect_combinations(&hits);
        assert_eq!(combos.len(), 1);
        assert_eq!(combos[0].kind, CandleCombination::HammerThenBullishEngulfing);
        assert_eq!(combos[0].kind.direction(), 1);
    }

    #[test]
    fn t_shooting_star_then_bearish_engulfing() {
        let hits = vec![
            hit(5, PatternKind::ShootingStar),
            hit(6, PatternKind::BearishEngulfing),
        ];
        let combos = detect_combinations(&hits);
        assert_eq!(combos.len(), 1);
        assert_eq!(
            combos[0].kind,
            CandleCombination::ShootingStarThenBearishEngulfing
        );
        assert_eq!(combos[0].kind.direction(), -1);
    }

    #[test]
    fn t_long_doji_then_big_bull() {
        let hits = vec![
            hit(10, PatternKind::LongDoji),
            hit(11, PatternKind::BigBullCandle),
        ];
        let combos = detect_combinations(&hits);
        assert_eq!(combos.len(), 1);
        assert_eq!(combos[0].kind, CandleCombination::LongDojiThenBigBull);
    }

    #[test]
    fn t_long_doji_then_big_bear() {
        let hits = vec![
            hit(10, PatternKind::LongDoji),
            hit(11, PatternKind::BigBearCandle),
        ];
        let combos = detect_combinations(&hits);
        assert_eq!(combos.len(), 1);
        assert_eq!(combos[0].kind, CandleCombination::LongDojiThenBigBear);
    }

    #[test]
    fn t_morning_star_confirmed() {
        let hits = vec![
            hit(20, PatternKind::MorningStar),
            hit(21, PatternKind::MarubozuBull),
        ];
        let combos = detect_combinations(&hits);
        assert_eq!(combos.len(), 1);
        assert_eq!(combos[0].kind, CandleCombination::MorningStarConfirmed);
    }

    #[test]
    fn t_bullish_engulfing_confirmed_with_big_bull() {
        // 看涨吞没 + 次日大阳 = 反转确认
        let hits = vec![
            hit(30, PatternKind::BullishEngulfing),
            hit(31, PatternKind::BigBullCandle),
        ];
        let combos = detect_combinations(&hits);
        assert_eq!(combos.len(), 1);
        assert_eq!(combos[0].kind, CandleCombination::BullishEngulfingConfirmed);
        assert_eq!(combos[0].kind.direction(), 1);
    }

    #[test]
    fn t_bullish_engulfing_confirmed_with_marubozu() {
        // 看涨吞没 + 次日光头光脚大阳 = 同样确认
        let hits = vec![
            hit(30, PatternKind::BullishEngulfing),
            hit(31, PatternKind::MarubozuBull),
        ];
        let combos = detect_combinations(&hits);
        assert_eq!(combos.len(), 1);
        assert_eq!(combos[0].kind, CandleCombination::BullishEngulfingConfirmed);
    }

    #[test]
    fn t_bearish_engulfing_confirmed_with_big_bear() {
        // 看跌吞没 + 次日大阴 = 反转确认
        let hits = vec![
            hit(40, PatternKind::BearishEngulfing),
            hit(41, PatternKind::BigBearCandle),
        ];
        let combos = detect_combinations(&hits);
        assert_eq!(combos.len(), 1);
        assert_eq!(combos[0].kind, CandleCombination::BearishEngulfingConfirmed);
        assert_eq!(combos[0].kind.direction(), -1);
    }

    #[test]
    fn t_evening_star_confirmed() {
        // 黄昏之星 + 次日大阴 = 顶部反转确认
        let hits = vec![
            hit(50, PatternKind::EveningStar),
            hit(51, PatternKind::BigBearCandle),
        ];
        let combos = detect_combinations(&hits);
        assert_eq!(combos.len(), 1);
        assert_eq!(combos[0].kind, CandleCombination::EveningStarConfirmed);
        assert_eq!(combos[0].kind.direction(), -1);
    }

    #[test]
    fn t_distant_hits_not_combined() {
        // 相隔 5 根 → 不算组合
        let hits = vec![
            hit(5, PatternKind::Hammer),
            hit(10, PatternKind::BullishEngulfing),
        ];
        let combos = detect_combinations(&hits);
        assert_eq!(combos.len(), 0);
    }

    #[test]
    fn t_non_matching_pair_no_combo() {
        // 锤头 + 大阴 → 不匹配任何组合
        let hits = vec![
            hit(5, PatternKind::Hammer),
            hit(6, PatternKind::BigBearCandle),
        ];
        let combos = detect_combinations(&hits);
        assert_eq!(combos.len(), 0);
    }

    #[test]
    fn t_empty_hits_empty() {
        let combos = detect_combinations(&[]);
        assert!(combos.is_empty());
    }

    #[test]
    fn t_strength_multiplier_1_5() {
        // 所有组合都 ×1.5
        assert_eq!(
            CandleCombination::HammerThenBullishEngulfing.strength_multiplier(),
            1.5
        );
        assert_eq!(
            CandleCombination::EveningStarConfirmed.strength_multiplier(),
            1.5
        );
    }

    #[test]
    fn t_unsorted_input_handled() {
        // 即使输入未排序，函数也应按 index 排序处理
        let hits = vec![
            hit(6, PatternKind::BullishEngulfing),
            hit(5, PatternKind::Hammer), // 逆序
        ];
        let combos = detect_combinations(&hits);
        assert_eq!(combos.len(), 1);
        assert_eq!(combos[0].first_index, 5);
        assert_eq!(combos[0].confirm_index, 6);
    }
}
