//! 聚合规则（CombineRule）求值
//!
//! 给定某根 bar 上各组件的触发事件（或无），按规则决定是否输出一个
//! 方向明确的体系信号（`CombinedSignal`）。
//!
//! # M2 实现（全 4 种规则）
//!
//! - ✅ `AllAligned`
//! - ✅ `MajorityK`
//! - ✅ `WeightedScore`
//! - ✅ `SequentialCascade`（需要 [`CombineCtx`] 提供 scan 结果 + 当前 bar）

use std::collections::HashMap;

use super::definition::CombineRule;
use super::scan::{ScanResult, TriggerEvent};

/// 聚合后的体系信号
#[derive(Debug, Clone)]
pub struct CombinedSignal {
    pub direction: i8,
    pub confidence: f64,
    pub contributing_components: Vec<String>,
}

/// 聚合求值所需的跨 bar 上下文（仅 `SequentialCascade` 需要）
#[derive(Debug, Clone, Copy)]
pub struct CombineCtx<'a> {
    pub scan: &'a ScanResult,
    pub current_bar: usize,
    /// 体系 `components` 完整 ID 列表（按声明顺序）
    pub components: &'a [String],
}

/// 核心入口：给定当前 bar 上各组件的触发，按规则产出体系信号
///
/// - `per_component`：`(component_id, Option<TriggerEvent>)` 切片
/// - `rule`：聚合规则
/// - `weights`：仅 `WeightedScore` 使用
/// - `ctx`：仅 `SequentialCascade` 使用；其他规则忽略
///
/// 返回 `None` = 本 bar 无体系信号（持仓不变）
pub fn evaluate_combine(
    per_component: &[(String, Option<&TriggerEvent>)],
    rule: &CombineRule,
    weights: &HashMap<String, f64>,
    ctx: Option<&CombineCtx>,
) -> Option<CombinedSignal> {
    // Cascade 不依赖 per_component（完全从 ctx 回溯），优先处理
    if let CombineRule::SequentialCascade { window_bars } = rule {
        return evaluate_cascade(*window_bars, ctx);
    }

    // 其他规则：只考虑真的有触发的组件
    let fired: Vec<(&String, &TriggerEvent)> = per_component
        .iter()
        .filter_map(|(id, e)| e.map(|ev| (id, ev)))
        .collect();
    if fired.is_empty() {
        return None;
    }

    match rule {
        CombineRule::AllAligned => {
            // 所有定义的组件必须都触发，且方向一致
            if fired.len() != per_component.len() {
                return None;
            }
            let d0 = fired[0].1.direction;
            if d0 == 0 {
                return None;
            }
            if !fired.iter().all(|(_, e)| e.direction == d0) {
                return None;
            }
            let conf = mean_conf(&fired);
            Some(CombinedSignal {
                direction: d0,
                confidence: conf,
                contributing_components: fired.iter().map(|(id, _)| (*id).clone()).collect(),
            })
        }

        CombineRule::MajorityK { k } => {
            let up = fired.iter().filter(|(_, e)| e.direction == 1).count();
            let dn = fired.iter().filter(|(_, e)| e.direction == -1).count();
            let k = *k;
            if up >= k && up > dn {
                let contrib: Vec<String> = fired
                    .iter()
                    .filter(|(_, e)| e.direction == 1)
                    .map(|(id, _)| (*id).clone())
                    .collect();
                Some(CombinedSignal {
                    direction: 1,
                    confidence: up as f64 / per_component.len() as f64,
                    contributing_components: contrib,
                })
            } else if dn >= k && dn > up {
                let contrib: Vec<String> = fired
                    .iter()
                    .filter(|(_, e)| e.direction == -1)
                    .map(|(id, _)| (*id).clone())
                    .collect();
                Some(CombinedSignal {
                    direction: -1,
                    confidence: dn as f64 / per_component.len() as f64,
                    contributing_components: contrib,
                })
            } else {
                None
            }
        }

        CombineRule::WeightedScore { threshold } => {
            let mut score = 0.0;
            let mut total_abs_weight = 0.0;
            for (id, e) in &fired {
                let w = weights.get(id.as_str()).copied().unwrap_or(1.0);
                total_abs_weight += w.abs();
                score += w * (e.direction as f64) * e.confidence;
            }
            if score >= *threshold {
                let contrib: Vec<String> = fired
                    .iter()
                    .filter(|(_, e)| e.direction == 1)
                    .map(|(id, _)| (*id).clone())
                    .collect();
                Some(CombinedSignal {
                    direction: 1,
                    confidence: (score / total_abs_weight.max(1e-9)).clamp(0.0, 1.0),
                    contributing_components: contrib,
                })
            } else if score <= -threshold {
                let contrib: Vec<String> = fired
                    .iter()
                    .filter(|(_, e)| e.direction == -1)
                    .map(|(id, _)| (*id).clone())
                    .collect();
                Some(CombinedSignal {
                    direction: -1,
                    confidence: (-score / total_abs_weight.max(1e-9)).clamp(0.0, 1.0),
                    contributing_components: contrib,
                })
            } else {
                None
            }
        }

        CombineRule::SequentialCascade { .. } => unreachable!("已在开头处理"),
    }
}

/// SequentialCascade 的实现
///
/// 要求 `components` 按声明顺序依次触发，且**最后一个组件必须在 `current_bar`
/// 触发**。每相邻两级的触发时间差必须 ≤ `window_bars`。所有级别方向一致。
///
/// # 返回
///
/// 成功时返回带方向的 `CombinedSignal`，`contributing_components` 列出链上所有组件。
fn evaluate_cascade(window_bars: usize, ctx: Option<&CombineCtx>) -> Option<CombinedSignal> {
    let ctx = ctx?;
    if ctx.components.is_empty() {
        return None;
    }
    // 最后一环必须在 current_bar 触发
    let last_cid = ctx.components.last().unwrap();
    let last_event = ctx.scan.get_trigger(last_cid, ctx.current_bar)?;
    let target_dir = last_event.direction;
    if target_dir == 0 {
        return None;
    }

    // 向前追溯
    let mut expected_upper = ctx.current_bar;
    let mut chain: Vec<String> = vec![last_cid.clone()];
    for cid in ctx.components.iter().rev().skip(1) {
        let min_bar = expected_upper.saturating_sub(window_bars);
        let events = ctx.scan.triggers.get(cid.as_str())?;
        // 在 [min_bar, expected_upper) 内找同方向触发（取最近的）
        let found = events.iter().rev().find(|e| {
            e.bar_index < expected_upper && e.bar_index >= min_bar && e.direction == target_dir
        })?;
        chain.push(cid.clone());
        expected_upper = found.bar_index;
    }

    chain.reverse();
    Some(CombinedSignal {
        direction: target_dir,
        confidence: last_event.confidence,
        contributing_components: chain,
    })
}

fn mean_conf(fired: &[(&String, &TriggerEvent)]) -> f64 {
    if fired.is_empty() {
        return 0.0;
    }
    fired.iter().map(|(_, e)| e.confidence).sum::<f64>() / fired.len() as f64
}

// ============================================================
// 测试
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn ev(dir: i8, conf: f64) -> TriggerEvent {
        TriggerEvent {
            bar_index: 0,
            direction: dir,
            confidence: conf,
            reason: "test".into(),
        }
    }

    #[test]
    fn t_all_aligned_all_up_passes() {
        let a = ev(1, 1.0);
        let b = ev(1, 1.0);
        let per = vec![("x".to_string(), Some(&a)), ("y".to_string(), Some(&b))];
        let r = evaluate_combine(&per, &CombineRule::AllAligned, &HashMap::new(), None).unwrap();
        assert_eq!(r.direction, 1);
        assert_eq!(r.contributing_components.len(), 2);
    }

    #[test]
    fn t_all_aligned_one_missing_rejects() {
        let a = ev(1, 1.0);
        let per = vec![("x".to_string(), Some(&a)), ("y".to_string(), None)];
        assert!(evaluate_combine(&per, &CombineRule::AllAligned, &HashMap::new(), None).is_none());
    }

    #[test]
    fn t_all_aligned_opposite_direction_rejects() {
        let a = ev(1, 1.0);
        let b = ev(-1, 1.0);
        let per = vec![("x".to_string(), Some(&a)), ("y".to_string(), Some(&b))];
        assert!(evaluate_combine(&per, &CombineRule::AllAligned, &HashMap::new(), None).is_none());
    }

    #[test]
    fn t_majority_k_2_of_3() {
        let a = ev(1, 1.0);
        let b = ev(1, 1.0);
        let per = vec![
            ("x".to_string(), Some(&a)),
            ("y".to_string(), Some(&b)),
            ("z".to_string(), None),
        ];
        let r = evaluate_combine(&per, &CombineRule::MajorityK { k: 2 }, &HashMap::new(), None)
            .expect("应通过 k=2");
        assert_eq!(r.direction, 1);
    }

    #[test]
    fn t_majority_k_tie_rejected() {
        let a = ev(1, 1.0);
        let b = ev(-1, 1.0);
        let per = vec![("x".to_string(), Some(&a)), ("y".to_string(), Some(&b))];
        assert!(evaluate_combine(&per, &CombineRule::MajorityK { k: 1 }, &HashMap::new(), None)
            .is_none());
    }

    #[test]
    fn t_weighted_score_above_threshold() {
        let a = ev(1, 1.0);
        let b = ev(1, 1.0);
        let per = vec![("x".to_string(), Some(&a)), ("y".to_string(), Some(&b))];
        let mut w = HashMap::new();
        w.insert("x".to_string(), 2.0);
        w.insert("y".to_string(), 1.0);
        let r = evaluate_combine(&per, &CombineRule::WeightedScore { threshold: 2.5 }, &w, None)
            .unwrap();
        assert_eq!(r.direction, 1);
    }

    #[test]
    fn t_weighted_score_below_threshold() {
        let a = ev(1, 0.5);
        let per = vec![("x".to_string(), Some(&a))];
        let mut w = HashMap::new();
        w.insert("x".to_string(), 1.0);
        assert!(
            evaluate_combine(&per, &CombineRule::WeightedScore { threshold: 1.0 }, &w, None)
                .is_none()
        );
    }

    #[test]
    fn t_cascade_without_ctx_returns_none() {
        let a = ev(1, 1.0);
        let per = vec![("x".to_string(), Some(&a))];
        // 不传 ctx → Cascade 无法工作
        assert!(evaluate_combine(
            &per,
            &CombineRule::SequentialCascade { window_bars: 5 },
            &HashMap::new(),
            None,
        )
        .is_none());
    }

    // ----- SequentialCascade 的测试需要构造 ScanResult 作为 ctx -----

    fn mk_scan(triggers: Vec<(&'static str, Vec<(usize, i8)>)>) -> ScanResult {
        let mut map: HashMap<&'static str, Vec<TriggerEvent>> = HashMap::new();
        for (id, events) in triggers {
            map.insert(
                id,
                events
                    .into_iter()
                    .map(|(bar, d)| TriggerEvent {
                        bar_index: bar,
                        direction: d,
                        confidence: 1.0,
                        reason: String::new(),
                    })
                    .collect(),
            );
        }
        ScanResult { triggers: map, atr: vec![] }
    }

    #[test]
    fn t_cascade_happy_path_in_order() {
        // 组件顺序 [a, b, c]；a@5 b@8 c@10；window=5 → 应通过，方向 +1
        let scan = mk_scan(vec![
            ("a", vec![(5, 1)]),
            ("b", vec![(8, 1)]),
            ("c", vec![(10, 1)]),
        ]);
        let comps = vec!["a".to_string(), "b".to_string(), "c".to_string()];
        let ctx = CombineCtx { scan: &scan, current_bar: 10, components: &comps };
        let r = evaluate_combine(
            &[],
            &CombineRule::SequentialCascade { window_bars: 5 },
            &HashMap::new(),
            Some(&ctx),
        )
        .expect("cascade should fire");
        assert_eq!(r.direction, 1);
        assert_eq!(r.contributing_components, vec!["a", "b", "c"]);
    }

    #[test]
    fn t_cascade_last_not_on_current_bar_rejected() {
        let scan = mk_scan(vec![("a", vec![(5, 1)]), ("b", vec![(8, 1)])]);
        let comps = vec!["a".to_string(), "b".to_string()];
        // current_bar = 10 但 b 只在 8 触发 → 应失败
        let ctx = CombineCtx { scan: &scan, current_bar: 10, components: &comps };
        assert!(evaluate_combine(
            &[],
            &CombineRule::SequentialCascade { window_bars: 5 },
            &HashMap::new(),
            Some(&ctx),
        )
        .is_none());
    }

    #[test]
    fn t_cascade_out_of_window_rejected() {
        // a@1, b@10；window=5 → 10-1=9 > 5 → 失败
        let scan = mk_scan(vec![("a", vec![(1, 1)]), ("b", vec![(10, 1)])]);
        let comps = vec!["a".to_string(), "b".to_string()];
        let ctx = CombineCtx { scan: &scan, current_bar: 10, components: &comps };
        assert!(evaluate_combine(
            &[],
            &CombineRule::SequentialCascade { window_bars: 5 },
            &HashMap::new(),
            Some(&ctx),
        )
        .is_none());
    }

    #[test]
    fn t_cascade_direction_mismatch_rejected() {
        // a 方向 -1，但 b 方向 +1 → 不同向，失败
        let scan = mk_scan(vec![("a", vec![(5, -1)]), ("b", vec![(8, 1)])]);
        let comps = vec!["a".to_string(), "b".to_string()];
        let ctx = CombineCtx { scan: &scan, current_bar: 8, components: &comps };
        assert!(evaluate_combine(
            &[],
            &CombineRule::SequentialCascade { window_bars: 5 },
            &HashMap::new(),
            Some(&ctx),
        )
        .is_none());
    }

    #[test]
    fn t_cascade_picks_nearest_same_direction() {
        // a 有多次触发：(2, +1), (7, +1)；b@8 +1；window=3 → 应选 7 那次
        let scan = mk_scan(vec![
            ("a", vec![(2, 1), (7, 1)]),
            ("b", vec![(8, 1)]),
        ]);
        let comps = vec!["a".to_string(), "b".to_string()];
        let ctx = CombineCtx { scan: &scan, current_bar: 8, components: &comps };
        let r = evaluate_combine(
            &[],
            &CombineRule::SequentialCascade { window_bars: 3 },
            &HashMap::new(),
            Some(&ctx),
        )
        .unwrap();
        assert_eq!(r.direction, 1);
    }
}
