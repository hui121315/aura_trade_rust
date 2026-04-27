//! 17 大均线特殊形态识别的正向验证测试。
//!
//! 每种特殊形态构造一段 K 线序列，验证 `scan_ma_special` 能在最后一根 K 线上命中。

use aura_trade::data::Kline;
use aura_trade::engine::ma::{
    alignment, compute, scan_ma_special, Alignment, MaKind, MaSpecialKind, SpecialParams,
};

// ========== 辅助构造 ==========

fn bar(idx: i64, o: f64, h: f64, l: f64, c: f64) -> Kline {
    let step = 3_600_000_i64;
    Kline {
        open_time: step * idx,
        close_time: step * (idx + 1) - 1,
        open: o,
        high: h,
        low: l,
        close: c,
        volume: 1.0,
    }
}

fn bar_at(idx: i64, price: f64) -> Kline {
    bar(idx, price, price + 0.02, price - 0.02, price)
}

/// 周期集合（覆盖短中长）—— 使用较短周期加快交叉出现，便于测试构造
/// 引擎用 ma_series[0..3] 作为 short/mid/long，ma_series[..] 作 Alignment
const PERIODS: &[usize] = &[3, 5, 10, 20];

/// 从 K 线算出所有需要的中间量并调用 scan_ma_special
fn analyze(klines: &[Kline], params: &SpecialParams) -> Vec<aura_trade::engine::ma::MaSpecialHit> {
    let closes: Vec<f64> = klines.iter().map(|k| k.close).collect();
    let ma_series: Vec<Vec<f64>> = PERIODS
        .iter()
        .map(|&p| compute::compute(MaKind::Sma, &closes, p))
        .collect();

    // Alignment at last bar
    let stack_refs: Vec<&[f64]> = ma_series.iter().map(|v| v.as_slice()).collect();
    let last = klines.len() - 1;
    let alignment = alignment::classify(&stack_refs, last, 0.005);

    // 基准 MA（取 10）的斜率序列
    let base_idx = PERIODS.iter().position(|&p| p == 10).unwrap();
    let slopes = compute::slope(&ma_series[base_idx], 5);

    // 交叉事件：相邻均线两两扫描
    let mut cross_bars: Vec<usize> = Vec::new();
    for i in 0..PERIODS.len() - 1 {
        for c in alignment::find_crosses(&ma_series[i], &ma_series[i + 1], PERIODS[i], PERIODS[i + 1]) {
            cross_bars.push(c.index);
        }
    }
    cross_bars.sort();
    cross_bars.dedup();

    scan_ma_special(
        &closes,
        &ma_series,
        PERIODS,
        alignment,
        &slopes,
        10,
        &cross_bars,
        last,
        params,
    )
}

fn assert_hit(klines: &[Kline], kind: MaSpecialKind, params: Option<&SpecialParams>) {
    let default = SpecialParams::default();
    let hits = analyze(klines, params.unwrap_or(&default));
    let found = hits.iter().any(|h| h.kind == kind);
    assert!(
        found,
        "期望命中 {:?} ({})，实际命中: {:?}",
        kind,
        kind.label(),
        hits.iter().map(|h| h.kind).collect::<Vec<_>>()
    );
}

/// 构造强势多头趋势序列（价格单调上升，带小幅噪声），长度 n
fn steady_uptrend(n: usize, start: f64, slope: f64) -> Vec<Kline> {
    (0..n)
        .map(|i| bar_at(i as i64, start + slope * i as f64))
        .collect()
}

fn steady_downtrend(n: usize, start: f64, slope: f64) -> Vec<Kline> {
    (0..n)
        .map(|i| bar_at(i as i64, start - slope * i as f64))
        .collect()
}

// ========== 排列形态 ==========

#[test]
fn t_bull_arrangement() {
    // 稳定上涨 200 根：MA5 > MA10 > MA20 > MA60
    let kl = steady_uptrend(200, 100.0, 0.5);
    assert_hit(&kl, MaSpecialKind::BullArrangement, None);
}

#[test]
fn t_bear_arrangement() {
    let kl = steady_downtrend(200, 200.0, 0.5);
    assert_hit(&kl, MaSpecialKind::BearArrangement, None);
}

// ========== 爬坡 / 加速 ==========

#[test]
fn t_uphill_climb() {
    // 温和上行：斜率平稳（不超过最近均值 ×2）
    let kl = steady_uptrend(200, 100.0, 0.3);
    assert_hit(&kl, MaSpecialKind::UphillClimb, None);
}

#[test]
fn t_downhill_slide() {
    let kl = steady_downtrend(200, 200.0, 0.3);
    assert_hit(&kl, MaSpecialKind::DownhillSlide, None);
}

#[test]
fn t_accelerating_up() {
    // 先慢后快：前 195 根温和上行，末端 5 根急速加速
    // 引擎用"最近 20 根斜率均值 × 2"作阈值；加速段只占 5 根，均值仍由平缓段主导
    let mut kl = Vec::new();
    for i in 0..195 {
        kl.push(bar_at(i as i64, 100.0 + 0.05 * i as f64));
    }
    let base = kl.last().unwrap().close;
    for i in 0..5 {
        kl.push(bar_at((195 + i) as i64, base + 3.0 * (i + 1) as f64));
    }
    assert_hit(&kl, MaSpecialKind::AcceleratingUp, None);
}

#[test]
fn t_accelerating_down() {
    let mut kl = Vec::new();
    for i in 0..195 {
        kl.push(bar_at(i as i64, 200.0 - 0.05 * i as f64));
    }
    let base = kl.last().unwrap().close;
    for i in 0..5 {
        kl.push(bar_at((195 + i) as i64, base - 3.0 * (i + 1) as f64));
    }
    assert_hit(&kl, MaSpecialKind::AcceleratingDown, None);
}

// ========== 快速 / 中枢 ==========

#[test]
fn t_rapid_up() {
    // MA3 相对 MA20 高出 > 8%：前 40 根横盘让 MA20 稳定，后 30 根急涨让 MA3 远超
    let mut kl = Vec::new();
    for i in 0..40 {
        kl.push(bar_at(i as i64, 100.0));
    }
    for i in 0..30 {
        kl.push(bar_at((40 + i) as i64, 100.0 + 2.5 * (i + 1) as f64));
    }
    assert_hit(&kl, MaSpecialKind::RapidUp, None);
}

#[test]
fn t_rapid_down() {
    let mut kl = Vec::new();
    for i in 0..40 {
        kl.push(bar_at(i as i64, 100.0));
    }
    for i in 0..30 {
        kl.push(bar_at((40 + i) as i64, 100.0 - 2.5 * (i + 1) as f64));
    }
    assert_hit(&kl, MaSpecialKind::RapidDown, None);
}

#[test]
fn t_wave_up() {
    // 在均线上下波动但中枢抬升：30 根内后半段 MA 均值 > 前半段 2%+
    // 构造：前 100 根横盘，后面 60 根锯齿但整体抬升
    let mut kl = Vec::new();
    for i in 0..100 {
        kl.push(bar_at(i as i64, 100.0));
    }
    // 锯齿上升：每 4 根一组，3 根涨 1 根跌
    for i in 0..80 {
        let phase = i % 4;
        let trend = 0.4 * i as f64;
        let zigzag = match phase {
            0 => 0.0,
            1 => 1.0,
            2 => -0.5,
            _ => 0.5,
        };
        kl.push(bar_at((100 + i) as i64, 100.0 + trend + zigzag));
    }
    assert_hit(&kl, MaSpecialKind::WaveUp, None);
}

#[test]
fn t_wave_down() {
    let mut kl = Vec::new();
    for i in 0..100 {
        kl.push(bar_at(i as i64, 100.0));
    }
    for i in 0..80 {
        let phase = i % 4;
        let trend = -0.4 * i as f64;
        let zigzag = match phase {
            0 => 0.0,
            1 => -1.0,
            2 => 0.5,
            _ => -0.5,
        };
        kl.push(bar_at((100 + i) as i64, 100.0 + trend + zigzag));
    }
    assert_hit(&kl, MaSpecialKind::WaveDown, None);
}

// ========== 粘合 / 分界 / 烂泥 ==========

#[test]
fn t_ma_bond() {
    // 长期横盘：所有均线价差极小
    let kl = steady_uptrend(200, 100.0, 0.001); // 微斜率让所有 MA 收敛在一起
    assert_hit(&kl, MaSpecialKind::MaBond, None);
}

#[test]
fn t_bull_bear_boundary() {
    // 价格紧贴 MA20：构造一段上升后趋于平缓的序列
    let mut kl = steady_uptrend(100, 100.0, 0.5);
    // 在末端让价格稳定在 MA20 附近（横盘一段）
    let last_price = kl.last().unwrap().close;
    for i in 0..50 {
        kl.push(bar_at((100 + i) as i64, last_price));
    }
    assert_hit(&kl, MaSpecialKind::BullBearBoundary, None);
}

#[test]
fn t_mire() {
    // 烂泥潭：近 20 根内至少 4 次均线交叉
    // 快速震荡：价格在窄区间反复穿越均线
    let mut kl = Vec::new();
    // 前 100 根 稳定让 MA 收敛
    for i in 0..100 {
        kl.push(bar_at(i as i64, 100.0));
    }
    // 后 40 根高频震荡：每 3 根一次极端摆动，制造快速穿越
    for i in 0..60 {
        let p = 100.0 + if i % 3 == 0 { 3.0 } else if i % 3 == 1 { -3.0 } else { 0.0 };
        kl.push(bar_at((100 + i) as i64, p));
    }
    assert_hit(&kl, MaSpecialKind::Mire, None);
}

// ========== 山谷 / 周期轮换 ==========

#[test]
fn t_cycle_swap() {
    // 周期轮换：最近 5 根内出现交叉 + 多头/空头排列
    // 反弹 8 根恰好让 MA10-MA20 最后完成交叉（在 last-5 窗口内）
    let mut kl = Vec::new();
    for i in 0..100 {
        kl.push(bar_at(i as i64, 200.0 - 1.0 * i as f64));
    }
    let bottom = kl.last().unwrap().close;
    for i in 0..8 {
        kl.push(bar_at((100 + i) as i64, bottom + 4.0 * (i + 1) as f64));
    }
    assert_hit(&kl, MaSpecialKind::CycleSwap, None);
}

#[test]
fn t_golden_valley() {
    // 金山谷：短>中>长 + Bullish + 最近 10 根内有交叉
    // 反弹 10 根：MA10-MA20 金叉刚完成，测试终点在交叉后 ≤10 根
    let mut kl = Vec::new();
    for i in 0..100 {
        kl.push(bar_at(i as i64, 200.0 - 1.0 * i as f64));
    }
    let bottom = kl.last().unwrap().close;
    for i in 0..10 {
        kl.push(bar_at((100 + i) as i64, bottom + 4.0 * (i + 1) as f64));
    }
    assert_hit(&kl, MaSpecialKind::GoldenValley, None);
}

#[test]
fn t_silver_valley() {
    // 银山谷：短(MA3)上穿中(MA5)，中(MA5)仍在长(MA10)之下，alignment=Mixed/Converging
    // 下跌 120 根 + 反弹 5 根（反弹坡度极小，MA3 勉强上穿 MA5 但 MA5 还低于 MA10）
    let mut kl = Vec::new();
    for i in 0..120 {
        kl.push(bar_at(i as i64, 200.0 - 0.8 * i as f64));
    }
    let bottom = kl.last().unwrap().close;
    for i in 0..5 {
        kl.push(bar_at((120 + i) as i64, bottom + 0.5 * (i + 1) as f64));
    }
    assert_hit(&kl, MaSpecialKind::SilverValley, None);
}

#[test]
fn t_death_valley() {
    // 死亡谷：短(MA3)下穿中(MA5)，中(MA5)仍压在长(MA10)之上，alignment=Mixed/Converging
    let mut kl = Vec::new();
    for i in 0..120 {
        kl.push(bar_at(i as i64, 100.0 + 0.8 * i as f64));
    }
    let top = kl.last().unwrap().close;
    for i in 0..5 {
        kl.push(bar_at((120 + i) as i64, top - 0.5 * (i + 1) as f64));
    }
    assert_hit(&kl, MaSpecialKind::DeathValley, None);
}

// ========== Patch 5 v2：原书追溯 / 强信号判定 ==========

#[test]
fn t_book_source_for_direct_kinds() {
    // 13 种原书直接对应形态都应返回 Some
    let direct = [
        MaSpecialKind::BullArrangement,
        MaSpecialKind::BearArrangement,
        MaSpecialKind::UphillClimb,
        MaSpecialKind::DownhillSlide,
        MaSpecialKind::WaveUp,
        MaSpecialKind::WaveDown,
        MaSpecialKind::MaBond,
        MaSpecialKind::AcceleratingUp,
        MaSpecialKind::AcceleratingDown,
        MaSpecialKind::SilverValley,
        MaSpecialKind::GoldenValley,
        MaSpecialKind::DeathValley,
    ];
    for k in direct {
        assert!(
            k.book_source().is_some(),
            "{:?} 应为原书直接对应形态（book_source 应返回 Some）",
            k
        );
        assert!(k.is_book_direct(), "{:?} is_book_direct 应为 true", k);
        let src = k.book_source().unwrap();
        assert!(
            src.starts_with("ma "),
            "{:?} 的 book_source 应以 'ma ' 开头，实际：{}",
            k,
            src
        );
    }
}

#[test]
fn t_book_source_for_aura_derived() {
    // 4 种 AURA 派生形态都应返回 None
    let derived = [
        MaSpecialKind::RapidUp,
        MaSpecialKind::RapidDown,
        MaSpecialKind::Mire,
        MaSpecialKind::BullBearBoundary,
        MaSpecialKind::CycleSwap,
    ];
    for k in derived {
        assert!(
            k.book_source().is_none(),
            "{:?} 应为 AURA 派生形态（book_source 应返回 None）",
            k
        );
        assert!(!k.is_book_direct(), "{:?} is_book_direct 应为 false", k);
    }
}

#[test]
fn t_severe_signal_classification() {
    // 原书强调的 6 种强信号
    let severe = [
        MaSpecialKind::BullArrangement,
        MaSpecialKind::BearArrangement,
        MaSpecialKind::GoldenValley,
        MaSpecialKind::DeathValley,
        MaSpecialKind::AcceleratingUp,
        MaSpecialKind::AcceleratingDown,
    ];
    for k in severe {
        assert!(k.severe_signal(), "{:?} 应为强信号（severe_signal 应返回 true）", k);
    }
    // 这些不应为强信号
    let non_severe = [
        MaSpecialKind::UphillClimb,
        MaSpecialKind::DownhillSlide,
        MaSpecialKind::WaveUp,
        MaSpecialKind::WaveDown,
        MaSpecialKind::SilverValley, // 银山谷弱于金山谷
        MaSpecialKind::MaBond,
        MaSpecialKind::Mire,
        MaSpecialKind::BullBearBoundary,
        MaSpecialKind::CycleSwap,
        MaSpecialKind::RapidUp,
        MaSpecialKind::RapidDown,
    ];
    for k in non_severe {
        assert!(!k.severe_signal(), "{:?} 不应为强信号", k);
    }
}

#[test]
fn t_weight_recalibration_v2() {
    // Patch 5 v2 权重校准断言（基于原书重要性）

    // 5 级：原书最重要的 4 种
    assert_eq!(MaSpecialKind::BullArrangement.weight(), 5);
    assert_eq!(MaSpecialKind::BearArrangement.weight(), 5);
    assert_eq!(MaSpecialKind::GoldenValley.weight(), 5);
    assert_eq!(MaSpecialKind::DeathValley.weight(), 5);

    // 4 级：原书重要但弱于 5 级
    assert_eq!(MaSpecialKind::AcceleratingUp.weight(), 4);
    assert_eq!(MaSpecialKind::AcceleratingDown.weight(), 4);
    assert_eq!(MaSpecialKind::UphillClimb.weight(), 4); // v2 提升（v1=3）
    assert_eq!(MaSpecialKind::DownhillSlide.weight(), 4); // v2 提升（v1=3）
    assert_eq!(MaSpecialKind::SilverValley.weight(), 4);

    // 3 级：标准趋势确认
    assert_eq!(MaSpecialKind::WaveUp.weight(), 3);
    assert_eq!(MaSpecialKind::WaveDown.weight(), 3);
    assert_eq!(MaSpecialKind::MaBond.weight(), 3);
    assert_eq!(MaSpecialKind::RapidUp.weight(), 3); // v2 降低（v1=4）
    assert_eq!(MaSpecialKind::RapidDown.weight(), 3); // v2 降低（v1=4）

    // 2 级：辅助状态
    assert_eq!(MaSpecialKind::BullBearBoundary.weight(), 2);
    assert_eq!(MaSpecialKind::CycleSwap.weight(), 2); // v2 降低（v1=3）

    // 1 级：噪声
    assert_eq!(MaSpecialKind::Mire.weight(), 1); // v2 降低（v1=2）
}

#[test]
fn t_severe_signals_have_high_weight() {
    // 不变性：所有强信号必有 weight ≥ 4
    for kind in [
        MaSpecialKind::BullArrangement,
        MaSpecialKind::BearArrangement,
        MaSpecialKind::GoldenValley,
        MaSpecialKind::DeathValley,
        MaSpecialKind::AcceleratingUp,
        MaSpecialKind::AcceleratingDown,
    ] {
        assert!(
            kind.severe_signal(),
            "{:?} severe_signal 应为 true",
            kind
        );
        assert!(
            kind.weight() >= 4,
            "{:?} 强信号权重应 ≥4，实际：{}",
            kind,
            kind.weight()
        );
    }
}
