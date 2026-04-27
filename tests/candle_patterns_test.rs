//! K 线形态识别的正向验证测试。
//!
//! 每个形态至少构造一个**典型的"应该命中"**的 K 线序列，
//! 验证 `candle::scan` 确实能在期望位置报出期望 kind。
//!
//! 设计约束：
//! - 形态所在位置：序列最后一根（便于统一断言）
//! - 前缀 K 线用来提供"趋势语境"（trend_context），以免因为前面不够长而漏判锤头/吊颈等需要语境的形态
//! - 所有构造严格符合 PRD 定义，若识别器有误则测试会失败

use aura_trade::data::Kline;
use aura_trade::engine::candle::{scan, PatternKind};

// ========== 辅助构造 ==========

/// 构造一根 K 线，时间戳按索引计
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

/// 构造一段带下跌语境的前缀（让 trend_context 返回 -1）。返回 12 根
fn downtrend_prefix() -> Vec<Kline> {
    let mut v = Vec::new();
    for i in 0..12 {
        let p = 120.0 - (i as f64) * 1.0;
        v.push(bar(i, p, p + 0.3, p - 0.3, p - 0.3));
    }
    v
}

/// 上涨语境前缀，12 根
fn uptrend_prefix() -> Vec<Kline> {
    let mut v = Vec::new();
    for i in 0..12 {
        let p = 80.0 + (i as f64) * 1.0;
        v.push(bar(i, p, p + 0.3, p - 0.3, p + 0.3));
    }
    v
}

/// 横盘语境
fn flat_prefix() -> Vec<Kline> {
    let mut v = Vec::new();
    for i in 0..12 {
        let p = 100.0 + ((i as i64) % 2) as f64 * 0.1;
        v.push(bar(i, p, p + 0.2, p - 0.2, p));
    }
    v
}

/// 断言：klines 的 scan 结果中，**最后一根** 有 kind 命中
fn assert_hit_at_last(kl: &[Kline], kind: PatternKind) {
    let hits = scan(kl);
    let last = kl.len() - 1;
    let found = hits.iter().any(|h| h.index == last && h.kind == kind);
    assert!(
        found,
        "期望在最后一根命中 {:?}\n所有命中={:?}",
        kind,
        hits.iter().filter(|h| h.index == last).map(|h| h.kind).collect::<Vec<_>>()
    );
}

/// 断言：kind 在序列中的任一位置命中
fn assert_hit_any(kl: &[Kline], kind: PatternKind) {
    let hits = scan(kl);
    let found = hits.iter().any(|h| h.kind == kind);
    assert!(
        found,
        "期望任一位置命中 {:?}，实际命中 kinds: {:?}",
        kind,
        hits.iter().map(|h| h.kind).collect::<Vec<_>>()
    );
}

// ========== 单根形态 ==========

#[test]
fn t_big_bull_candle() {
    // 大阳线：实体 > 4% 且为阳线
    let mut kl = flat_prefix();
    kl.push(bar(12, 100.0, 107.0, 99.5, 106.5)); // 实体 6.5%
    assert_hit_at_last(&kl, PatternKind::BigBullCandle);
}

#[test]
fn t_big_bear_candle() {
    let mut kl = flat_prefix();
    kl.push(bar(12, 106.0, 106.5, 99.0, 99.5)); // 实体 -6.1%
    assert_hit_at_last(&kl, PatternKind::BigBearCandle);
}

#[test]
fn t_doji_star() {
    let mut kl = flat_prefix();
    kl.push(bar(12, 100.0, 100.8, 99.2, 100.02)); // 实体极小
    assert_hit_at_last(&kl, PatternKind::DojiStar);
}

#[test]
fn t_long_doji() {
    let mut kl = flat_prefix();
    // 实体极小 + 上下影都很长
    kl.push(bar(12, 100.0, 102.0, 98.0, 100.02));
    assert_hit_at_last(&kl, PatternKind::LongDoji);
}

#[test]
fn t_spinning_top() {
    let mut kl = flat_prefix();
    // 小实体 + 上下长影
    kl.push(bar(12, 100.0, 102.0, 98.0, 100.5));
    assert_hit_at_last(&kl, PatternKind::SpinningTop);
}

#[test]
fn t_flat_line() {
    let mut kl = flat_prefix();
    kl.push(bar(12, 100.0, 100.0, 100.0, 100.0)); // 四价合一
    assert_hit_at_last(&kl, PatternKind::FlatLine);
}

#[test]
fn t_t_shape() {
    let mut kl = flat_prefix();
    // 十字 + 长下影、几乎无上影
    kl.push(bar(12, 100.0, 100.05, 98.0, 100.0));
    assert_hit_at_last(&kl, PatternKind::TShape);
}

#[test]
fn t_inv_t_shape() {
    let mut kl = flat_prefix();
    // 十字 + 长上影、几乎无下影
    kl.push(bar(12, 100.0, 102.0, 99.95, 100.0));
    assert_hit_at_last(&kl, PatternKind::InvTShape);
}

#[test]
fn t_hammer_in_downtrend() {
    // 锤头 = LongLower 类：body_ratio 在 5%~35%，lower_ratio > 55%
    // range=4, body=1(阳线), upper=0.3, lower=2.7 → body_ratio=0.25, lower_ratio=0.675
    let mut kl = downtrend_prefix();
    let p = kl.last().unwrap().close;
    kl.push(bar(12, p - 1.0, p + 0.1, p - 3.7, p - 0.2));
    assert_hit_at_last(&kl, PatternKind::Hammer);
}

#[test]
fn t_hanging_man_in_uptrend() {
    let mut kl = uptrend_prefix();
    let p = kl.last().unwrap().close;
    kl.push(bar(12, p - 1.0, p + 0.1, p - 3.7, p - 0.2));
    assert_hit_at_last(&kl, PatternKind::HangingMan);
}

#[test]
fn t_inverted_hammer_in_downtrend() {
    // 倒锤 = LongUpper 类：body_ratio 5%~35%，upper_ratio > 55%
    let mut kl = downtrend_prefix();
    let p = kl.last().unwrap().close;
    kl.push(bar(12, p - 0.2, p + 2.8, p - 0.3, p + 0.8));
    assert_hit_at_last(&kl, PatternKind::InvertedHammer);
}

#[test]
fn t_shooting_star_in_uptrend() {
    let mut kl = uptrend_prefix();
    let p = kl.last().unwrap().close;
    kl.push(bar(12, p + 0.8, p + 3.8, p + 0.7, p - 0.2));
    assert_hit_at_last(&kl, PatternKind::ShootingStar);
}

#[test]
fn t_marubozu_bull() {
    let mut kl = flat_prefix();
    // 实体占比 > 90%，阳线
    kl.push(bar(12, 100.0, 106.0, 100.0, 106.0));
    assert_hit_at_last(&kl, PatternKind::MarubozuBull);
}

#[test]
fn t_marubozu_bear() {
    let mut kl = flat_prefix();
    kl.push(bar(12, 106.0, 106.0, 100.0, 100.0));
    assert_hit_at_last(&kl, PatternKind::MarubozuBear);
}

#[test]
fn t_gravestone_doji_in_uptrend() {
    let mut kl = uptrend_prefix();
    let p = kl.last().unwrap().close;
    // 十字 + 极长上影 + 几乎无下影 + 上涨语境
    kl.push(bar(12, p, p + 3.0, p - 0.01, p));
    assert_hit_at_last(&kl, PatternKind::GravestoneDoji);
}

#[test]
fn t_dragonfly_doji_in_downtrend() {
    let mut kl = downtrend_prefix();
    let p = kl.last().unwrap().close;
    kl.push(bar(12, p, p + 0.01, p - 3.0, p));
    assert_hit_at_last(&kl, PatternKind::DragonflyDoji);
}

#[test]
fn t_open_marubozu_bull() {
    let mut kl = flat_prefix();
    // 阳线 + 实体 > 60% + 无上影（开盘 == 高）+ 有下影
    kl.push(bar(12, 100.0, 104.0, 98.0, 104.0));
    assert_hit_at_last(&kl, PatternKind::OpenMarubozuBull);
}

#[test]
fn t_open_marubozu_bear() {
    let mut kl = flat_prefix();
    // 阴线 + 实体 > 60% + 无上影（开盘 == 高）
    kl.push(bar(12, 104.0, 104.0, 98.0, 100.0));
    assert_hit_at_last(&kl, PatternKind::OpenMarubozuBear);
}

#[test]
fn t_close_marubozu_bull() {
    let mut kl = flat_prefix();
    // 阳线 + 无下影（低 == 开盘），有上影
    kl.push(bar(12, 100.0, 106.0, 100.0, 104.0));
    assert_hit_at_last(&kl, PatternKind::CloseMarubozuBull);
}

#[test]
fn t_close_marubozu_bear() {
    let mut kl = flat_prefix();
    // 阴线 + 无下影（低 == 收盘），有上影
    kl.push(bar(12, 104.0, 106.0, 100.0, 100.0));
    assert_hit_at_last(&kl, PatternKind::CloseMarubozuBear);
}

// ========== 双根形态 ==========

#[test]
fn t_bullish_engulfing() {
    let mut kl = flat_prefix();
    kl.push(bar(12, 100.0, 100.3, 99.0, 99.5));       // 阴线
    kl.push(bar(13, 99.3, 101.5, 99.1, 101.0));       // 阳线，实体吞没前根
    assert_hit_at_last(&kl, PatternKind::BullishEngulfing);
}

#[test]
fn t_bearish_engulfing() {
    let mut kl = flat_prefix();
    kl.push(bar(12, 99.5, 100.3, 99.4, 100.0));       // 阳
    kl.push(bar(13, 100.2, 100.3, 98.5, 99.0));        // 阴吞没
    assert_hit_at_last(&kl, PatternKind::BearishEngulfing);
}

#[test]
fn t_bullish_harami() {
    let mut kl = flat_prefix();
    kl.push(bar(12, 106.0, 106.2, 99.5, 100.0));      // 大阴
    kl.push(bar(13, 101.5, 102.0, 101.0, 102.5));     // 小阳孕线，被 a 实体完全包含
    // 实际构造：小实体要在 a 实体 [100, 106] 之间
    let n = kl.len();
    kl[n - 1] = bar(13, 102.0, 103.0, 101.5, 102.5);
    assert_hit_at_last(&kl, PatternKind::BullishHarami);
}

#[test]
fn t_bearish_harami() {
    let mut kl = flat_prefix();
    kl.push(bar(12, 100.0, 106.5, 99.8, 106.0));      // 大阳
    kl.push(bar(13, 103.0, 104.0, 102.5, 102.8));     // 小阴孕线
    assert_hit_at_last(&kl, PatternKind::BearishHarami);
}

#[test]
fn t_piercing_line() {
    let mut kl = flat_prefix();
    kl.push(bar(12, 106.0, 106.2, 99.8, 100.0));                   // 大阴
    kl.push(bar(13, 99.0, 104.0, 98.5, 103.5));                    // 阳，开盘低于 a.close，收盘高于 a 中点 103，但低于 a.open=106
    assert_hit_at_last(&kl, PatternKind::PiercingLine);
}

#[test]
fn t_dark_cloud_cover() {
    let mut kl = flat_prefix();
    kl.push(bar(12, 100.0, 106.5, 99.8, 106.0));                   // 大阳
    kl.push(bar(13, 107.0, 107.2, 101.5, 102.0));                  // 阴，开盘高于 a.close，收盘低于 a 中点 103，但高于 a.open=100
    assert_hit_at_last(&kl, PatternKind::DarkCloudCover);
}

#[test]
fn t_tweezers_top() {
    let mut kl = uptrend_prefix();
    // 两根高点相同
    kl.push(bar(12, 100.0, 105.0, 99.5, 104.5));
    kl.push(bar(13, 104.0, 105.0, 102.0, 103.0));
    assert_hit_at_last(&kl, PatternKind::TweezersTop);
}

#[test]
fn t_tweezers_bottom() {
    let mut kl = downtrend_prefix();
    kl.push(bar(12, 105.0, 106.0, 100.0, 101.0));
    kl.push(bar(13, 101.5, 103.0, 100.0, 102.5));
    assert_hit_at_last(&kl, PatternKind::TweezersBottom);
}

#[test]
fn t_inside_bar() {
    let mut kl = flat_prefix();
    kl.push(bar(12, 100.0, 110.0, 90.0, 108.0));     // 大波幅
    kl.push(bar(13, 104.0, 106.0, 99.0, 105.0));     // 区间被 a 包裹
    assert_hit_at_last(&kl, PatternKind::InsideBar);
}

#[test]
fn t_outside_bar() {
    let mut kl = flat_prefix();
    kl.push(bar(12, 100.0, 104.0, 99.5, 103.0));
    kl.push(bar(13, 99.0, 105.0, 98.0, 104.5));      // 包裹 a
    assert_hit_at_last(&kl, PatternKind::OutsideBar);
}

#[test]
fn t_bullish_harami_cross() {
    let mut kl = downtrend_prefix();
    kl.push(bar(12, 110.0, 110.5, 100.0, 100.5));                // 大阴
    kl.push(bar(13, 105.0, 105.3, 104.7, 105.02));               // 十字，在 a 实体内
    assert_hit_at_last(&kl, PatternKind::BullishHaramiCross);
}

#[test]
fn t_bearish_harami_cross() {
    let mut kl = uptrend_prefix();
    kl.push(bar(12, 100.0, 110.5, 99.8, 110.0));                 // 大阳
    kl.push(bar(13, 105.0, 105.3, 104.7, 105.02));
    assert_hit_at_last(&kl, PatternKind::BearishHaramiCross);
}

#[test]
fn t_bullish_counter_attack() {
    let mut kl = downtrend_prefix();
    kl.push(bar(12, 108.0, 108.2, 100.0, 100.0));                // 大阴 close=100
    kl.push(bar(13, 96.0, 101.0, 95.5, 100.0));                  // 大阳 close=100 (= a.close)
    assert_hit_at_last(&kl, PatternKind::BullishCounterAttack);
}

#[test]
fn t_bearish_counter_attack() {
    let mut kl = uptrend_prefix();
    kl.push(bar(12, 100.0, 110.0, 99.8, 110.0));                 // 大阳 close=110
    kl.push(bar(13, 115.0, 116.0, 109.5, 110.0));                // 大阴 close=110
    assert_hit_at_last(&kl, PatternKind::BearishCounterAttack);
}

#[test]
fn t_matching_high_in_uptrend() {
    let mut kl = uptrend_prefix();
    kl.push(bar(12, 98.0, 106.0, 97.5, 105.0));
    kl.push(bar(13, 102.0, 107.0, 101.0, 105.0));     // 收盘同 a.close
    assert_hit_at_last(&kl, PatternKind::MatchingHigh);
}

#[test]
fn t_matching_low_in_downtrend() {
    let mut kl = downtrend_prefix();
    kl.push(bar(12, 108.0, 110.0, 99.0, 100.0));
    kl.push(bar(13, 102.0, 103.0, 98.5, 100.0));
    assert_hit_at_last(&kl, PatternKind::MatchingLow);
}

#[test]
fn t_upside_gap_two_crows() {
    let mut kl = flat_prefix();
    kl.push(bar(12, 100.0, 105.0, 99.8, 104.5));         // 大阳
    kl.push(bar(13, 106.0, 107.0, 105.5, 105.8));        // 跳空阴（仍高于 a.close=104.5）
    kl.push(bar(14, 108.0, 108.5, 104.8, 105.0));        // 吞没 b 但收盘仍高于 a.close
    // 对 c: open > b.open, close < b.close, close > a.close
    // b.open=106, b.close=105.8; c.open=108>106, c.close=105>105.8? No, need c.close < b.close=105.8
    let n = kl.len();
    kl[n - 1] = bar(14, 108.0, 108.5, 104.8, 105.2);
    assert_hit_at_last(&kl, PatternKind::UpsideGapTwoCrows);
}

// ========== 三根形态 ==========

#[test]
fn t_morning_star() {
    let mut kl = flat_prefix();
    kl.push(bar(12, 110.0, 110.5, 100.5, 101.0));         // 大阴 range=10, body=9
    kl.push(bar(13, 100.5, 101.0, 99.5, 100.5));          // 小实体，跳空到 a.close 以下
    kl.push(bar(14, 101.0, 108.0, 100.5, 106.5));         // 大阳，收盘 > a 中点(105.5)
    assert_hit_at_last(&kl, PatternKind::MorningStar);
}

#[test]
fn t_evening_star() {
    let mut kl = flat_prefix();
    kl.push(bar(12, 100.0, 110.5, 99.5, 110.0));          // 大阳
    kl.push(bar(13, 110.5, 111.0, 110.0, 110.5));         // 跳空小阳
    kl.push(bar(14, 110.0, 110.5, 103.0, 104.0));         // 大阴，收盘 < a 中点(105)
    assert_hit_at_last(&kl, PatternKind::EveningStar);
}

#[test]
fn t_three_white_soldiers() {
    let mut kl = flat_prefix();
    kl.push(bar(12, 100.0, 103.0, 99.5, 102.5));
    kl.push(bar(13, 101.5, 105.0, 101.0, 104.5));     // 开在 a 实体内，收高于 a
    kl.push(bar(14, 103.5, 107.0, 103.0, 106.5));     // 开在 b 实体内，收高于 b
    assert_hit_at_last(&kl, PatternKind::ThreeWhiteSoldiers);
}

#[test]
fn t_three_black_crows() {
    let mut kl = flat_prefix();
    kl.push(bar(12, 105.0, 105.5, 101.5, 102.0));
    kl.push(bar(13, 104.0, 104.5, 99.5, 100.0));      // 开在 a 实体内，收低于 a
    kl.push(bar(14, 102.0, 102.5, 97.5, 98.0));
    assert_hit_at_last(&kl, PatternKind::ThreeBlackCrows);
}

#[test]
fn t_three_inside_up() {
    let mut kl = flat_prefix();
    kl.push(bar(12, 110.0, 110.2, 99.8, 100.0));                 // 大阴
    kl.push(bar(13, 103.0, 105.0, 102.0, 104.5));                // 孕线
    kl.push(bar(14, 104.0, 112.0, 103.5, 111.0));                // 阳，突破 a.open(110)
    assert_hit_at_last(&kl, PatternKind::ThreeInsideUp);
}

#[test]
fn t_three_inside_down() {
    let mut kl = flat_prefix();
    kl.push(bar(12, 100.0, 110.2, 99.8, 110.0));                 // 大阳
    kl.push(bar(13, 103.5, 105.0, 102.0, 104.5));                // 孕线
    kl.push(bar(14, 104.0, 105.5, 97.0, 99.0));                  // 阴，跌破 a.open(100)
    assert_hit_at_last(&kl, PatternKind::ThreeInsideDown);
}

#[test]
fn t_three_outside_up() {
    let mut kl = flat_prefix();
    kl.push(bar(12, 100.0, 100.3, 99.0, 99.5));                  // 阴
    kl.push(bar(13, 99.3, 102.0, 99.1, 101.5));                  // 阳吞没
    kl.push(bar(14, 101.5, 104.0, 101.2, 103.5));                // 继续创新高
    assert_hit_at_last(&kl, PatternKind::ThreeOutsideUp);
}

#[test]
fn t_three_outside_down() {
    let mut kl = flat_prefix();
    kl.push(bar(12, 99.5, 100.3, 99.4, 100.0));                  // 阳
    kl.push(bar(13, 100.2, 100.3, 98.5, 99.0));                  // 阴吞没
    kl.push(bar(14, 99.0, 99.1, 96.5, 97.0));
    assert_hit_at_last(&kl, PatternKind::ThreeOutsideDown);
}

#[test]
fn t_bullish_abandoned_baby() {
    let mut kl = downtrend_prefix();
    kl.push(bar(12, 110.0, 110.5, 102.0, 102.5));                // 大阴
    kl.push(bar(13, 100.5, 101.0, 100.0, 100.5));                // 十字，high < a.low(102)
    kl.push(bar(14, 103.0, 108.0, 102.5, 107.5));                // 阳，low > b.high(101)
    assert_hit_at_last(&kl, PatternKind::BullishAbandonedBaby);
}

#[test]
fn t_bearish_abandoned_baby() {
    let mut kl = uptrend_prefix();
    kl.push(bar(12, 100.0, 108.0, 99.5, 107.5));                 // 大阳
    kl.push(bar(13, 109.5, 110.0, 109.0, 109.5));                // 十字，low > a.high(108)
    kl.push(bar(14, 107.0, 107.5, 101.0, 101.5));                // 阴，high < b.low(109)
    assert_hit_at_last(&kl, PatternKind::BearishAbandonedBaby);
}

#[test]
fn t_morning_doji_star() {
    let mut kl = flat_prefix();
    kl.push(bar(12, 110.0, 110.5, 100.0, 100.5));                // 大阴
    kl.push(bar(13, 100.3, 100.6, 99.8, 100.32));                // 十字
    kl.push(bar(14, 100.5, 108.0, 100.0, 107.0));                // 大阳，收盘 > a 中点(105.25)
    assert_hit_at_last(&kl, PatternKind::MorningDojiStar);
}

#[test]
fn t_evening_doji_star() {
    let mut kl = flat_prefix();
    kl.push(bar(12, 100.0, 110.0, 99.5, 109.5));                 // 大阳
    kl.push(bar(13, 109.8, 110.2, 109.5, 109.82));               // 十字
    kl.push(bar(14, 109.0, 109.5, 102.0, 102.5));                // 大阴，收盘 < a 中点(104.75)
    assert_hit_at_last(&kl, PatternKind::EveningDojiStar);
}

#[test]
fn t_bullish_strike() {
    let mut kl = flat_prefix();
    kl.push(bar(12, 100.0, 103.0, 99.8, 102.5));                 // 阳
    kl.push(bar(13, 102.0, 102.5, 99.5, 100.5));                 // 阴
    kl.push(bar(14, 100.0, 105.0, 99.8, 104.0));                 // 阳，c.close(104) > a.close(102.5)，c.open(100) < b.close(100.5)
    assert_hit_at_last(&kl, PatternKind::BullishStrike);
}

#[test]
fn t_bearish_strike() {
    let mut kl = flat_prefix();
    kl.push(bar(12, 105.0, 105.3, 102.0, 102.5));                // 阴
    kl.push(bar(13, 103.0, 105.0, 102.8, 104.5));                // 阳
    kl.push(bar(14, 105.0, 105.5, 100.0, 100.5));                // 阴，c.close(100.5) < a.close(102.5)，c.open(105) > b.close(104.5)
    assert_hit_at_last(&kl, PatternKind::BearishStrike);
}

#[test]
fn t_stick_sandwich_bull() {
    let mut kl = downtrend_prefix();
    kl.push(bar(12, 105.0, 105.5, 99.5, 100.0));                 // 阴 close=100
    kl.push(bar(13, 100.0, 103.0, 99.5, 102.5));                 // 阳
    kl.push(bar(14, 102.5, 103.0, 99.0, 100.0));                 // 阴 close=100（同 a.close）
    assert_hit_at_last(&kl, PatternKind::StickSandwichBull);
}

#[test]
fn t_stick_sandwich_bear() {
    let mut kl = uptrend_prefix();
    kl.push(bar(12, 95.0, 100.0, 94.5, 100.0));                  // 阳 close=100
    kl.push(bar(13, 100.0, 100.5, 97.0, 97.5));                  // 阴
    kl.push(bar(14, 97.5, 100.5, 97.0, 100.0));                  // 阳 close=100
    assert_hit_at_last(&kl, PatternKind::StickSandwichBear);
}

// ========== 五根形态 ==========

#[test]
fn t_rising_three_methods() {
    let mut kl = flat_prefix();
    // 大阳 → 3 根小阴（留在 a 实体内） → 大阳突破
    kl.push(bar(12, 100.0, 110.5, 99.5, 110.0));                 // 大阳 a, body=10
    kl.push(bar(13, 108.5, 109.0, 106.5, 107.0));                // 小阴
    kl.push(bar(14, 107.0, 108.0, 105.5, 106.0));                // 小阴
    kl.push(bar(15, 106.5, 107.5, 104.0, 105.5));                // 小阴
    kl.push(bar(16, 105.5, 112.0, 104.5, 111.0));                // 大阳，收盘 > a.close(110)
    assert_hit_at_last(&kl, PatternKind::RisingThreeMethods);
}

#[test]
fn t_falling_three_methods() {
    let mut kl = flat_prefix();
    kl.push(bar(12, 110.0, 110.5, 99.5, 100.0));                 // 大阴
    kl.push(bar(13, 101.5, 104.0, 101.0, 103.0));                // 小阳
    kl.push(bar(14, 103.0, 105.0, 102.0, 104.0));                // 小阳
    kl.push(bar(15, 104.0, 106.0, 103.5, 105.0));                // 小阳
    kl.push(bar(16, 105.0, 105.5, 98.5, 99.0));                  // 大阴，收盘 < a.close(100)
    assert_hit_at_last(&kl, PatternKind::FallingThreeMethods);
}

// ========== 多根：塔形 / 岛型 ==========

#[test]
fn t_tower_top() {
    let mut kl = flat_prefix();
    kl.push(bar(12, 100.0, 110.5, 99.5, 110.0));                 // 大阳 a
    kl.push(bar(13, 110.0, 111.0, 109.0, 110.5));                // 小
    kl.push(bar(14, 110.5, 111.2, 109.5, 110.2));                // 小
    kl.push(bar(15, 110.2, 110.8, 108.5, 109.5));                // 小
    kl.push(bar(16, 109.5, 110.0, 98.5, 99.0));                  // 大阴，close < a.open(100)
    assert_hit_at_last(&kl, PatternKind::TowerTop);
}

#[test]
fn t_tower_bottom() {
    let mut kl = flat_prefix();
    kl.push(bar(12, 110.0, 110.5, 99.5, 100.0));                 // 大阴 a
    kl.push(bar(13, 100.0, 101.0, 99.0, 100.5));                 // 小
    kl.push(bar(14, 100.5, 101.2, 99.5, 100.2));                 // 小
    kl.push(bar(15, 100.2, 100.8, 98.5, 99.5));                  // 小
    kl.push(bar(16, 99.5, 111.0, 99.0, 110.5));                  // 大阳，close > a.open(110)
    assert_hit_at_last(&kl, PatternKind::TowerBottom);
}

#[test]
fn t_island_reversal_top() {
    let mut kl = uptrend_prefix();
    // 向上跳空进入小区间
    kl.push(bar(12, 100.0, 102.0, 99.5, 101.5));        // 基准
    kl.push(bar(13, 105.0, 106.0, 104.5, 105.5));        // 向上跳空（low=104.5 > prev.high=102）
    kl.push(bar(14, 105.2, 105.8, 104.8, 105.3));        // 岛内
    kl.push(bar(15, 105.0, 105.5, 104.6, 104.9));        // 岛内
    kl.push(bar(16, 101.0, 101.5, 99.0, 100.0));        // 向下跳空脱离（high=101.5 < prev.low=104.6）
    assert_hit_at_last(&kl, PatternKind::IslandReversalTop);
}

#[test]
fn t_island_reversal_bottom() {
    let mut kl = downtrend_prefix();
    kl.push(bar(12, 108.0, 110.0, 107.5, 108.5));
    kl.push(bar(13, 104.0, 104.5, 103.0, 103.5));        // 向下跳空
    kl.push(bar(14, 103.5, 104.0, 102.8, 103.2));        // 岛内
    kl.push(bar(15, 103.3, 104.0, 103.0, 103.8));        // 岛内
    kl.push(bar(16, 106.0, 107.0, 105.0, 106.5));        // 向上跳空脱离
    assert_hit_at_last(&kl, PatternKind::IslandReversalBottom);
}
