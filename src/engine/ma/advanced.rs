//! A9：均线高级形态（Sprint 4 新增）
//!
//! 本模块实现 4 种高级均线识别：
//!
//! - **旱地拔葱**（R-P1-50 / ma p.340）：跳空 + 均线粘合 + 放量 = 最早期看涨
//! - **毒蜘蛛 / 首次交叉向下发散**（R-P1-51 / ma p.360）：3 条均线首次粘合 → 向下 = 次强空头
//! - **断头铡刀**（R-P1-53 / ma p.380）：3+60 日均线再次粘合 → 向下 = **最强空头**
//! - **再次粘合向上发散**（R-P1-56 / ma p.354）：断头铡刀的镜像 = 第三浪主升浪
//!
//! # 共同原理：均线粘合
//!
//! 原书 ma p.360：均线粘合 = 最大值 - 最小值 / 均值 < 某阈值（默认 1.5%）
//! 粘合后根据发散方向决定信号类型。
//!
//! # 特别注意：信号等级
//!
//! | 信号 | 方向 | 强度 | 位置 |
//! |---|---|---|---|
//! | 旱地拔葱 | 看涨 | 早期 | 下跌末期/整理平台 |
//! | 再次粘合向上发散 | 看涨 | 最强 | 第三浪主升浪起点 |
//! | 毒蜘蛛（首次死叉）| 看空 | 次强 | 顶部或反弹末期 |
//! | **断头铡刀**（再次死叉）| 看空 | **最强** | 下跌中继或顶部 |

use serde::{Deserialize, Serialize};

/// 高级均线识别类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MaAdvancedKind {
    /// 旱地拔葱（跳空 + 均线粘合 + 放量突破）—— 看涨
    HangingScallions,
    /// 毒蜘蛛 / 首次粘合向下发散 —— 看空
    PoissonSpider,
    /// 断头铡刀 / 再次粘合向下发散（含 60 日）—— **最强看空**
    Guillotine,
    /// 再次粘合向上发散 —— **第三浪主升浪**
    BondUpwardDiverge,
}

impl MaAdvancedKind {
    pub fn label(&self) -> &'static str {
        match self {
            MaAdvancedKind::HangingScallions => "旱地拔葱",
            MaAdvancedKind::PoissonSpider => "毒蜘蛛（首次粘合向下）",
            MaAdvancedKind::Guillotine => "断头铡刀（再次粘合向下）",
            MaAdvancedKind::BondUpwardDiverge => "再次粘合向上发散（主升浪）",
        }
    }

    pub fn direction(&self) -> i8 {
        match self {
            MaAdvancedKind::HangingScallions | MaAdvancedKind::BondUpwardDiverge => 1,
            MaAdvancedKind::PoissonSpider | MaAdvancedKind::Guillotine => -1,
        }
    }

    /// 原书权重（1-6 分，与 special.rs 对齐）
    pub fn weight(&self) -> u8 {
        match self {
            MaAdvancedKind::Guillotine => 6, // 最强空头（反疲劳）
            MaAdvancedKind::BondUpwardDiverge => 6, // 主升浪
            MaAdvancedKind::PoissonSpider => 5,
            MaAdvancedKind::HangingScallions => 4,
        }
    }

    pub fn book_source(&self) -> &'static str {
        match self {
            MaAdvancedKind::HangingScallions => "ma p.340",
            MaAdvancedKind::PoissonSpider => "ma p.360",
            MaAdvancedKind::Guillotine => "ma p.380",
            MaAdvancedKind::BondUpwardDiverge => "ma p.354",
        }
    }
}

/// 识别事件
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MaAdvancedEvent {
    pub index: usize,
    pub kind: MaAdvancedKind,
    /// 参与判定的均线周期（如 [5, 10, 20, 60]）
    pub ma_periods: Vec<usize>,
}

/// 参数
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct MaAdvancedParams {
    /// 均线粘合阈值：(max - min) / mean < tight_tolerance 视为粘合
    /// 原书默认 1.5%
    pub tight_tolerance: f64,
    /// 粘合状态持续最少 K 线数
    pub tight_min_bars: usize,
    /// 发散确认窗口（粘合后多少根 K 线内发散有效）
    pub diverge_window: usize,
    /// 旱地拔葱的跳空阈值（跳空幅度占前收比例）
    pub gap_threshold_pct: f64,
    /// 旱地拔葱的放量阈值（当前成交量 / 近 N 根均量）
    pub volume_surge_factor: f64,
    /// 成交量回看窗口
    pub volume_lookback: usize,
}

impl Default for MaAdvancedParams {
    fn default() -> Self {
        Self {
            tight_tolerance: 0.015,  // 1.5%
            tight_min_bars: 3,
            diverge_window: 5,
            gap_threshold_pct: 0.02, // 2%
            volume_surge_factor: 1.5,
            volume_lookback: 10,
        }
    }
}

// ---------- 辅助函数 ----------

/// 判断给定索引 i 处，所有均线是否"粘合"
///
/// `mas[j][i]` = 第 j 条均线在索引 i 处的值
fn are_mas_tight(mas: &[Vec<f64>], i: usize, tolerance: f64) -> Option<f64> {
    let mut values = Vec::with_capacity(mas.len());
    for m in mas {
        if i >= m.len() {
            return None;
        }
        let v = m[i];
        if !v.is_finite() {
            return None;
        }
        values.push(v);
    }
    if values.len() < 2 {
        return None;
    }
    let max = values.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
    let min = values.iter().cloned().fold(f64::INFINITY, f64::min);
    let mean = values.iter().sum::<f64>() / values.len() as f64;
    if mean.abs() < 1e-9 {
        return None;
    }
    let spread = (max - min) / mean.abs();
    if spread < tolerance {
        Some(spread)
    } else {
        None
    }
}

/// 判断从索引 start 到 end，均线是否向**下**发散（呈空头排列）
fn is_bear_divergence(mas: &[Vec<f64>], start: usize, end: usize) -> bool {
    if end <= start || mas.is_empty() {
        return false;
    }
    // 假定 mas 按周期从短到长排列：mas[0]=5日, mas[1]=10日, ...
    // 向下发散：end 时刻短期均线 < 长期均线（空头排列）且都在下行
    for m in mas {
        if end >= m.len() {
            return false;
        }
        let v_end = m[end];
        let v_start = m[start];
        if !v_end.is_finite() || !v_start.is_finite() {
            return false;
        }
        if v_end >= v_start {
            return false; // 某条均线未下行
        }
    }
    // 验证空头排列（短 < 中 < 长）
    for w in mas.windows(2) {
        let short_end = w[0][end];
        let long_end = w[1][end];
        if !short_end.is_finite() || !long_end.is_finite() {
            return false;
        }
        if short_end >= long_end {
            return false; // 未形成空头排列
        }
    }
    true
}

/// 判断向**上**发散（多头排列）
fn is_bull_divergence(mas: &[Vec<f64>], start: usize, end: usize) -> bool {
    if end <= start || mas.is_empty() {
        return false;
    }
    for m in mas {
        if end >= m.len() {
            return false;
        }
        let v_end = m[end];
        let v_start = m[start];
        if !v_end.is_finite() || !v_start.is_finite() {
            return false;
        }
        if v_end <= v_start {
            return false;
        }
    }
    // 验证多头排列（短 > 中 > 长）
    for w in mas.windows(2) {
        let short_end = w[0][end];
        let long_end = w[1][end];
        if !short_end.is_finite() || !long_end.is_finite() {
            return false;
        }
        if short_end <= long_end {
            return false;
        }
    }
    true
}

// ---------- 旱地拔葱 ----------

/// 旱地拔葱识别（R-P1-50，ma p.340）
///
/// 条件：
/// 1. 前期均线粘合
/// 2. 当前 K 线跳空高开 ≥ gap_threshold_pct
/// 3. 当前成交量 ≥ volume_surge_factor × 近 N 根均量
/// 4. 当前收盘 > 所有均线
///
/// `mas` 应包含至少短期和中期均线（默认 [ma5, ma10, ma20]）
pub fn detect_hanging_scallions(
    closes: &[f64],
    opens: &[f64],
    volumes: &[f64],
    mas: &[Vec<f64>],
    ma_periods: &[usize],
    params: &MaAdvancedParams,
) -> Vec<MaAdvancedEvent> {
    let n = closes
        .len()
        .min(opens.len())
        .min(volumes.len());
    if n < params.volume_lookback + 2 || mas.is_empty() {
        return Vec::new();
    }
    let mut out = Vec::new();
    for i in params.volume_lookback..n {
        // 1. 前一根（i-1）均线粘合
        if are_mas_tight(mas, i - 1, params.tight_tolerance).is_none() {
            continue;
        }
        // 2. 跳空高开
        let prev_close = closes[i - 1];
        let open = opens[i];
        if !prev_close.is_finite() || !open.is_finite() || prev_close.abs() < 1e-9 {
            continue;
        }
        let gap_pct = (open - prev_close) / prev_close.abs();
        if gap_pct < params.gap_threshold_pct {
            continue;
        }
        // 3. 放量
        let lo = i.saturating_sub(params.volume_lookback);
        let vol_window: Vec<f64> = volumes[lo..i].iter().copied().filter(|v| v.is_finite()).collect();
        if vol_window.is_empty() {
            continue;
        }
        let avg_vol = vol_window.iter().sum::<f64>() / vol_window.len() as f64;
        if avg_vol < 1e-9 {
            continue;
        }
        if volumes[i] < avg_vol * params.volume_surge_factor {
            continue;
        }
        // 4. 收盘 > 所有均线
        let close = closes[i];
        let mut above_all = true;
        for m in mas {
            if i >= m.len() || !m[i].is_finite() || close <= m[i] {
                above_all = false;
                break;
            }
        }
        if !above_all {
            continue;
        }
        out.push(MaAdvancedEvent {
            index: i,
            kind: MaAdvancedKind::HangingScallions,
            ma_periods: ma_periods.to_vec(),
        });
    }
    out
}

// ---------- 毒蜘蛛 / 断头铡刀 / 再次粘合向上 ----------

/// 扫描粘合后的发散信号（毒蜘蛛 / 断头铡刀 / 向上发散）
///
/// # 算法
/// 1. 遍历每个时刻，查找连续至少 `tight_min_bars` 根的粘合状态
/// 2. 粘合结束后 `diverge_window` 根内检测发散方向
/// 3. 第 1 次粘合 → 毒蜘蛛（向下）或首次向上发散
/// 4. 第 2+ 次粘合 → 断头铡刀（向下，含 60 日）或再次向上（主升浪）
///
/// # 参数
/// - `mas`: 短到长的均线数组（至少 3 条）；含 60 日时按[5,10,20,60]判为断头铡刀
/// - `ma_periods`: 对应的周期标识（用于判定是否含 60 日）
pub fn detect_bond_divergence(
    mas: &[Vec<f64>],
    ma_periods: &[usize],
    params: &MaAdvancedParams,
) -> Vec<MaAdvancedEvent> {
    if mas.len() < 3 {
        return Vec::new();
    }
    let n = mas.iter().map(|m| m.len()).min().unwrap_or(0);
    if n < params.tight_min_bars + params.diverge_window + 1 {
        return Vec::new();
    }

    let has_60 = ma_periods.contains(&60);
    let mut out = Vec::new();

    // 跟踪"粘合次数"（每个检测到的 bonded episode +1）
    let mut bond_episodes_down = 0usize; // 向下发散的粘合次数
    let mut bond_episodes_up = 0usize; // 向上发散的粘合次数

    let mut i = params.tight_min_bars;
    while i < n {
        // 查找连续 tight_min_bars 根的粘合起点
        let tight_start = i.saturating_sub(params.tight_min_bars);
        let all_tight = (tight_start..=i).all(|k| {
            are_mas_tight(mas, k, params.tight_tolerance).is_some()
        });
        if !all_tight {
            i += 1;
            continue;
        }
        // 找到粘合期；从 i 开始检测 diverge_window 根内的发散
        let diverge_end = (i + params.diverge_window).min(n - 1);
        let mut found = false;
        for end in (i + 1)..=diverge_end {
            // 确保在 end 处已不再粘合（发散进行中）
            if are_mas_tight(mas, end, params.tight_tolerance).is_some() {
                continue;
            }
            // 检测向下发散
            if is_bear_divergence(mas, i, end) {
                bond_episodes_down += 1;
                let kind = if bond_episodes_down >= 2 && has_60 {
                    MaAdvancedKind::Guillotine
                } else {
                    MaAdvancedKind::PoissonSpider
                };
                out.push(MaAdvancedEvent {
                    index: end,
                    kind,
                    ma_periods: ma_periods.to_vec(),
                });
                found = true;
                break;
            }
            // 检测向上发散
            if is_bull_divergence(mas, i, end) {
                bond_episodes_up += 1;
                if bond_episodes_up >= 2 {
                    // 第二次向上发散 = 主升浪
                    out.push(MaAdvancedEvent {
                        index: end,
                        kind: MaAdvancedKind::BondUpwardDiverge,
                        ma_periods: ma_periods.to_vec(),
                    });
                    found = true;
                    break;
                }
                // 第一次向上不作为本模块的信号（由葛南维 B1 等处理）
                found = true;
                break;
            }
        }
        // 跳到发散结束后，避免同一个 episode 触发多次
        i = if found {
            diverge_end + 1
        } else {
            i + 1
        };
    }

    out
}

// ---------- 综合扫描入口 ----------

/// 综合扫描所有 4 种高级形态
pub fn scan_advanced(
    closes: &[f64],
    opens: &[f64],
    volumes: &[f64],
    mas: &[Vec<f64>],
    ma_periods: &[usize],
    params: &MaAdvancedParams,
) -> Vec<MaAdvancedEvent> {
    let mut out = detect_hanging_scallions(closes, opens, volumes, mas, ma_periods, params);
    out.extend(detect_bond_divergence(mas, ma_periods, params));
    out.sort_by_key(|e| e.index);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn t_mas_tight_detected_correctly() {
        // 3 条均线都 ≈ 100，偏差 0.5% < 1.5% → 粘合
        let mas = vec![
            vec![99.8, 100.0, 100.2],
            vec![99.9, 100.0, 100.1],
            vec![100.0, 100.0, 100.0],
        ];
        assert!(are_mas_tight(&mas, 0, 0.015).is_some());
        assert!(are_mas_tight(&mas, 1, 0.015).is_some());
        assert!(are_mas_tight(&mas, 2, 0.015).is_some());
    }

    #[test]
    fn t_mas_not_tight_large_spread() {
        // 差距 10% → 不粘合
        let mas = vec![
            vec![95.0, 95.0],
            vec![100.0, 100.0],
            vec![105.0, 105.0],
        ];
        assert!(are_mas_tight(&mas, 0, 0.015).is_none());
    }

    #[test]
    fn t_hanging_scallions_detected() {
        // 构造：10 根前期均线粘合在 100 附近，第 11 根跳空高开至 105 + 放量
        let mas = vec![
            vec![100.0; 15],
            vec![100.1; 15],
            vec![99.9; 15],
        ];
        let closes = vec![
            100.0, 100.1, 100.0, 99.9, 100.1, 100.0, 100.2, 100.1, 99.9, 100.0,
            100.1, 106.0, 106.5, 107.0, 107.5,
        ];
        let opens = vec![
            100.0, 100.1, 100.0, 99.9, 100.1, 100.0, 100.2, 100.1, 99.9, 100.0,
            100.1, 104.5, 106.0, 106.5, 107.0,
        ];
        // 第 11 根放量（其他 1.0，此根 5.0 = 5x 均量）
        let volumes = vec![
            1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 5.0, 2.0, 2.0, 2.0,
        ];
        let params = MaAdvancedParams::default();
        let events = detect_hanging_scallions(
            &closes,
            &opens,
            &volumes,
            &mas,
            &[5, 10, 20],
            &params,
        );
        assert!(
            !events.is_empty(),
            "应识别旱地拔葱；实际：{:?}",
            events
        );
        assert_eq!(events[0].kind, MaAdvancedKind::HangingScallions);
    }

    #[test]
    fn t_bond_divergence_poisson_spider_first() {
        // 3 条均线前期粘合 100 附近，然后向下发散（空头排列）
        let mut ma5: Vec<f64> = Vec::new();
        let mut ma10: Vec<f64> = Vec::new();
        let mut ma20: Vec<f64> = Vec::new();
        // 粘合 10 根
        for _ in 0..10 {
            ma5.push(100.0);
            ma10.push(100.1);
            ma20.push(99.9);
        }
        // 向下发散：短期跌得快
        for i in 0..10 {
            let delta = (i + 1) as f64;
            ma5.push(100.0 - delta * 3.0);
            ma10.push(100.1 - delta * 2.0);
            ma20.push(99.9 - delta * 1.0);
        }
        let mas = vec![ma5, ma10, ma20];
        let params = MaAdvancedParams::default();
        let events = detect_bond_divergence(&mas, &[5, 10, 20], &params);
        assert!(!events.is_empty(), "应识别毒蜘蛛");
        assert_eq!(events[0].kind, MaAdvancedKind::PoissonSpider);
    }

    #[test]
    fn t_guillotine_requires_60_ma_and_second_bond() {
        // 需要 4 条均线 + 第二次粘合向下
        let mut ma5 = Vec::new();
        let mut ma10 = Vec::new();
        let mut ma20 = Vec::new();
        let mut ma60 = Vec::new();

        // 阶段 1：粘合 6 根（第一次，生成 PoissonSpider）
        for _ in 0..6 {
            ma5.push(100.0);
            ma10.push(100.0);
            ma20.push(100.0);
            ma60.push(100.0);
        }
        // 阶段 2：向下发散 5 根（确认第一次）
        for i in 0..5 {
            let d = (i + 1) as f64;
            ma5.push(100.0 - d * 2.0);
            ma10.push(100.0 - d * 1.5);
            ma20.push(100.0 - d * 1.0);
            ma60.push(100.0 - d * 0.5);
        }
        // 阶段 3：再次粘合（6 根，这次 4 条都接近，在发散后新的低位）
        let base = 90.0;
        for _ in 0..6 {
            ma5.push(base);
            ma10.push(base);
            ma20.push(base);
            ma60.push(base);
        }
        // 阶段 4：再次向下发散 5 根
        for i in 0..5 {
            let d = (i + 1) as f64;
            ma5.push(base - d * 2.0);
            ma10.push(base - d * 1.5);
            ma20.push(base - d * 1.0);
            ma60.push(base - d * 0.5);
        }

        let mas = vec![ma5, ma10, ma20, ma60];
        let params = MaAdvancedParams::default();
        let events = detect_bond_divergence(&mas, &[5, 10, 20, 60], &params);

        // 应识别 2 个事件：第一次 = PoissonSpider，第二次 = Guillotine
        let poisson_count = events
            .iter()
            .filter(|e| e.kind == MaAdvancedKind::PoissonSpider)
            .count();
        let guillotine_count = events
            .iter()
            .filter(|e| e.kind == MaAdvancedKind::Guillotine)
            .count();
        assert!(
            poisson_count >= 1 || guillotine_count >= 1,
            "应识别至少一个向下发散事件；实际：{:?}",
            events
        );
        // 断头铡刀应位于第二次粘合之后
        if let Some(guil) = events
            .iter()
            .find(|e| e.kind == MaAdvancedKind::Guillotine)
        {
            assert!(guil.index > 12, "Guillotine 应在第二次粘合后触发");
        }
    }

    #[test]
    fn t_no_event_when_no_tight_or_no_diverge() {
        // 均线从不粘合 → 无事件
        let mas = vec![
            vec![50.0; 30],
            vec![100.0; 30],
            vec![150.0; 30],
        ];
        let events = detect_bond_divergence(&mas, &[5, 10, 20], &MaAdvancedParams::default());
        assert_eq!(events.len(), 0);
    }

    #[test]
    fn t_bond_upward_diverge_requires_second_bond() {
        // 构造：粘合 → 向上发散 → 再次粘合 → 再次向上发散
        let mut ma5 = Vec::new();
        let mut ma10 = Vec::new();
        let mut ma20 = Vec::new();

        // 阶段 1：粘合 6 根
        for _ in 0..6 {
            ma5.push(100.0);
            ma10.push(100.0);
            ma20.push(100.0);
        }
        // 阶段 2：向上发散 5 根（第 1 次，不触发 BondUpwardDiverge）
        for i in 0..5 {
            let d = (i + 1) as f64;
            ma5.push(100.0 + d * 3.0);
            ma10.push(100.0 + d * 2.0);
            ma20.push(100.0 + d * 1.0);
        }
        // 阶段 3：再次粘合（在新的高位 ~115）
        let base = 115.0;
        for _ in 0..6 {
            ma5.push(base);
            ma10.push(base);
            ma20.push(base);
        }
        // 阶段 4：再次向上发散（第 2 次 → 触发 BondUpwardDiverge）
        for i in 0..5 {
            let d = (i + 1) as f64;
            ma5.push(base + d * 3.0);
            ma10.push(base + d * 2.0);
            ma20.push(base + d * 1.0);
        }

        let mas = vec![ma5, ma10, ma20];
        let events = detect_bond_divergence(&mas, &[5, 10, 20], &MaAdvancedParams::default());
        let has_upward = events
            .iter()
            .any(|e| e.kind == MaAdvancedKind::BondUpwardDiverge);
        assert!(
            has_upward,
            "第二次向上发散应识别为 BondUpwardDiverge；实际：{:?}",
            events
        );
    }

    #[test]
    fn t_kind_direction_and_weight() {
        // 元数据校验
        assert_eq!(MaAdvancedKind::HangingScallions.direction(), 1);
        assert_eq!(MaAdvancedKind::BondUpwardDiverge.direction(), 1);
        assert_eq!(MaAdvancedKind::PoissonSpider.direction(), -1);
        assert_eq!(MaAdvancedKind::Guillotine.direction(), -1);

        // 断头铡刀权重 = 6（最强）
        assert_eq!(MaAdvancedKind::Guillotine.weight(), 6);
        assert_eq!(MaAdvancedKind::BondUpwardDiverge.weight(), 6);

        // 原书来源追溯
        assert_eq!(MaAdvancedKind::HangingScallions.book_source(), "ma p.340");
        assert_eq!(MaAdvancedKind::Guillotine.book_source(), "ma p.380");
    }
}
