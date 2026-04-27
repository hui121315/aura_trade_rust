//! 四维共振评分：将 A/B/C/D 信号投影到 [-100, +100] 区间

use serde::{Deserialize, Serialize};

use crate::data::Kline;
use crate::engine::candle::PatternHit;
use crate::engine::chartpattern::ChartPattern;
use crate::engine::ma::{MaSpecialHit, MaState};
use crate::engine::trend::{DowPhase, TrendState};

/// 单一维度的评分结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DimensionScore {
    pub name: String,
    pub score: f64,        // -100 ~ +100
    pub weight: f64,       // 维度权重（用于合成）
    pub contributions: Vec<String>, // 人类可读的贡献项
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Stance {
    StrongBull,
    Bull,
    WeakBull,
    Neutral,
    WeakBear,
    Bear,
    StrongBear,
}

impl Stance {
    pub fn label(&self) -> &'static str {
        match self {
            Stance::StrongBull => "强烈看涨",
            Stance::Bull => "看涨",
            Stance::WeakBull => "偏多",
            Stance::Neutral => "中性 / 观望",
            Stance::WeakBear => "偏空",
            Stance::Bear => "看跌",
            Stance::StrongBear => "强烈看跌",
        }
    }
    pub fn from_score(s: f64) -> Stance {
        if s >= 60.0 { Stance::StrongBull }
        else if s >= 30.0 { Stance::Bull }
        else if s >= 10.0 { Stance::WeakBull }
        else if s > -10.0 { Stance::Neutral }
        else if s > -30.0 { Stance::WeakBear }
        else if s > -60.0 { Stance::Bear }
        else { Stance::StrongBear }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResonanceScore {
    pub total: f64,                      // -100 ~ +100
    pub stance: Stance,
    pub stance_label: String,
    pub dimensions: Vec<DimensionScore>, // A/B/C/D 分数明细
    /// 参与评分的最新 K 线索引
    pub bar_index: usize,
    /// 维度方向一致性：所有维度方向相同 → 高；反之低
    pub alignment: f64, // 0~1
}

/// 维度权重（可调）
pub struct ResonanceWeights {
    pub ma: f64,
    pub trend: f64,
    pub candle: f64,
    pub chart: f64,
}

impl Default for ResonanceWeights {
    fn default() -> Self {
        Self { ma: 0.3, trend: 0.3, candle: 0.2, chart: 0.2 }
    }
}

impl ResonanceWeights {
    fn sanitized(&self) -> Self {
        let clean = |v: f64| if v.is_finite() && v > 0.0 { v } else { 0.0 };
        let out = Self {
            ma: clean(self.ma),
            trend: clean(self.trend),
            candle: clean(self.candle),
            chart: clean(self.chart),
        };
        if out.ma + out.trend + out.candle + out.chart > 1e-9 {
            out
        } else {
            Self::default()
        }
    }
}

pub fn compute_resonance(
    klines: &[Kline],
    ma_state: &MaState,
    ma_specials: &[MaSpecialHit],
    trend: &TrendState,
    recent_candles: &[PatternHit],
    recent_charts: &[ChartPattern],
    weights: &ResonanceWeights,
) -> ResonanceScore {
    let weights = weights.sanitized();
    let bar_index = klines.len().saturating_sub(1);

    // ---------- A：均线维度 ----------
    let mut dim_a = score_ma(ma_state, ma_specials);
    dim_a.weight = weights.ma;
    // ---------- B：趋势维度 ----------
    let mut dim_b = score_trend(trend);
    dim_b.weight = weights.trend;
    // ---------- C：K线形态维度 ----------
    let mut dim_c = score_candle(recent_candles, bar_index);
    dim_c.weight = weights.candle;
    // ---------- D：技术图形维度 ----------
    let mut dim_d = score_chart(recent_charts, bar_index);
    dim_d.weight = weights.chart;

    let dims = vec![dim_a, dim_b, dim_c, dim_d];
    // 合成：加权平均
    let total_w = weights.ma + weights.trend + weights.candle + weights.chart;
    let total =
        (dims[0].score * weights.ma
            + dims[1].score * weights.trend
            + dims[2].score * weights.candle
            + dims[3].score * weights.chart)
            / total_w.max(1e-9);
    let total = total.clamp(-100.0, 100.0);

    // 方向一致性：计算非零维度中符号一致的比例
    let non_zero: Vec<&DimensionScore> = dims.iter().filter(|d| d.score.abs() > 1.0).collect();
    let alignment = if non_zero.is_empty() {
        0.0
    } else {
        let signs: Vec<i8> = non_zero.iter().map(|d| d.score.signum() as i8).collect();
        let pos = signs.iter().filter(|&&s| s > 0).count();
        let neg = signs.iter().filter(|&&s| s < 0).count();
        let max = pos.max(neg) as f64;
        max / signs.len() as f64
    };

    let stance = Stance::from_score(total);

    ResonanceScore {
        total,
        stance,
        stance_label: stance.label().to_string(),
        dimensions: dims,
        bar_index,
        alignment,
    }
}

// ================= 各维度计分 =================

fn score_ma(ma: &MaState, specials: &[MaSpecialHit]) -> DimensionScore {
    let mut s = 0.0;
    let mut contribs = Vec::new();

    // 排列基础分
    use crate::engine::ma::Alignment;
    match ma.alignment {
        Alignment::Bullish => { s += 30.0; contribs.push("多头排列 +30".into()); }
        Alignment::Bearish => { s -= 30.0; contribs.push("空头排列 -30".into()); }
        Alignment::Converging => { s += 2.0; contribs.push("均线收敛 +2（蓄势）".into()); }
        Alignment::Diverging => { s -= 5.0; contribs.push("均线发散 -5（波动加大）".into()); }
        Alignment::Stuck => { contribs.push("均线粘合".into()); }
        _ => {}
    }

    // 斜率方向（基准均线）
    let base_slope = ma.slopes.get(
        ma.periods.iter().position(|&p| p == ma.bias_base_period).unwrap_or(0)
    ).copied().unwrap_or(0.0);
    if base_slope.abs() > 1e-6 {
        let val = base_slope * 1000.0; // 放大
        let clipped = val.clamp(-15.0, 15.0);
        s += clipped;
        contribs.push(format!("基准均线斜率 {:+.3} → {:+.1}", base_slope, clipped));
    }

    // BIAS：过高（+）或过低（-）都算反向力量
    if ma.bias_base.abs() > 0.08 {
        let penalty = (ma.bias_base.abs() - 0.08) * 200.0; // 0.1 → 4 分
        if ma.bias_base > 0.0 { s -= penalty; contribs.push(format!("BIAS 过高 {:+.1}%（超买） -{:.1}", ma.bias_base * 100.0, penalty)); }
        else { s += penalty; contribs.push(format!("BIAS 过低 {:+.1}%（超卖） +{:.1}", ma.bias_base * 100.0, penalty)); }
    }

    // 特殊形态贡献
    for sp in specials {
        let w = sp.weight as f64;
        let contrib = sp.direction as f64 * w * 3.0;
        if contrib.abs() > 0.1 {
            s += contrib;
            contribs.push(format!("{} {:+.1}", sp.label, contrib));
        }
    }

    DimensionScore {
        name: "A 均线".to_string(),
        score: (s as f64).clamp(-100.0, 100.0),
        weight: 0.0,
        contributions: contribs,
    }
}

fn score_trend(trend: &TrendState) -> DimensionScore {
    let mut s = 0.0;
    let mut contribs = Vec::new();
    match trend.dow.phase {
        DowPhase::Uptrend => { s += 40.0; contribs.push("道氏：上升趋势 +40".into()); }
        DowPhase::Downtrend => { s -= 40.0; contribs.push("道氏：下降趋势 -40".into()); }
        DowPhase::Consolidation => { contribs.push("道氏：整固".into()); }
        DowPhase::Unknown => {}
    }
    // 通道位置：在下轨附近加多头倾向（均值回归），上轨附近加空头倾向
    if let Some(pos) = trend.channel_position {
        if pos < 0.2 { s += 10.0; contribs.push(format!("通道下轨 ({:.0}%) +10", pos * 100.0)); }
        else if pos > 0.8 { s -= 10.0; contribs.push(format!("通道上轨 ({:.0}%) -10", pos * 100.0)); }
    }
    // 结构延续年龄：过久可能反转
    if trend.dow.structure_age_bars > 50 {
        contribs.push(format!("结构延续 {} bars（关注反转风险）", trend.dow.structure_age_bars));
    }
    // 缺口：未回补的突破方向缺口加分
    for g in &trend.gaps {
        if !g.filled {
            use crate::engine::trend::GapKind;
            let weight = match g.kind {
                GapKind::Breakaway => 10.0,
                GapKind::Runaway => 8.0,
                GapKind::Exhaustion => -5.0,
                GapKind::Common => 2.0,
            };
            let sign = match g.dir {
                crate::engine::trend::gap::GapDir::Up => 1.0,
                crate::engine::trend::gap::GapDir::Down => -1.0,
            };
            s += weight * sign;
            contribs.push(format!("{} {:?} {:+.1}", g.kind.label(), g.dir, weight * sign));
        }
    }
    DimensionScore {
        name: "B 趋势".to_string(),
        score: (s as f64).clamp(-100.0, 100.0),
        weight: 0.0,
        contributions: contribs,
    }
}

fn score_candle(patterns: &[PatternHit], bar_index: usize) -> DimensionScore {
    let mut s = 0.0;
    let mut contribs = Vec::new();
    // 仅考虑最近 5 根内的形态
    for p in patterns
        .iter()
        .rev()
        .filter(|p| p.index <= bar_index && bar_index - p.index <= 5)
    {
        let contrib = (p.direction as f64) * (p.strength as f64) * 4.0;
        s += contrib;
        if contribs.len() < 8 {
            contribs.push(format!("{} [{}星] {:+.1}", p.kind.label(), p.strength, contrib));
        }
    }
    DimensionScore {
        name: "C K线形态".to_string(),
        score: (s as f64).clamp(-100.0, 100.0),
        weight: 0.0,
        contributions: contribs,
    }
}

fn score_chart(patterns: &[ChartPattern], bar_index: usize) -> DimensionScore {
    let mut s = 0.0;
    let mut contribs = Vec::new();
    // 最近 30 根内完成的技术图形
    for p in patterns
        .iter()
        .filter(|p| p.completion_index <= bar_index && bar_index - p.completion_index <= 30)
    {
        let contrib = (p.direction as f64) * (p.strength as f64) * 5.0;
        s += contrib;
        if contribs.len() < 6 {
            contribs.push(format!("{} [{}星] {:+.1}", p.label, p.strength, contrib));
        }
    }
    DimensionScore {
        name: "D 技术图形".to_string(),
        score: (s as f64).clamp(-100.0, 100.0),
        weight: 0.0,
        contributions: contribs,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::candle::{PatternHit, PatternKind};
    use crate::engine::chartpattern::{ChartPattern, ChartPatternKind};
    use crate::engine::ma::{
        compute_ma_state, Alignment, MaKind, MaSpecialHit, MaSpecialKind, MaState,
    };
    use crate::engine::resonance::suggestion::{compute_suggestion, SuggestionInput};
    use crate::engine::trend::gap::GapDir;
    use crate::engine::trend::{
        compute_trend_state, DowPhase, DowState, Gap, GapKind, SwingKind, SwingPoint, TrendState,
    };

    fn mk_kline(idx: i64, o: f64, h: f64, l: f64, c: f64) -> Kline {
        Kline {
            open_time: idx * 86_400_000,
            close_time: (idx + 1) * 86_400_000 - 1,
            open: o,
            high: h,
            low: l,
            close: c,
            volume: 1.0,
        }
    }

    fn mk_ma_state(alignment: Alignment, bias_base: f64, base_slope: f64) -> MaState {
        MaState {
            symbol: "TEST".to_string(),
            timeframe: "1d".to_string(),
            kind: MaKind::Sma,
            periods: vec![20],
            last_values: vec![100.0],
            series: vec![vec![100.0]],
            alignment,
            alignment_aliases: vec![],
            spread_state: None,
            bias_base,
            bias_base_period: 20,
            slopes: vec![base_slope],
            crosses: vec![],
            granville: vec![],
            price_vs_base: "near",
        }
    }

    fn mk_special(kind: MaSpecialKind, bar_index: usize) -> MaSpecialHit {
        MaSpecialHit {
            kind,
            label: kind.label().to_string(),
            direction: kind.direction(),
            weight: kind.weight(),
            bar_index,
            description: String::new(),
        }
    }

    fn mk_swing(index: usize, price: f64, kind: SwingKind) -> SwingPoint {
        SwingPoint {
            index,
            time: index as i64,
            price,
            kind,
        }
    }

    fn mk_trend(
        phase: DowPhase,
        channel_position: Option<f64>,
        gaps: Vec<Gap>,
    ) -> TrendState {
        TrendState {
            swings: vec![],
            dow: DowState {
                phase,
                phase_label: phase.label().to_string(),
                recent_swings: vec![],
                last_highs: vec![],
                last_lows: vec![],
                structure_age_bars: 0,
                last_bar_index: 100,
            },
            trend_lines: vec![],
            channel: None,
            sr_levels: vec![],
            gaps,
            bars: 101,
            channel_position,
        }
    }

    fn mk_gap(dir: GapDir, kind: GapKind, filled: bool) -> Gap {
        Gap {
            index: 90,
            time: 90,
            dir,
            kind,
            label: kind.label().to_string(),
            top: 110.0,
            bottom: 100.0,
            size_pct: 0.1,
            filled,
            filled_index: None,
        }
    }

    fn mk_chart(kind: ChartPatternKind, completion_index: usize) -> ChartPattern {
        ChartPattern {
            kind,
            label: kind.label().to_string(),
            direction: kind.direction(),
            strength: kind.strength(),
            points: vec![
                mk_swing(0, 100.0, SwingKind::Low),
                mk_swing(completion_index, 110.0, SwingKind::High),
            ],
            neckline: None,
            target_price: None,
            completion_index,
            span_bars: completion_index,
            book_reliable: true,
        }
    }

    fn assert_close(actual: f64, expected: f64) {
        assert!(
            (actual - expected).abs() < 1e-9,
            "actual={}, expected={}",
            actual,
            expected
        );
    }

    // -------- Stance --------

    #[test]
    fn t_stance_from_score_all_7_buckets() {
        assert_eq!(Stance::from_score(80.0), Stance::StrongBull);
        assert_eq!(Stance::from_score(60.0), Stance::StrongBull); // ≥ 60
        assert_eq!(Stance::from_score(45.0), Stance::Bull);
        assert_eq!(Stance::from_score(30.0), Stance::Bull); // ≥ 30
        assert_eq!(Stance::from_score(20.0), Stance::WeakBull);
        assert_eq!(Stance::from_score(10.0), Stance::WeakBull); // ≥ 10
        assert_eq!(Stance::from_score(0.0), Stance::Neutral);
        assert_eq!(Stance::from_score(-9.9), Stance::Neutral);
        assert_eq!(Stance::from_score(-20.0), Stance::WeakBear);
        assert_eq!(Stance::from_score(-45.0), Stance::Bear);
        assert_eq!(Stance::from_score(-70.0), Stance::StrongBear);
    }

    #[test]
    fn t_stance_label_all_variants() {
        assert_eq!(Stance::StrongBull.label(), "强烈看涨");
        assert_eq!(Stance::Bull.label(), "看涨");
        assert_eq!(Stance::WeakBull.label(), "偏多");
        assert_eq!(Stance::Neutral.label(), "中性 / 观望");
        assert_eq!(Stance::WeakBear.label(), "偏空");
        assert_eq!(Stance::Bear.label(), "看跌");
        assert_eq!(Stance::StrongBear.label(), "强烈看跌");
    }

    // -------- ResonanceWeights --------

    #[test]
    fn t_resonance_weights_default_sum_to_one() {
        let w = ResonanceWeights::default();
        let sum = w.ma + w.trend + w.candle + w.chart;
        assert!((sum - 1.0).abs() < 1e-9, "默认权重之和应 = 1.0，实际 {}", sum);
    }

    #[test]
    fn t_resonance_weights_sanitize_invalid_values() {
        let w = ResonanceWeights {
            ma: -1.0,
            trend: f64::NAN,
            candle: 0.2,
            chart: 0.0,
        }
        .sanitized();
        assert_close(w.ma, 0.0);
        assert_close(w.trend, 0.0);
        assert_close(w.candle, 0.2);
        assert_close(w.chart, 0.0);
    }

    #[test]
    fn t_resonance_weights_sanitize_falls_back_to_default_when_all_invalid() {
        let w = ResonanceWeights {
            ma: -1.0,
            trend: 0.0,
            candle: f64::NAN,
            chart: f64::NEG_INFINITY,
        }
        .sanitized();
        let default = ResonanceWeights::default();
        assert_close(w.ma, default.ma);
        assert_close(w.trend, default.trend);
        assert_close(w.candle, default.candle);
        assert_close(w.chart, default.chart);
    }

    #[test]
    fn t_score_ma_counts_alignment_slope_bias_and_specials() {
        let ma = mk_ma_state(Alignment::Bullish, 0.10, 0.01);
        let specials = vec![mk_special(MaSpecialKind::BearArrangement, 100)];
        let score = score_ma(&ma, &specials);
        assert_close(score.score, 21.0);
        assert!(score.contributions.iter().any(|c| c.contains("多头排列")));
        assert!(score.contributions.iter().any(|c| c.contains("基准均线斜率")));
        assert!(score.contributions.iter().any(|c| c.contains("BIAS 过高")));
        assert!(score.contributions.iter().any(|c| c.contains("空头排列")));
    }

    #[test]
    fn t_score_ma_counts_low_bias_as_bullish_reversion() {
        let ma = mk_ma_state(Alignment::Bearish, -0.10, -0.01);
        let specials = vec![mk_special(MaSpecialKind::GoldenValley, 100)];
        let score = score_ma(&ma, &specials);
        assert_close(score.score, -21.0);
        assert!(score.contributions.iter().any(|c| c.contains("BIAS 过低")));
        assert!(score.contributions.iter().any(|c| c.contains("金山谷")));
    }

    #[test]
    fn t_score_trend_counts_dow_channel_and_unfilled_gap() {
        let trend = mk_trend(
            DowPhase::Uptrend,
            Some(0.1),
            vec![mk_gap(GapDir::Up, GapKind::Breakaway, false)],
        );
        let score = score_trend(&trend);
        assert_close(score.score, 60.0);
        assert!(score.contributions.iter().any(|c| c.contains("上升趋势")));
        assert!(score.contributions.iter().any(|c| c.contains("通道下轨")));
        assert!(score.contributions.iter().any(|c| c.contains("突破缺口")));
    }

    #[test]
    fn t_score_trend_ignores_filled_gap() {
        let trend = mk_trend(
            DowPhase::Downtrend,
            Some(0.9),
            vec![mk_gap(GapDir::Down, GapKind::Breakaway, true)],
        );
        let score = score_trend(&trend);
        assert_close(score.score, -50.0);
        assert!(!score.contributions.iter().any(|c| c.contains("突破缺口")));
    }

    #[test]
    fn t_score_candle_counts_only_bar_aligned_recent_patterns() {
        let patterns = vec![
            PatternHit {
                index: 96,
                kind: PatternKind::Hammer,
                direction: 1,
                strength: 2,
            },
            PatternHit {
                index: 100,
                kind: PatternKind::ShootingStar,
                direction: -1,
                strength: 3,
            },
            PatternHit {
                index: 101,
                kind: PatternKind::Hammer,
                direction: 1,
                strength: 5,
            },
            PatternHit {
                index: 94,
                kind: PatternKind::BearishEngulfing,
                direction: -1,
                strength: 5,
            },
        ];
        let score = score_candle(&patterns, 100);
        assert_close(score.score, -4.0);
        assert_eq!(score.contributions.len(), 2);
        assert!(score.contributions.iter().any(|c| c.contains("锤头线")));
        assert!(score.contributions.iter().any(|c| c.contains("射击之星")));
    }

    #[test]
    fn t_score_chart_counts_only_completed_recent_patterns() {
        let patterns = vec![
            mk_chart(ChartPatternKind::InverseHeadAndShoulders, 80),
            mk_chart(ChartPatternKind::HeadAndShoulders, 101),
            mk_chart(ChartPatternKind::DoubleTop, 69),
        ];
        let score = score_chart(&patterns, 100);
        assert_close(score.score, 25.0);
        assert_eq!(score.contributions.len(), 1);
        assert!(score.contributions[0].contains("头肩底"));
    }

    // -------- compute_resonance 集成路径 --------

    fn build_uptrend_klines(n: usize) -> Vec<Kline> {
        // 稳步上涨：每根 +1，带小幅振动
        (0..n)
            .map(|i| {
                let base = 100.0 + i as f64;
                mk_kline(i as i64, base, base + 0.6, base - 0.4, base + 0.5)
            })
            .collect()
    }

    fn build_downtrend_klines(n: usize) -> Vec<Kline> {
        (0..n)
            .map(|i| {
                let base = 200.0 - i as f64;
                mk_kline(i as i64, base, base + 0.4, base - 0.6, base - 0.5)
            })
            .collect()
    }

    #[test]
    fn t_compute_resonance_uptrend_yields_bullish_total() {
        let klines = build_uptrend_klines(80);
        let periods = vec![5, 10, 20];
        let ma_state = compute_ma_state("TEST", "1d", MaKind::Sma, &klines, &periods);
        let trend = compute_trend_state(&klines);
        let score = compute_resonance(
            &klines,
            &ma_state,
            &[],
            &trend,
            &[],
            &[],
            &ResonanceWeights::default(),
        );
        assert!(score.total > 0.0, "上升行情 total 应 > 0，实际 {}", score.total);
        assert!(
            matches!(score.stance, Stance::WeakBull | Stance::Bull | Stance::StrongBull),
            "上升行情 stance 应为多头方向，实际 {:?}", score.stance
        );
        assert_eq!(score.dimensions.len(), 4, "应有 A/B/C/D 四维");
    }

    #[test]
    fn t_compute_resonance_downtrend_yields_bearish_total() {
        let klines = build_downtrend_klines(80);
        let periods = vec![5, 10, 20];
        let ma_state = compute_ma_state("TEST", "1d", MaKind::Sma, &klines, &periods);
        let trend = compute_trend_state(&klines);
        let score = compute_resonance(
            &klines,
            &ma_state,
            &[],
            &trend,
            &[],
            &[],
            &ResonanceWeights::default(),
        );
        assert!(score.total < 0.0, "下跌行情 total 应 < 0，实际 {}", score.total);
        assert!(
            matches!(score.stance, Stance::WeakBear | Stance::Bear | Stance::StrongBear),
            "下跌行情 stance 应为空头方向，实际 {:?}", score.stance
        );
    }

    #[test]
    fn t_compute_resonance_clamped_to_100() {
        // 无论信号多强，total 应 ∈ [-100, 100]
        let klines = build_uptrend_klines(80);
        let periods = vec![5, 10, 20];
        let ma_state = compute_ma_state("TEST", "1d", MaKind::Sma, &klines, &periods);
        let trend = compute_trend_state(&klines);
        let score = compute_resonance(
            &klines,
            &ma_state,
            &[],
            &trend,
            &[],
            &[],
            &ResonanceWeights::default(),
        );
        assert!(score.total <= 100.0 && score.total >= -100.0);
        for d in &score.dimensions {
            assert!(d.score <= 100.0 && d.score >= -100.0, "维度 {} 分数越界：{}", d.name, d.score);
        }
    }

    #[test]
    fn t_compute_resonance_alignment_ranges_from_0_to_1() {
        let klines = build_uptrend_klines(80);
        let periods = vec![5, 10, 20];
        let ma_state = compute_ma_state("TEST", "1d", MaKind::Sma, &klines, &periods);
        let trend = compute_trend_state(&klines);
        let score = compute_resonance(
            &klines,
            &ma_state,
            &[],
            &trend,
            &[],
            &[],
            &ResonanceWeights::default(),
        );
        assert!(score.alignment >= 0.0 && score.alignment <= 1.0);
    }

    #[test]
    fn t_compute_resonance_stance_label_matches_stance() {
        let klines = build_uptrend_klines(60);
        let periods = vec![5, 10, 20];
        let ma_state = compute_ma_state("TEST", "1d", MaKind::Sma, &klines, &periods);
        let trend = compute_trend_state(&klines);
        let score = compute_resonance(
            &klines,
            &ma_state,
            &[],
            &trend,
            &[],
            &[],
            &ResonanceWeights::default(),
        );
        assert_eq!(score.stance_label, score.stance.label());
    }

    #[test]
    fn t_compute_resonance_exposes_dimension_weights() {
        let klines = build_uptrend_klines(60);
        let periods = vec![5, 10, 20];
        let ma_state = compute_ma_state("TEST", "1d", MaKind::Sma, &klines, &periods);
        let trend = compute_trend_state(&klines);
        let weights = ResonanceWeights {
            ma: 0.4,
            trend: 0.3,
            candle: 0.2,
            chart: 0.1,
        };
        let score = compute_resonance(&klines, &ma_state, &[], &trend, &[], &[], &weights);
        assert_close(score.dimensions[0].weight, 0.4);
        assert_close(score.dimensions[1].weight, 0.3);
        assert_close(score.dimensions[2].weight, 0.2);
        assert_close(score.dimensions[3].weight, 0.1);
    }

    #[test]
    fn t_compute_suggestion_uses_atr_risk_and_rr_for_long() {
        let score = ResonanceScore {
            total: 65.0,
            stance: Stance::StrongBull,
            stance_label: Stance::StrongBull.label().to_string(),
            dimensions: vec![DimensionScore {
                name: "A 均线".to_string(),
                score: 65.0,
                weight: 1.0,
                contributions: vec![],
            }],
            bar_index: 100,
            alignment: 1.0,
        };
        let input = SuggestionInput {
            account_equity: 10_000.0,
            current_price: 100.0,
            atr: 2.0,
            max_risk_pct: 0.02,
            rr_target: 2.0,
            atr_stop_mult: 1.5,
        };
        let suggestion = compute_suggestion(&score, &input);
        assert_eq!(suggestion.direction, 1);
        assert_close(suggestion.confidence, 1.0);
        assert_close(suggestion.risk_amount, 200.0);
        assert_close(suggestion.entry_price, 100.0);
        assert_close(suggestion.stop_loss, 97.0);
        assert_close(suggestion.take_profit, 106.0);
        assert_close(suggestion.suggested_position_size, 200.0 / 3.0);
        assert_close(suggestion.reward_amount, 400.0);
    }

    #[test]
    fn t_compute_suggestion_downgrades_confidence_when_dimensions_conflict() {
        let score = ResonanceScore {
            total: 35.0,
            stance: Stance::Bull,
            stance_label: Stance::Bull.label().to_string(),
            dimensions: vec![],
            bar_index: 100,
            alignment: 0.5,
        };
        let input = SuggestionInput {
            account_equity: 10_000.0,
            current_price: 100.0,
            atr: 2.0,
            max_risk_pct: 0.02,
            rr_target: 2.0,
            atr_stop_mult: 1.5,
        };
        let suggestion = compute_suggestion(&score, &input);
        assert_eq!(suggestion.direction, 1);
        assert_close(suggestion.confidence, 0.35);
        assert_close(suggestion.risk_amount, 70.0);
    }
}
