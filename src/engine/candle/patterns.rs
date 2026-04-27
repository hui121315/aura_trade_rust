//! C2：K线形态识别（Phase 1.4 子集）
//!
//! 覆盖邱立波《K线技术分析》第二章中核心的单/双/三根 K线形态。
//! Phase 4 会扩展到 55+ 形态。

use serde::{Deserialize, Serialize};

use crate::data::Kline;

use super::metrics::{metrics_for, CandleClass};

/// 形态种类（对应原书术语，严格中文命名）
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PatternKind {
    // --- 单根 ---
    BigBullCandle,      // 大阳线
    BigBearCandle,      // 大阴线
    DojiStar,           // 十字星
    LongDoji,           // 长十字线
    SpinningTop,        // 螺旋桨
    FlatLine,           // 一字线
    TShape,             // T 字线
    InvTShape,          // 倒 T 字线
    Hammer,             // 锤头线（下跌末端）
    HangingMan,         // 吊颈线 / 绞刑线（上涨末端）
    InvertedHammer,     // 倒锤头（下跌末端）
    ShootingStar,       // 射击之星 / 流星（上涨末端）
    MarubozuBull,       // 光头光脚大阳线
    MarubozuBear,       // 光头光脚大阴线

    // --- 双根 ---
    BullishEngulfing,   // 看涨吞没（穿头破脚 · 阳包阴）
    BearishEngulfing,   // 看跌吞没（穿头破脚 · 阴包阳）
    BullishHarami,      // 看涨身怀六甲
    BearishHarami,      // 看跌身怀六甲
    PiercingLine,       // 曙光初现
    DarkCloudCover,     // 乌云盖顶
    TweezersTop,        // 镊子顶 / 平顶
    TweezersBottom,     // 镊子底 / 平底

    // --- 单根扩展 ---
    GravestoneDoji,     // 墓碑十字线（顶）
    DragonflyDoji,      // 蜻蜓十字线（底）
    OpenMarubozuBull,   // 光头阳线（无上影，阳）
    OpenMarubozuBear,   // 光头阴线（无上影，阴）
    CloseMarubozuBull,  // 光脚阳线（无下影，阳）
    CloseMarubozuBear,  // 光脚阴线（无下影，阴）

    // --- 双根扩展 ---
    InsideBar,          // 内含线 / 孕线结构（不分阴阳）
    OutsideBar,         // 外包线
    BullishHaramiCross, // 看涨十字孕线
    BearishHaramiCross, // 看跌十字孕线
    BullishCounterAttack, // 看涨反击线
    BearishCounterAttack, // 看跌反击线
    UpsideGapTwoCrows,  // 向上跳空两只乌鸦（顶）
    MatchingLow,        // 相同低点 / 对应底
    MatchingHigh,       // 相同高点 / 对应顶

    // --- 三根扩展 ---
    ThreeInsideUp,      // 三内部上涨（反转）
    ThreeInsideDown,    // 三内部下跌
    ThreeOutsideUp,     // 三外部上涨
    ThreeOutsideDown,   // 三外部下跌
    BullishAbandonedBaby, // 看涨弃婴
    BearishAbandonedBaby, // 看跌弃婴
    MorningDojiStar,    // 早晨十字星
    EveningDojiStar,    // 黄昏十字星
    BullishStrike,      // 多方炮（阳-阴-阳，第三根吞没前两根）
    BearishStrike,      // 空方炮
    StickSandwichBull,  // 夹心饼（看涨）
    StickSandwichBear,  // 夹心饼（看跌）

    // --- 多根（4-5）扩展 ---
    RisingThreeMethods,  // 上升三部曲
    FallingThreeMethods, // 下降三部曲
    TowerTop,            // 塔形顶
    TowerBottom,         // 塔形底
    IslandReversalTop,   // 顶部岛型反转
    IslandReversalBottom,// 底部岛型反转

    // --- 三根（原有） ---
    MorningStar,        // 早晨之星
    EveningStar,        // 黄昏之星
    ThreeWhiteSoldiers, // 红三兵（三个白色武士）
    ThreeBlackCrows,    // 黑三兵 / 三只乌鸦
}

impl PatternKind {
    pub fn label(&self) -> &'static str {
        match self {
            PatternKind::BigBullCandle => "大阳线",
            PatternKind::BigBearCandle => "大阴线",
            PatternKind::DojiStar => "十字星",
            PatternKind::LongDoji => "长十字线",
            PatternKind::SpinningTop => "螺旋桨",
            PatternKind::FlatLine => "一字线",
            PatternKind::TShape => "T 字线",
            PatternKind::InvTShape => "倒 T 字线",
            PatternKind::Hammer => "锤头线",
            PatternKind::HangingMan => "吊颈线",
            PatternKind::InvertedHammer => "倒锤头线",
            PatternKind::ShootingStar => "射击之星",
            PatternKind::MarubozuBull => "光头光脚大阳线",
            PatternKind::MarubozuBear => "光头光脚大阴线",
            PatternKind::BullishEngulfing => "看涨吞没（穿头破脚）",
            PatternKind::BearishEngulfing => "看跌吞没（穿头破脚）",
            PatternKind::BullishHarami => "看涨身怀六甲",
            PatternKind::BearishHarami => "看跌身怀六甲",
            PatternKind::PiercingLine => "曙光初现",
            PatternKind::DarkCloudCover => "乌云盖顶",
            PatternKind::TweezersTop => "镊子顶（平顶）",
            PatternKind::TweezersBottom => "镊子底（平底）",
            PatternKind::MorningStar => "早晨之星",
            PatternKind::EveningStar => "黄昏之星",
            PatternKind::ThreeWhiteSoldiers => "红三兵",
            PatternKind::ThreeBlackCrows => "黑三兵（三只乌鸦）",
            PatternKind::GravestoneDoji => "墓碑十字线",
            PatternKind::DragonflyDoji => "蜻蜓十字线",
            PatternKind::OpenMarubozuBull => "光头阳线",
            PatternKind::OpenMarubozuBear => "光头阴线",
            PatternKind::CloseMarubozuBull => "光脚阳线",
            PatternKind::CloseMarubozuBear => "光脚阴线",
            PatternKind::InsideBar => "内含线",
            PatternKind::OutsideBar => "外包线",
            PatternKind::BullishHaramiCross => "看涨十字孕线",
            PatternKind::BearishHaramiCross => "看跌十字孕线",
            PatternKind::BullishCounterAttack => "看涨反击线",
            PatternKind::BearishCounterAttack => "看跌反击线",
            PatternKind::UpsideGapTwoCrows => "向上跳空两只乌鸦",
            PatternKind::MatchingLow => "对应底",
            PatternKind::MatchingHigh => "对应顶",
            PatternKind::ThreeInsideUp => "三内部上涨",
            PatternKind::ThreeInsideDown => "三内部下跌",
            PatternKind::ThreeOutsideUp => "三外部上涨",
            PatternKind::ThreeOutsideDown => "三外部下跌",
            PatternKind::BullishAbandonedBaby => "看涨弃婴",
            PatternKind::BearishAbandonedBaby => "看跌弃婴",
            PatternKind::MorningDojiStar => "早晨十字星",
            PatternKind::EveningDojiStar => "黄昏十字星",
            PatternKind::BullishStrike => "多方炮",
            PatternKind::BearishStrike => "空方炮",
            PatternKind::StickSandwichBull => "看涨夹心饼",
            PatternKind::StickSandwichBear => "看跌夹心饼",
            PatternKind::RisingThreeMethods => "上升三部曲",
            PatternKind::FallingThreeMethods => "下降三部曲",
            PatternKind::TowerTop => "塔形顶",
            PatternKind::TowerBottom => "塔形底",
            PatternKind::IslandReversalTop => "顶部岛型反转",
            PatternKind::IslandReversalBottom => "底部岛型反转",
        }
    }

    /// 1~6 星强度（**基于 9 数据集真实评估**重新排布，详见 PATTERN_EFFECTIVENESS_REPORT.md）
    ///
    /// - 6 星：跨 3 个时间框架 3/3 稳定 + α > 0.6% + 胜率 ≥ 56%
    /// - 5 星：日线级别强信号 + 跨币种 3/3 全正
    /// - 4 星：一般可用 (α 0.1%~0.3%，胜率 51%+)
    /// - 3 星：常见组合但效应弱
    /// - 2 星：中性形态（波动率指示，不含方向）
    /// - 1 星：**真实评估反向失效**（触发频繁但胜率 < 50%，建议屏蔽）
    pub fn strength(&self) -> u8 {
        use PatternKind::*;
        match self {
            // 6 星：真实评估"强可用 ★★★"
            //   光脚阴线：5/5 数据集正，α +1.14%，周线 α +12.5%
            //   看涨反击线：1d/4h 3/3 全正，α +0.76%~+1.18%
            //   看跌反击线：1d 3/3 全正，α +2.05%
            //   塔形顶：1d 3/3 全正，α +1.73%
            //   光头光脚大阳线 / 光头光脚大阴线 (日线强反转)
            BullishCounterAttack | BearishCounterAttack
            | CloseMarubozuBull | CloseMarubozuBear
            | MarubozuBull | MarubozuBear
            | TowerTop => 6,

            // 5 星：日线级别强可用
            DarkCloudCover | PiercingLine
            | ThreeInsideUp | ThreeInsideDown
            | MorningDojiStar | EveningStar | EveningDojiStar
            | GravestoneDoji
            | TowerBottom
            | FlatLine
            | BullishAbandonedBaby | BearishAbandonedBaby
            | IslandReversalTop | IslandReversalBottom => 5,

            // 4 星：一般可用或信号较频繁但 α 轻度正
            ShootingStar | InvertedHammer
            | OpenMarubozuBull | OpenMarubozuBear
            | ThreeWhiteSoldiers | ThreeBlackCrows
            | ThreeOutsideUp | ThreeOutsideDown
            | RisingThreeMethods | FallingThreeMethods
            | BullishStrike | BearishStrike => 4,

            // 3 星：组合但效应不稳
            TweezersTop | TweezersBottom
            | BullishHarami | BearishHarami
            | BullishHaramiCross | BearishHaramiCross
            | UpsideGapTwoCrows
            | MatchingHigh | MatchingLow => 3,

            // 2 星：中性形态（α 恒为 0，只是波动率标签）
            DojiStar | LongDoji | SpinningTop
            | TShape | InvTShape
            | InsideBar | OutsideBar
            | DragonflyDoji => 2,

            // 1 星：**真实评估反向失效**（触发多但胜率 < 50%）
            //   大阳线：9 数据集中 0/4 正 α，α -1.15%
            //   早晨之星：跨数据集 2/5 正 α，α -0.20%
            //   看涨夹心饼 / 看跌夹心饼：α -0.5% ~ -0.65%
            //   吞没线：加密市场反例，1/3 级别反向
            //   锤头 / 吊颈：跨级别反向
            BigBullCandle | BigBearCandle
            | Hammer | HangingMan
            | BullishEngulfing | BearishEngulfing
            | MorningStar
            | StickSandwichBull | StickSandwichBear => 1,
        }
    }

    /// 方向 +1 看涨 / -1 看跌 / 0 中性
    pub fn direction(&self) -> i8 {
        match self {
            PatternKind::BigBullCandle
            | PatternKind::MarubozuBull
            | PatternKind::Hammer
            | PatternKind::InvertedHammer
            | PatternKind::BullishEngulfing
            | PatternKind::BullishHarami
            | PatternKind::PiercingLine
            | PatternKind::TweezersBottom
            | PatternKind::MorningStar
            | PatternKind::ThreeWhiteSoldiers
            | PatternKind::TShape
            | PatternKind::DragonflyDoji
            | PatternKind::OpenMarubozuBull
            | PatternKind::CloseMarubozuBull
            | PatternKind::BullishHaramiCross
            | PatternKind::BullishCounterAttack
            | PatternKind::MatchingLow
            | PatternKind::ThreeInsideUp
            | PatternKind::ThreeOutsideUp
            | PatternKind::BullishAbandonedBaby
            | PatternKind::MorningDojiStar
            | PatternKind::BullishStrike
            | PatternKind::StickSandwichBull
            | PatternKind::RisingThreeMethods
            | PatternKind::TowerBottom
            | PatternKind::IslandReversalBottom => 1,
            PatternKind::BigBearCandle
            | PatternKind::MarubozuBear
            | PatternKind::HangingMan
            | PatternKind::ShootingStar
            | PatternKind::BearishEngulfing
            | PatternKind::BearishHarami
            | PatternKind::DarkCloudCover
            | PatternKind::TweezersTop
            | PatternKind::EveningStar
            | PatternKind::ThreeBlackCrows
            | PatternKind::InvTShape
            | PatternKind::GravestoneDoji
            | PatternKind::OpenMarubozuBear
            | PatternKind::CloseMarubozuBear
            | PatternKind::BearishHaramiCross
            | PatternKind::BearishCounterAttack
            | PatternKind::UpsideGapTwoCrows
            | PatternKind::MatchingHigh
            | PatternKind::ThreeInsideDown
            | PatternKind::ThreeOutsideDown
            | PatternKind::BearishAbandonedBaby
            | PatternKind::EveningDojiStar
            | PatternKind::BearishStrike
            | PatternKind::StickSandwichBear
            | PatternKind::FallingThreeMethods
            | PatternKind::TowerTop
            | PatternKind::IslandReversalTop => -1,
            PatternKind::DojiStar
            | PatternKind::LongDoji
            | PatternKind::SpinningTop
            | PatternKind::FlatLine
            | PatternKind::InsideBar
            | PatternKind::OutsideBar => 0,
        }
    }
}

/// 一次命中
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct PatternHit {
    pub index: usize,
    pub kind: PatternKind,
    pub direction: i8,
    pub strength: u8,
}

/// 判定"位于下跌末端/上涨末端"：用最近 `n` 根收盘价斜率简单判断。
fn trend_context(closes: &[f64], i: usize, n: usize) -> i8 {
    if i < n {
        return 0;
    }
    let start = closes[i - n];
    let end = closes[i];
    if start <= 0.0 {
        return 0;
    }
    let change = (end - start) / start;
    if change > 0.02 {
        1
    } else if change < -0.02 {
        -1
    } else {
        0
    }
}

/// 扫描 K线序列，返回所有形态命中。
pub fn scan(klines: &[Kline]) -> Vec<PatternHit> {
    let mut out = Vec::new();
    let n = klines.len();
    if n == 0 {
        return out;
    }
    let closes: Vec<f64> = klines.iter().map(|k| k.close).collect();

    for i in 0..n {
        let prev_close = if i == 0 { None } else { Some(klines[i - 1].close) };
        let m = metrics_for(&klines[i], prev_close);
        let class = super::metrics::classify(&m);
        let trend = trend_context(&closes, i, 10);

        let push = |out: &mut Vec<PatternHit>, kind: PatternKind| {
            out.push(PatternHit {
                index: i,
                kind,
                direction: kind.direction(),
                strength: kind.strength(),
            });
        };

        // --- 单根 ---
        match class {
            CandleClass::DojiFlat => push(&mut out, PatternKind::FlatLine),
            CandleClass::Doji => {
                if m.upper_ratio > 0.45 && m.lower_ratio < 0.1 {
                    push(&mut out, PatternKind::InvTShape);
                } else if m.lower_ratio > 0.45 && m.upper_ratio < 0.1 {
                    push(&mut out, PatternKind::TShape);
                } else {
                    push(&mut out, PatternKind::DojiStar);
                }
            }
            CandleClass::SpinningTop => push(&mut out, PatternKind::SpinningTop),
            CandleClass::LongLower => {
                if trend < 0 {
                    push(&mut out, PatternKind::Hammer);
                } else if trend > 0 {
                    push(&mut out, PatternKind::HangingMan);
                }
            }
            CandleClass::LongUpper => {
                if trend < 0 {
                    push(&mut out, PatternKind::InvertedHammer);
                } else if trend > 0 {
                    push(&mut out, PatternKind::ShootingStar);
                }
            }
            CandleClass::Marubozu => {
                if m.bullish {
                    push(&mut out, PatternKind::MarubozuBull);
                } else {
                    push(&mut out, PatternKind::MarubozuBear);
                }
            }
            CandleClass::BigBull => push(&mut out, PatternKind::BigBullCandle),
            CandleClass::BigBear => push(&mut out, PatternKind::BigBearCandle),
            _ => {}
        }

        // 识别长十字线：doji + 上下都有长影
        if matches!(class, CandleClass::Doji) && m.upper_ratio > 0.35 && m.lower_ratio > 0.35 {
            push(&mut out, PatternKind::LongDoji);
        }

        // 墓碑 / 蜻蜓十字：doji + 极端单侧影
        if matches!(class, CandleClass::Doji) {
            if m.upper_ratio > 0.55 && m.lower_ratio < 0.08 && trend > 0 {
                push(&mut out, PatternKind::GravestoneDoji);
            }
            if m.lower_ratio > 0.55 && m.upper_ratio < 0.08 && trend < 0 {
                push(&mut out, PatternKind::DragonflyDoji);
            }
        }

        // 光头 / 光脚阳阴线（仅一端无影，且实体 ≥ 60%）
        let k = &klines[i];
        let body_pct = m.body_ratio;
        if body_pct >= 0.6 {
            let upper_tiny = m.upper_ratio < 0.05;
            let lower_tiny = m.lower_ratio < 0.05;
            if upper_tiny && !lower_tiny {
                if m.bullish { push(&mut out, PatternKind::OpenMarubozuBull); }
                else { push(&mut out, PatternKind::OpenMarubozuBear); }
            } else if lower_tiny && !upper_tiny {
                if m.bullish { push(&mut out, PatternKind::CloseMarubozuBull); }
                else { push(&mut out, PatternKind::CloseMarubozuBear); }
            }
        }
        let _ = k; // avoid unused

        // --- 双根 ---
        if i >= 1 {
            if let Some(k) = two_bar_pattern(&klines[i - 1], &klines[i]) {
                push(&mut out, k);
            }
            for k in two_bar_extra(&klines[i - 1], &klines[i], trend) {
                push(&mut out, k);
            }
        }

        // --- 三根 ---
        if i >= 2 {
            if let Some(k) = three_bar_pattern(&klines[i - 2], &klines[i - 1], &klines[i], trend) {
                push(&mut out, k);
            }
            for k in three_bar_extra(&klines[i - 2], &klines[i - 1], &klines[i], trend) {
                push(&mut out, k);
            }
        }

        // --- 五根 ---
        if i >= 4 {
            if let Some(k) = five_bar_pattern(&klines[i - 4..=i]) {
                push(&mut out, k);
            }
        }

        // --- 多根（塔形、岛型，需 5+ 根） ---
        if i >= 4 {
            if let Some(k) = tower_pattern(&klines[..=i]) {
                push(&mut out, k);
            }
        }
        if i >= 2 {
            if let Some(k) = island_reversal(&klines[..=i]) {
                push(&mut out, k);
            }
        }
    }
    out
}

fn two_bar_pattern(a: &Kline, b: &Kline) -> Option<PatternKind> {
    let body_a = a.body();
    let body_b = b.body();

    // 看涨吞没：a 阴 b 阳，b 实体完全包裹 a 实体
    if !a.is_bullish()
        && b.is_bullish()
        && b.open <= a.close
        && b.close >= a.open
        && body_b > body_a * 1.0
    {
        return Some(PatternKind::BullishEngulfing);
    }
    // 看跌吞没
    if a.is_bullish()
        && !b.is_bullish()
        && b.open >= a.close
        && b.close <= a.open
        && body_b > body_a * 1.0
    {
        return Some(PatternKind::BearishEngulfing);
    }

    // 看涨孕线：a 大阴，b 小阳/小阴，b 实体被 a 实体包含
    let a_body_top = a.open.max(a.close);
    let a_body_bot = a.open.min(a.close);
    let b_body_top = b.open.max(b.close);
    let b_body_bot = b.open.min(b.close);
    if !a.is_bullish()
        && body_a > body_b * 2.0
        && b_body_top <= a_body_top
        && b_body_bot >= a_body_bot
    {
        return Some(PatternKind::BullishHarami);
    }
    if a.is_bullish()
        && body_a > body_b * 2.0
        && b_body_top <= a_body_top
        && b_body_bot >= a_body_bot
    {
        return Some(PatternKind::BearishHarami);
    }

    // 曙光初现：a 大阴，b 开盘低于 a.low（或接近），收盘深入 a 实体 50%+
    if !a.is_bullish() && b.is_bullish() && b.open < a.close {
        let midpoint = (a.open + a.close) / 2.0;
        if b.close > midpoint && b.close < a.open {
            return Some(PatternKind::PiercingLine);
        }
    }
    // 乌云盖顶
    if a.is_bullish() && !b.is_bullish() && b.open > a.close {
        let midpoint = (a.open + a.close) / 2.0;
        if b.close < midpoint && b.close > a.open {
            return Some(PatternKind::DarkCloudCover);
        }
    }

    None
}

/// 额外的双根形态（可一次返回多种）
fn two_bar_extra(a: &Kline, b: &Kline, trend: i8) -> Vec<PatternKind> {
    let mut hits = Vec::new();
    let body_a = a.body();
    let body_b = b.body();
    let a_top = a.open.max(a.close);
    let a_bot = a.open.min(a.close);
    let b_top = b.open.max(b.close);
    let b_bot = b.open.min(b.close);
    let tol = (a.close.abs() + b.close.abs()) * 0.001;

    // 镊子顶 / 底：两根高点或两根低点几乎相同（PRD 定义的反转确认形态）
    //   - 比 Tweezer 的常规实现稍严格：要求收盘方向相反或者出现在趋势末端更可信，
    //     但作为底层标注，我们只做几何判定；上层"方向/强度"已反映了置信度
    let hi_tol = (a.high.abs() + b.high.abs()) * 0.0005 + 1e-6;
    if (a.high - b.high).abs() < hi_tol {
        hits.push(PatternKind::TweezersTop);
    }
    let lo_tol = (a.low.abs() + b.low.abs()) * 0.0005 + 1e-6;
    if (a.low - b.low).abs() < lo_tol {
        hits.push(PatternKind::TweezersBottom);
    }

    // 内含线：b 的整个高低区间被 a 包裹
    if b.high <= a.high && b.low >= a.low {
        hits.push(PatternKind::InsideBar);
    }
    // 外包线：a 的整个高低区间被 b 包裹
    if a.high <= b.high && a.low >= b.low && (a.high != b.high || a.low != b.low) {
        hits.push(PatternKind::OutsideBar);
    }
    // 十字孕线（十字 + 前一根大实体）
    let b_range = b.range().max(1e-9);
    let b_is_doji = body_b / b_range < 0.1;
    if b_is_doji && b_top <= a_top && b_bot >= a_bot {
        if !a.is_bullish() && body_a > b_range * 2.0 && trend <= 0 {
            hits.push(PatternKind::BullishHaramiCross);
        }
        if a.is_bullish() && body_a > b_range * 2.0 && trend >= 0 {
            hits.push(PatternKind::BearishHaramiCross);
        }
    }
    // 反击线：a 大阴 b 大阳，且 b.close ≈ a.close（下跌末端反击）
    let close_close = (a.close - b.close).abs() < tol.max(0.0005);
    if !a.is_bullish() && b.is_bullish() && body_a > 0.0 && body_b > 0.0 && close_close && trend < 0 {
        hits.push(PatternKind::BullishCounterAttack);
    }
    if a.is_bullish() && !b.is_bullish() && body_a > 0.0 && body_b > 0.0 && close_close && trend > 0 {
        hits.push(PatternKind::BearishCounterAttack);
    }

    // 对应顶/对应底：两根收盘价几乎相同（与 Tweezer 的差异在于这里用收盘）
    if close_close {
        if trend > 0 { hits.push(PatternKind::MatchingHigh); }
        if trend < 0 { hits.push(PatternKind::MatchingLow); }
    }

    hits
}

fn three_bar_pattern(a: &Kline, b: &Kline, c: &Kline, _trend: i8) -> Option<PatternKind> {
    let body_a = a.body();
    let body_b = b.body();
    let body_c = c.body();
    let range_avg = (a.range() + c.range()) / 2.0;
    let is_small_b = body_b < range_avg * 0.35;

    // 早晨之星：大阴 → 跳空小实体 → 大阳（收盘 > a 中点）
    if !a.is_bullish() && body_a > range_avg * 0.6 && is_small_b && c.is_bullish() {
        let a_mid = (a.open + a.close) / 2.0;
        let gap_down = b.open.max(b.close) < a.close + 1e-9;
        if gap_down && c.close > a_mid {
            return Some(PatternKind::MorningStar);
        }
    }
    // 黄昏之星
    if a.is_bullish() && body_a > range_avg * 0.6 && is_small_b && !c.is_bullish() {
        let a_mid = (a.open + a.close) / 2.0;
        let gap_up = b.open.min(b.close) > a.close - 1e-9;
        if gap_up && c.close < a_mid {
            return Some(PatternKind::EveningStar);
        }
    }

    // 红三兵
    if a.is_bullish()
        && b.is_bullish()
        && c.is_bullish()
        && b.close > a.close
        && c.close > b.close
        && b.open > a.open
        && b.open < a.close
        && c.open > b.open
        && c.open < b.close
        && body_a > 0.0
        && body_b > 0.0
        && body_c > 0.0
    {
        return Some(PatternKind::ThreeWhiteSoldiers);
    }
    // 黑三兵 / 三只乌鸦
    if !a.is_bullish()
        && !b.is_bullish()
        && !c.is_bullish()
        && b.close < a.close
        && c.close < b.close
        && b.open < a.open
        && b.open > a.close
        && c.open < b.open
        && c.open > b.close
        && body_a > 0.0
        && body_b > 0.0
        && body_c > 0.0
    {
        return Some(PatternKind::ThreeBlackCrows);
    }
    None
}

/// 三根扩展形态（可能命中多个）
fn three_bar_extra(a: &Kline, b: &Kline, c: &Kline, trend: i8) -> Vec<PatternKind> {
    let mut hits = Vec::new();
    let body_a = a.body();
    let body_b = b.body();
    let body_c = c.body();
    let a_top = a.open.max(a.close);
    let a_bot = a.open.min(a.close);
    let b_top = b.open.max(b.close);
    let b_bot = b.open.min(b.close);
    let tol = a.close.abs() * 0.001;

    // 三内部上涨 / 下跌：第 1 根大实体，第 2 根被 a 实体包裹（孕线），第 3 根突破 a
    if !a.is_bullish() && body_a > body_b * 2.0
        && b_top <= a_top && b_bot >= a_bot
        && c.is_bullish() && c.close > a_top {
        hits.push(PatternKind::ThreeInsideUp);
    }
    if a.is_bullish() && body_a > body_b * 2.0
        && b_top <= a_top && b_bot >= a_bot
        && !c.is_bullish() && c.close < a_bot {
        hits.push(PatternKind::ThreeInsideDown);
    }

    // 三外部上涨 / 下跌：第 1 根被第 2 根吞没，第 3 根继续第 2 根方向
    if !a.is_bullish() && b.is_bullish()
        && b.open <= a.close && b.close >= a.open && body_b > body_a
        && c.is_bullish() && c.close > b.close {
        hits.push(PatternKind::ThreeOutsideUp);
    }
    if a.is_bullish() && !b.is_bullish()
        && b.open >= a.close && b.close <= a.open && body_b > body_a
        && !c.is_bullish() && c.close < b.close {
        hits.push(PatternKind::ThreeOutsideDown);
    }

    // 弃婴：b 是十字/微小实体，与 a、c 之间都存在跳空
    let b_range = b.range().max(1e-9);
    let b_is_doji = body_b / b_range < 0.1;
    if b_is_doji {
        // 看涨弃婴
        if !a.is_bullish() && c.is_bullish()
            && b.high < a.low && b.high < c.low && trend < 0 {
            hits.push(PatternKind::BullishAbandonedBaby);
        }
        // 看跌弃婴
        if a.is_bullish() && !c.is_bullish()
            && b.low > a.high && b.low > c.high && trend > 0 {
            hits.push(PatternKind::BearishAbandonedBaby);
        }
    }

    // 早晨/黄昏十字星（中间为十字，不强要求大跳空）
    let small_mid = body_b / b_range < 0.25;
    if !a.is_bullish() && body_a > 0.0 && small_mid && c.is_bullish() && c.close > (a.open + a.close) / 2.0 {
        if b_is_doji { hits.push(PatternKind::MorningDojiStar); }
    }
    if a.is_bullish() && body_a > 0.0 && small_mid && !c.is_bullish() && c.close < (a.open + a.close) / 2.0 {
        if b_is_doji { hits.push(PatternKind::EveningDojiStar); }
    }

    // 多方炮：阳-阴-阳，第 3 根收盘高于第 1 根
    if a.is_bullish() && !b.is_bullish() && c.is_bullish()
        && c.close > a.close && c.open < b.close {
        hits.push(PatternKind::BullishStrike);
    }
    // 空方炮
    if !a.is_bullish() && b.is_bullish() && !c.is_bullish()
        && c.close < a.close && c.open > b.close {
        hits.push(PatternKind::BearishStrike);
    }

    // 夹心饼（Stick Sandwich）：a,c 同方向 + 收盘价接近，b 反向
    let ac_close_close = (a.close - c.close).abs() < tol.max(0.0005);
    if !a.is_bullish() && b.is_bullish() && !c.is_bullish() && ac_close_close && trend < 0 {
        hits.push(PatternKind::StickSandwichBull);
    }
    if a.is_bullish() && !b.is_bullish() && c.is_bullish() && ac_close_close && trend > 0 {
        hits.push(PatternKind::StickSandwichBear);
    }

    // 向上跳空两只乌鸦：a 阳 → b 跳空阴 → c 吞没 b，收盘仍高于 a
    if a.is_bullish() && !b.is_bullish() && !c.is_bullish()
        && b.open > a.close && c.open > b.open && c.close < b.close && c.close > a.close {
        hits.push(PatternKind::UpsideGapTwoCrows);
    }

    let _ = (body_a, body_b, body_c);
    hits
}

/// 五根形态：上升三部曲 / 下降三部曲
fn five_bar_pattern(window: &[Kline]) -> Option<PatternKind> {
    if window.len() < 5 { return None; }
    let a = &window[0];
    let mids = &window[1..4];
    let e = &window[4];
    let a_top = a.open.max(a.close);
    let a_bot = a.open.min(a.close);
    let body_a = a.body();
    // 上升三部曲：第 1 根大阳 → 中间 3 根小阴（留在 a 实体内） → 第 5 根大阳突破 a
    let mid_in_range = mids.iter().all(|m| m.high <= a_top + body_a * 0.1 && m.low >= a_bot - body_a * 0.1);
    let mid_body_small = mids.iter().all(|m| m.body() < body_a * 0.6);
    let mids_mostly_bear = mids.iter().filter(|m| !m.is_bullish()).count() >= 2;
    if a.is_bullish() && body_a > 0.0 && mid_in_range && mid_body_small && mids_mostly_bear
        && e.is_bullish() && e.close > a.close {
        return Some(PatternKind::RisingThreeMethods);
    }
    let mids_mostly_bull = mids.iter().filter(|m| m.is_bullish()).count() >= 2;
    if !a.is_bullish() && body_a > 0.0 && mid_in_range && mid_body_small && mids_mostly_bull
        && !e.is_bullish() && e.close < a.close {
        return Some(PatternKind::FallingThreeMethods);
    }
    None
}

/// 塔形顶 / 塔形底（当前及前 4 根总计 5 根）
fn tower_pattern(window: &[Kline]) -> Option<PatternKind> {
    let n = window.len();
    if n < 5 { return None; }
    let w = &window[n - 5..];
    let a = &w[0];
    let b = &w[1];
    let c = &w[2];
    let d = &w[3];
    let e = &w[4];
    let big_a_body = a.body();
    let big_e_body = e.body();
    let small_mid = |k: &Kline| k.body() < big_a_body * 0.4 && k.body() < big_e_body * 0.4;
    if a.is_bullish() && !e.is_bullish()
        && big_a_body > 0.0 && big_e_body > 0.0
        && small_mid(b) && small_mid(c) && small_mid(d)
        && e.close < a.open {
        return Some(PatternKind::TowerTop);
    }
    if !a.is_bullish() && e.is_bullish()
        && big_a_body > 0.0 && big_e_body > 0.0
        && small_mid(b) && small_mid(c) && small_mid(d)
        && e.close > a.open {
        return Some(PatternKind::TowerBottom);
    }
    None
}

/// 岛型反转（顶/底）：一个向上/下跳空簇，内部连续 K 线在价格岛内，随后反向跳空脱离
fn island_reversal(window: &[Kline]) -> Option<PatternKind> {
    let n = window.len();
    if n < 3 { return None; }
    // 寻找最近 N 根内形成的岛：当前根 c 与 c-1 存在一个缺口，且上一次同向缺口在更早的 b
    let c = &window[n - 1];
    let b = &window[n - 2];
    // 岛顶：先有向上跳空进入，后有向下跳空脱离
    if b.low > c.high {
        // 回看最多 8 根，找入场的向上跳空
        let start = n.saturating_sub(10);
        for i in start..n.saturating_sub(2) {
            if i + 1 < n && window[i + 1].low > window[i].high {
                // 确认中间高点都在 island 区间
                let island_low = window[i + 1..n - 1].iter().map(|k| k.low).fold(f64::INFINITY, f64::min);
                if island_low > window[i].high {
                    return Some(PatternKind::IslandReversalTop);
                }
            }
        }
    }
    // 岛底：先有向下跳空进入，后有向上跳空脱离
    if b.high < c.low {
        let start = n.saturating_sub(10);
        for i in start..n.saturating_sub(2) {
            if i + 1 < n && window[i + 1].high < window[i].low {
                let island_high = window[i + 1..n - 1].iter().map(|k| k.high).fold(f64::NEG_INFINITY, f64::max);
                if island_high < window[i].low {
                    return Some(PatternKind::IslandReversalBottom);
                }
            }
        }
    }
    None
}
