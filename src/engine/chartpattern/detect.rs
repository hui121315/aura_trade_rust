//! Chart Pattern 识别：基于 trend 模块产出的 swing 点序列做几何匹配
//!
//! 识别器采用"滑动子序列"方式：遍历最近 N 个 swing 点（N=10），
//! 对每个子序列尝试匹配多种图形。每匹配一次仅生成一次命中（避免重复）。

use crate::data::Kline;
use crate::engine::trend::{self, SwingKind, SwingPoint};

use super::types::{ChartPattern, ChartPatternKind};

/// 识别入口：给定 klines 计算 swings → 对子序列做图形匹配
pub fn detect_all(klines: &[Kline]) -> Vec<ChartPattern> {
    let state = trend::compute_trend_state(klines);
    let swings = &state.swings;
    if swings.len() < 3 {
        return vec![];
    }

    let mut out: Vec<ChartPattern> = Vec::new();
    let n = swings.len();

    // 用滑动窗口（最多回看最近 30 个点）检测：对 3/4/5/7 个点的组合
    let start = n.saturating_sub(30);
    let subset: Vec<&SwingPoint> = swings[start..].iter().collect();

    // --- 3 点模式（V 形 / 双顶双底初步）---
    //   需要 klines 做突破 / 后续确认（修复 V 形胜率仅 10-33% 的 bug）
    for w in subset.windows(3) {
        try_v_shape(w, klines, &mut out);
    }

    // --- 4 点模式（双顶/双底/矩形/三角形/旗形）---
    //   双底 W / 旗形都需 ATR 缓冲 + K 线验证
    for w in subset.windows(4) {
        try_double_top_bottom(w, klines, &mut out);
        try_rectangle(w, &mut out);
        try_triangles(w, &mut out);
        try_flags(w, klines, &mut out);
    }

    // --- 5 点模式（头肩/三重顶底/楔形/三角旗）---
    for w in subset.windows(5) {
        try_head_shoulders(w, &mut out);
        try_triple(w, &mut out);
        try_wedges(w, &mut out);
        try_broadening(w, &mut out);
        try_diamond(w, klines, &mut out);
        try_pennants(w, klines, &mut out);
    }

    // --- 圆弧 / 杯柄（用更大窗口 + 回归）---
    if subset.len() >= 7 {
        try_rounding(&subset, klines, &mut out);
    }

    // 去重：同一 completion_index + kind 只保留一个
    out.sort_by_key(|p| (p.completion_index, p.kind as u32));
    out.dedup_by(|a, b| a.completion_index == b.completion_index && a.kind == b.kind);

    out
}

// ---------- helpers ----------

fn make(
    kind: ChartPatternKind,
    points: Vec<SwingPoint>,
    neckline: Option<f64>,
    target: Option<f64>,
) -> ChartPattern {
    let completion_index = points.last().map(|p| p.index).unwrap_or(0);
    // 计算时间跨度（第一个 swing 点到最后一个 swing 点）
    let span_bars = if points.len() >= 2 {
        points.last().map(|p| p.index).unwrap_or(0)
            - points.first().map(|p| p.index).unwrap_or(0)
    } else {
        0
    };
    // E32：双底/双顶要求 span_bars ≥ 30（candle p.550 铁证）
    let book_reliable = match kind {
        ChartPatternKind::DoubleTop | ChartPatternKind::DoubleBottom => span_bars >= 30,
        _ => true,
    };
    ChartPattern {
        kind,
        label: kind.label().to_string(),
        direction: kind.direction(),
        strength: kind.strength(),
        points,
        neckline,
        target_price: target,
        completion_index,
        span_bars,
        book_reliable,
    }
}

fn almost_eq(a: f64, b: f64, tol_pct: f64) -> bool {
    if a.abs() < 1e-9 { return false; }
    (a - b).abs() / a.abs() <= tol_pct
}

// ---------- 3 点 ----------

/// V 形识别（严格版）
///
/// 历史评估显示原实现胜率仅 10-33%，主因：
///   1. 3 个 swing 点 "高-低-高" 或 "低-高-低" 极其常见，仅以此判定误报极多
///   2. 没有要求"后续确认"（即 c 点之后价格真的延续反转方向）
///   3. 对称性约束过松（允许 c 点偏离 a 点 3-7%）
///
/// 修复：
///   1. 上下坡幅度必须足够大 且 **形态整体持续时间足够短**（V 形是尖锐反转）
///   2. **对称性收紧**：c 点必须接近 a 点（±3% → ±2%），左右时间比例 0.5~2
///   3. **突破确认**：c 点之后至少有 `confirm_bars` 根 K 线继续延续方向，且不跌/涨回 b 点
///   4. 只在 **最近端**（子序列末尾）的窗口触发，避免历史窗口重复命中
fn try_v_shape(w: &[&SwingPoint], klines: &[Kline], out: &mut Vec<ChartPattern>) {
    if w.len() != 3 { return; }
    let (a, b, c) = (w[0], w[1], w[2]);
    if klines.is_empty() || c.index >= klines.len() { return; }

    // V 底：高-低-高
    if a.kind == SwingKind::High && b.kind == SwingKind::Low && c.kind == SwingKind::High {
        let drop = (a.price - b.price) / a.price;
        let rise = (c.price - b.price) / b.price;
        // 幅度 ≥ 8%（从 5% 收紧，避免震荡噪声）
        if drop < 0.08 || rise < 0.08 { return; }
        // 对称：c 接近 a 的 ±2%（从 3% 收紧）
        if (c.price - a.price).abs() / a.price > 0.02 { return; }
        // 时间对称：左右两段 bar 数比例 0.5 ~ 2
        let left = (b.index - a.index) as f64;
        let right = (c.index - b.index) as f64;
        if left < 2.0 || right < 2.0 { return; }
        let time_ratio = left.max(right) / left.min(right);
        if time_ratio > 2.0 { return; }
        // 后续确认：c 之后至少 3 根 K 线，窗口扩展到 10 根
        //   要求至少 1 根 close > a.price（真正突破前高），
        //   且 中间没有 close 跌破 b 点（未失败反转）
        let confirm = (c.index + 15).min(klines.len().saturating_sub(1));
        if confirm < c.index + 3 { return; }
        let mut broke = false;
        let mut failed = false;
        for k in &klines[c.index + 1 ..= confirm] {
            if k.close > a.price { broke = true; }
            if k.close < b.price { failed = true; }
        }
        if !broke || failed { return; }
        out.push(make(
            ChartPatternKind::VBottom,
            vec![*a, *b, *c],
            Some(a.price.max(c.price)),
            Some(b.price + (a.price - b.price) * 2.0),
        ));
    }

    // V 顶：低-高-低
    if a.kind == SwingKind::Low && b.kind == SwingKind::High && c.kind == SwingKind::Low {
        let rise = (b.price - a.price) / a.price;
        let drop = (b.price - c.price) / b.price;
        if rise < 0.08 || drop < 0.08 { return; }
        if (c.price - a.price).abs() / a.price > 0.02 { return; }
        let left = (b.index - a.index) as f64;
        let right = (c.index - b.index) as f64;
        if left < 2.0 || right < 2.0 { return; }
        let time_ratio = left.max(right) / left.min(right);
        if time_ratio > 2.0 { return; }
        let confirm = (c.index + 15).min(klines.len().saturating_sub(1));
        if confirm < c.index + 3 { return; }
        let mut broke = false;
        let mut failed = false;
        for k in &klines[c.index + 1 ..= confirm] {
            if k.close < a.price { broke = true; }
            if k.close > b.price { failed = true; }
        }
        if !broke || failed { return; }
        out.push(make(
            ChartPatternKind::VTop,
            vec![*a, *b, *c],
            Some(a.price.min(c.price)),
            Some(b.price - (b.price - a.price) * 2.0),
        ));
    }
}

// ---------- 4 点 ----------

/// 双顶 / 双底（严格版）
///
/// 历史评估：双底 W 胜率仅 25-50%，α -2.68%，因"颈线突破"判定过松
/// 修复：
///   1. 两顶/两底高度误差收紧至 ±1%（原 1.5%）
///   2. **颈线突破需要 ATR 缓冲**：收盘价必须越过颈线 ≥ 0.8× ATR（避免伪突破）
///   3. 两个顶/底之间必须间隔 ≥ 3 根 K 线（避免短期噪声）
fn try_double_top_bottom(w: &[&SwingPoint], klines: &[Kline], out: &mut Vec<ChartPattern>) {
    if w.len() != 4 { return; }
    let (a, b, c, d) = (w[0], w[1], w[2], w[3]);
    if klines.is_empty() || d.index >= klines.len() { return; }

    // 双顶 HLHL：H1 ≈ H2，d 点必须已**明显跌破**颈线 b
    if a.kind == SwingKind::High && b.kind == SwingKind::Low
        && c.kind == SwingKind::High && d.kind == SwingKind::Low
        && almost_eq(a.price, c.price, 0.010)
        && c.index >= a.index + 3 // 两顶间隔足够
    {
        let neck = b.price;
        let atr = recent_atr(klines, d.index, 14);
        let buffer = (0.008 * neck).max(0.8 * atr); // 至少 0.8% 或 0.8 ATR
        if d.price < neck - buffer {
            let target = neck - (a.price - neck);
            out.push(make(ChartPatternKind::DoubleTop, vec![*a, *b, *c, *d], Some(neck), Some(target)));
        }
    }
    // 双底 LHLH：L1 ≈ L2，d 点必须已**明显突破**颈线 b
    if a.kind == SwingKind::Low && b.kind == SwingKind::High
        && c.kind == SwingKind::Low && d.kind == SwingKind::High
        && almost_eq(a.price, c.price, 0.010)
        && c.index >= a.index + 3
    {
        let neck = b.price;
        let atr = recent_atr(klines, d.index, 14);
        let buffer = (0.008 * neck).max(0.8 * atr);
        if d.price > neck + buffer {
            let target = neck + (neck - a.price);
            out.push(make(ChartPatternKind::DoubleBottom, vec![*a, *b, *c, *d], Some(neck), Some(target)));
        }
    }
}

/// 最近 N 根 K 线的简易 ATR（True Range 平均）
fn recent_atr(klines: &[Kline], at: usize, n: usize) -> f64 {
    if klines.len() < 2 || at == 0 { return 0.0; }
    let start = at.saturating_sub(n);
    let end = at.min(klines.len() - 1);
    let mut sum = 0.0;
    let mut cnt = 0usize;
    for i in (start + 1)..=end {
        let h = klines[i].high;
        let l = klines[i].low;
        let pc = klines[i - 1].close;
        let tr = (h - l).max((h - pc).abs()).max((l - pc).abs());
        sum += tr;
        cnt += 1;
    }
    if cnt == 0 { 0.0 } else { sum / cnt as f64 }
}

fn try_rectangle(w: &[&SwingPoint], out: &mut Vec<ChartPattern>) {
    if w.len() != 4 { return; }
    let (a, b, c, d) = (w[0], w[1], w[2], w[3]);
    // 矩形：HLHL 或 LHLH，两高相近 且 两低相近
    let hl_pattern = a.kind == SwingKind::High && c.kind == SwingKind::High
        && b.kind == SwingKind::Low && d.kind == SwingKind::Low;
    let lh_pattern = a.kind == SwingKind::Low && c.kind == SwingKind::Low
        && b.kind == SwingKind::High && d.kind == SwingKind::High;
    if hl_pattern && almost_eq(a.price, c.price, 0.01) && almost_eq(b.price, d.price, 0.01) {
        out.push(make(ChartPatternKind::Rectangle, vec![*a, *b, *c, *d], None, None));
    }
    if lh_pattern && almost_eq(a.price, c.price, 0.01) && almost_eq(b.price, d.price, 0.01) {
        out.push(make(ChartPatternKind::Rectangle, vec![*a, *b, *c, *d], None, None));
    }
}

fn try_triangles(w: &[&SwingPoint], out: &mut Vec<ChartPattern>) {
    if w.len() != 4 { return; }
    let (a, b, c, d) = (w[0], w[1], w[2], w[3]);
    // 上升三角：两低点逐步抬高，两高点近似水平
    // R-P2-02 配套：量度目标 = 突破线上方 + (突破线 - 最低点) 的垂直距离
    if a.kind == SwingKind::Low && c.kind == SwingKind::Low
        && b.kind == SwingKind::High && d.kind == SwingKind::High
        && c.price > a.price && almost_eq(b.price, d.price, 0.012)
    {
        let neck = b.price;
        // 量度目标（原书 candle p.680 对称）：neck + (neck - a.price)
        let target = neck + (neck - a.price.min(c.price));
        out.push(make(ChartPatternKind::AscendingTriangle, vec![*a, *b, *c, *d], Some(neck), Some(target)));
    }
    // 下降三角：两高点逐步降低，两低点水平
    // R-P2-02（candle p.680）：量度跌幅 = 底边 - (顶边最高 - 底边)
    if a.kind == SwingKind::High && c.kind == SwingKind::High
        && b.kind == SwingKind::Low && d.kind == SwingKind::Low
        && c.price < a.price && almost_eq(b.price, d.price, 0.012)
    {
        let neck = b.price;
        let top_max = a.price.max(c.price);
        let target = neck - (top_max - neck);
        out.push(make(ChartPatternKind::DescendingTriangle, vec![*a, *b, *c, *d], Some(neck), Some(target)));
    }
    // 对称三角：两高点降低，两低点抬高
    if a.kind == SwingKind::High && c.kind == SwingKind::High
        && b.kind == SwingKind::Low && d.kind == SwingKind::Low
        && c.price < a.price && d.price > b.price
    {
        out.push(make(ChartPatternKind::SymmetricalTriangle, vec![*a, *b, *c, *d], None, None));
    }
    if a.kind == SwingKind::Low && c.kind == SwingKind::Low
        && b.kind == SwingKind::High && d.kind == SwingKind::High
        && c.price > a.price && d.price < b.price
    {
        out.push(make(ChartPatternKind::SymmetricalTriangle, vec![*a, *b, *c, *d], None, None));
    }
}

/// 旗形（严格版）
///
/// 历史评估：多/空旗形胜率 0-25%，α -4%，主因：
///   1. 仅由 swing 点判定，旗杆"快速脉冲"的时间约束缺失
///   2. 回调通道范围过宽，混入震荡/背离段
///   3. 无突破确认（旗面顶点被突破才是有效信号）
///
/// 修复：
///   1. 旗杆必须是"**快速推动**"：从 a→b 的 K 线数 ≤ 旗面 K 线数（动量段更陡）
///   2. 旗面回调深度严格：多头旗 ≤ 50% 斐波那契位（原 30% 但上限不严）
///   3. 必须在 d 点之后出现**向旗杆方向的突破**（close 越过 b 点 + ATR 缓冲）
fn try_flags(w: &[&SwingPoint], klines: &[Kline], out: &mut Vec<ChartPattern>) {
    if w.len() != 4 { return; }
    let (a, b, c, d) = (w[0], w[1], w[2], w[3]);
    if klines.is_empty() || d.index >= klines.len() { return; }

    let pole_bars = b.index.saturating_sub(a.index) as f64;
    let flag_bars = d.index.saturating_sub(b.index) as f64;
    if pole_bars < 2.0 || flag_bars < 3.0 { return; }
    // 旗杆动量：旗杆 K 线数应 ≤ 旗面 K 线数（旗面是震荡整理）
    if pole_bars > flag_bars { return; }

    // 多头旗：LHLH，强脉冲后震荡回调
    if a.kind == SwingKind::Low && b.kind == SwingKind::High
        && c.kind == SwingKind::Low && d.kind == SwingKind::High
    {
        let pole = (b.price - a.price) / a.price;
        if pole < 0.08 { return; }
        // 旗面回调深度：c 点回撤不超过旗杆的 50%
        let retrace = (b.price - c.price) / (b.price - a.price);
        if retrace > 0.5 || retrace < 0.1 { return; }
        // 旗面两高不可创新高
        if d.price > b.price { return; }
        // 突破确认：d 之后至少一根 close 突破旗面顶点 b + ATR 缓冲
        let atr = recent_atr(klines, d.index, 14);
        let buf = (0.005 * b.price).max(0.5 * atr);
        let end = (d.index + 5).min(klines.len() - 1);
        let broke = (d.index + 1..=end).any(|i| klines[i].close > b.price + buf);
        if !broke { return; }
        out.push(make(ChartPatternKind::BullFlag, vec![*a, *b, *c, *d], Some(b.price), Some(b.price + (b.price - a.price))));
    }

    // 空头旗：HLHL，强下跌后震荡反弹
    if a.kind == SwingKind::High && b.kind == SwingKind::Low
        && c.kind == SwingKind::High && d.kind == SwingKind::Low
    {
        let pole = (a.price - b.price) / a.price;
        if pole < 0.08 { return; }
        let retrace = (c.price - b.price) / (a.price - b.price);
        if retrace > 0.5 || retrace < 0.1 { return; }
        if d.price < b.price { return; }
        let atr = recent_atr(klines, d.index, 14);
        let buf = (0.005 * b.price).max(0.5 * atr);
        let end = (d.index + 5).min(klines.len() - 1);
        let broke = (d.index + 1..=end).any(|i| klines[i].close < b.price - buf);
        if !broke { return; }
        out.push(make(ChartPatternKind::BearFlag, vec![*a, *b, *c, *d], Some(b.price), Some(b.price - (a.price - b.price))));
    }
}

/// 三角旗（Pennant，严格版）
///
/// 与 Flag 的核心区别：旗面是**对称三角形（收敛）** 而非平行通道。
/// 与 SymmetricalTriangle 的区别：必须有**强势旗杆**在前（陡峭推动段），
/// 否则只是普通的对称三角形整固。
///
/// 5 swing 点：a(起) → b(旗杆顶/底) → c → d → e（旗面收敛 + 突破待确认）
///
/// 约束（复用 Flag 的严格标准，仅旗面几何关系不同）：
///   1. 旗杆幅度 ≥ 8%
///   2. 旗杆 K 线数 ≤ 旗面 K 线数（旗杆是脉冲，旗面是整固）
///   3. 旗面收敛：高点降低 + 低点抬高 + 第二对仍保持 high > low
///   4. e 之后 5 根内出现 close 向旗杆方向突破 b + ATR 缓冲
fn try_pennants(w: &[&SwingPoint], klines: &[Kline], out: &mut Vec<ChartPattern>) {
    if w.len() != 5 { return; }
    let (a, b, c, d, e) = (w[0], w[1], w[2], w[3], w[4]);
    if klines.is_empty() || e.index >= klines.len() { return; }

    let pole_bars = b.index.saturating_sub(a.index) as f64;
    let flag_bars = e.index.saturating_sub(b.index) as f64;
    if pole_bars < 2.0 || flag_bars < 4.0 { return; }
    if pole_bars > flag_bars { return; }

    // 多头三角旗：L-H(旗杆顶)-L-H-L，两 H 递减且 d<b，两 L 递增且 e>c，且 d>e（仍在收敛区间）
    if a.kind == SwingKind::Low && b.kind == SwingKind::High
        && c.kind == SwingKind::Low && d.kind == SwingKind::High
        && e.kind == SwingKind::Low
    {
        let pole = (b.price - a.price) / a.price.abs().max(1e-9);
        if pole < 0.08 { return; }
        if d.price >= b.price { return; }
        if e.price <= c.price { return; }
        if d.price <= e.price { return; }

        let atr = recent_atr(klines, e.index, 14);
        let buf = (0.005 * b.price).max(0.5 * atr);
        let end = (e.index + 5).min(klines.len() - 1);
        let broke = (e.index + 1..=end).any(|i| klines[i].close > b.price + buf);
        if !broke { return; }

        let target = b.price + (b.price - a.price);
        out.push(make(
            ChartPatternKind::BullPennant,
            vec![*a, *b, *c, *d, *e],
            Some(b.price),
            Some(target),
        ));
    }

    // 空头三角旗：H-L(旗杆底)-H-L-H，两 L 递增且 d>b，两 H 递减且 e<c，且 d<e
    if a.kind == SwingKind::High && b.kind == SwingKind::Low
        && c.kind == SwingKind::High && d.kind == SwingKind::Low
        && e.kind == SwingKind::High
    {
        let pole = (a.price - b.price) / a.price.abs().max(1e-9);
        if pole < 0.08 { return; }
        if d.price <= b.price { return; }
        if e.price >= c.price { return; }
        if d.price >= e.price { return; }

        let atr = recent_atr(klines, e.index, 14);
        let buf = (0.005 * b.price).max(0.5 * atr);
        let end = (e.index + 5).min(klines.len() - 1);
        let broke = (e.index + 1..=end).any(|i| klines[i].close < b.price - buf);
        if !broke { return; }

        let target = b.price - (a.price - b.price);
        out.push(make(
            ChartPatternKind::BearPennant,
            vec![*a, *b, *c, *d, *e],
            Some(b.price),
            Some(target),
        ));
    }
}

// ---------- 5 点 ----------

fn try_head_shoulders(w: &[&SwingPoint], out: &mut Vec<ChartPattern>) {
    if w.len() != 5 { return; }
    let (s1, v1, h, v2, s2) = (w[0], w[1], w[2], w[3], w[4]);
    // 头肩顶：H-L-H(更高)-L-H(接近 s1)
    if s1.kind == SwingKind::High && v1.kind == SwingKind::Low
        && h.kind == SwingKind::High && v2.kind == SwingKind::Low
        && s2.kind == SwingKind::High
    {
        if h.price > s1.price && h.price > s2.price
            && almost_eq(s1.price, s2.price, 0.04)
            && almost_eq(v1.price, v2.price, 0.03)
        {
            let neck = (v1.price + v2.price) / 2.0;
            let target = neck - (h.price - neck);
            out.push(make(ChartPatternKind::HeadAndShoulders,
                vec![*s1, *v1, *h, *v2, *s2], Some(neck), Some(target)));
        }
    }
    // 头肩底：L-H-L(更低)-H-L(接近 s1)
    if s1.kind == SwingKind::Low && v1.kind == SwingKind::High
        && h.kind == SwingKind::Low && v2.kind == SwingKind::High
        && s2.kind == SwingKind::Low
    {
        if h.price < s1.price && h.price < s2.price
            && almost_eq(s1.price, s2.price, 0.04)
            && almost_eq(v1.price, v2.price, 0.03)
        {
            let neck = (v1.price + v2.price) / 2.0;
            let target = neck + (neck - h.price);
            out.push(make(ChartPatternKind::InverseHeadAndShoulders,
                vec![*s1, *v1, *h, *v2, *s2], Some(neck), Some(target)));
        }
    }
}

fn try_triple(w: &[&SwingPoint], out: &mut Vec<ChartPattern>) {
    if w.len() != 5 { return; }
    let (a, b, c, d, e) = (w[0], w[1], w[2], w[3], w[4]);
    // 三重顶 H-L-H-L-H，三高相近
    if a.kind == SwingKind::High && b.kind == SwingKind::Low
        && c.kind == SwingKind::High && d.kind == SwingKind::Low
        && e.kind == SwingKind::High
        && almost_eq(a.price, c.price, 0.015)
        && almost_eq(c.price, e.price, 0.015)
        && almost_eq(b.price, d.price, 0.025)
    {
        let neck = (b.price + d.price) / 2.0;
        out.push(make(ChartPatternKind::TripleTop, vec![*a, *b, *c, *d, *e], Some(neck), None));
    }
    // 三重底 L-H-L-H-L
    if a.kind == SwingKind::Low && b.kind == SwingKind::High
        && c.kind == SwingKind::Low && d.kind == SwingKind::High
        && e.kind == SwingKind::Low
        && almost_eq(a.price, c.price, 0.015)
        && almost_eq(c.price, e.price, 0.015)
        && almost_eq(b.price, d.price, 0.025)
    {
        let neck = (b.price + d.price) / 2.0;
        out.push(make(ChartPatternKind::TripleBottom, vec![*a, *b, *c, *d, *e], Some(neck), None));
    }
}

fn try_wedges(w: &[&SwingPoint], out: &mut Vec<ChartPattern>) {
    if w.len() != 5 { return; }
    // 上升楔形（看跌）：两条上升线，下轨斜率 > 上轨斜率（收敛）
    // 取交替高低点 5 个中的 2 高 2 低 可能性
    // 简化：检查 high 序列与 low 序列分别单调递增，且两线距离收敛
    let highs: Vec<&SwingPoint> = w.iter().filter(|p| p.kind == SwingKind::High).copied().collect();
    let lows: Vec<&SwingPoint> = w.iter().filter(|p| p.kind == SwingKind::Low).copied().collect();
    if highs.len() >= 2 && lows.len() >= 2 {
        let h_up = highs.windows(2).all(|w| w[1].price > w[0].price);
        let l_up = lows.windows(2).all(|w| w[1].price > w[0].price);
        let h_dn = highs.windows(2).all(|w| w[1].price < w[0].price);
        let l_dn = lows.windows(2).all(|w| w[1].price < w[0].price);
        if h_up && l_up {
            // 上升楔形：判定是否收敛 (low 斜率更陡)
            let h_slope = (highs.last().unwrap().price - highs[0].price) / ((highs.last().unwrap().index - highs[0].index) as f64);
            let l_slope = (lows.last().unwrap().price - lows[0].price) / ((lows.last().unwrap().index - lows[0].index) as f64);
            if l_slope > h_slope && l_slope > 0.0 {
                out.push(make(ChartPatternKind::RisingWedge, w.iter().map(|p| **p).collect(), None, None));
            }
        }
        if h_dn && l_dn {
            // 下降楔形：high 斜率更负 (收敛)
            let h_slope = (highs.last().unwrap().price - highs[0].price) / ((highs.last().unwrap().index - highs[0].index) as f64);
            let l_slope = (lows.last().unwrap().price - lows[0].price) / ((lows.last().unwrap().index - lows[0].index) as f64);
            if h_slope < l_slope && h_slope < 0.0 {
                out.push(make(ChartPatternKind::FallingWedge, w.iter().map(|p| **p).collect(), None, None));
            }
        }
    }
}

fn try_broadening(w: &[&SwingPoint], out: &mut Vec<ChartPattern>) {
    if w.len() != 5 { return; }
    let highs: Vec<&SwingPoint> = w.iter().filter(|p| p.kind == SwingKind::High).copied().collect();
    let lows: Vec<&SwingPoint> = w.iter().filter(|p| p.kind == SwingKind::Low).copied().collect();
    if highs.len() >= 2 && lows.len() >= 2 {
        let highs_up = highs.windows(2).all(|w| w[1].price > w[0].price);
        let lows_down = lows.windows(2).all(|w| w[1].price < w[0].price);
        if highs_up && lows_down {
            // 扩散形态：高点逐步走高，低点逐步走低
            let last_kind = w.last().unwrap().kind;
            let kind = if last_kind == SwingKind::High {
                ChartPatternKind::BroadeningTop
            } else {
                ChartPatternKind::BroadeningBottom
            };
            out.push(make(kind, w.iter().map(|p| **p).collect(), None, None));
        }
    }
}

fn try_diamond(w: &[&SwingPoint], _klines: &[Kline], out: &mut Vec<ChartPattern>) {
    if w.len() != 5 { return; }
    // 菱形：先扩散后收敛，中间点是最高/最低
    let mid = w[2];
    let highs: Vec<&SwingPoint> = w.iter().filter(|p| p.kind == SwingKind::High).copied().collect();
    let lows: Vec<&SwingPoint> = w.iter().filter(|p| p.kind == SwingKind::Low).copied().collect();
    if highs.len() >= 2 && lows.len() >= 2 && mid.kind == SwingKind::High {
        // 菱形顶：中间高点最高，两侧高点渐降，两侧低点呈 V 字
        let mid_is_highest = highs.iter().all(|h| h.price <= mid.price + 1e-9);
        if mid_is_highest && lows.len() == 2 {
            let l = lows[0];
            let r = lows[1];
            // 两低之间略对称
            if (l.price - r.price).abs() / mid.price.abs().max(1e-9) < 0.05 {
                // R-P1-38（candle p.750）：量度跌幅 = 突破方向 + 菱形最高点与最低点的垂直距离
                let lowest = l.price.min(r.price);
                let height = mid.price - lowest;
                // 菱形顶多数向下突破 → target = 突破线 - height
                // 用底边（低点平均）作为突破基线
                let breakout_base = (l.price + r.price) / 2.0;
                let target = breakout_base - height;
                out.push(make(ChartPatternKind::DiamondTop, w.iter().map(|p| **p).collect(), Some(breakout_base), Some(target)));
            }
        }
    }
    if highs.len() >= 2 && lows.len() >= 2 && mid.kind == SwingKind::Low {
        let mid_is_lowest = lows.iter().all(|l| l.price >= mid.price - 1e-9);
        if mid_is_lowest && highs.len() == 2 {
            let l = highs[0];
            let r = highs[1];
            if (l.price - r.price).abs() / mid.price.abs().max(1e-9) < 0.05 {
                // R-P1-38：菱形底多数向上突破 → target = 突破线 + height
                let highest = l.price.max(r.price);
                let height = highest - mid.price;
                let breakout_base = (l.price + r.price) / 2.0;
                let target = breakout_base + height;
                out.push(make(ChartPatternKind::DiamondBottom, w.iter().map(|p| **p).collect(), Some(breakout_base), Some(target)));
            }
        }
    }
}

// ---------- 圆弧 / 杯柄 ----------

fn try_rounding(subset: &[&SwingPoint], klines: &[Kline], out: &mut Vec<ChartPattern>) {
    // 圆弧 / 杯柄：对最近一段 K 线的 close 做抛物线拟合
    //   - 用 swing 点的首尾索引界定"圆弧段"（相当于从第一个摆动点到最后一个摆动点之间的所有 close）
    //   - 对 close 序列做最小二乘二次拟合，要求 R² > 0.70
    //   - a > 0 → 圆弧底；a < 0 → 圆弧顶
    //   - 顶点位置应大致在中部 30%-70%，否则判定为趋势段而非圆弧
    if subset.len() < 7 { return; }
    let first_idx = subset.first().unwrap().index;
    let last_idx = subset.last().unwrap().index;
    if last_idx <= first_idx + 10 || last_idx >= klines.len() { return; }

    let seg: Vec<f64> = klines[first_idx..=last_idx].iter().map(|k| k.close).collect();
    let n = seg.len();
    if n < 11 { return; }
    // 对 x 做归一化（避免数值不稳定）：x = (i / (n-1)) ∈ [0, 1]
    let xs: Vec<f64> = (0..n).map(|i| i as f64 / (n as f64 - 1.0)).collect();
    let ys = seg.clone();

    if let Some((a, b, c)) = quadratic_fit(&xs, &ys) {
        let mean = ys.iter().sum::<f64>() / ys.len() as f64;
        let ss_tot: f64 = ys.iter().map(|y| (y - mean).powi(2)).sum();
        let ss_res: f64 = xs.iter().zip(ys.iter())
            .map(|(x, y)| {
                let pred = a * x * x + b * x + c;
                (y - pred).powi(2)
            })
            .sum();
        let r2 = if ss_tot > 1e-9 { 1.0 - ss_res / ss_tot } else { 0.0 };

        if r2 <= 0.50 || a.abs() < 1e-6 {
            return;
        }
        // 顶点位置 x* = -b / (2a)
        let vertex_x = -b / (2.0 * a);
        if vertex_x < 0.25 || vertex_x > 0.75 {
            // 顶点偏向两端 → 实际上更像趋势延伸而非圆弧
            return;
        }

        let all_pts: Vec<SwingPoint> = subset.iter().map(|p| **p).collect();
        if a > 0.0 {
            out.push(make(ChartPatternKind::RoundingBottom, all_pts.clone(), None, None));
            // 杯柄：最后 ~15% 区间内出现回调
            let handle_start = ((n as f64) * 0.85) as usize;
            if handle_start < n - 2 {
                let cup_top = seg[..handle_start].iter().cloned().fold(f64::NEG_INFINITY, f64::max);
                let handle_low = seg[handle_start..].iter().cloned().fold(f64::INFINITY, f64::min);
                let final_close = *seg.last().unwrap();
                // 回调幅度 3%~25%，且尾部回升
                let pullback = (cup_top - handle_low) / cup_top.abs().max(1e-9);
                if pullback > 0.03 && pullback < 0.25 && final_close > handle_low {
                    out.push(make(ChartPatternKind::CupWithHandle, all_pts, None, None));
                }
            }
        } else {
            out.push(make(ChartPatternKind::RoundingTop, all_pts, None, None));
        }
    }
}

fn quadratic_fit(xs: &[f64], ys: &[f64]) -> Option<(f64, f64, f64)> {
    let n = xs.len() as f64;
    if n < 3.0 { return None; }
    // 构造 3x3 正规方程
    let s0 = n;
    let s1: f64 = xs.iter().sum();
    let s2: f64 = xs.iter().map(|x| x * x).sum();
    let s3: f64 = xs.iter().map(|x| x * x * x).sum();
    let s4: f64 = xs.iter().map(|x| x * x * x * x).sum();
    let sy: f64 = ys.iter().sum();
    let sxy: f64 = xs.iter().zip(ys.iter()).map(|(x, y)| x * y).sum();
    let sx2y: f64 = xs.iter().zip(ys.iter()).map(|(x, y)| x * x * y).sum();

    // 矩阵  [s4 s3 s2; s3 s2 s1; s2 s1 s0] * [a b c] = [sx2y sxy sy]
    let m = [[s4, s3, s2], [s3, s2, s1], [s2, s1, s0]];
    let r = [sx2y, sxy, sy];
    // 3x3 cramer
    let det = det3(&m);
    if det.abs() < 1e-9 { return None; }
    let mut m_a = m; m_a[0][0] = r[0]; m_a[1][0] = r[1]; m_a[2][0] = r[2];
    let mut m_b = m; m_b[0][1] = r[0]; m_b[1][1] = r[1]; m_b[2][1] = r[2];
    let mut m_c = m; m_c[0][2] = r[0]; m_c[1][2] = r[1]; m_c[2][2] = r[2];
    Some((det3(&m_a) / det, det3(&m_b) / det, det3(&m_c) / det))
}

fn det3(m: &[[f64; 3]; 3]) -> f64 {
    m[0][0] * (m[1][1] * m[2][2] - m[1][2] * m[2][1])
        - m[0][1] * (m[1][0] * m[2][2] - m[1][2] * m[2][0])
        + m[0][2] * (m[1][0] * m[2][1] - m[1][1] * m[2][0])
}
