//! 强化学习模块（Sprint B：Thompson Sampling Bandit）
//!
//! 给每个信号/策略 arm 维护 Beta(α, β) 后验，每次触发按 Thompson 采样选执行者，
//! 然后在 `horizon` 根 K 线后根据收益结算并更新后验。
//!
//! 设计文档：`RL_EFFECTIVENESS_DESIGN.md`
//!
//! 主要流程：
//!
//! ```text
//! 触发 register_trigger  →  pending push
//!                             ↓
//! 新 K 线 on_new_bar     →  bars_elapsed += 1
//!                             ↓
//!                   到 horizon 时 apply_settlement
//!                             ↓
//!           arm.settle → α/β 更新
//! ```

pub mod bandit;
pub mod evaluator;
pub mod persistence;
pub mod rng;
pub mod types;

pub use bandit::{choose, merge_report, rank_snapshot, SelectionPolicy};
pub use evaluator::{on_new_bar, register_trigger, settle_all, NEUTRAL_THRESHOLD_PCT};
pub use persistence::{load_or_default, save, state_path, STATE_FILE_NAME};
pub use rng::Xoshiro256;
pub use types::{ArmCategory, ArmState, BanditState, PendingEvaluation};
