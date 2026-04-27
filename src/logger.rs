//! 极简 stderr 日志器
//!
//! 目的：避免引入 `env_logger` / `tracing` 等重量级依赖，同时支持 `log::info!`。
//! 通过环境变量 `AURA_LOG` 控制级别（trace/debug/info/warn/error），默认 info。

use log::{Level, LevelFilter, Log, Metadata, Record, SetLoggerError};

static LOGGER: MicroLogger = MicroLogger;

struct MicroLogger;

impl Log for MicroLogger {
    fn enabled(&self, _metadata: &Metadata) -> bool {
        true
    }
    fn log(&self, record: &Record) {
        if !self.enabled(record.metadata()) {
            return;
        }
        let ts = current_time_rfc3339();
        let tag = match record.level() {
            Level::Error => "ERROR",
            Level::Warn => "WARN ",
            Level::Info => "INFO ",
            Level::Debug => "DEBUG",
            Level::Trace => "TRACE",
        };
        eprintln!(
            "{} {} [{}] {}",
            ts,
            tag,
            record.target(),
            record.args()
        );
    }
    fn flush(&self) {}
}

/// 安装日志器
pub fn init() -> Result<(), SetLoggerError> {
    let level = match std::env::var("AURA_LOG").unwrap_or_default().to_ascii_lowercase().as_str() {
        "trace" => LevelFilter::Trace,
        "debug" => LevelFilter::Debug,
        "warn" => LevelFilter::Warn,
        "error" => LevelFilter::Error,
        _ => LevelFilter::Info,
    };
    log::set_logger(&LOGGER).map(|()| log::set_max_level(level))
}

/// 生成当前 UTC 时间的 RFC3339 字符串（秒级）。
///
/// 不依赖 `chrono`，直接基于 `SystemTime` 计算，保持零额外依赖。
fn current_time_rfc3339() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default();
    let secs = now.as_secs() as i64;
    let (y, mo, d, h, mi, s) = ymdhms_utc(secs);
    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
        y, mo, d, h, mi, s
    )
}

/// 将 Unix 时间戳（秒）转为 UTC (y, m, d, h, mi, s)
fn ymdhms_utc(epoch: i64) -> (i32, u32, u32, u32, u32, u32) {
    let days = epoch.div_euclid(86400);
    let sod = epoch.rem_euclid(86400) as u32;
    let h = sod / 3600;
    let mi = (sod / 60) % 60;
    let s = sod % 60;
    let (y, mo, d) = civil_from_days(days);
    (y, mo, d, h, mi, s)
}

/// Howard Hinnant 的天数 → 日历算法
fn civil_from_days(z: i64) -> (i32, u32, u32) {
    let z = z + 719468;
    let era = z.div_euclid(146097);
    let doe = z.rem_euclid(146097) as u32;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    (y as i32, m, d)
}
