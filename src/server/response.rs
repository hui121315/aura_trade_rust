//! HTTP 响应构造辅助
//!
//! 统一所有 API 的 JSON 返回格式 + 错误处理。

use serde::Serialize;
use tiny_http::{Header, Response};

/// 统一 API 响应信封
#[derive(Debug, Serialize)]
pub struct ApiEnvelope<T: Serialize> {
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<T>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl<T: Serialize> ApiEnvelope<T> {
    pub fn ok(data: T) -> Self {
        Self { ok: true, data: Some(data), error: None }
    }
}

#[derive(Debug, Serialize)]
struct ApiError {
    ok: bool,
    error: String,
}

fn json_header() -> Header {
    Header::from_bytes(&b"Content-Type"[..], &b"application/json; charset=utf-8"[..])
        .expect("static header is valid")
}

fn cache_control_no_cache() -> Header {
    Header::from_bytes(&b"Cache-Control"[..], &b"no-cache"[..])
        .expect("static header is valid")
}

/// 序列化并返回 200 JSON
pub fn json_ok<T: Serialize>(data: T) -> Response<std::io::Cursor<Vec<u8>>> {
    let envelope = ApiEnvelope::ok(data);
    let body = serde_json::to_vec(&envelope).unwrap_or_else(|e| {
        format!(r#"{{"ok":false,"error":"serialize: {}"}}"#, e).into_bytes()
    });
    Response::from_data(body)
        .with_header(json_header())
        .with_header(cache_control_no_cache())
}

/// 返回 4xx/5xx 错误 JSON
pub fn json_err(status: u16, message: impl Into<String>) -> Response<std::io::Cursor<Vec<u8>>> {
    let err = ApiError { ok: false, error: message.into() };
    let body = serde_json::to_vec(&err).unwrap_or_else(|_| {
        br#"{"ok":false,"error":"internal"}"#.to_vec()
    });
    Response::from_data(body)
        .with_status_code(status)
        .with_header(json_header())
        .with_header(cache_control_no_cache())
}
