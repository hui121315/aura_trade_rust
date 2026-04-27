//! 静态前端文件服务
//!
//! 将 web/ 目录下的资源暴露给浏览器。支持：
//! - `/` → `index.html`
//! - `/app.js`, `/style.css`, `/assets/**`
//! - 基本的 Content-Type 识别
//! - 目录穿越防护

use std::fs;
use std::io;
use std::path::{Component, Path, PathBuf};

use tiny_http::{Header, Request, Response};

use super::response::json_err;

/// 服务一个静态文件请求
pub fn serve(req: Request, web_root: &str, url_path: &str) -> io::Result<()> {
    // 将 / 映射为 /index.html
    let rel = if url_path == "/" || url_path.is_empty() {
        "index.html".to_string()
    } else {
        url_path.trim_start_matches('/').to_string()
    };

    // 安全检查：禁止 .. 穿越
    let rel_path = Path::new(&rel);
    for comp in rel_path.components() {
        if matches!(comp, Component::ParentDir | Component::RootDir) {
            return req.respond(json_err(400, "非法路径"));
        }
    }

    let root = PathBuf::from(web_root);
    let full = root.join(rel_path);

    match fs::read(&full) {
        Ok(bytes) => {
            let content_type = content_type_for(&full);
            let resp = Response::from_data(bytes).with_header(
                Header::from_bytes(&b"Content-Type"[..], content_type.as_bytes())
                    .expect("valid header"),
            );
            req.respond(resp)
        }
        Err(e) if e.kind() == io::ErrorKind::NotFound => {
            req.respond(json_err(404, format!("未找到资源: {}", rel)))
        }
        Err(e) => req.respond(json_err(500, format!("读取失败: {}", e))),
    }
}

fn content_type_for(path: &Path) -> String {
    let ext = path
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    match ext.as_str() {
        "html" | "htm" => "text/html; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "js" | "mjs" => "application/javascript; charset=utf-8",
        "json" => "application/json; charset=utf-8",
        "svg" => "image/svg+xml",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "ico" => "image/x-icon",
        "woff2" => "font/woff2",
        "woff" => "font/woff",
        "ttf" => "font/ttf",
        _ => "application/octet-stream",
    }
    .to_string()
}
