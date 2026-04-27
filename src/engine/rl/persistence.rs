//! Bandit 状态持久化：原子写 JSON + 读取 + 版本迁移
//!
//! 存储路径：`<cache_dir>/bandit_state.v1.json`

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use super::types::BanditState;

pub const STATE_FILE_NAME: &str = "bandit_state.v1.json";

/// 根据 cache_dir 推出状态文件完整路径
pub fn state_path(cache_dir: impl AsRef<Path>) -> PathBuf {
    cache_dir.as_ref().join(STATE_FILE_NAME)
}

/// 读取状态；若文件不存在或损坏则返回全新默认值
pub fn load_or_default(cache_dir: impl AsRef<Path>) -> BanditState {
    let path = state_path(&cache_dir);
    match fs::read(&path) {
        Ok(bytes) => match serde_json::from_slice::<BanditState>(&bytes) {
            Ok(s) => {
                if s.version != BanditState::CURRENT_VERSION {
                    log::warn!(
                        "Bandit state version mismatch ({} vs current {}), resetting",
                        s.version,
                        BanditState::CURRENT_VERSION
                    );
                    BanditState::new()
                } else {
                    s
                }
            }
            Err(e) => {
                log::warn!(
                    "Bandit state file broken ({}): {}, resetting",
                    path.display(),
                    e
                );
                BanditState::new()
            }
        },
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => BanditState::new(),
        Err(e) => {
            log::warn!("Bandit state read failed: {}", e);
            BanditState::new()
        }
    }
}

/// 原子写入：先写 `.tmp`，再 rename
pub fn save(cache_dir: impl AsRef<Path>, state: &BanditState) -> std::io::Result<()> {
    let cache_dir = cache_dir.as_ref();
    fs::create_dir_all(cache_dir)?;
    let path = state_path(cache_dir);
    let tmp = path.with_extension("json.tmp");

    let bytes = serde_json::to_vec(state)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e.to_string()))?;

    {
        let mut f = fs::File::create(&tmp)?;
        f.write_all(&bytes)?;
        f.sync_all()?;
    }
    // rename 是原子的（同一 filesystem 上）
    fs::rename(&tmp, &path)?;
    Ok(())
}

// ============================================================
// 测试
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::rl::types::{ArmCategory, ArmState};

    fn tmp_dir(suffix: &str) -> PathBuf {
        let mut p = std::env::temp_dir();
        p.push(format!(
            "aura_bandit_test_{}_{}",
            std::process::id(),
            suffix
        ));
        let _ = fs::remove_dir_all(&p);
        fs::create_dir_all(&p).unwrap();
        p
    }

    #[test]
    fn t_load_default_when_missing() {
        let dir = tmp_dir("missing");
        let s = load_or_default(&dir);
        assert_eq!(s.version, BanditState::CURRENT_VERSION);
        assert!(s.arms.is_empty());
        fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn t_save_then_load_roundtrip() {
        let dir = tmp_dir("roundtrip");
        let mut s = BanditState::new();
        let a = s.get_or_insert("signal.x", "X", ArmCategory::Signal, Some("p.1"));
        a.alpha = 7.0;
        a.beta = 3.0;
        a.total_triggers = 9;
        a.total_wins = 6;
        a.total_losses = 3;

        save(&dir, &s).unwrap();
        let loaded = load_or_default(&dir);
        assert_eq!(loaded.arms.len(), 1);
        let arm = &loaded.arms["signal.x"];
        assert_eq!(arm.alpha, 7.0);
        assert_eq!(arm.total_wins, 6);
        assert_eq!(arm.book_source.as_deref(), Some("p.1"));
        fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn t_corrupt_file_resets_to_default() {
        let dir = tmp_dir("corrupt");
        let path = state_path(&dir);
        fs::write(&path, b"{not valid json").unwrap();
        let loaded = load_or_default(&dir);
        assert!(loaded.arms.is_empty());
        assert_eq!(loaded.version, BanditState::CURRENT_VERSION);
        fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn t_version_mismatch_resets() {
        let dir = tmp_dir("version");
        let path = state_path(&dir);
        // 手工写一个 version=999 的 state
        let mut s = BanditState::new();
        s.version = 999;
        s.arms.insert(
            "x".to_string(),
            ArmState::new("x", "X", ArmCategory::Signal, None::<String>),
        );
        fs::write(&path, serde_json::to_vec(&s).unwrap()).unwrap();

        let loaded = load_or_default(&dir);
        assert_eq!(loaded.version, BanditState::CURRENT_VERSION);
        assert!(loaded.arms.is_empty()); // 被重置
        fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn t_save_is_atomic_tmp_cleared() {
        let dir = tmp_dir("atomic");
        let s = BanditState::new();
        save(&dir, &s).unwrap();
        // .tmp 应已被 rename 走
        let tmp_path = state_path(&dir).with_extension("json.tmp");
        assert!(!tmp_path.exists());
        fs::remove_dir_all(dir).ok();
    }
}
