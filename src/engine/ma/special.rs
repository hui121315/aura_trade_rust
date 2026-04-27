//! A6：均线 17 大特殊形态识别（邱立波《均线技术分析》）
//!
//! 这些形态是原书最核心的"均线语言"，用于判定趋势阶段与拐点。
//!
//! # 原书追溯（Patch 5 v2 校准）
//!
//! 每个 enum 通过 [`MaSpecialKind::book_source`] 提供原书章节追溯。
//! 通过 [`MaSpecialKind::is_book_direct`] 区分 "原书直接对应" 与 "AURA 派生" 形态。
//! 通过 [`MaSpecialKind::severe_signal`] 标记原书强调的强信号形态。
//!
//! # 17 形态分类
//!
//! ## 原书直接对应（13 项）
//! - **多头排列**（Ch3·3·1，p.204）/ **空头排列**（Ch3·3·2，p.204）—— 杀伤力强（瀑布飞泻）
//! - **均线粘合**（Ch3·3·5）—— 收敛之后可能交叉
//! - **加速上行**（Ch4·1·1）/ **加速下行**（Ch4·1·2）—— 强势/弱势确认
//! - **银山谷**（Ch4·1·7）/ **金山谷**（Ch4·1·8）/ **死亡谷**（Ch4·1·9）
//! - **上山爬坡**（Ch3·3·1 慢牛）/ **下山滑坡**（Ch3·3·2 慢熊）
//! - **逐浪上升** / **逐浪下降**（Ch3·3·3-4）
//!
//! ## AURA 派生（4 项，工程辅助分类，非原书独立形态）
//! - **快速上升 / 快速下降**：基于均线间距百分比的量化形态（与加速重叠）
//! - **烂泥潭**：高频交叉震荡（原书 Ch3·3·5 粘合的弱化版）
//! - **牛熊分界**：原书 ma p.155 称 60 日均线为牛熊分界 —— 此 enum 是"价格紧贴牛熊线"的瞬时状态
//! - **周期轮换**：原书称为"普通交叉"（无方向同步的初次交叉）
//!
//! # 与 trend 书 "多级趋势线策略矩阵" 配套
//!
//! 强信号（[`MaSpecialKind::severe_signal`] = true）应触发 PRD R-P1-15 决策树。

use serde::{Deserialize, Serialize};

use super::alignment::Alignment;

/// 每种特殊形态的命中（同一 bar 可能命中多个）
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MaSpecialKind {
    UphillClimb,        // 上山爬坡
    AcceleratingUp,     // 加速上行
    DownhillSlide,      // 下山滑坡
    AcceleratingDown,   // 加速下行
    WaveUp,             // 逐浪上升
    WaveDown,           // 逐浪下降
    RapidUp,            // 快速上升
    RapidDown,          // 快速下降
    MaBond,             // 均线粘合
    SilverValley,       // 银山谷
    GoldenValley,       // 金山谷
    DeathValley,        // 死亡谷
    Mire,               // 烂泥潭
    BullArrangement,    // 多头排列（大信号）
    BearArrangement,    // 空头排列（大信号）
    BullBearBoundary,   // 牛熊分界
    CycleSwap,          // 周期轮换
}

impl MaSpecialKind {
    pub fn label(&self) -> &'static str {
        use MaSpecialKind::*;
        match self {
            UphillClimb => "上山爬坡",
            AcceleratingUp => "加速上行",
            DownhillSlide => "下山滑坡",
            AcceleratingDown => "加速下行",
            WaveUp => "逐浪上升",
            WaveDown => "逐浪下降",
            RapidUp => "快速上升",
            RapidDown => "快速下降",
            MaBond => "均线粘合",
            SilverValley => "银山谷",
            GoldenValley => "金山谷",
            DeathValley => "死亡谷",
            Mire => "烂泥潭",
            BullArrangement => "多头排列",
            BearArrangement => "空头排列",
            BullBearBoundary => "牛熊分界",
            CycleSwap => "周期轮换",
        }
    }

    pub fn direction(&self) -> i8 {
        use MaSpecialKind::*;
        match self {
            UphillClimb | AcceleratingUp | WaveUp | RapidUp | SilverValley | GoldenValley | BullArrangement => 1,
            DownhillSlide | AcceleratingDown | WaveDown | RapidDown | DeathValley | BearArrangement => -1,
            MaBond | Mire | BullBearBoundary | CycleSwap => 0,
        }
    }

    /// 权重（用于四维共振评分）—— Patch 5 v2 基于原书重要性校准
    ///
    /// 校准依据：
    /// - 原书 ma p.204 "形成空头排列后股价如瀑布般飞泻" → BullArrangement/BearArrangement = 5
    /// - 原书 Ch4·1·8/9 金山谷/死亡谷 "形成后股价大概率展开新一轮主升/主跌" → 5
    /// - 原书 ma p.165 "加速上行/下行" 是趋势加速段，定性强 → 4
    /// - SilverValley 是 GoldenValley 前置（信号弱于金山谷）→ 4
    /// - 原书强调慢牛/慢熊 "非常规则的赚钱机会" → UphillClimb/DownhillSlide = 4（v1 为 3，提升）
    /// - WaveUp/WaveDown 中枢逐次抬升/下移 = 标准趋势确认 → 3
    /// - MaBond 粘合（Ch3·3·5）= 趋势变盘前奏 → 3
    /// - RapidUp/Down 与加速重叠（AURA 派生）→ 3（v1 为 4，降低避免重复加权）
    /// - CycleSwap 普通交叉无方向同步 → 2（v1 为 3，降低，因 E5 修复后已不算交易信号）
    /// - BullBearBoundary 短暂状态 → 2
    /// - Mire 高频震荡 = 噪声为主 → 1（v1 为 2，降低）
    pub fn weight(&self) -> u8 {
        use MaSpecialKind::*;
        match self {
            BullArrangement | BearArrangement => 5,
            GoldenValley | DeathValley => 5,
            AcceleratingUp | AcceleratingDown => 4,
            UphillClimb | DownhillSlide => 4,
            SilverValley => 4,
            WaveUp | WaveDown => 3,
            MaBond => 3,
            RapidUp | RapidDown => 3,
            BullBearBoundary => 2,
            CycleSwap => 2,
            Mire => 1,
        }
    }

    /// 原书章节追溯 —— Patch 5 v2 新增
    ///
    /// 返回该形态对应的《均线技术分析》（邱立波）章节引用。
    /// 返回 `None` 表示该形态是 AURA 工程辅助分类，非原书独立形态。
    pub fn book_source(&self) -> Option<&'static str> {
        use MaSpecialKind::*;
        match self {
            BullArrangement => Some("ma p.196 Ch3·3·1 / p.204 形态原文"),
            BearArrangement => Some("ma p.204 Ch3·3·2 形态原文"),
            UphillClimb => Some("ma Ch3·3·1 上山爬坡（慢牛）"),
            DownhillSlide => Some("ma Ch3·3·2 下山滑坡（慢熊）"),
            WaveUp => Some("ma Ch3·3·3 逐浪上升"),
            WaveDown => Some("ma Ch3·3·4 逐浪下降"),
            MaBond => Some("ma Ch3·3·5 均线粘合"),
            AcceleratingUp => Some("ma p.165 Ch4·1·1 加速上行"),
            AcceleratingDown => Some("ma Ch4·1·2 加速下行"),
            SilverValley => Some("ma Ch4·1·7 银山谷"),
            GoldenValley => Some("ma Ch4·1·8 金山谷"),
            DeathValley => Some("ma Ch4·1·9 死亡谷"),
            // AURA 派生形态
            RapidUp | RapidDown => None, // 与加速形态重叠的量化变体
            Mire => None,                // 原书 Ch3·3·5 粘合的弱化版本
            BullBearBoundary => None,    // 原书 ma p.155 称 60 日均线为"牛熊分界"，但此 enum 表示瞬时状态
            CycleSwap => None,           // 原书称"普通交叉"，无方向同步
        }
    }

    /// 是否为原书直接对应的形态（非 AURA 派生）
    pub fn is_book_direct(&self) -> bool {
        self.book_source().is_some()
    }

    /// 是否为原书强调的强信号形态（用于 SignalLevel::Trigger 判定）
    ///
    /// 原书警句依据：
    /// - 多头/空头排列（瀑布飞泻）—— ma p.204
    /// - 金山谷/死亡谷（主升/主跌前奏）—— ma Ch4·1
    /// - 加速上行/下行（趋势加速段）—— ma p.165
    pub fn severe_signal(&self) -> bool {
        use MaSpecialKind::*;
        matches!(
            self,
            BullArrangement
                | BearArrangement
                | GoldenValley
                | DeathValley
                | AcceleratingUp
                | AcceleratingDown
        )
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MaSpecialHit {
    pub kind: MaSpecialKind,
    pub label: String,
    pub direction: i8,
    pub weight: u8,
    pub bar_index: usize,
    pub description: String,
}

/// 识别参数
#[derive(Debug, Clone, Copy)]
pub struct SpecialParams {
    /// 粘合阈值：所有均线两两价差 / 均值 < 该百分比
    pub bond_pct: f64,
    /// 加速阈值：最新斜率 > `accel_factor` × 长期平均斜率
    pub accel_factor: f64,
    /// 烂泥潭：最近 N 根 K线内至少 M 次交叉
    pub mire_window: usize,
    pub mire_crosses: usize,
    /// 牛熊分界附近（价格与长均线价差 < pct）
    pub boundary_pct: f64,
}

impl Default for SpecialParams {
    fn default() -> Self {
        Self {
            bond_pct: 0.008,
            accel_factor: 2.0,
            mire_window: 20,
            mire_crosses: 4,
            boundary_pct: 0.005,
        }
    }
}

/// 扫描整个序列，返回最后一根 K线的状态快照
///
/// 为了保证性能，我们只对"最后一根 K线"输出所有命中形态，而不是每根 K线都输出（避免 O(n) 爆炸）。
/// 回测引擎可以在循环内逐根调用 `scan_at`。
pub fn scan_at(
    closes: &[f64],
    ma_series: &[Vec<f64>],      // 每条均线的完整序列
    periods: &[usize],           // 对应周期
    alignment: Alignment,
    slopes: &[f64],              // 基准均线的斜率序列
    base_period: usize,
    cross_bars: &[usize],        // 近期交叉事件发生的 bar 索引
    bar_index: usize,
    p: &SpecialParams,
) -> Vec<MaSpecialHit> {
    let mut hits = Vec::new();
    if ma_series.is_empty() || closes.is_empty() {
        return hits;
    }
    let last = bar_index.min(closes.len().saturating_sub(1));
    let price = closes[last];

    // --- 排列形态（直接映射）---
    match alignment {
        Alignment::Bullish => hits.push(make(MaSpecialKind::BullArrangement, last, "所有均线依次向上排列".into())),
        Alignment::Bearish => hits.push(make(MaSpecialKind::BearArrangement, last, "所有均线依次向下排列".into())),
        _ => {}
    }

    // --- 粘合：所有均线两两价差 / 均值 < pct
    let mut mvals = Vec::new();
    for s in ma_series {
        if let Some(&v) = s.get(last) {
            if v.is_finite() { mvals.push(v); }
        }
    }
    if mvals.len() >= 2 {
        let avg = mvals.iter().sum::<f64>() / mvals.len() as f64;
        let max = mvals.iter().cloned().fold(f64::MIN, f64::max);
        let min = mvals.iter().cloned().fold(f64::MAX, f64::min);
        if avg.abs() > 1e-9 && (max - min) / avg.abs() < p.bond_pct {
            hits.push(make(MaSpecialKind::MaBond, last, format!("所有均线在 {:.2}% 区间内", (max - min) / avg * 100.0)));
        }
    }

    // --- 牛熊分界：价格与基准均线的偏离度
    if let Some(base_idx) = periods.iter().position(|&x| x == base_period) {
        if let Some(&base_v) = ma_series[base_idx].get(last) {
            if base_v.abs() > 1e-9 && (price - base_v).abs() / base_v.abs() < p.boundary_pct {
                hits.push(make(MaSpecialKind::BullBearBoundary, last, format!("价格紧贴 MA{}", base_period)));
            }
        }
    }

    // --- 上山爬坡 / 下山滑坡：基于斜率
    if let Some(&slope) = slopes.get(last) {
        let recent_slopes: Vec<f64> = slopes
            .iter()
            .rev()
            .take(20)
            .filter(|s| s.is_finite())
            .copied()
            .collect();
        if !recent_slopes.is_empty() {
            let avg_slope: f64 = recent_slopes.iter().sum::<f64>() / recent_slopes.len() as f64;
            if matches!(alignment, Alignment::Bullish) {
                if slope > 0.0 {
                    if slope > avg_slope * p.accel_factor {
                        hits.push(make(MaSpecialKind::AcceleratingUp, last, "基准均线斜率显著放大".into()));
                    } else {
                        hits.push(make(MaSpecialKind::UphillClimb, last, "均线温和向上".into()));
                    }
                }
            }
            if matches!(alignment, Alignment::Bearish) {
                if slope < 0.0 {
                    if slope.abs() > avg_slope.abs() * p.accel_factor {
                        hits.push(make(MaSpecialKind::AcceleratingDown, last, "基准均线斜率显著转陡".into()));
                    } else {
                        hits.push(make(MaSpecialKind::DownhillSlide, last, "均线温和向下".into()));
                    }
                }
            }
        }
    }

    // --- 快速上升 / 快速下降：短均线与长均线价差快速扩大
    if ma_series.len() >= 2 {
        let (first, last_s) = (&ma_series[0], ma_series.last().unwrap());
        if let (Some(&f), Some(&l)) = (first.get(last), last_s.get(last)) {
            if f.is_finite() && l.is_finite() && l.abs() > 1e-9 {
                let spread = (f - l) / l.abs();
                if spread > 0.08 {
                    hits.push(make(MaSpecialKind::RapidUp, last, format!("MA{} 相对 MA{} 高出 {:.1}%", periods[0], periods.last().unwrap(), spread * 100.0)));
                } else if spread < -0.08 {
                    hits.push(make(MaSpecialKind::RapidDown, last, format!("MA{} 相对 MA{} 低出 {:.1}%", periods[0], periods.last().unwrap(), (-spread) * 100.0)));
                }
            }
        }
    }

    // --- 烂泥潭：近 mire_window 根内有 mire_crosses+ 次交叉
    let recent_crosses = cross_bars.iter().filter(|&&idx| last.saturating_sub(idx) < p.mire_window).count();
    if recent_crosses >= p.mire_crosses {
        hits.push(make(MaSpecialKind::Mire, last, format!("近 {} 根内出现 {} 次交叉", p.mire_window, recent_crosses)));
    }

    // --- 周期轮换：近 5 根发生过交叉，且方向与当前排列一致
    let recent_cross_bar = cross_bars.iter().rev().find(|&&idx| last >= idx && last - idx < 5);
    if recent_cross_bar.is_some() && matches!(alignment, Alignment::Bullish | Alignment::Bearish) {
        hits.push(make(MaSpecialKind::CycleSwap, last, "近期短期均线穿越长期均线".into()));
    }

    // --- 逐浪上升 / 逐浪下降：价格围绕 base_period 来回波动，但中枢抬升/下移
    if let Some(base_idx) = periods.iter().position(|&x| x == base_period) {
        let ma = &ma_series[base_idx];
        let n = closes.len().min(last + 1);
        if n >= 30 {
            let recent_ma = &ma[n.saturating_sub(30)..n];
            let valid: Vec<f64> = recent_ma.iter().filter(|v| v.is_finite()).copied().collect();
            if valid.len() >= 10 {
                let mid = valid.len() / 2;
                let first_half: f64 = valid[..mid].iter().sum::<f64>() / mid as f64;
                let second_half: f64 = valid[mid..].iter().sum::<f64>() / (valid.len() - mid) as f64;
                let delta = (second_half - first_half) / first_half.abs().max(1e-9);
                if delta > 0.02 && matches!(alignment, Alignment::Bullish | Alignment::Mixed | Alignment::Converging | Alignment::Diverging) {
                    hits.push(make(MaSpecialKind::WaveUp, last, format!("30 根内中枢抬升 {:.1}%", delta * 100.0)));
                }
                if delta < -0.02 && matches!(alignment, Alignment::Bearish | Alignment::Mixed | Alignment::Converging | Alignment::Diverging) {
                    hits.push(make(MaSpecialKind::WaveDown, last, format!("30 根内中枢下移 {:.1}%", (-delta) * 100.0)));
                }
            }
        }
    }

    // --- 银山谷 / 金山谷 / 死亡谷：需要短中长三条均线
    if ma_series.len() >= 3 {
        let s = &ma_series[0];
        let m = &ma_series[1];
        let l = &ma_series[2];
        if let (Some(&sv), Some(&mv), Some(&lv)) = (s.get(last), m.get(last), l.get(last)) {
            if sv.is_finite() && mv.is_finite() && lv.is_finite() {
                // 银山谷：短刚上穿中，中仍下穿长（或相反）形成向下三角；出现在下跌末端
                let short_above_mid = sv > mv;
                let mid_below_long = mv < lv;
                if short_above_mid && mid_below_long && matches!(alignment, Alignment::Mixed | Alignment::Converging) {
                    if let Some(&first_cross) = cross_bars.iter().rev().find(|&&idx| last >= idx && last - idx < 15) {
                        hits.push(make(MaSpecialKind::SilverValley, last,
                            format!("短上穿中，长线仍压制，首次金叉 @bar{}", first_cross)));
                    }
                }
                // 金山谷：银山谷之后再次形成向上三角（短继续在中之上，且短也上穿长）
                let short_above_long = sv > lv;
                if short_above_mid && short_above_long && matches!(alignment, Alignment::Bullish) {
                    // 必须近期刚完成突破
                    if let Some(&first_cross) = cross_bars.iter().rev().find(|&&idx| last >= idx && last - idx < 10) {
                        hits.push(make(MaSpecialKind::GoldenValley, last,
                            format!("短均线先后上穿中/长线，@bar{}", first_cross)));
                    }
                }
                // 死亡谷：短下穿长，中下穿长（空头结构形成）
                let short_below_mid = sv < mv;
                let mid_above_long = mv > lv;
                if short_below_mid && mid_above_long && matches!(alignment, Alignment::Mixed | Alignment::Converging) {
                    if let Some(&first_cross) = cross_bars.iter().rev().find(|&&idx| last >= idx && last - idx < 15) {
                        hits.push(make(MaSpecialKind::DeathValley, last,
                            format!("短下穿中，长线仍支撑，首次死叉 @bar{}", first_cross)));
                    }
                }
            }
        }
    }

    hits
}

fn make(kind: MaSpecialKind, bar_index: usize, description: String) -> MaSpecialHit {
    MaSpecialHit {
        kind,
        label: kind.label().to_string(),
        direction: kind.direction(),
        weight: kind.weight(),
        bar_index,
        description,
    }
}
