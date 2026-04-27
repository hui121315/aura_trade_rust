//! C3：K 线形态高级识别与分类（Sprint 6/7 新增）
//!
//! 本模块实现以下原书铁证形态：
//!
//! - **R-P1-43 长十字线 4 场景分类**（candle p.100）
//! - **R-P1-44 红三兵 3 因素评分 + 三个白色武士强化**（candle p.250）
//! - **R-P1-45 徐缓下降形**（candle p.380）
//! - **R-P1-46 倒三阳主力出货**（candle p.400）
//! - **R-P1-47 K 线形态层级结构映射**（candle p.420）
//! - **R-P1-58 上涨两颗星**（candle p.580）
//! - **R-P1-59 岛形反转时间→级别映射**（candle p.660）
//! - **R-P1-48 圆底"倒春寒"+ 颈线候选**（candle p.500，Sprint 7）
//! - **R-P1-28 圆底完整规则 3 阶段**（Sprint 7）
//! - **R-P1-57 复杂头肩顶左肩判定**（candle p.470，Sprint 7）
//! - **R-P1-23 头肩底量价对称**（candle Ch6，Sprint 7）

use serde::{Deserialize, Serialize};

use super::patterns::PatternKind;
use crate::data::Kline;
use crate::engine::trend::{SwingKind, SwingPoint};

// ==================== R-P1-43 长十字线 4 场景 ====================

/// 长十字线的 4 种场景（candle p.100）
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum LongDojiContext {
    /// 场景 1：上涨趋势中的长十字，次日阳线深入上影 → 看涨强烈
    BullishContinuation,
    /// 场景 2：转势长十字（从下跌→上涨过程中），中阳线突破上影 → 继续看涨
    ReversalToBull,
    /// 场景 3：下跌途中长十字 → 持币观望继续看跌
    BearishContinuation,
    /// 场景 4：下跌趋势中长十字，次日无反弹 → 空方借惯性击溃多方（高山泄洪）
    HighMountainFlood,
}

impl LongDojiContext {
    pub fn direction(&self) -> i8 {
        match self {
            LongDojiContext::BullishContinuation | LongDojiContext::ReversalToBull => 1,
            LongDojiContext::BearishContinuation | LongDojiContext::HighMountainFlood => -1,
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            LongDojiContext::BullishContinuation => "上涨中长十字（看涨强烈）",
            LongDojiContext::ReversalToBull => "转势长十字（继续看涨）",
            LongDojiContext::BearishContinuation => "下跌中长十字（继续看跌）",
            LongDojiContext::HighMountainFlood => "高山泄洪（空方击溃）",
        }
    }
}

/// 分类长十字线的场景
///
/// # 参数
/// - `klines`: K 线序列
/// - `doji_index`: 长十字线所在索引
/// - `prior_trend`: 之前的趋势方向（+1 上升 / -1 下降 / 0 中性）
pub fn classify_long_doji(
    klines: &[Kline],
    doji_index: usize,
    prior_trend: i8,
) -> Option<LongDojiContext> {
    // 需要下一根 K 线存在（prior_trend 由外部提供，不需要历史 K 线）
    if doji_index + 1 >= klines.len() {
        return None;
    }
    let doji = &klines[doji_index];
    let next = &klines[doji_index + 1];
    let upper_shadow = doji.high - doji.open.max(doji.close);
    let lower_shadow = doji.open.min(doji.close) - doji.low;

    if prior_trend > 0 {
        // 上涨趋势：看下一根是否深入长十字上影（close > doji close）
        if next.close > doji.close && next.close > doji.open.max(doji.close) {
            Some(LongDojiContext::BullishContinuation)
        } else {
            None
        }
    } else if prior_trend < 0 {
        // 下跌趋势：下一根是否继续无力
        if next.close < doji.close && lower_shadow > 0.0 {
            Some(LongDojiContext::HighMountainFlood)
        } else {
            Some(LongDojiContext::BearishContinuation)
        }
    } else {
        // 中性：如果上影明显 + 次日突破 → 转势
        if upper_shadow > lower_shadow && next.close > doji.high {
            Some(LongDojiContext::ReversalToBull)
        } else {
            None
        }
    }
}

// ==================== R-P1-44 红三兵 3 因素评分 ====================

/// 红三兵强度评分（candle p.250）
///
/// 原书 3 因素：
/// 1. 处在整理形态末端 + 向上突破
/// 2. 成交量稳步放出（价涨量增）
/// 3. 每根阳线都以最高价或次高价收盘（强势收盘）
///
/// 满足 3 条 = **三个白色武士**（红三兵的特殊强化形态）
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct ThreeSoldiersScore {
    pub factor1_breakout_from_consolidation: bool,
    pub factor2_rising_volume: bool,
    pub factor3_close_at_high: bool,
    /// 满足 3/3 = 三个白色武士（强化形态）
    pub is_white_soldiers: bool,
    /// 评分 0-3
    pub score: u8,
}

/// 评估红三兵强度
///
/// # 参数
/// - `klines`: K 线序列（至少包含 3 根红三兵及之前的整理期）
/// - `soldiers_start`: 红三兵第一根的索引
/// - `consolidation_window`: 之前的整理窗口（默认 10 根）
pub fn score_three_white_soldiers(
    klines: &[Kline],
    soldiers_start: usize,
    consolidation_window: usize,
) -> Option<ThreeSoldiersScore> {
    if soldiers_start + 3 > klines.len() {
        return None;
    }
    let soldiers = &klines[soldiers_start..soldiers_start + 3];

    // 所有 3 根必须是阳线
    if !soldiers.iter().all(|k| k.close > k.open) {
        return None;
    }

    // 因素 1：整理末端突破 → 前 consolidation_window 根的高点 < 第一根红兵收盘
    let pre_start = soldiers_start.saturating_sub(consolidation_window);
    let factor1 = if pre_start < soldiers_start {
        let pre_high = klines[pre_start..soldiers_start]
            .iter()
            .map(|k| k.high)
            .fold(f64::NEG_INFINITY, f64::max);
        soldiers[0].close > pre_high
    } else {
        false
    };

    // 因素 2：成交量稳步放出
    let factor2 = soldiers[0].volume <= soldiers[1].volume
        && soldiers[1].volume <= soldiers[2].volume
        && soldiers[2].volume > soldiers[0].volume; // 末端放量

    // 因素 3：每根都以最高价/次高价收盘（body 占整根 K 线 ≥ 80%）
    let factor3 = soldiers.iter().all(|k| {
        let range = k.high - k.low;
        if range < 1e-9 {
            false
        } else {
            let upper_shadow = k.high - k.close;
            upper_shadow / range <= 0.20
        }
    });

    let score = factor1 as u8 + factor2 as u8 + factor3 as u8;
    let is_white_soldiers = score == 3
        // + 最后一根实体最长（三个白色武士特征）
        && {
            let body_last = (soldiers[2].close - soldiers[2].open).abs();
            let body_0 = (soldiers[0].close - soldiers[0].open).abs();
            let body_1 = (soldiers[1].close - soldiers[1].open).abs();
            body_last >= body_0 && body_last >= body_1
        };

    Some(ThreeSoldiersScore {
        factor1_breakout_from_consolidation: factor1,
        factor2_rising_volume: factor2,
        factor3_close_at_high: factor3,
        is_white_soldiers,
        score,
    })
}

// ==================== R-P1-45 徐缓下降形 ====================

/// 徐缓下降形事件（candle p.380）
///
/// 下跌趋势中先收几根小阴线 → 接着中阴/大阴线 → 空方完全主导
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GradualDeclineEvent {
    /// 序列起始索引
    pub start_index: usize,
    /// 序列结束索引（大阴线所在）
    pub end_index: usize,
    /// 序列中小阴线数量
    pub small_bear_count: usize,
}

/// 检测徐缓下降形
///
/// # 参数
/// - `klines`: K 线序列
/// - `small_threshold`: 小阴线最大跌幅（默认 1%）
/// - `big_threshold`: 大阴线最小跌幅（默认 3%）
/// - `min_small_bars`: 最少小阴线数（默认 3）
pub fn detect_gradual_decline(
    klines: &[Kline],
    small_threshold: f64,
    big_threshold: f64,
    min_small_bars: usize,
) -> Vec<GradualDeclineEvent> {
    let n = klines.len();
    if n < min_small_bars + 1 {
        return Vec::new();
    }
    let mut out = Vec::new();
    let mut i = 0;
    while i + min_small_bars < n {
        // 查找连续 min_small_bars 根小阴线
        let mut small_count = 0;
        let mut j = i;
        while j < n {
            let k = &klines[j];
            if k.close >= k.open {
                break; // 非阴线
            }
            let pct = (k.open - k.close) / k.open.abs().max(1e-9);
            if pct > small_threshold {
                break; // 过大不算小
            }
            small_count += 1;
            j += 1;
        }
        if small_count >= min_small_bars && j < n {
            // 下一根应为中/大阴线
            let big = &klines[j];
            if big.close < big.open {
                let pct = (big.open - big.close) / big.open.abs().max(1e-9);
                if pct >= big_threshold {
                    out.push(GradualDeclineEvent {
                        start_index: i,
                        end_index: j,
                        small_bear_count: small_count,
                    });
                    i = j + 1;
                    continue;
                }
            }
        }
        i += 1;
    }
    out
}

// ==================== R-P1-46 倒三阳 ====================

/// 倒三阳事件（candle p.400）—— 主力出货识别
///
/// 特征：3 根阳线但**第一根低开放量**
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InvertedThreeRedSoldiersEvent {
    pub start_index: usize,
    /// 第一根的跳空幅度（前收 - 开盘）/ 前收
    pub gap_down_pct: f64,
    /// 第一根的成交量倍率（相对之前 10 根均量）
    pub volume_surge: f64,
}

/// 检测倒三阳
///
/// # 核心判定
/// - 3 根阳线
/// - **第一根低开**（open < 前收，gap down ≥ gap_threshold）
/// - **第一根放量**（成交量 ≥ 近 10 根均量 × surge_factor）
pub fn detect_inverted_three_red(
    klines: &[Kline],
    gap_threshold: f64,
    surge_factor: f64,
    lookback: usize,
) -> Vec<InvertedThreeRedSoldiersEvent> {
    let n = klines.len();
    if n < lookback + 3 {
        return Vec::new();
    }
    let mut out = Vec::new();
    for i in lookback..(n - 2) {
        let first = &klines[i];
        let second = &klines[i + 1];
        let third = &klines[i + 2];

        // 3 根都是阳线
        if first.close <= first.open
            || second.close <= second.open
            || third.close <= third.open
        {
            continue;
        }

        let prev_close = klines[i - 1].close;
        if prev_close <= 1e-9 {
            continue;
        }
        let gap_pct = (prev_close - first.open) / prev_close;
        if gap_pct < gap_threshold {
            continue;
        }

        // 放量判定
        let avg_vol: f64 = klines[i - lookback..i].iter().map(|k| k.volume).sum::<f64>()
            / lookback as f64;
        if avg_vol < 1e-9 {
            continue;
        }
        let surge = first.volume / avg_vol;
        if surge < surge_factor {
            continue;
        }

        out.push(InvertedThreeRedSoldiersEvent {
            start_index: i,
            gap_down_pct: gap_pct,
            volume_surge: surge,
        });
    }
    out
}

// ==================== R-P1-58 上涨两颗星 ====================

/// 上涨两颗星事件（candle p.580）
///
/// 特征：**大阳线后连续 2 根小阳线** = 看涨确认
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TwoRisingStarsEvent {
    pub big_bull_index: usize,
    pub star1_index: usize,
    pub star2_index: usize,
}

/// 检测上涨两颗星
///
/// # 参数
/// - `big_bull_min_pct`: 大阳线最小涨幅（默认 5%）
/// - `star_max_pct`: 小阳线最大涨幅（默认 2%）
pub fn detect_two_rising_stars(
    klines: &[Kline],
    big_bull_min_pct: f64,
    star_max_pct: f64,
) -> Vec<TwoRisingStarsEvent> {
    let n = klines.len();
    if n < 3 {
        return Vec::new();
    }
    let mut out = Vec::new();
    for i in 0..(n - 2) {
        let big = &klines[i];
        let s1 = &klines[i + 1];
        let s2 = &klines[i + 2];

        // 大阳线
        if big.close <= big.open {
            continue;
        }
        let big_pct = (big.close - big.open) / big.open.abs().max(1e-9);
        if big_pct < big_bull_min_pct {
            continue;
        }

        // 两根小阳线
        for (star, _) in [(s1, 0), (s2, 0)] {
            if star.close <= star.open {
                continue;
            }
        }
        let is_bull_1 = s1.close > s1.open;
        let is_bull_2 = s2.close > s2.open;
        if !is_bull_1 || !is_bull_2 {
            continue;
        }
        let p1 = (s1.close - s1.open) / s1.open.abs().max(1e-9);
        let p2 = (s2.close - s2.open) / s2.open.abs().max(1e-9);
        if p1 > star_max_pct || p2 > star_max_pct {
            continue;
        }
        // 小阳线应开在大阳线实体之上（表示惯性持续）
        if s1.open < big.open {
            continue;
        }

        out.push(TwoRisingStarsEvent {
            big_bull_index: i,
            star1_index: i + 1,
            star2_index: i + 2,
        });
    }
    out
}

// ==================== R-P1-59 岛形反转时间→级别 ====================

/// 岛形反转的趋势级别（candle p.660）
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum IslandTrendLevel {
    /// 短期反转（整理 1-5 根 K 线）
    ShortTerm,
    /// 中期反转（整理 6-20 根）
    MidTerm,
    /// 长期反转（整理 20+ 根）—— 最可靠
    LongTerm,
}

impl IslandTrendLevel {
    pub fn label(&self) -> &'static str {
        match self {
            IslandTrendLevel::ShortTerm => "短期反转",
            IslandTrendLevel::MidTerm => "中期反转",
            IslandTrendLevel::LongTerm => "长期反转（最可靠）",
        }
    }

    /// 信号可靠度 0-1
    pub fn reliability(&self) -> f64 {
        match self {
            IslandTrendLevel::ShortTerm => 0.4,
            IslandTrendLevel::MidTerm => 0.7,
            IslandTrendLevel::LongTerm => 0.95,
        }
    }

    /// 短线/中线/长线交易建议（减仓比例）
    pub fn recommended_exit_fraction(&self) -> f64 {
        match self {
            IslandTrendLevel::ShortTerm => 0.50, // 中长线部分减仓
            IslandTrendLevel::MidTerm => 0.75,
            IslandTrendLevel::LongTerm => 1.00, // 全部清仓
        }
    }
}

/// 根据岛形内整理 K 线数返回趋势级别
pub fn island_trend_level(bars_in_island: usize) -> IslandTrendLevel {
    if bars_in_island <= 5 {
        IslandTrendLevel::ShortTerm
    } else if bars_in_island <= 20 {
        IslandTrendLevel::MidTerm
    } else {
        IslandTrendLevel::LongTerm
    }
}

// ==================== R-P1-47 K 线形态层级结构 ====================

/// K 线形态的父形态映射（candle p.420）
///
/// 原书铁证：子形态是父形态的**组成部分**。例如：
/// - 两阴夹一阳 ⊂ 圆顶形态
/// - 十字星 ⊂ 早晨/黄昏之星
///
/// 返回给定子形态的所有可能父形态
pub fn parent_patterns_of(child: PatternKind) -> Vec<PatternKind> {
    use PatternKind::*;
    match child {
        // 十字系 → 星系
        DojiStar | LongDoji => vec![MorningStar, EveningStar, MorningDojiStar, EveningDojiStar],
        // 穿头破脚 → 圆顶/圆底
        BullishEngulfing => vec![MorningStar, ThreeInsideUp, ThreeOutsideUp],
        BearishEngulfing => vec![EveningStar, ThreeInsideDown, ThreeOutsideDown],
        // 孕线 → 三内部形态
        BullishHarami => vec![ThreeInsideUp],
        BearishHarami => vec![ThreeInsideDown],
        // 覆盖 / 曙光 → 星系
        PiercingLine => vec![MorningStar, MorningDojiStar],
        DarkCloudCover => vec![EveningStar, EveningDojiStar],
        // 锤头 → 早晨之星的第三根
        Hammer => vec![MorningStar],
        HangingMan => vec![EveningStar],
        InvertedHammer => vec![MorningStar],
        ShootingStar => vec![EveningStar],
        _ => vec![],
    }
}

/// 判断两个形态是否为同一父形态下的兄弟形态
pub fn are_siblings(a: PatternKind, b: PatternKind) -> bool {
    let parents_a = parent_patterns_of(a);
    let parents_b = parent_patterns_of(b);
    parents_a.iter().any(|p| parents_b.contains(p))
}

// ==================== R-P1-28 + R-P1-48 圆底完整规则 3 阶段 ====================

/// 圆底 3 阶段（candle p.500）
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum RoundingBottomPhase {
    /// 阶段 1：下跌探底（左侧）—— 价格从起点逐步下降到最低点
    DownLeft,
    /// 阶段 2：被套者抛盘（倒春寒）—— 小型反弹 + 又创新低
    SpringCold,
    /// 阶段 3：多方吸筹 + 突破（右侧）—— 价格回升至颈线
    RightRising,
}

impl RoundingBottomPhase {
    pub fn label(&self) -> &'static str {
        match self {
            RoundingBottomPhase::DownLeft => "左侧下探",
            RoundingBottomPhase::SpringCold => "倒春寒（被套抛盘）",
            RoundingBottomPhase::RightRising => "右侧回升",
        }
    }
}

/// 圆底分析结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoundingBottomAnalysis {
    pub phase_at_index: Vec<(usize, RoundingBottomPhase)>,
    /// 是否检测到倒春寒（R-P1-48 核心）
    pub has_spring_cold: bool,
    /// 倒春寒的创新低索引
    pub spring_cold_index: Option<usize>,
    /// 候选颈线（多个，原书 candle p.500 "不限单一高点"）
    pub neckline_candidates: Vec<f64>,
}

/// 分析圆底的 3 阶段 + 倒春寒 + 多候选颈线
///
/// # 参数
/// - `closes`：整段收盘价序列
/// - `start_index`：圆底起点
/// - `end_index`：圆底终点
///
/// # 原书铁证（candle p.500）
///
/// 圆底**形成时间越长 → 积聚动力越充足 → 后市上涨越有力**
///
/// 倒春寒特征：在形成小型上升趋势后**突然杀跌创新低**
pub fn analyze_rounding_bottom(
    closes: &[f64],
    start_index: usize,
    end_index: usize,
) -> Option<RoundingBottomAnalysis> {
    if start_index >= end_index || end_index >= closes.len() {
        return None;
    }
    let seg = &closes[start_index..=end_index];
    let n = seg.len();
    if n < 10 {
        return None;
    }

    // 找到最低点（圆底最深处）
    let (min_idx_local, &min_price) = seg
        .iter()
        .enumerate()
        .min_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))?;
    let min_idx_global = start_index + min_idx_local;

    // 阶段划分：
    // - DownLeft: start..min_idx
    // - SpringCold: 检测最低点附近是否有二次创新低
    // - RightRising: min_idx..end
    let mut phase_at_index = Vec::with_capacity(n);
    for i in 0..n {
        let phase = if start_index + i < min_idx_global {
            RoundingBottomPhase::DownLeft
        } else if start_index + i == min_idx_global {
            // 最低点本身归属 RightRising 起点
            RoundingBottomPhase::RightRising
        } else {
            RoundingBottomPhase::RightRising
        };
        phase_at_index.push((start_index + i, phase));
    }

    // 倒春寒检测：在右侧上升阶段是否出现"二次探底"（close < min_price * 1.01）
    let mut has_spring_cold = false;
    let mut spring_cold_index = None;
    // 向后扫描：从最低点 +3 开始查找是否有新低
    let scan_start_local = (min_idx_local + 3).min(n);
    for i in scan_start_local..n {
        if seg[i] <= min_price * 1.005 {
            // 二次探底到 0.5% 范围内
            has_spring_cold = true;
            spring_cold_index = Some(start_index + i);
            // 标记该段为 SpringCold
            // （用索引位置回填 phase_at_index）
            let start_span = min_idx_local + 1;
            let end_span = i;
            for j in start_span..=end_span {
                if j < phase_at_index.len() {
                    phase_at_index[j].1 = RoundingBottomPhase::SpringCold;
                }
            }
            break;
        }
    }

    // 候选颈线：
    // 1. 左边最高点（起点附近）
    // 2. 圆底过程中的所有"局部高点"（前 20% 与后 20% 的最高）
    let mut neckline_candidates = Vec::new();
    let left_20 = (n as f64 * 0.20) as usize;
    let right_20_start = (n as f64 * 0.80) as usize;
    if left_20 >= 1 {
        let left_high = seg[..left_20].iter().cloned().fold(f64::NEG_INFINITY, f64::max);
        if left_high.is_finite() {
            neckline_candidates.push(left_high);
        }
    }
    if right_20_start < n {
        let right_high = seg[right_20_start..].iter().cloned().fold(f64::NEG_INFINITY, f64::max);
        if right_high.is_finite() {
            neckline_candidates.push(right_high);
        }
    }
    // 去重
    neckline_candidates.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    neckline_candidates.dedup_by(|a, b| (*a - *b).abs() < 1e-6);

    Some(RoundingBottomAnalysis {
        phase_at_index,
        has_spring_cold,
        spring_cold_index,
        neckline_candidates,
    })
}

// ==================== R-P1-57 复杂头肩顶左肩判定 ====================

/// 复杂头肩顶左肩分析（candle p.470）
///
/// 原书：双峰左肩倾向于**一个左肩 + B 浪反弹**
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComplexLeftShoulderAnalysis {
    /// 是否为复杂左肩（双峰）
    pub is_complex: bool,
    /// 主峰索引（左肩真实位置）
    pub main_peak_index: usize,
    /// 次峰索引（B 浪反弹）
    pub sub_peak_index: Option<usize>,
}

/// 分析一组 swing 点中，前 N 个是否构成"双峰左肩"
///
/// # 参数
/// - `swings`：swing 点序列（按索引升序）
/// - `max_before_head`：在主头部索引之前允许检查的最大 swing 点数
pub fn analyze_complex_left_shoulder(
    swings: &[SwingPoint],
    head_index: usize,
) -> ComplexLeftShoulderAnalysis {
    // 收集 head 之前的高点 swing
    let highs_before_head: Vec<&SwingPoint> = swings
        .iter()
        .filter(|s| s.kind == SwingKind::High && s.index < head_index)
        .collect();

    // 如果高点 < 2，不是复杂左肩
    if highs_before_head.len() < 2 {
        return ComplexLeftShoulderAnalysis {
            is_complex: false,
            main_peak_index: highs_before_head
                .last()
                .map(|s| s.index)
                .unwrap_or(head_index),
            sub_peak_index: None,
        };
    }

    // 取最后两个高点：main = 最高，sub = 较低的那个
    let last2 = &highs_before_head[highs_before_head.len() - 2..];
    let (main, sub) = if last2[0].price > last2[1].price {
        (last2[0], last2[1])
    } else {
        (last2[1], last2[0])
    };

    // 复杂左肩：两个高点相差 < 10%（形态相似但有高低）
    let is_complex =
        ((main.price - sub.price) / main.price.abs().max(1e-9)).abs() < 0.10;

    ComplexLeftShoulderAnalysis {
        is_complex,
        main_peak_index: main.index,
        sub_peak_index: if is_complex { Some(sub.index) } else { None },
    }
}

// ==================== R-P1-23 头肩底量价对称 ====================

/// 头肩底/顶量价对称分析
///
/// 原书铁证：
/// - **头肩底**：左肩量 > 头部量 > 右肩量（递减）+ 突破颈线放量
/// - **头肩顶**：与底相反，但多数呈"头部放量，右肩缩量"
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct VolumeSymmetry {
    /// 左肩成交量
    pub left_shoulder_vol: f64,
    /// 头部成交量
    pub head_vol: f64,
    /// 右肩成交量
    pub right_shoulder_vol: f64,
    /// 量递减特征（头肩底应为 true）
    pub is_descending: bool,
    /// 量对称（|左肩量 - 右肩量| / 左肩量 < 30%）
    pub is_symmetric: bool,
}

/// 检查头肩底/顶的量价对称性
///
/// # 参数
/// - `klines`：K 线序列
/// - `left_shoulder_idx` / `head_idx` / `right_shoulder_idx`：3 个关键点索引
pub fn check_head_shoulders_volume(
    klines: &[Kline],
    left_shoulder_idx: usize,
    head_idx: usize,
    right_shoulder_idx: usize,
) -> Option<VolumeSymmetry> {
    if right_shoulder_idx >= klines.len() {
        return None;
    }
    let ls_vol = klines[left_shoulder_idx].volume;
    let head_vol = klines[head_idx].volume;
    let rs_vol = klines[right_shoulder_idx].volume;

    if !ls_vol.is_finite() || !head_vol.is_finite() || !rs_vol.is_finite() {
        return None;
    }

    let is_descending = ls_vol > head_vol && head_vol > rs_vol;
    let is_symmetric = if ls_vol.abs() > 1e-9 {
        ((ls_vol - rs_vol).abs() / ls_vol.abs()) < 0.30
    } else {
        false
    };

    Some(VolumeSymmetry {
        left_shoulder_vol: ls_vol,
        head_vol,
        right_shoulder_vol: rs_vol,
        is_descending,
        is_symmetric,
    })
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

    // -------- R-P1-43 长十字线场景测试 --------

    #[test]
    fn t_long_doji_bullish_continuation() {
        // 上涨中长十字，次日阳线收盘高于长十字 → BullishContinuation
        let klines = vec![
            mk_kline(0, 100.0, 100.0, 102.0, 98.0, 1.0),
            mk_kline(1, 102.0, 105.0, 106.0, 101.5, 1.0),
        ];
        let ctx = classify_long_doji(&klines, 0, 1);
        assert_eq!(ctx, Some(LongDojiContext::BullishContinuation));
    }

    #[test]
    fn t_long_doji_high_mountain_flood() {
        // 下跌中长十字，次日继续下跌 → HighMountainFlood
        let klines = vec![
            mk_kline(0, 100.0, 100.0, 102.0, 98.0, 1.0),
            mk_kline(1, 100.0, 95.0, 100.5, 94.0, 1.0),
        ];
        let ctx = classify_long_doji(&klines, 0, -1);
        assert_eq!(ctx, Some(LongDojiContext::HighMountainFlood));
    }

    #[test]
    fn t_long_doji_direction_correct() {
        assert_eq!(LongDojiContext::BullishContinuation.direction(), 1);
        assert_eq!(LongDojiContext::ReversalToBull.direction(), 1);
        assert_eq!(LongDojiContext::BearishContinuation.direction(), -1);
        assert_eq!(LongDojiContext::HighMountainFlood.direction(), -1);
    }

    #[test]
    fn t_long_doji_reversal_to_bull_from_neutral() {
        // 中性趋势 + 长上影 + 次日 close 突破 doji.high → ReversalToBull
        // doji: open=100, close=100.1 (几乎平), high=110 (长上影), low=99 (短下影)
        let klines = vec![
            mk_kline(0, 100.0, 100.1, 110.0, 99.0, 1.0),
            mk_kline(1, 101.0, 112.0, 112.5, 100.8, 1.0),
        ];
        let ctx = classify_long_doji(&klines, 0, 0); // prior_trend=0 中性
        assert_eq!(ctx, Some(LongDojiContext::ReversalToBull));
    }

    #[test]
    fn t_long_doji_bearish_continuation_in_downtrend() {
        // 下跌趋势 + 次日 close >= doji.close → else 分支 → BearishContinuation
        // （反向 = next.close < doji.close + lower_shadow > 0 → HighMountainFlood）
        let klines = vec![
            mk_kline(0, 100.0, 100.0, 102.0, 98.0, 1.0),
            mk_kline(1, 100.5, 101.0, 101.5, 100.0, 1.0),
        ];
        let ctx = classify_long_doji(&klines, 0, -1); // prior_trend=-1 下跌
        assert_eq!(ctx, Some(LongDojiContext::BearishContinuation));
    }

    // -------- R-P1-44 红三兵评分测试 --------

    #[test]
    fn t_three_white_soldiers_score_3_of_3() {
        // 构造：10 根整理（最高 100）+ 3 根递增阳线（101/103/106，量递增，收盘几乎最高）
        let mut klines: Vec<_> = (0..10)
            .map(|i| mk_kline(i, 99.5, 100.0, 100.5, 99.0, 1.0))
            .collect();
        klines.push(mk_kline(10, 100.0, 101.0, 101.1, 99.8, 2.0)); // 突破整理 + 量起
        klines.push(mk_kline(11, 101.0, 103.0, 103.1, 100.9, 3.0));
        klines.push(mk_kline(12, 103.0, 106.0, 106.1, 102.9, 4.0));
        let score = score_three_white_soldiers(&klines, 10, 10).unwrap();
        assert!(score.factor1_breakout_from_consolidation);
        assert!(score.factor2_rising_volume);
        assert!(score.factor3_close_at_high);
        assert_eq!(score.score, 3);
        assert!(score.is_white_soldiers, "应为三个白色武士");
    }

    #[test]
    fn t_three_white_soldiers_not_white_if_body_not_longest_last() {
        // 3/3 因素都满足，但最后一根实体小于第一根 → 不是三个白色武士
        let mut klines: Vec<_> = (0..10)
            .map(|i| mk_kline(i, 99.5, 100.0, 100.5, 99.0, 1.0))
            .collect();
        klines.push(mk_kline(10, 100.0, 106.0, 106.1, 99.9, 2.0)); // 最大实体
        klines.push(mk_kline(11, 106.0, 107.0, 107.1, 105.9, 3.0));
        klines.push(mk_kline(12, 107.0, 108.0, 108.1, 106.9, 4.0));
        let score = score_three_white_soldiers(&klines, 10, 10).unwrap();
        assert!(!score.is_white_soldiers, "最后实体不最长，不是白色武士");
    }

    // -------- R-P1-45 徐缓下降形测试 --------

    #[test]
    fn t_gradual_decline_detected() {
        // 3 根小阴（各跌 0.5%）+ 1 根大阴（跌 5%）
        let klines = vec![
            mk_kline(0, 100.0, 99.5, 100.0, 99.4, 1.0),
            mk_kline(1, 99.5, 99.0, 99.5, 98.9, 1.0),
            mk_kline(2, 99.0, 98.5, 99.0, 98.4, 1.0),
            mk_kline(3, 98.5, 93.5, 98.5, 93.0, 2.0), // 大阴 -5%
        ];
        let events = detect_gradual_decline(&klines, 0.01, 0.03, 3);
        assert!(!events.is_empty());
        assert_eq!(events[0].small_bear_count, 3);
    }

    #[test]
    fn t_gradual_decline_no_event_without_big_bear() {
        // 只有小阴，没有大阴 → 不算徐缓下降
        let klines = vec![
            mk_kline(0, 100.0, 99.5, 100.0, 99.4, 1.0),
            mk_kline(1, 99.5, 99.0, 99.5, 98.9, 1.0),
            mk_kline(2, 99.0, 98.5, 99.0, 98.4, 1.0),
            mk_kline(3, 98.5, 98.0, 98.5, 97.9, 1.0), // 还是小阴
        ];
        let events = detect_gradual_decline(&klines, 0.01, 0.03, 3);
        assert!(events.is_empty());
    }

    // -------- R-P1-46 倒三阳测试 --------

    #[test]
    fn t_inverted_three_red_detected() {
        // 10 根前置（量 1.0，close=110），然后低开放量 3 阳
        let mut klines: Vec<_> = (0..10)
            .map(|i| mk_kline(i, 109.0, 110.0, 111.0, 108.0, 1.0))
            .collect();
        klines.push(mk_kline(10, 106.0, 108.0, 109.0, 105.0, 3.0)); // 低开 ~3.6% + 放量 3x
        klines.push(mk_kline(11, 108.0, 109.0, 110.0, 107.0, 2.0));
        klines.push(mk_kline(12, 109.0, 110.0, 111.0, 108.0, 2.0));
        let events = detect_inverted_three_red(&klines, 0.02, 1.5, 10);
        assert!(!events.is_empty(), "应识别倒三阳；实际：{:?}", events);
        assert!(events[0].gap_down_pct > 0.02);
    }

    #[test]
    fn t_inverted_three_red_rejected_no_gap() {
        // 无低开 → 非倒三阳
        let mut klines: Vec<_> = (0..10)
            .map(|i| mk_kline(i, 99.0, 100.0, 101.0, 98.0, 1.0))
            .collect();
        klines.push(mk_kline(10, 100.0, 101.0, 102.0, 99.5, 3.0));
        klines.push(mk_kline(11, 101.0, 102.0, 103.0, 100.5, 2.0));
        klines.push(mk_kline(12, 102.0, 103.0, 104.0, 101.5, 2.0));
        let events = detect_inverted_three_red(&klines, 0.02, 1.5, 10);
        assert!(events.is_empty());
    }

    // -------- R-P1-58 上涨两颗星测试 --------

    #[test]
    fn t_two_rising_stars_detected() {
        // 大阳（+5%）+ 2 根小阳（+1% 每根）
        let klines = vec![
            mk_kline(0, 100.0, 105.5, 106.0, 99.5, 1.0),  // big bull +5.5%
            mk_kline(1, 105.5, 106.5, 107.0, 105.0, 1.0), // small bull +1%
            mk_kline(2, 106.5, 107.5, 108.0, 106.0, 1.0), // small bull +1%
        ];
        let events = detect_two_rising_stars(&klines, 0.05, 0.02);
        assert!(!events.is_empty());
    }

    #[test]
    fn t_two_rising_stars_rejected_if_small_bull_too_large() {
        // 小阳涨幅 > 阈值 → 不是两颗星
        let klines = vec![
            mk_kline(0, 100.0, 106.0, 106.5, 99.5, 1.0),
            mk_kline(1, 106.0, 112.0, 113.0, 105.5, 1.0), // 大阳不是小阳
            mk_kline(2, 112.0, 113.0, 114.0, 111.5, 1.0),
        ];
        let events = detect_two_rising_stars(&klines, 0.05, 0.02);
        assert!(events.is_empty());
    }

    // -------- R-P1-59 岛形时间→级别测试 --------

    #[test]
    fn t_island_trend_level_mapping() {
        assert_eq!(island_trend_level(1), IslandTrendLevel::ShortTerm);
        assert_eq!(island_trend_level(5), IslandTrendLevel::ShortTerm);
        assert_eq!(island_trend_level(6), IslandTrendLevel::MidTerm);
        assert_eq!(island_trend_level(20), IslandTrendLevel::MidTerm);
        assert_eq!(island_trend_level(21), IslandTrendLevel::LongTerm);
        assert_eq!(island_trend_level(60), IslandTrendLevel::LongTerm);
    }

    #[test]
    fn t_island_reliability_ordering() {
        assert!(
            IslandTrendLevel::LongTerm.reliability()
                > IslandTrendLevel::MidTerm.reliability()
        );
        assert!(
            IslandTrendLevel::MidTerm.reliability()
                > IslandTrendLevel::ShortTerm.reliability()
        );
    }

    #[test]
    fn t_island_exit_fraction_correct() {
        assert_eq!(
            IslandTrendLevel::LongTerm.recommended_exit_fraction(),
            1.00
        );
        assert_eq!(IslandTrendLevel::MidTerm.recommended_exit_fraction(), 0.75);
        assert_eq!(
            IslandTrendLevel::ShortTerm.recommended_exit_fraction(),
            0.50
        );
    }

    // -------- R-P1-47 层级结构测试 --------

    #[test]
    fn t_parent_patterns_doji_maps_to_stars() {
        let parents = parent_patterns_of(PatternKind::DojiStar);
        assert!(parents.contains(&PatternKind::MorningStar));
        assert!(parents.contains(&PatternKind::EveningStar));
    }

    #[test]
    fn t_parent_patterns_hammer_maps_to_morning_star() {
        let parents = parent_patterns_of(PatternKind::Hammer);
        assert!(parents.contains(&PatternKind::MorningStar));
    }

    #[test]
    fn t_are_siblings_doji_hammer_share_parent() {
        // 十字星和锤头都是早晨之星的可能组成
        assert!(are_siblings(PatternKind::DojiStar, PatternKind::Hammer));
    }

    #[test]
    fn t_are_not_siblings_unrelated() {
        // 光头大阳和光头大阴无共同父
        assert!(!are_siblings(
            PatternKind::MarubozuBull,
            PatternKind::MarubozuBear
        ));
    }

    #[test]
    fn t_parent_patterns_empty_for_leaf() {
        // 早晨之星本身是父级形态，无更高父
        let parents = parent_patterns_of(PatternKind::MorningStar);
        assert!(parents.is_empty());
    }

    // -------- Sprint 7：R-P1-28/48 圆底 3 阶段 + 倒春寒 --------

    #[test]
    fn t_rounding_bottom_three_phases() {
        // 构造圆底：下降（10）→ 上升（10）
        let closes: Vec<f64> = (0..20)
            .map(|i| {
                if i <= 10 {
                    100.0 - i as f64 * 2.0
                } else {
                    80.0 + (i - 10) as f64 * 2.0
                }
            })
            .collect();
        let analysis = analyze_rounding_bottom(&closes, 0, 19).unwrap();
        assert!(!analysis.has_spring_cold, "无倒春寒");
        // 前段应为 DownLeft
        let first = analysis.phase_at_index.first().unwrap();
        assert_eq!(first.1, RoundingBottomPhase::DownLeft);
        // 后段应为 RightRising
        let last = analysis.phase_at_index.last().unwrap();
        assert_eq!(last.1, RoundingBottomPhase::RightRising);
    }

    #[test]
    fn t_rounding_bottom_spring_cold_detected() {
        // 构造：下降到 87 → 反弹 → 再次回落接近 87（倒春寒，在最低点之后）
        // 原书 p.500："突然杀跌，股价创出新低"—— 二次探底接近但不必严格低于
        let closes: Vec<f64> = vec![
            100.0, 98.0, 96.0, 94.0, 92.0, 90.0, // 下降
            89.0, 88.0, 87.0,  // idx 8 最低点
            89.0, 91.0, 93.0, 92.0, // 反弹
            87.3,              // idx 13 二次探底（接近 87，倒春寒）
            89.0, 91.0, 93.0, 95.0, 98.0, 100.0, // 最终上升
        ];
        let analysis = analyze_rounding_bottom(&closes, 0, closes.len() - 1).unwrap();
        assert!(analysis.has_spring_cold, "应检测到倒春寒；实际 analysis={:?}", analysis);
        assert!(analysis.spring_cold_index.is_some());
        // 加强：phase_at_index 里应至少有一个 SpringCold variant
        assert!(
            analysis
                .phase_at_index
                .iter()
                .any(|(_, p)| *p == RoundingBottomPhase::SpringCold),
            "phase_at_index 应至少有一个 SpringCold；实际 phases={:?}",
            analysis.phase_at_index
        );
    }

    #[test]
    fn t_rounding_bottom_multiple_neckline_candidates() {
        // 颈线候选：左端高点 + 右端高点
        let closes: Vec<f64> = (0..30)
            .map(|i| {
                if i < 15 {
                    100.0 - i as f64 * 1.5
                } else {
                    77.5 + (i - 15) as f64 * 1.8
                }
            })
            .collect();
        let analysis = analyze_rounding_bottom(&closes, 0, closes.len() - 1).unwrap();
        assert!(
            analysis.neckline_candidates.len() >= 1,
            "应至少有 1 个颈线候选"
        );
    }

    #[test]
    fn t_rounding_bottom_too_short_returns_none() {
        let closes = vec![100.0, 95.0, 90.0];
        assert!(analyze_rounding_bottom(&closes, 0, 2).is_none());
    }

    // -------- Sprint 7：R-P1-57 复杂头肩顶左肩 --------

    #[test]
    fn t_complex_left_shoulder_detected() {
        // 双峰左肩：110（主峰）+ 105（次峰，B 浪反弹）→ 然后头部 120
        let swings = vec![
            SwingPoint {
                index: 0,
                time: 0,
                price: 110.0,
                kind: SwingKind::High, // 主峰
            },
            SwingPoint {
                index: 5,
                time: 5 * 86_400_000,
                price: 100.0,
                kind: SwingKind::Low,
            },
            SwingPoint {
                index: 10,
                time: 10 * 86_400_000,
                price: 105.0, // 次峰（较低）
                kind: SwingKind::High,
            },
            SwingPoint {
                index: 15,
                time: 15 * 86_400_000,
                price: 98.0,
                kind: SwingKind::Low,
            },
            SwingPoint {
                index: 20,
                time: 20 * 86_400_000,
                price: 120.0, // 头部
                kind: SwingKind::High,
            },
        ];
        let analysis = analyze_complex_left_shoulder(&swings, 20);
        assert!(analysis.is_complex, "应识别为复杂左肩");
        assert_eq!(analysis.main_peak_index, 0); // 主峰在 index 0（110）
        assert_eq!(analysis.sub_peak_index, Some(10));
    }

    #[test]
    fn t_complex_left_shoulder_single_peak_not_complex() {
        // 仅一个高点 → 非复杂
        let swings = vec![
            SwingPoint {
                index: 0,
                time: 0,
                price: 110.0,
                kind: SwingKind::High,
            },
            SwingPoint {
                index: 5,
                time: 5 * 86_400_000,
                price: 100.0,
                kind: SwingKind::Low,
            },
            SwingPoint {
                index: 10,
                time: 10 * 86_400_000,
                price: 120.0,
                kind: SwingKind::High, // head
            },
        ];
        let analysis = analyze_complex_left_shoulder(&swings, 10);
        assert!(!analysis.is_complex);
    }

    // -------- Sprint 7：R-P1-23 头肩底量价对称 --------

    #[test]
    fn t_head_shoulders_volume_descending_pattern() {
        // 头肩底：左肩量 5 > 头部量 3 > 右肩量 2（递减）
        let klines = vec![
            mk_kline(0, 100.0, 100.0, 101.0, 99.0, 5.0), // left shoulder
            mk_kline(1, 100.0, 100.0, 101.0, 99.0, 1.0),
            mk_kline(2, 100.0, 100.0, 101.0, 99.0, 3.0), // head
            mk_kline(3, 100.0, 100.0, 101.0, 99.0, 1.0),
            mk_kline(4, 100.0, 100.0, 101.0, 99.0, 2.0), // right shoulder
        ];
        let vs = check_head_shoulders_volume(&klines, 0, 2, 4).unwrap();
        assert!(vs.is_descending, "左肩5 > 头部3 > 右肩2");
        // 量对称：|5 - 2| / 5 = 60% > 30%，不对称
        assert!(!vs.is_symmetric);
    }

    #[test]
    fn t_head_shoulders_volume_symmetric_but_not_descending() {
        // 左肩量 5，头部量 7，右肩量 5 → 不递减但量对称
        let klines = vec![
            mk_kline(0, 100.0, 100.0, 101.0, 99.0, 5.0),
            mk_kline(1, 100.0, 100.0, 101.0, 99.0, 1.0),
            mk_kline(2, 100.0, 100.0, 101.0, 99.0, 7.0),
            mk_kline(3, 100.0, 100.0, 101.0, 99.0, 1.0),
            mk_kline(4, 100.0, 100.0, 101.0, 99.0, 5.0),
        ];
        let vs = check_head_shoulders_volume(&klines, 0, 2, 4).unwrap();
        assert!(!vs.is_descending);
        assert!(vs.is_symmetric);
    }

    #[test]
    fn t_head_shoulders_volume_out_of_bounds_returns_none() {
        let klines = vec![mk_kline(0, 100.0, 100.0, 101.0, 99.0, 1.0)];
        assert!(check_head_shoulders_volume(&klines, 0, 0, 5).is_none());
    }
}
