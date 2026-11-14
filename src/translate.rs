//! Optional online translation.
//!
//! Translation is deliberately isolated from the rest of the app: it is only
//! called when the user asks for it, it never touches the mail store, and any
//! failure is surfaced as a normal UI error while the rest of the client keeps
//! working entirely offline.
//!
//! The default provider is MyMemory's public API, which needs no account or
//! API key. Only the body of the message being translated is sent.

use std::time::Duration;

use anyhow::{anyhow, Result};
use serde_json::Value;

/// MyMemory caps anonymous queries; keep chunks small enough to stay reliable.
const MAX_CHARS: usize = 400;

pub struct Translation {
    pub text: String,
}

pub fn translate(text: &str) -> Result<Translation> {
    let text = text.trim();
    if text.is_empty() {
        return Err(anyhow!("这封邮件没有可翻译的正文"));
    }

    let chinese = looks_chinese(text);
    let (from, to) = if chinese {
        ("zh-CN", "en")
    } else {
        ("en", "zh-CN")
    };

    let mut parts = Vec::new();
    for (index, chunk) in chunk(text, MAX_CHARS).into_iter().enumerate() {
        if index > 0 {
            std::thread::sleep(Duration::from_millis(180));
        }
        parts.push(translate_chunk(&chunk, from, to)?);
    }

    Ok(Translation {
        text: parts.join("\n\n"),
    })
}

fn translate_chunk(text: &str, from: &str, to: &str) -> Result<String> {
    let query = urlencoding::encode(text);
    let url = format!(
        "https://api.mymemory.translated.net/get?q={query}&langpair={from}|{to}"
    );

    let response = minreq::get(&url)
        .with_timeout(25)
        .send()
        .map_err(|err| anyhow!("无法连接翻译服务：{err}"))?;

    if response.status_code != 200 {
        return Err(anyhow!(
            "翻译服务返回 HTTP {}，请稍后重试",
            response.status_code
        ));
    }

    let body = response
        .as_str()
        .map_err(|err| anyhow!("读取翻译结果失败：{err}"))?;
    let json: Value = serde_json::from_str(body)
        .map_err(|err| anyhow!("翻译服务返回了无法解析的数据：{err}"))?;

    let status = json["responseStatus"]
        .as_i64()
        .or_else(|| {
            json["responseStatus"]
                .as_str()
                .and_then(|value| value.parse::<i64>().ok())
        })
        .unwrap_or(200);
    if status != 200 {
        let details = json["responseDetails"]
            .as_str()
            .unwrap_or("翻译服务暂时不可用");
        return Err(anyhow!("{details}"));
    }

    let translated = json["responseData"]["translatedText"]
        .as_str()
        .ok_or_else(|| anyhow!("翻译结果为空"))?;

    if translated.contains("MYMEMORY WARNING") {
        return Err(anyhow!("今天的免费翻译额度已用完，请明天再试"));
    }

    Ok(unescape(translated))
}

fn looks_chinese(text: &str) -> bool {
    let mut cjk = 0usize;
    let mut latin = 0usize;
    for ch in text.chars() {
        if matches!(ch as u32, 0x4E00..=0x9FFF) {
            cjk += 1;
        } else if ch.is_ascii_alphabetic() {
            latin += 1;
        }
    }
    cjk > 0 && cjk * 2 >= latin.max(1)
}

fn chunk(text: &str, max_chars: usize) -> Vec<String> {
    let mut chunks = Vec::new();
    let mut current = String::new();

    for paragraph in text.split_inclusive("\n\n") {
        if current.chars().count() + paragraph.chars().count() <= max_chars {
            current.push_str(paragraph);
            continue;
        }
        if !current.trim().is_empty() {
            chunks.push(std::mem::take(&mut current));
        }
        let mut buffer = String::new();
        for ch in paragraph.chars() {
            buffer.push(ch);
            if buffer.chars().count() >= max_chars {
                chunks.push(std::mem::take(&mut buffer));
            }
        }
        current = buffer;
    }

    if !current.trim().is_empty() {
        chunks.push(current);
    }
    chunks
}

fn unescape(text: &str) -> String {
    let mut out = text
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&apos;", "'")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&nbsp;", " ")
        .replace("&amp;", "&");

    // Decode the handful of numeric entities the service occasionally emits.
    let mut decoded = String::with_capacity(out.len());
    let mut chars = out.char_indices().peekable();
    while let Some((index, ch)) = chars.next() {
        if ch == '&' {
            let rest = &out[index + 1..];
            if let Some(end) = rest.find(';') {
                let entity = &rest[..end];
                let value = if let Some(hex) = entity.strip_prefix("#x").or_else(|| entity.strip_prefix("#X")) {
                    u32::from_str_radix(hex, 16).ok()
                } else if let Some(dec) = entity.strip_prefix('#') {
                    dec.parse::<u32>().ok()
                } else {
                    None
                };
                if let Some(value) = value.and_then(char::from_u32) {
                    decoded.push(value);
                    for _ in 0..=end {
                        chars.next();
                    }
                    continue;
                }
            }
        }
        decoded.push(ch);
    }
    out = decoded;
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_long_text() {
        let text = "一二三四五六七八九十".repeat(100);
        let chunks = chunk(&text, 40);
        assert!(chunks.len() > 1);
        assert!(chunks.iter().all(|chunk| chunk.chars().count() <= 40));
        assert_eq!(chunks.concat(), text);
    }

    #[test]
    fn detects_language() {
        assert!(looks_chinese("这是一封中文邮件"));
        assert!(!looks_chinese("This is an English email"));
    }

    #[test]
    fn decodes_entities() {
        assert_eq!(unescape("a &amp; b &#39;c&#39;"), "a & b 'c'");
    }
}

#[cfg(test)]
mod live {
    use super::*;

    #[test]
    #[ignore = "requires network access"]
    fn live_translation() {
        let translated = translate("Hello, this is a test of the mail client.").expect("network");
        assert!(!translated.text.trim().is_empty());
        println!("{}", translated.text);
    }
}
