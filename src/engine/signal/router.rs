//! F7：模块 Priority 路由（R-P1-05）
//!
//! 当多种信号同时存在时，用**原书等级**路由决定：
//!
//! 1. **卖出优先于买入**（E20 "果断卖出"，卖出权重 ×1.3）
//! 2. **Strong > Medium > Weak > Noise**（SignalLevel 级别）
//! 3. **跨书铁证信号**（断头铡刀/多合一 3+类/SELL-1）排在**最前**
//! 4. 同级信号中，较早的索引优先（便于及时决策）
//!
//! # 使用场景
//!
//! 多识别器同时在同一时刻产生信号时，决定"哪个信号应当作为主要操作依据"。
//!
//! ```
//! use aura_trade::engine::signal::*;
//! use aura_trade::engine::signal::router::*;
//!
//! let mut router = SignalRouter::new();
//! router.push(RoutedSignal::new(
//!     "断头铡刀", SignalLevel::Strong, -1, 100,
//! ).with_book_tag("ma p.380"));
//! router.push(RoutedSignal::new(
//!     "红三兵", SignalLevel::Medium, 1, 105,
//! ));
//! // 卖出 Strong 排最前
//! let top = router.top();
//! assert_eq!(top.unwrap().name, "断头铡刀");
//! ```

use serde::{Deserialize, Serialize};

use super::level::SignalLevel;

/// 已路由的信号条目
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoutedSignal {
    /// 人类可读名
    pub name: String,
    /// 信号级别（R-P1-11）
    pub level: SignalLevel,
    /// 方向：+1 买 / -1 卖 / 0 中性
    pub direction: i8,
    /// K 线索引（时间顺序）
    pub index: usize,
    /// 原书出处（可选）
    pub book_tag: Option<String>,
    /// 是否为原书铁证 Priority 信号（断头铡刀/多合一/SELL-1 等）
    pub is_iron_evidence: bool,
}

impl RoutedSignal {
    pub fn new(
        name: impl Into<String>,
        level: SignalLevel,
        direction: i8,
        index: usize,
    ) -> Self {
        Self {
            name: name.into(),
            level,
            direction,
            index,
            book_tag: None,
            is_iron_evidence: false,
        }
    }

    pub fn with_book_tag(mut self, tag: impl Into<String>) -> Self {
        self.book_tag = Some(tag.into());
        self
    }

    pub fn iron_evidence(mut self) -> Self {
        self.is_iron_evidence = true;
        self
    }

    /// 路由优先级数值（越大越优先）
    ///
    /// 计算规则（从高到低加权）：
    /// - 铁证信号：+10000
    /// - 卖出：+1000（E20 "果断卖出"）
    /// - SignalLevel 权重 × 100：Strong=150, Medium=100, Weak=50, Noise=10
    pub fn priority_score(&self) -> i32 {
        let mut score = 0;
        if self.is_iron_evidence {
            score += 10_000;
        }
        if self.direction < 0 {
            score += 1_000; // 卖出加权
        }
        score += (self.level.weight_multiplier() * 100.0) as i32;
        score
    }
}

/// 信号路由器
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SignalRouter {
    signals: Vec<RoutedSignal>,
}

impl SignalRouter {
    pub fn new() -> Self {
        Self::default()
    }

    /// 添加一个信号
    pub fn push(&mut self, sig: RoutedSignal) {
        self.signals.push(sig);
    }

    /// 添加多个信号
    pub fn extend(&mut self, sigs: impl IntoIterator<Item = RoutedSignal>) {
        self.signals.extend(sigs);
    }

    /// 返回按优先级降序排列的信号（不消耗）
    pub fn sorted(&self) -> Vec<RoutedSignal> {
        let mut out = self.signals.clone();
        out.sort_by(|a, b| {
            b.priority_score()
                .cmp(&a.priority_score())
                .then_with(|| a.index.cmp(&b.index))
        });
        out
    }

    /// 返回优先级最高的单个信号
    pub fn top(&self) -> Option<RoutedSignal> {
        self.sorted().into_iter().next()
    }

    /// 返回前 N 个信号
    pub fn top_n(&self, n: usize) -> Vec<RoutedSignal> {
        self.sorted().into_iter().take(n).collect()
    }

    /// 过滤：仅返回指定方向的信号
    pub fn only_direction(&self, direction: i8) -> Vec<RoutedSignal> {
        self.sorted()
            .into_iter()
            .filter(|s| s.direction == direction)
            .collect()
    }

    /// 清空
    pub fn clear(&mut self) {
        self.signals.clear();
    }

    pub fn len(&self) -> usize {
        self.signals.len()
    }

    pub fn is_empty(&self) -> bool {
        self.signals.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn t_iron_evidence_beats_strong() {
        // 铁证级信号优先于普通 Strong
        let iron = RoutedSignal::new("断头铡刀", SignalLevel::Strong, -1, 100).iron_evidence();
        let strong = RoutedSignal::new("黄昏之星", SignalLevel::Strong, -1, 100);
        assert!(iron.priority_score() > strong.priority_score());
    }

    #[test]
    fn t_sell_priority_higher_than_buy_at_same_level() {
        // E20：同级卖出 > 买入
        let sell = RoutedSignal::new("S1 跌破", SignalLevel::Strong, -1, 100);
        let buy = RoutedSignal::new("B1 突破", SignalLevel::Strong, 1, 100);
        assert!(sell.priority_score() > buy.priority_score());
    }

    #[test]
    fn t_strong_beats_medium_weak_noise() {
        let strong = RoutedSignal::new("s", SignalLevel::Strong, 1, 0);
        let medium = RoutedSignal::new("m", SignalLevel::Medium, 1, 0);
        let weak = RoutedSignal::new("w", SignalLevel::Weak, 1, 0);
        let noise = RoutedSignal::new("n", SignalLevel::Noise, 1, 0);
        assert!(strong.priority_score() > medium.priority_score());
        assert!(medium.priority_score() > weak.priority_score());
        assert!(weak.priority_score() > noise.priority_score());
    }

    #[test]
    fn t_router_sorts_correctly() {
        let mut router = SignalRouter::new();
        router.push(RoutedSignal::new("n1", SignalLevel::Noise, 1, 5));
        router.push(RoutedSignal::new("m1", SignalLevel::Medium, 1, 10));
        router.push(
            RoutedSignal::new("断头铡刀", SignalLevel::Strong, -1, 15).iron_evidence(),
        );
        router.push(RoutedSignal::new("s1", SignalLevel::Strong, 1, 20));

        let top = router.top().unwrap();
        assert_eq!(top.name, "断头铡刀");

        let top2 = router.top_n(2);
        assert_eq!(top2.len(), 2);
        assert_eq!(top2[0].name, "断头铡刀");
        assert_eq!(top2[1].name, "s1"); // Strong 卖 < Strong 买？ 让我验证
                                         // priority = iron ? 10000 : 0; sell ? 1000 : 0; level * 100
                                         // 断头铡刀：10000 + 1000 + 150 = 11150
                                         // s1 买 Strong：0 + 0 + 150 = 150
                                         // m1 买 Medium：0 + 0 + 100 = 100
                                         // 顺序：断头铡刀 > s1 > m1 > n1
    }

    #[test]
    fn t_only_direction_filter() {
        let mut router = SignalRouter::new();
        router.push(RoutedSignal::new("buy1", SignalLevel::Medium, 1, 1));
        router.push(RoutedSignal::new("sell1", SignalLevel::Medium, -1, 2));
        router.push(RoutedSignal::new("buy2", SignalLevel::Strong, 1, 3));

        let buys = router.only_direction(1);
        assert_eq!(buys.len(), 2);
        // 应按强度降序
        assert_eq!(buys[0].name, "buy2");
        assert_eq!(buys[1].name, "buy1");

        let sells = router.only_direction(-1);
        assert_eq!(sells.len(), 1);
    }

    #[test]
    fn t_same_priority_earlier_index_first() {
        let mut router = SignalRouter::new();
        router.push(RoutedSignal::new("later", SignalLevel::Medium, 1, 20));
        router.push(RoutedSignal::new("earlier", SignalLevel::Medium, 1, 10));
        let sorted = router.sorted();
        // 同等级、同方向 → 早的排前
        assert_eq!(sorted[0].name, "earlier");
    }

    #[test]
    fn t_empty_router_returns_none() {
        let router = SignalRouter::new();
        assert!(router.top().is_none());
        assert!(router.is_empty());
        assert_eq!(router.len(), 0);
    }

    #[test]
    fn t_clear_resets() {
        let mut router = SignalRouter::new();
        router.push(RoutedSignal::new("s", SignalLevel::Strong, 1, 1));
        router.clear();
        assert!(router.is_empty());
    }

    #[test]
    fn t_with_book_tag_stored() {
        let sig = RoutedSignal::new("断头铡刀", SignalLevel::Strong, -1, 100)
            .with_book_tag("ma p.380");
        assert_eq!(sig.book_tag.as_deref(), Some("ma p.380"));
    }
}
