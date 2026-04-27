// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

//! Aura-Trade macOS 桌面壳
//!
//! 启动流程：
//! 1. 选一个空闲端口 (127.0.0.1:0 → 拿到端口号 → 立即释放)
//! 2. 计算 web_root（bundle Resources/web）和 cache_dir（AppData 目录）
//! 3. 后台线程启动 tiny_http 服务
//! 4. 等待端口就绪
//! 5. 创建 WebView 窗口，指向 http://127.0.0.1:<port>/

use std::net::TcpListener;
use std::path::PathBuf;
use std::thread;
use std::time::{Duration, Instant};

use tauri::{Manager, WebviewUrl, WebviewWindowBuilder};

fn main() {
    // 日志：交给主 crate 的 logger 初始化
    if let Err(e) = aura_trade::logger::init() {
        eprintln!("logger init failed: {}", e);
    }

    tauri::Builder::default()
        .setup(|app| {
            // ---- 1. 选端口（bind 到 :0 拿 OS 分配的端口，再 drop 释放）----
            let listener = TcpListener::bind("127.0.0.1:0")
                .map_err(|e| format!("bind probe failed: {}", e))?;
            let port = listener
                .local_addr()
                .map_err(|e| format!("local_addr failed: {}", e))?
                .port();
            drop(listener);

            // ---- 2. 解析资源目录 ----
            let web_root = resolve_web_root(app.handle())?;
            let cache_dir = resolve_cache_dir(app.handle())?;
            log::info!(
                "[aura] web_root = {}, cache_dir = {}, port = {}",
                web_root.display(),
                cache_dir.display(),
                port,
            );

            // ---- 3. 后台启动 HTTP 服务 ----
            let cfg = aura_trade::config::Config {
                http_bind: format!("127.0.0.1:{}", port),
                web_root: web_root.to_string_lossy().to_string(),
                cache_dir: cache_dir.to_string_lossy().to_string(),
                binance_base: std::env::var("AURA_BINANCE_BASE")
                    .unwrap_or_else(|_| "https://api.binance.com".to_string()),
            };
            thread::Builder::new()
                .name("aura-http".into())
                .spawn(move || {
                    if let Err(e) = aura_trade::server::run(cfg) {
                        log::error!("[aura] http server exit: {}", e);
                    }
                })
                .map_err(|e| format!("spawn http thread failed: {}", e))?;

            // ---- 4. 等端口就绪（最多 5 秒）----
            wait_for_port(port, Duration::from_secs(5))
                .map_err(|e| format!("backend not ready: {}", e))?;

            // ---- 5. 创建 WebView 窗口 ----
            let url = format!("http://127.0.0.1:{}/", port);
            let parsed = url
                .parse::<tauri::Url>()
                .map_err(|e| format!("invalid url: {}", e))?;

            WebviewWindowBuilder::new(app, "main", WebviewUrl::External(parsed))
                .title("Aura Trade")
                .inner_size(1440.0, 900.0)
                .min_inner_size(1200.0, 720.0)
                .resizable(true)
                .build()
                .map_err(|e| format!("window build failed: {}", e))?;

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

/// 探测 web_root：
/// - Release：`<bundle>/Contents/Resources/_up_/web`
/// - Dev：项目根的 `web/`（启动目录通常是 app-tauri，所以 ../web）
fn resolve_web_root(handle: &tauri::AppHandle) -> Result<PathBuf, String> {
    // 1) 优先尝试 bundle resource 目录下的 web/
    if let Ok(res_dir) = handle.path().resource_dir() {
        // Tauri v2 会把 frontendDist（../web）打包到 Resources/_up_/web
        let candidates = [
            res_dir.join("_up_").join("web"),
            res_dir.join("web"),
        ];
        for c in candidates.iter() {
            if c.join("index.html").is_file() {
                return Ok(c.clone());
            }
        }
    }
    // 2) Dev fallback：相对当前可执行文件找项目根 web/
    if let Ok(exe) = std::env::current_exe() {
        // target/debug/aura_trade_desktop → 往上找 web/
        let mut p = exe.clone();
        for _ in 0..6 {
            p.pop();
            let candidate = p.join("web");
            if candidate.join("index.html").is_file() {
                return Ok(candidate);
            }
        }
    }
    // 3) 最后兜底：cwd/web
    let cwd = std::env::current_dir().map_err(|e| e.to_string())?;
    let fallback = cwd.join("web");
    if fallback.join("index.html").is_file() {
        return Ok(fallback);
    }
    Err(format!(
        "web_root not found (cwd={:?})",
        std::env::current_dir().ok()
    ))
}

/// 探测 cache_dir：
/// - Release / Dev 均用 `~/Library/Application Support/com.aura.trade/cache`
/// - 若环境变量 `AURA_CACHE_DIR` 被显式设置则优先使用它（便于开发时复用老缓存）
fn resolve_cache_dir(handle: &tauri::AppHandle) -> Result<PathBuf, String> {
    if let Ok(v) = std::env::var("AURA_CACHE_DIR") {
        let p = PathBuf::from(v);
        std::fs::create_dir_all(&p).map_err(|e| e.to_string())?;
        return Ok(p);
    }
    let base = handle
        .path()
        .app_data_dir()
        .map_err(|e| format!("app_data_dir: {}", e))?;
    let dir = base.join("cache");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir)
}

/// 循环 TCP 连接，直到能连上或超时
fn wait_for_port(port: u16, timeout: Duration) -> Result<(), String> {
    let start = Instant::now();
    let addr = format!("127.0.0.1:{}", port);
    while start.elapsed() < timeout {
        if std::net::TcpStream::connect_timeout(
            &addr.parse().map_err(|e: std::net::AddrParseError| e.to_string())?,
            Duration::from_millis(200),
        )
        .is_ok()
        {
            return Ok(());
        }
        thread::sleep(Duration::from_millis(100));
    }
    Err(format!("port {} not ready after {:?}", port, timeout))
}
