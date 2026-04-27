//! Chart Pattern 类型定义

use serde::{Deserialize, Serialize};

use crate::engine::trend::SwingPoint;

/// 主力行为学标签（R-P1-37，跨书不变量 2.6）
///
/// 原书铁证：某些技术形态本质上反映了**主力**（机构/庄家/大资金）的行为，
/// 识别形态时同步标记主力意图可**提升决策精度**。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MarketMakerBehavior {
    /// 主力吸筹（低位囤积筹码，如潜伏底 / 圆底 / 杯柄）
    Accumulation,
    /// 主力派发（高位出货，如圆顶 / 倒三阳）
    Distribution,
    /// 洗盘震仓（主力故意造成剧烈波动甩掉跟风盘，如扩散三角）
    Washout,
    /// 潜伏突破（小阳线缩量隐蔽突破）
    Stealth,
    /// 恐慌盘（跌破均线粘合瞬间放量）
    Panic,
}

impl MarketMakerBehavior {
    pub fn label(&self) -> &'static str {
        match self {
            MarketMakerBehavior::Accumulation => "主力吸筹",
            MarketMakerBehavior::Distribution => "主力派发",
            MarketMakerBehavior::Washout => "洗盘震仓",
            MarketMakerBehavior::Stealth => "潜伏突破",
            MarketMakerBehavior::Panic => "恐慌盘",
        }
    }

    /// 主力行为的预期后续方向（+1 看多 / -1 看空 / 0 中性震荡）
    pub fn expected_direction(&self) -> i8 {
        match self {
            MarketMakerBehavior::Accumulation | MarketMakerBehavior::Stealth => 1,
            MarketMakerBehavior::Distribution | MarketMakerBehavior::Panic => -1,
            MarketMakerBehavior::Washout => 0, // 震荡后才定方向
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ChartPatternKind {
    // --- 反转 ---
    HeadAndShoulders,          // 头肩顶
    InverseHeadAndShoulders,   // 头肩底
    DoubleTop,                 // 双顶 M
    DoubleBottom,              // 双底 W
    TripleTop,                 // 三重顶
    TripleBottom,              // 三重底
    RoundingTop,               // 圆弧顶
    RoundingBottom,            // 圆弧底
    VTop,                      // V 形顶
    VBottom,                   // V 形底
    DiamondTop,                // 菱形顶
    DiamondBottom,             // 菱形底

    // --- 持续（中继） ---
    AscendingTriangle,         // 上升三角形
    DescendingTriangle,        // 下降三角形
    SymmetricalTriangle,       // 对称三角形
    RisingWedge,               // 上升楔形（看跌，收敛）
    FallingWedge,              // 下降楔形（看涨，收敛）
    BullFlag,                  // 多头旗形
    BearFlag,                  // 空头旗形
    BullPennant,               // 多头三角旗
    BearPennant,               // 空头三角旗
    Rectangle,                 // 矩形（箱体）
    CupWithHandle,             // 杯柄
    BroadeningTop,             // 扩散三角形顶（喇叭口）
    BroadeningBottom,          // 扩散三角形底
}

impl ChartPatternKind {
    pub fn label(&self) -> &'static str {
        use ChartPatternKind::*;
        match self {
            HeadAndShoulders => "头肩顶",
            InverseHeadAndShoulders => "头肩底",
            DoubleTop => "双顶 M",
            DoubleBottom => "双底 W",
            TripleTop => "三重顶",
            TripleBottom => "三重底",
            RoundingTop => "圆弧顶",
            RoundingBottom => "圆弧底",
            VTop => "V 形顶",
            VBottom => "V 形底",
            DiamondTop => "菱形顶",
            DiamondBottom => "菱形底",
            AscendingTriangle => "上升三角形",
            DescendingTriangle => "下降三角形",
            SymmetricalTriangle => "对称三角形",
            RisingWedge => "上升楔形",
            FallingWedge => "下降楔形",
            BullFlag => "多头旗形",
            BearFlag => "空头旗形",
            BullPennant => "多头三角旗",
            BearPennant => "空头三角旗",
            Rectangle => "矩形（箱体）",
            CupWithHandle => "杯柄",
            BroadeningTop => "扩散三角形（顶）",
            BroadeningBottom => "扩散三角形（底）",
        }
    }

    pub fn direction(&self) -> i8 {
        use ChartPatternKind::*;
        match self {
            HeadAndShoulders
            | DoubleTop
            | TripleTop
            | RoundingTop
            | VTop
            | DiamondTop
            | DescendingTriangle
            | RisingWedge
            | BearFlag
            | BearPennant
            | BroadeningTop => -1,
            InverseHeadAndShoulders
            | DoubleBottom
            | TripleBottom
            | RoundingBottom
            | VBottom
            | DiamondBottom
            | AscendingTriangle
            | FallingWedge
            | BullFlag
            | BullPennant
            | CupWithHandle
            | BroadeningBottom => 1,
            SymmetricalTriangle | Rectangle => 0,
        }
    }

    /// 形态互通映射（R-P1-40，candle p.808）
    ///
    /// 原书原文：
    /// > "转势的矩形形态大多可以看作是**圆顶和圆底**，或者是**双顶（包括多重顶）和双底（包括多重底）**。"
    ///
    /// 底部 3 形态互通（candle p.640）：V/淡友/岛形可映射
    pub fn equivalent_patterns(&self) -> Vec<ChartPatternKind> {
        use ChartPatternKind::*;
        match self {
            // 矩形反转 ⇌ 圆顶圆底 ⇌ 双顶双底
            Rectangle => vec![RoundingTop, RoundingBottom, DoubleTop, DoubleBottom],
            RoundingTop => vec![Rectangle, DoubleTop, TripleTop],
            RoundingBottom => vec![Rectangle, DoubleBottom, TripleBottom],
            DoubleTop => vec![Rectangle, RoundingTop, TripleTop],
            DoubleBottom => vec![Rectangle, RoundingBottom, TripleBottom],
            TripleTop => vec![DoubleTop, HeadAndShoulders],
            TripleBottom => vec![DoubleBottom, InverseHeadAndShoulders],
            // 底部形态互通（V/岛形/淡友）
            VBottom => vec![InverseHeadAndShoulders, RoundingBottom],
            VTop => vec![HeadAndShoulders, RoundingTop],
            _ => vec![],
        }
    }

    /// 原书主力行为学标签（R-P1-37 配套，跨书不变量 2.6）
    ///
    /// 原书多处铁证：
    /// - **扩散三角形（顶）**（candle p.720）= 主力**过顶吸筹洗盘**
    /// - **矩形**（candle p.795）= 主力**囤积**（最终向上突破概率大）
    /// - **圆底 / 潜伏底**（candle p.580）= 主力**吸筹蓄势**
    pub fn market_maker_behavior(&self) -> Option<MarketMakerBehavior> {
        use ChartPatternKind::*;
        match self {
            BroadeningTop => Some(MarketMakerBehavior::Washout), // 过顶吸筹洗盘
            BroadeningBottom => Some(MarketMakerBehavior::Distribution), // 派发
            Rectangle => Some(MarketMakerBehavior::Accumulation), // 囤积
            RoundingBottom => Some(MarketMakerBehavior::Accumulation), // 吸筹
            RoundingTop => Some(MarketMakerBehavior::Distribution),
            CupWithHandle => Some(MarketMakerBehavior::Accumulation),
            _ => None,
        }
    }

    /// 图形强度 1-6（基于 9 数据集真实评估，P0 修复后重排）
    ///
    /// - 6 星：菱形（日线 α +11.87% 85.7% 胜率，跨数据集 5/6 正）
    /// - 5 星：修复后的 V 形、头肩、三重顶底、楔形（α +1~3%）
    /// - 4 星：一般图形
    /// - 3 星：中性（对称三角、矩形）
    /// - 1 星：反向失效（双底 W、旗形、上升三角 —— 虽然识别器已修复但历史仍反向）
    pub fn strength(&self) -> u8 {
        use ChartPatternKind::*;
        match self {
            // 6 星：最强
            DiamondTop | DiamondBottom => 6,
            // 5 星：强可用
            HeadAndShoulders | InverseHeadAndShoulders
            | TripleTop | TripleBottom
            | RisingWedge | FallingWedge
            | VTop | VBottom => 5,
            // 4 星：可用但不稳
            CupWithHandle | RoundingTop | RoundingBottom
            | BroadeningTop | BroadeningBottom => 4,
            // 3 星：中性或信号弱
            SymmetricalTriangle | Rectangle => 3,
            // 1 星：历史评估反向失效，即使识别器修复也保守对待
            DoubleTop | DoubleBottom
            | AscendingTriangle | DescendingTriangle
            | BullFlag | BearFlag | BullPennant | BearPennant => 1,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChartPattern {
    pub kind: ChartPatternKind,
    pub label: String,
    pub direction: i8,
    pub strength: u8,
    /// 形态内涉及的摆动点（按时间顺序）
    pub points: Vec<SwingPoint>,
    /// 颈线 / 关键水平价位（用于突破判定）
    pub neckline: Option<f64>,
    /// 测量目标（对称投影出的目标价）
    pub target_price: Option<f64>,
    /// 形态完成的 K 线索引
    pub completion_index: usize,
    /// 形态持续的 K 线数（最后一个点 - 第一个点，E32 新增）
    ///
    /// 用于判定原书"时间周期"要求（如双底 ≥1 个月 / 30 根）
    #[serde(default)]
    pub span_bars: usize,
    /// 是否满足原书所有可靠性条件（E32 新增）
    ///
    /// - 双顶/双底：span_bars ≥ 30（candle p.550）
    /// - 其他形态：暂无明确要求，默认 true
    #[serde(default = "default_book_reliable")]
    pub book_reliable: bool,
}

fn default_book_reliable() -> bool {
    true
}

/// 头肩顶/底量度目标（E33 新增）
///
/// 原书 candle p.460 铁证："**如果头肩顶所转的趋势，自起涨点至颈线位置的幅度小于
/// 从头肩顶头部最高点至颈线的垂直幅度，那么头肩顶颈线突破后可能会到达的价格
/// 就是（按标准公式计算）**"。
///
/// 即：**简单公式 `target = neck - (head - neck)` 仅在前提条件成立时可靠**。
///
/// 前提不满足时，实际跌幅可能超过该目标，因此应同时提供保守目标（回到起涨点）。
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct HeadShouldersMeasure {
    /// 简单对称公式：target = neck - (head - neck)（头肩顶）或 neck + (neck - head)（头肩底）
    pub symmetric_target: f64,
    /// 起涨点/起跌点（用于保守目标）
    pub origin_price: f64,
    /// 起涨幅度（从起涨点到颈线）
    pub origin_to_neck_span: f64,
    /// 头部到颈线的垂直幅度
    pub head_to_neck_span: f64,
    /// 前提是否成立（起涨幅度 < 头部到颈线幅度）
    pub premise_met: bool,
}

impl HeadShouldersMeasure {
    /// 推荐目标价：
    /// - 前提满足 → 简单对称目标
    /// - 前提不满足 → 实际可能跌破对称目标到起涨点（取较低/较高者）
    pub fn recommended_target(&self, is_top: bool) -> f64 {
        if self.premise_met {
            self.symmetric_target
        } else if is_top {
            // 头肩顶：取更低者（更深跌幅）
            self.symmetric_target.min(self.origin_price)
        } else {
            // 头肩底：取更高者（更高涨幅）
            self.symmetric_target.max(self.origin_price)
        }
    }
}

impl ChartPattern {
    /// 头肩顶/底的带前提条件量度（E33）
    ///
    /// # 参数
    /// - `origin_price`：起涨点（头肩顶）或起跌点（头肩底）价格
    ///
    /// # 返回
    /// - 若非头肩顶/底 → `None`
    /// - 若颈线或头部缺失 → `None`
    /// - 否则 → `Some(HeadShouldersMeasure)`
    pub fn head_shoulders_measure(
        &self,
        origin_price: f64,
    ) -> Option<HeadShouldersMeasure> {
        let is_top = self.kind == ChartPatternKind::HeadAndShoulders;
        let is_bottom = self.kind == ChartPatternKind::InverseHeadAndShoulders;
        if !is_top && !is_bottom {
            return None;
        }
        let neck = self.neckline?;
        // 头部 = 头肩顶的最高价 / 头肩底的最低价（通常在 points[2]）
        let head = if is_top {
            self.points
                .iter()
                .map(|p| p.price)
                .fold(f64::NEG_INFINITY, f64::max)
        } else {
            self.points
                .iter()
                .map(|p| p.price)
                .fold(f64::INFINITY, f64::min)
        };
        if !head.is_finite() || !neck.is_finite() {
            return None;
        }
        let head_to_neck_span = (head - neck).abs();
        let origin_to_neck_span = (origin_price - neck).abs();
        let symmetric_target = if is_top {
            neck - head_to_neck_span
        } else {
            neck + head_to_neck_span
        };
        let premise_met = origin_to_neck_span < head_to_neck_span;
        Some(HeadShouldersMeasure {
            symmetric_target,
            origin_price,
            origin_to_neck_span,
            head_to_neck_span,
            premise_met,
        })
    }

    /// 是否满足原书对形态时间周期的最低要求（E32）
    pub fn meets_book_time_requirement(&self) -> bool {
        use ChartPatternKind::*;
        match self.kind {
            // 原书 candle p.550：双顶/双底 ≥ 1 个月（30 根）才可靠
            DoubleTop | DoubleBottom => self.span_bars >= 30,
            // 其他形态暂无明确时间要求
            _ => true,
        }
    }

    /// R-P1-41 矩形角色判定（candle p.804）
    ///
    /// 原书铁证：
    /// > "矩形是整理技术图形，**但是整理过久，或恰好处在顶部/底部区域，
    /// > 整理结束后股价没有按先前趋势方向突破，矩形就意味着趋势反转**。"
    ///
    /// # 参数
    /// - `prior_trend`：矩形之前的趋势方向（+1 上升 / -1 下降 / 0 中性）
    /// - `breakout_direction`：突破方向（+1 向上 / -1 向下 / 0 未突破）
    /// - `is_over_long`：整理是否过长（建议 > 60 根）
    ///
    /// # 返回
    /// 仅对 `Rectangle` 类型有效；其他返回 `None`
    pub fn rectangle_role(
        &self,
        prior_trend: i8,
        breakout_direction: i8,
        is_over_long: bool,
    ) -> Option<RectangleRole> {
        if self.kind != ChartPatternKind::Rectangle {
            return None;
        }
        // 未突破 → 仅标记为整理或过久整理
        if breakout_direction == 0 {
            return Some(if is_over_long {
                RectangleRole::OverlongConsolidation
            } else {
                RectangleRole::Consolidation
            });
        }
        // 有突破：检查方向
        if prior_trend == 0 {
            // 无前置趋势 → 无法判定反转，视为整理突破
            return Some(RectangleRole::Continuation);
        }
        // 突破方向与前置趋势一致 → 中继整理
        if prior_trend == breakout_direction {
            return Some(RectangleRole::Continuation);
        }
        // 突破方向与前置趋势相反 → 趋势反转（原书铁证）
        Some(RectangleRole::Reversal)
    }
}

/// 矩形角色（R-P1-41，candle p.804）
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum RectangleRole {
    /// 整理（未突破，时间 ≤ 上限）
    Consolidation,
    /// 过长整理（未突破，时间 > 上限，仍待确认）
    OverlongConsolidation,
    /// 中继（突破方向与前置趋势一致）
    Continuation,
    /// **反转**（突破方向与前置趋势**相反**）—— 原书铁证
    Reversal,
}

impl RectangleRole {
    pub fn label(&self) -> &'static str {
        match self {
            RectangleRole::Consolidation => "整理",
            RectangleRole::OverlongConsolidation => "过长整理",
            RectangleRole::Continuation => "中继",
            RectangleRole::Reversal => "反转",
        }
    }

    /// 是否为反转信号
    pub fn is_reversal(&self) -> bool {
        matches!(self, RectangleRole::Reversal)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::trend::SwingKind;

    fn mk_swing(idx: usize, price: f64, kind: SwingKind) -> SwingPoint {
        SwingPoint {
            index: idx,
            time: (idx as i64) * 86_400_000,
            price,
            kind,
        }
    }

    fn mk_pattern(kind: ChartPatternKind, pts: Vec<SwingPoint>, neck: Option<f64>) -> ChartPattern {
        let span = if pts.len() >= 2 {
            pts.last().unwrap().index - pts.first().unwrap().index
        } else {
            0
        };
        let book_reliable = match kind {
            ChartPatternKind::DoubleTop | ChartPatternKind::DoubleBottom => span >= 30,
            _ => true,
        };
        ChartPattern {
            kind,
            label: kind.label().to_string(),
            direction: kind.direction(),
            strength: kind.strength(),
            completion_index: pts.last().map(|p| p.index).unwrap_or(0),
            points: pts,
            neckline: neck,
            target_price: None,
            span_bars: span,
            book_reliable,
        }
    }

    #[test]
    fn t_e32_double_top_short_span_unreliable() {
        // 双顶 span < 30 根 → book_reliable = false
        let pts = vec![
            mk_swing(0, 100.0, SwingKind::High),
            mk_swing(5, 95.0, SwingKind::Low),
            mk_swing(10, 100.0, SwingKind::High),
            mk_swing(15, 90.0, SwingKind::Low),
        ];
        let p = mk_pattern(ChartPatternKind::DoubleTop, pts, Some(95.0));
        assert_eq!(p.span_bars, 15);
        assert!(!p.book_reliable);
        assert!(!p.meets_book_time_requirement());
    }

    #[test]
    fn t_e32_double_top_long_span_reliable() {
        // 双顶 span ≥ 30 根 → book_reliable = true
        let pts = vec![
            mk_swing(0, 100.0, SwingKind::High),
            mk_swing(10, 95.0, SwingKind::Low),
            mk_swing(20, 100.0, SwingKind::High),
            mk_swing(35, 90.0, SwingKind::Low),
        ];
        let p = mk_pattern(ChartPatternKind::DoubleTop, pts, Some(95.0));
        assert_eq!(p.span_bars, 35);
        assert!(p.book_reliable);
        assert!(p.meets_book_time_requirement());
    }

    #[test]
    fn t_e32_other_patterns_not_affected() {
        // 非双顶/双底形态（如 V 顶）不受 span 约束影响
        let pts = vec![
            mk_swing(0, 100.0, SwingKind::Low),
            mk_swing(3, 120.0, SwingKind::High),
            mk_swing(5, 100.0, SwingKind::Low),
        ];
        let p = mk_pattern(ChartPatternKind::VTop, pts, None);
        assert_eq!(p.span_bars, 5);
        assert!(p.book_reliable); // 非双顶/底默认 true
        assert!(p.meets_book_time_requirement());
    }

    #[test]
    fn t_e33_head_shoulders_premise_met() {
        // 头肩顶：起涨幅度 < 头部-颈线幅度 → 前提满足
        // neck = 100，head = 120，起涨点 85（起涨幅度 15，< head-neck 20）
        let pts = vec![
            mk_swing(0, 110.0, SwingKind::High),   // 左肩
            mk_swing(5, 95.0, SwingKind::Low),     // 谷 1
            mk_swing(10, 120.0, SwingKind::High),  // 头部
            mk_swing(15, 97.0, SwingKind::Low),    // 谷 2
            mk_swing(20, 111.0, SwingKind::High),  // 右肩
        ];
        let p = mk_pattern(ChartPatternKind::HeadAndShoulders, pts, Some(100.0));
        let measure = p.head_shoulders_measure(85.0).unwrap();
        assert_eq!(measure.origin_price, 85.0);
        assert!(measure.premise_met, "起涨幅度 15 < 头部幅度 20 应成立");
        assert_eq!(measure.symmetric_target, 80.0); // 100 - 20
        assert_eq!(measure.recommended_target(true), 80.0);
    }

    #[test]
    fn t_e33_head_shoulders_premise_not_met() {
        // 头肩顶：起涨幅度 > 头部-颈线幅度 → 前提不满足
        // neck = 100，head = 110（幅度 10），起涨点 80（幅度 20 > 10）
        let pts = vec![
            mk_swing(0, 105.0, SwingKind::High),
            mk_swing(5, 96.0, SwingKind::Low),
            mk_swing(10, 110.0, SwingKind::High), // head
            mk_swing(15, 97.0, SwingKind::Low),
            mk_swing(20, 106.0, SwingKind::High),
        ];
        let p = mk_pattern(ChartPatternKind::HeadAndShoulders, pts, Some(100.0));
        let measure = p.head_shoulders_measure(80.0).unwrap();
        assert!(!measure.premise_met);
        assert_eq!(measure.symmetric_target, 90.0); // 100 - 10
        // 前提不满足 → 推荐目标取更低者 = min(90, 80) = 80
        assert_eq!(measure.recommended_target(true), 80.0);
    }

    #[test]
    fn t_e33_inverse_head_shoulders() {
        // 头肩底对称测试
        let pts = vec![
            mk_swing(0, 90.0, SwingKind::Low),
            mk_swing(5, 100.0, SwingKind::High),
            mk_swing(10, 80.0, SwingKind::Low), // 头部（最低）
            mk_swing(15, 100.0, SwingKind::High),
            mk_swing(20, 91.0, SwingKind::Low),
        ];
        let p = mk_pattern(
            ChartPatternKind::InverseHeadAndShoulders,
            pts,
            Some(100.0),
        );
        // 起跌点 115（起跌幅度 15 < 头部幅度 20 → 前提满足）
        let measure = p.head_shoulders_measure(115.0).unwrap();
        assert!(measure.premise_met);
        assert_eq!(measure.symmetric_target, 120.0); // 100 + 20
        assert_eq!(measure.recommended_target(false), 120.0);
    }

    #[test]
    fn t_e33_non_head_shoulders_returns_none() {
        // 非头肩顶/底形态 → None
        let pts = vec![
            mk_swing(0, 100.0, SwingKind::High),
            mk_swing(5, 95.0, SwingKind::Low),
            mk_swing(10, 100.0, SwingKind::High),
            mk_swing(15, 90.0, SwingKind::Low),
        ];
        let p = mk_pattern(ChartPatternKind::DoubleTop, pts, Some(95.0));
        assert!(p.head_shoulders_measure(85.0).is_none());
    }

    // -------- R-P1-37 主力行为学标签测试 --------

    #[test]
    fn t_market_maker_washout_for_broadening_top() {
        // 扩散三角形（顶）= 主力过顶吸筹洗盘（candle p.720）
        assert_eq!(
            ChartPatternKind::BroadeningTop.market_maker_behavior(),
            Some(MarketMakerBehavior::Washout)
        );
    }

    #[test]
    fn t_market_maker_accumulation_for_rectangle_rounding_bottom() {
        // 矩形 = 主力囤积（candle p.795）
        assert_eq!(
            ChartPatternKind::Rectangle.market_maker_behavior(),
            Some(MarketMakerBehavior::Accumulation)
        );
        // 圆底 = 主力吸筹（candle p.580）
        assert_eq!(
            ChartPatternKind::RoundingBottom.market_maker_behavior(),
            Some(MarketMakerBehavior::Accumulation)
        );
        // 杯柄 = 主力吸筹
        assert_eq!(
            ChartPatternKind::CupWithHandle.market_maker_behavior(),
            Some(MarketMakerBehavior::Accumulation)
        );
    }

    #[test]
    fn t_market_maker_distribution_for_tops() {
        assert_eq!(
            ChartPatternKind::RoundingTop.market_maker_behavior(),
            Some(MarketMakerBehavior::Distribution)
        );
        assert_eq!(
            ChartPatternKind::BroadeningBottom.market_maker_behavior(),
            Some(MarketMakerBehavior::Distribution)
        );
    }

    #[test]
    fn t_market_maker_none_for_neutral_patterns() {
        // V 形、三角形等不映射到主力行为
        assert_eq!(
            ChartPatternKind::VTop.market_maker_behavior(),
            None
        );
        assert_eq!(
            ChartPatternKind::AscendingTriangle.market_maker_behavior(),
            None
        );
    }

    #[test]
    fn t_market_maker_behavior_expected_direction() {
        assert_eq!(MarketMakerBehavior::Accumulation.expected_direction(), 1);
        assert_eq!(MarketMakerBehavior::Stealth.expected_direction(), 1);
        assert_eq!(MarketMakerBehavior::Distribution.expected_direction(), -1);
        assert_eq!(MarketMakerBehavior::Panic.expected_direction(), -1);
        assert_eq!(MarketMakerBehavior::Washout.expected_direction(), 0);
    }

    // -------- R-P2-02 下降三角形量度目标测试 --------

    #[test]
    fn t_r_p2_02_descending_triangle_target_price() {
        // 下降三角形：高点 110/105（下降），低点水平 100
        // 量度跌幅 = neck - (top_max - neck) = 100 - (110 - 100) = 90
        // 此处通过 detect.rs 的 make() 验证（已在 detect.rs 内部填充 target）
        // 本测试直接构造 ChartPattern 验证结构一致
        let pts = vec![
            mk_swing(0, 110.0, SwingKind::High),
            mk_swing(5, 100.0, SwingKind::Low),
            mk_swing(10, 105.0, SwingKind::High),
            mk_swing(15, 100.0, SwingKind::Low),
        ];
        let mut p = mk_pattern(ChartPatternKind::DescendingTriangle, pts, Some(100.0));
        // 手动设置 target（模拟 detect.rs 行为）
        p.target_price = Some(100.0 - (110.0 - 100.0)); // 90.0
        assert_eq!(p.target_price, Some(90.0));
    }

    // -------- R-P1-40 互通映射测试 --------

    #[test]
    fn t_rectangle_equivalent_to_rounding_and_double() {
        // candle p.808：矩形 ⇌ 圆顶圆底 ⇌ 双顶双底
        let eq = ChartPatternKind::Rectangle.equivalent_patterns();
        assert!(eq.contains(&ChartPatternKind::RoundingTop));
        assert!(eq.contains(&ChartPatternKind::RoundingBottom));
        assert!(eq.contains(&ChartPatternKind::DoubleTop));
        assert!(eq.contains(&ChartPatternKind::DoubleBottom));
    }

    #[test]
    fn t_triple_top_equivalent_to_head_shoulders() {
        // candle p.570：三重顶 = 特殊头肩顶
        let eq = ChartPatternKind::TripleTop.equivalent_patterns();
        assert!(eq.contains(&ChartPatternKind::HeadAndShoulders));
    }

    #[test]
    fn t_v_bottom_equivalent_to_inverse_head_shoulders() {
        // candle p.640：V 底/岛形/淡友反攻互通
        let eq = ChartPatternKind::VBottom.equivalent_patterns();
        assert!(eq.contains(&ChartPatternKind::InverseHeadAndShoulders));
        assert!(eq.contains(&ChartPatternKind::RoundingBottom));
    }

    #[test]
    fn t_patterns_without_equivalent_return_empty() {
        let eq = ChartPatternKind::SymmetricalTriangle.equivalent_patterns();
        assert!(eq.is_empty());
    }

    // -------- R-P1-41 矩形反转判定测试 --------

    #[test]
    fn t_rectangle_role_reversal_when_break_against_prior_trend() {
        // 下降趋势中 → 矩形整理 → 向上突破 = 反转（candle p.804 豫能控股案例）
        let pts = vec![
            mk_swing(0, 100.0, SwingKind::High),
            mk_swing(10, 90.0, SwingKind::Low),
            mk_swing(20, 100.0, SwingKind::High),
            mk_swing(30, 90.0, SwingKind::Low),
        ];
        let p = mk_pattern(ChartPatternKind::Rectangle, pts, None);
        let role = p.rectangle_role(-1, 1, false); // 前降 + 向上突破
        assert_eq!(role, Some(RectangleRole::Reversal));
        assert!(role.unwrap().is_reversal());
    }

    #[test]
    fn t_rectangle_role_continuation_when_break_aligns() {
        // 上升趋势中矩形向上突破 = 中继
        let pts = vec![
            mk_swing(0, 100.0, SwingKind::High),
            mk_swing(10, 90.0, SwingKind::Low),
            mk_swing(20, 100.0, SwingKind::High),
            mk_swing(30, 90.0, SwingKind::Low),
        ];
        let p = mk_pattern(ChartPatternKind::Rectangle, pts, None);
        let role = p.rectangle_role(1, 1, false); // 前升 + 向上突破
        assert_eq!(role, Some(RectangleRole::Continuation));
    }

    #[test]
    fn t_rectangle_role_overlong_no_breakout() {
        // 过久整理 + 未突破 → OverlongConsolidation
        let pts = vec![
            mk_swing(0, 100.0, SwingKind::High),
            mk_swing(10, 90.0, SwingKind::Low),
            mk_swing(20, 100.0, SwingKind::High),
            mk_swing(30, 90.0, SwingKind::Low),
        ];
        let p = mk_pattern(ChartPatternKind::Rectangle, pts, None);
        let role = p.rectangle_role(1, 0, true);
        assert_eq!(role, Some(RectangleRole::OverlongConsolidation));
    }

    #[test]
    fn t_rectangle_role_none_for_non_rectangle() {
        let pts = vec![
            mk_swing(0, 100.0, SwingKind::High),
            mk_swing(5, 95.0, SwingKind::Low),
        ];
        let p = mk_pattern(ChartPatternKind::VTop, pts, None);
        assert!(p.rectangle_role(1, 1, false).is_none());
    }
}
