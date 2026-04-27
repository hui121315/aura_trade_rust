//! B3：趋势线自动拟合
//!
//! 策略：用最近若干个摆动高点连线 = 阻力趋势线；
//! 用最近若干个摆动低点连线 = 支撑趋势线。
//! 保留"碰到最多点数"的最佳组合作为主趋势线。
//!
//! # 坐标系（E29）
//!
//! 原书 trend p.188 明确：
//! > "对数坐标系中，所有当日涨跌幅相等的股票价格，K 线长度都是一样的。"
//!
//! 原书 trend p.193 上证指数案例：普通坐标系看似突破下降通道，对数坐标系下通道仍稳定。
//!
//! 本模块通过 [`CoordinateSystem`] 支持两种坐标系。**长期趋势线（≥60 根）必须用对数坐标**。

use serde::{Deserialize, Serialize};

use super::swing::{SwingKind, SwingPoint};
use crate::data::Kline;

/// 「有效突破」阈值 —— 候选趋势线若有 K 线实体穿越 ≥ 此值，
/// 即视为"已破位"，应从候选里剔除。
///
/// 原书 trend p.203 用 3%；我们收紧至 **2%** 以过滤更多"边缘候选"，
/// 避免跑出形似"线穿多根实体"的图形。
pub const EFFECTIVE_BREAK_TOLERANCE: f64 = 0.02;

/// 包络线软容差 —— 允许单个 swing 点在线的"错误侧"凸出最多此比例
///
/// 0 = 严格包络（所有点必须在正确侧）；0.005 = 允许 0.5% 容差（更 robust）
const ENVELOPE_SOFT_TOLERANCE: f64 = 0.005;

/// 包络线法考虑的最近同类 swing 点数
const ENVELOPE_RECENT_POINTS: usize = 8;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TrendLineKind {
    Support,
    Resistance,
}

/// 坐标系（E29）—— 决定趋势线在线性还是对数空间拟合
///
/// 原书 trend p.188 / p.193：长期趋势线必须用对数坐标系，否则会失真。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum CoordinateSystem {
    /// 线性坐标系（默认，适合短期趋势）
    #[default]
    Linear,
    /// 对数坐标系（**必须用于长期趋势线 ≥ 60 根 K 线**）
    Logarithmic,
}

impl CoordinateSystem {
    /// 根据趋势线跨度自动选择坐标系（E29 推荐用法）
    ///
    /// - 跨度 < 60 根：线性
    /// - 跨度 ≥ 60 根：对数
    pub fn auto_for_span(span_bars: usize) -> Self {
        if span_bars >= 60 {
            CoordinateSystem::Logarithmic
        } else {
            CoordinateSystem::Linear
        }
    }

    /// 将原始价格映射到坐标系空间
    pub fn map(&self, price: f64) -> f64 {
        match self {
            CoordinateSystem::Linear => price,
            CoordinateSystem::Logarithmic => {
                if price > 0.0 {
                    price.ln()
                } else {
                    f64::NAN
                }
            }
        }
    }

    /// 将坐标系空间值反映射回价格
    pub fn unmap(&self, mapped: f64) -> f64 {
        match self {
            CoordinateSystem::Linear => mapped,
            CoordinateSystem::Logarithmic => mapped.exp(),
        }
    }
}

/// 用时间（ms）-价格 形式存储，前端可直接按时间定位
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrendLine {
    pub kind: TrendLineKind,
    pub p1_index: usize,
    pub p1_time: i64,
    pub p1_price: f64,
    pub p2_index: usize,
    pub p2_time: i64,
    pub p2_price: f64,
    /// index-空间的斜率（**在所用坐标系下**：线性 = price per bar；对数 = ln(price) per bar）
    pub slope_per_bar: f64,
    /// 线上触及的 swing 点数（越多越可靠）
    pub touches: usize,
    /// 平均偏差（越小越干净）
    pub avg_deviation: f64,
    /// 坐标系（E29 新增）
    #[serde(default)]
    pub coordinate: CoordinateSystem,
}

impl TrendLine {
    /// 在指定 K 线索引处投影计算趋势线价格
    ///
    /// 自动处理坐标系反映射。
    pub fn project_price(&self, index: usize) -> f64 {
        let p1_mapped = self.coordinate.map(self.p1_price);
        let mapped =
            p1_mapped + self.slope_per_bar * ((index as f64) - (self.p1_index as f64));
        self.coordinate.unmap(mapped)
    }

    /// 检测当前 K 线收盘价是否有效突破/跌破趋势线
    ///
    /// 返回 `Some(true)` = 有效突破（向上）；`Some(false)` = 有效跌破（向下）；`None` = 未达 3% 价差
    ///
    /// 原书 trend p.203 铁证："**未达 3% 的价差，上升趋势线继续有效**"
    pub fn check_effective_break(&self, current_price: f64, current_index: usize) -> Option<bool> {
        const EFFECTIVE_THRESHOLD: f64 = 0.03; // 原书 3%
        let line_price = self.project_price(current_index);
        if line_price.abs() < 1e-9 {
            return None;
        }
        let diff_pct = (current_price - line_price) / line_price.abs();
        if diff_pct.abs() < EFFECTIVE_THRESHOLD {
            None
        } else if diff_pct > 0.0 {
            Some(true)
        } else {
            Some(false)
        }
    }

    /// E31：趋势线画法校验 —— 禁穿 K 线实体（原书 trend p.201 铁证）
    ///
    /// 原书：
    /// > "绘制趋势线必须遵循一个重要规则：**趋势线不能穿越 K 线实体**。"
    ///
    /// # 规则
    /// - **支撑线**：K 线实体下沿（`min(open, close)`）**不得低于**趋势线投影价（允许影线穿过）
    /// - **阻力线**：K 线实体上沿（`max(open, close)`）**不得高于**趋势线投影价
    /// - 两个端点本身（p1/p2）除外
    /// - `tolerance_pct` 容差（默认 0，严格模式）
    ///
    /// # 返回
    /// - `true` = 无 K 线实体穿越（画法有效）
    /// - `false` = 至少一根 K 线实体穿越（画法违规）
    pub fn validate_no_body_pierce(
        &self,
        klines: &[crate::data::Kline],
        tolerance_pct: f64,
    ) -> bool {
        if klines.is_empty() {
            return true;
        }
        let start = self.p1_index;
        let end = self.p2_index.min(klines.len().saturating_sub(1));
        if start >= end {
            return true;
        }
        for i in (start + 1)..end {
            // 排除两个端点
            if i == self.p1_index || i == self.p2_index {
                continue;
            }
            let k = &klines[i];
            let line_price = self.project_price(i);
            if !line_price.is_finite() {
                continue;
            }
            let body_low = k.open.min(k.close);
            let body_high = k.open.max(k.close);
            match self.kind {
                TrendLineKind::Support => {
                    // 支撑线：实体下沿应 >= 趋势线投影价
                    if body_low < line_price * (1.0 - tolerance_pct) {
                        return false;
                    }
                }
                TrendLineKind::Resistance => {
                    // 阻力线：实体上沿应 <= 趋势线投影价
                    if body_high > line_price * (1.0 + tolerance_pct) {
                        return false;
                    }
                }
            }
        }
        true
    }
}

/// 拟合趋势线（默认线性坐标）
///
/// `klines` 用于 E31 画法校验 —— 候选若有 K 线实体穿越 ≥
/// [`EFFECTIVE_BREAK_TOLERANCE`] 则剔除。传 `&[]` 跳过校验（用于单元测试）。
pub fn fit_lines(
    swings: &[SwingPoint],
    klines: &[Kline],
    tolerance_pct: f64,
) -> Vec<TrendLine> {
    fit_lines_with_coord(swings, klines, tolerance_pct, CoordinateSystem::Linear)
}

/// 使用指定坐标系拟合趋势线（E29 新增）
///
/// 长期趋势线（跨度 ≥ 60 根）应使用 [`CoordinateSystem::Logarithmic`]。
///
/// `klines` 用于 E31 画法校验。
pub fn fit_lines_with_coord(
    swings: &[SwingPoint],
    klines: &[Kline],
    tolerance_pct: f64,
    coord: CoordinateSystem,
) -> Vec<TrendLine> {
    let mut out = Vec::new();
    let highs: Vec<&SwingPoint> = swings.iter().filter(|s| s.kind == SwingKind::High).collect();
    let lows: Vec<&SwingPoint> = swings.iter().filter(|s| s.kind == SwingKind::Low).collect();

    if let Some(r) = best_line(&highs, klines, true, tolerance_pct, coord) {
        out.push(r);
    }
    if let Some(s) = best_line(&lows, klines, false, tolerance_pct, coord) {
        out.push(s);
    }
    out
}

/// 对每对（最近若干摆动点的组合）做评分，挑最合适的一条
///
/// **算法优先级**：
/// 1. **包络线法**（首选）：所有其他 swing 点都在线的"正确侧"（含
///    [`ENVELOPE_SOFT_TOLERANCE`] 软容差）。这是原书 trend p.188 要求的
///    "**所有摆动点在线同一侧**"的严格定义，真正的包络线。
/// 2. **退化：touches 排序**（包络线找不到时）：按现有的 "touches 多、
///    avg_dev 小" 评分选最好的。确保极端市场下"起码有线可画"。
///
/// 两种模式都会额外做 E31 画法校验（原书 trend p.201 + p.203）：
/// 实体穿越 ≥ [`EFFECTIVE_BREAK_TOLERANCE`] 的候选被剔除。
fn best_line(
    pts: &[&SwingPoint],
    klines: &[Kline],
    is_resistance: bool,
    tol_pct: f64,
    coord: CoordinateSystem,
) -> Option<TrendLine> {
    let len = pts.len();
    if len < 2 {
        return None;
    }
    let take = ENVELOPE_RECENT_POINTS.min(len);
    let recent: Vec<&SwingPoint> = pts.iter().rev().take(take).rev().copied().collect();

    // 第 1 趟：包络线候选（所有点在正确侧）
    let mut envelope: Option<TrendLine> = None;
    // 第 2 趟：保底候选（传统 touches 排序）
    let mut fallback: Option<TrendLine> = None;

    for i in 0..recent.len() {
        for j in (i + 1)..recent.len() {
            let a = recent[i];
            let b = recent[j];
            if a.index == b.index {
                continue;
            }
            let a_mapped = coord.map(a.price);
            let b_mapped = coord.map(b.price);
            if !a_mapped.is_finite() || !b_mapped.is_finite() {
                continue;
            }
            let slope = (b_mapped - a_mapped) / ((b.index as f64) - (a.index as f64));

            // 计算所有 swing 点的统计
            let mut touches = 0usize;
            let mut dev_sum = 0.0;
            let mut all_on_correct_side = true;
            for p in &recent {
                let proj_mapped = a_mapped + slope * ((p.index as f64) - (a.index as f64));
                let proj_price = coord.unmap(proj_mapped);
                let dev = (p.price - proj_price).abs() / proj_price.abs().max(1e-9);
                dev_sum += dev;
                // 阻力线要求点 ≤ 线（不向上凸出）；支撑线反之
                let on_correct_side = if is_resistance {
                    p.price <= proj_price * (1.0 + ENVELOPE_SOFT_TOLERANCE)
                } else {
                    p.price >= proj_price * (1.0 - ENVELOPE_SOFT_TOLERANCE)
                };
                if !on_correct_side {
                    all_on_correct_side = false;
                }
                // touches = 距离线 <= tol_pct 的点数
                if dev <= tol_pct && on_correct_side {
                    touches += 1;
                }
            }
            let avg_dev = dev_sum / recent.len() as f64;

            let cand = TrendLine {
                kind: if is_resistance {
                    TrendLineKind::Resistance
                } else {
                    TrendLineKind::Support
                },
                p1_index: a.index,
                p1_time: a.time,
                p1_price: a.price,
                p2_index: b.index,
                p2_time: b.time,
                p2_price: b.price,
                slope_per_bar: slope,
                touches,
                avg_deviation: avg_dev,
                coordinate: coord,
            };

            // E31 画法校验：实体穿越 ≥ 2% 的候选剔除
            if !klines.is_empty()
                && !cand.validate_no_body_pierce(klines, EFFECTIVE_BREAK_TOLERANCE)
            {
                continue;
            }

            // 排序规则
            let better = |prev: &TrendLine, cand: &TrendLine| -> bool {
                // 先比 touches，再比 avg_dev（越小越好），再比 p2_index（越大越新）
                cand.touches > prev.touches
                    || (cand.touches == prev.touches
                        && (cand.avg_deviation < prev.avg_deviation
                            || (cand.avg_deviation == prev.avg_deviation
                                && cand.p2_index > prev.p2_index)))
            };

            if all_on_correct_side {
                match &envelope {
                    None => envelope = Some(cand.clone()),
                    Some(prev) if better(prev, &cand) => envelope = Some(cand.clone()),
                    _ => {}
                }
            }
            match &fallback {
                None => fallback = Some(cand),
                Some(prev) => {
                    if better(prev, &cand) {
                        fallback = Some(cand);
                    }
                }
            }
        }
    }

    // 优先返回包络线候选，否则退化到 fallback
    envelope.or(fallback)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sp(idx: usize, price: f64, kind: SwingKind) -> SwingPoint {
        SwingPoint {
            index: idx,
            time: (idx as i64) * 86_400_000,
            price,
            kind,
        }
    }

    #[test]
    fn t_coord_system_auto_select() {
        // E29 自动选择：< 60 根用线性
        assert_eq!(CoordinateSystem::auto_for_span(30), CoordinateSystem::Linear);
        assert_eq!(CoordinateSystem::auto_for_span(59), CoordinateSystem::Linear);
        // ≥ 60 根用对数
        assert_eq!(
            CoordinateSystem::auto_for_span(60),
            CoordinateSystem::Logarithmic
        );
        assert_eq!(
            CoordinateSystem::auto_for_span(120),
            CoordinateSystem::Logarithmic
        );
    }

    #[test]
    fn t_coord_map_unmap_inverse() {
        // 不变性：unmap(map(x)) ≈ x
        let prices = [10.0, 100.0, 1000.0, 0.5, 1.0];
        for p in prices {
            for coord in [CoordinateSystem::Linear, CoordinateSystem::Logarithmic] {
                let mapped = coord.map(p);
                let back = coord.unmap(mapped);
                assert!(
                    (back - p).abs() < 1e-9,
                    "{:?}.unmap(map({})) = {}",
                    coord,
                    p,
                    back
                );
            }
        }
    }

    #[test]
    fn t_log_coord_invalid_for_negative() {
        // 对数坐标对负值/零返回 NaN
        assert!(CoordinateSystem::Logarithmic.map(-1.0).is_nan());
        assert!(CoordinateSystem::Logarithmic.map(0.0).is_nan() || CoordinateSystem::Logarithmic.map(0.0).is_infinite());
    }

    #[test]
    fn t_log_coord_compresses_large_prices() {
        // 对数坐标系下，大价格变化被压缩（这是关键属性）
        // 100 → 200（涨 100%）和 10 → 20（涨 100%）在对数坐标下增量相同
        let log = CoordinateSystem::Logarithmic;
        let delta_high = log.map(200.0) - log.map(100.0);
        let delta_low = log.map(20.0) - log.map(10.0);
        assert!(
            (delta_high - delta_low).abs() < 1e-9,
            "100%涨幅在对数坐标下应该有相同增量：{} vs {}",
            delta_high,
            delta_low
        );
    }

    #[test]
    fn t_fit_lines_default_is_linear() {
        // 向后兼容：fit_lines() 默认线性
        let swings = vec![
            sp(0, 100.0, SwingKind::Low),
            sp(10, 110.0, SwingKind::High),
            sp(20, 105.0, SwingKind::Low),
            sp(30, 115.0, SwingKind::High),
        ];
        let lines = fit_lines(&swings, &[], 0.05);
        for line in &lines {
            assert_eq!(line.coordinate, CoordinateSystem::Linear);
        }
    }

    #[test]
    fn t_fit_lines_with_log_coord() {
        // E29：使用对数坐标拟合
        let swings = vec![
            sp(0, 100.0, SwingKind::Low),
            sp(60, 200.0, SwingKind::Low),
            sp(120, 400.0, SwingKind::Low),
            sp(30, 150.0, SwingKind::High),
            sp(90, 300.0, SwingKind::High),
        ];
        let lines =
            fit_lines_with_coord(&swings, &[], 0.05, CoordinateSystem::Logarithmic);
        for line in &lines {
            assert_eq!(line.coordinate, CoordinateSystem::Logarithmic);
        }
    }

    #[test]
    fn t_e31_fit_lines_rejects_candidate_with_body_pierce_above_3pct() {
        // 回归：有候选支撑线但被 K 线实体穿越 >3%，应该被剔除
        // 构造两条竞争支撑线：
        //   线 A：idx0→idx40 连 Low，中间 K 线实体穿越 >3%（应被剔除）
        //   线 B：idx20→idx40 连 Low，无 K 线实体穿越
        let swings = vec![
            sp(0, 100.0, SwingKind::Low),
            sp(20, 102.0, SwingKind::Low),
            sp(40, 104.0, SwingKind::Low),
        ];
        // idx10 的 K 线实体深度下探 90，远低于线 A 投影 ~101（约 -10.9% 穿越）
        let mut klines: Vec<_> = (0..=40)
            .map(|i| mk_kline(i as i64, 105.0, 106.0, 107.0, 99.0))
            .collect();
        klines[10] = mk_kline(10, 92.0, 90.0, 94.0, 89.0);

        let lines = fit_lines(&swings, &klines, 0.015);
        let support = lines.iter().find(|l| l.kind == TrendLineKind::Support);
        if let Some(s) = support {
            // 若有支撑线，必定不是线 A（p1_index=0）
            assert!(
                s.p1_index > 0,
                "E31 应剔除 idx0→idx40 的线段（中间实体穿越 >3%），实际 p1={}",
                s.p1_index
            );
        }
        // 反之：klines=&[] 时，线 A 不会被剔除，应能选出 p1=0 的版本
        let lines_no_validate = fit_lines(&swings, &[], 0.015);
        let support_nv = lines_no_validate
            .iter()
            .find(|l| l.kind == TrendLineKind::Support);
        assert!(
            support_nv.is_some(),
            "不带 klines 校验时应能选出支撑线"
        );
    }

    #[test]
    fn t_envelope_line_prefers_all_points_on_correct_side() {
        // 验证包络线优先：当存在"所有点在正确侧"的候选时，应选它
        // 而非"穿过多个点的中线"。
        //
        // 场景：支撑线候选
        //   - 线 A（中线）：过 idx0 和 idx40 的低点，中间点 idx20 高高在上 (大 touches)
        //   - 线 B（包络）：过 idx20 和 idx40 的低点，idx0 严格在线下方
        // 现有算法的 touches 会偏爱 A；包络线法应选 B。
        let swings = vec![
            sp(0, 100.0, SwingKind::Low),
            sp(20, 108.0, SwingKind::Low), // 中间"跳高"
            sp(40, 120.0, SwingKind::Low),
        ];
        let lines = fit_lines(&swings, &[], 0.02);
        let support = lines
            .iter()
            .find(|l| l.kind == TrendLineKind::Support)
            .expect("应选出支撑线");
        // 包络线：p1=idx0→p2=idx40 的斜率 = (120-100)/40 = 0.5
        // idx20 的投影 = 100 + 0.5*20 = 110，而 swing 低点 108 < 110 → 凸出线下，违规
        // 包络线应该是过 idx20→idx40（斜率 0.6），idx0 对应投影 = 108-20*0.6 = 96，100 > 96 ✓
        // 或者过 idx0→idx20（斜率 0.4），idx40 对应投影 = 100+40*0.4 = 116，120 > 116 ✓
        // 任一包络线都比"穿点"的版本好
        let slope = support.slope_per_bar;
        // 在 idx20 的投影
        let proj_at_20 = support.p1_price + slope * (20.0 - support.p1_index as f64);
        // idx20 的 swing 低点应 >= 投影（支撑线语义）
        assert!(
            108.0 >= proj_at_20 * (1.0 - ENVELOPE_SOFT_TOLERANCE),
            "包络线：idx20 的低点 (108) 应在支撑线 (投影 {:.2}) 之上或等高",
            proj_at_20
        );
    }

    #[test]
    fn t_effective_break_tolerance_tightened_to_2pct() {
        // 回归：确认常量已收紧至 2%
        assert!(
            (EFFECTIVE_BREAK_TOLERANCE - 0.02).abs() < 1e-9,
            "EFFECTIVE_BREAK_TOLERANCE 应为 2%"
        );
    }

    #[test]
    fn t_check_effective_break_3pct_threshold() {
        // 原书 trend p.203 铁证：未达 3% 价差，趋势线继续有效
        let line = TrendLine {
            kind: TrendLineKind::Support,
            p1_index: 0,
            p1_time: 0,
            p1_price: 100.0,
            p2_index: 10,
            p2_time: 0,
            p2_price: 100.0,
            slope_per_bar: 0.0,
            touches: 2,
            avg_deviation: 0.0,
            coordinate: CoordinateSystem::Linear,
        };
        // 未达 3%：返回 None
        assert_eq!(line.check_effective_break(102.0, 5), Option::None);
        assert_eq!(line.check_effective_break(98.0, 5), Option::None);
        // 超过 3%：返回 Some(方向)
        assert_eq!(line.check_effective_break(104.0, 5), Some(true));
        assert_eq!(line.check_effective_break(96.0, 5), Some(false));
    }

    // -------- E31 趋势线画法校验测试 --------

    fn mk_kline(idx: i64, open: f64, close: f64, high: f64, low: f64) -> crate::data::Kline {
        crate::data::Kline {
            open_time: idx * 86_400_000,
            close_time: (idx + 1) * 86_400_000 - 1,
            open,
            high,
            low,
            close,
            volume: 1.0,
        }
    }

    #[test]
    fn t_e31_support_line_valid_no_body_pierce() {
        // 支撑线：所有 K 线实体下沿都在趋势线之上 → 有效
        let line = TrendLine {
            kind: TrendLineKind::Support,
            p1_index: 0,
            p1_time: 0,
            p1_price: 100.0,
            p2_index: 10,
            p2_time: 10 * 86_400_000,
            p2_price: 110.0,
            slope_per_bar: 1.0,
            touches: 2,
            avg_deviation: 0.0,
            coordinate: CoordinateSystem::Linear,
        };
        // 中间 K 线实体全部高于趋势线投影价
        let klines: Vec<_> = (0..=10)
            .map(|i| {
                let proj = 100.0 + i as f64; // 趋势线投影价
                mk_kline(i as i64, proj + 1.0, proj + 2.0, proj + 3.0, proj + 0.5)
            })
            .collect();
        assert!(line.validate_no_body_pierce(&klines, 0.0));
    }

    #[test]
    fn t_e31_support_line_body_pierces_invalid() {
        // 支撑线：某根 K 线实体下沿低于趋势线 → 无效
        let line = TrendLine {
            kind: TrendLineKind::Support,
            p1_index: 0,
            p1_time: 0,
            p1_price: 100.0,
            p2_index: 10,
            p2_time: 10 * 86_400_000,
            p2_price: 110.0,
            slope_per_bar: 1.0,
            touches: 2,
            avg_deviation: 0.0,
            coordinate: CoordinateSystem::Linear,
        };
        let mut klines: Vec<_> = (0..=10)
            .map(|i| {
                let proj = 100.0 + i as f64;
                mk_kline(i as i64, proj + 1.0, proj + 2.0, proj + 3.0, proj + 0.5)
            })
            .collect();
        // 第 5 根 K 线实体下沿跌破趋势线（proj=105，body_low=90）
        klines[5] = mk_kline(5, 92.0, 90.0, 94.0, 89.0);
        assert!(!line.validate_no_body_pierce(&klines, 0.0));
    }

    #[test]
    fn t_e31_resistance_line_valid() {
        // 阻力线：所有实体上沿都在趋势线之下 → 有效
        let line = TrendLine {
            kind: TrendLineKind::Resistance,
            p1_index: 0,
            p1_time: 0,
            p1_price: 120.0,
            p2_index: 10,
            p2_time: 10 * 86_400_000,
            p2_price: 110.0,
            slope_per_bar: -1.0,
            touches: 2,
            avg_deviation: 0.0,
            coordinate: CoordinateSystem::Linear,
        };
        let klines: Vec<_> = (0..=10)
            .map(|i| {
                let proj = 120.0 - i as f64;
                mk_kline(i as i64, proj - 2.0, proj - 1.0, proj - 0.5, proj - 3.0)
            })
            .collect();
        assert!(line.validate_no_body_pierce(&klines, 0.0));
    }

    #[test]
    fn t_e31_allow_shadow_pierce() {
        // 原书明确允许影线穿过，仅实体不得穿过
        let line = TrendLine {
            kind: TrendLineKind::Support,
            p1_index: 0,
            p1_time: 0,
            p1_price: 100.0,
            p2_index: 10,
            p2_time: 10 * 86_400_000,
            p2_price: 100.0,
            slope_per_bar: 0.0,
            touches: 2,
            avg_deviation: 0.0,
            coordinate: CoordinateSystem::Linear,
        };
        // 第 5 根 K 线 low=95（影线穿过）但 open=102, close=101（实体在线上）
        let mut klines: Vec<_> = (0..=10)
            .map(|i| mk_kline(i as i64, 101.0, 102.0, 103.0, 100.5))
            .collect();
        klines[5] = mk_kline(5, 102.0, 101.0, 103.0, 95.0); // 影线 low=95，实体 100-102
        assert!(line.validate_no_body_pierce(&klines, 0.0));
    }
}
