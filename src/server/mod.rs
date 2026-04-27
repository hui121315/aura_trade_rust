//! HTTP 服务层
//!
//! 基于 `tiny_http` 实现一个零异步依赖的单进程 Web 服务器。
//! 负责：
//! - 分发 API 请求到各业务模块
//! - 提供前端静态文件（web/）

pub mod response;
pub mod routes;
pub mod server;
pub mod static_files;
pub mod system_routes;
pub mod url_decode;

pub use server::run;
