//! Aura-Trade 二进制入口
//!
//! 启动 HTTP 服务（默认 127.0.0.1:3000），后续所有功能通过浏览器访问。

use aura_trade::{config::Config, logger, server};

fn main() {
    // 日志初始化（默认 info，可通过 AURA_LOG=debug 调整）
    if let Err(e) = logger::init() {
        eprintln!("日志初始化失败: {}", e);
    }

    let cfg = Config::from_env();
    log::info!("配置加载完成: {:?}", cfg);

    if let Err(e) = server::run(cfg) {
        log::error!("服务退出: {}", e);
        std::process::exit(1);
    }
}
