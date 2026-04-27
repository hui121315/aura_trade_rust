//! 极简 URL `application/x-www-form-urlencoded` 解码
//!
//! 仅实现我们需要的功能：
//! - `%HH` 百分号转义 → 字节
//! - `+` → 空格
//! 不引入 `url` / `urlencoding` 等依赖。

/// 将 URL 编码的字符串解码为 UTF-8（非法 UTF-8 会保留原始字节映射）
pub fn decode(input: &str) -> String {
    let bytes = input.as_bytes();
    let mut out: Vec<u8> = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'%' if i + 2 < bytes.len() => {
                let hi = hex_val(bytes[i + 1]);
                let lo = hex_val(bytes[i + 2]);
                match (hi, lo) {
                    (Some(h), Some(l)) => {
                        out.push((h << 4) | l);
                        i += 3;
                    }
                    _ => {
                        out.push(bytes[i]);
                        i += 1;
                    }
                }
            }
            b'+' => {
                out.push(b' ');
                i += 1;
            }
            b => {
                out.push(b);
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn hex_val(c: u8) -> Option<u8> {
    match c {
        b'0'..=b'9' => Some(c - b'0'),
        b'a'..=b'f' => Some(c - b'a' + 10),
        b'A'..=b'F' => Some(c - b'A' + 10),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn basic() {
        assert_eq!(decode("hello"), "hello");
        assert_eq!(decode("a+b"), "a b");
        assert_eq!(decode("a%20b"), "a b");
        assert_eq!(decode("%E4%B8%AD%E6%96%87"), "中文");
    }
}
