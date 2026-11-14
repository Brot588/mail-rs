use std::path::PathBuf;

use mail_parser::{MessageParser, MimeHeaders};

use crate::models::{Attachment, Message};
use crate::store;

/// Parse an RFC 822 message into the app's [`Message`] shape.
pub fn parse_raw(
    raw: &[u8],
    account_id: i64,
    folder: &str,
    uid: u32,
    is_read: bool,
    is_starred: bool,
    fallback_ts: i64,
) -> Option<Message> {
    let parser = MessageParser::default();
    let parsed = parser.parse(raw)?;

    let (from_name, from_email) = parsed
        .from()
        .and_then(|address| address.first())
        .map(|addr| {
            (
                addr.name().unwrap_or_default().to_string(),
                addr.address().unwrap_or_default().to_string(),
            )
        })
        .unwrap_or_default();

    let collect_addrs = |address: Option<&mail_parser::Address<'_>>| -> Vec<String> {
        address
            .map(|address| {
                address
                    .iter()
                    .filter_map(|addr| {
                        let email = addr.address()?;
                        Some(match addr.name() {
                            Some(name) if !name.is_empty() => format!("{name} <{email}>"),
                            _ => email.to_string(),
                        })
                    })
                    .collect()
            })
            .unwrap_or_default()
    };

    let mut body = parsed
        .body_text(0)
        .map(|text| text.into_owned())
        .unwrap_or_default();

    if body.trim().is_empty() {
        if let Some(html) = parsed.body_html(0) {
            body = html2text::from_read(html.as_bytes(), 92).unwrap_or_default();
        }
    }

    let body = normalize_body(&body);
    let preview = make_preview(&body, 150);

    let attachments = parsed
        .attachments()
        .enumerate()
        .map(|(index, part)| {
            let name = part
                .attachment_name()
                .filter(|name| !name.trim().is_empty())
                .map(|name| name.to_string())
                .unwrap_or_else(|| format!("附件 {}", index + 1));
            let contents = part.contents();
            let mime = part
                .content_type()
                .map(|content_type| {
                    let subtype = content_type.c_subtype.as_deref().unwrap_or("octet-stream");
                    format!("{}/{}", content_type.c_type, subtype)
                })
                .unwrap_or_else(|| "application/octet-stream".to_string());
            let mut attachment = Attachment {
                name,
                size: contents.len() as u64,
                mime,
                path: None,
            };
            // Cache small attachments on disk so they can be saved or opened later.
            if contents.len() <= 8 * 1024 * 1024 && !contents.is_empty() {
                let path: PathBuf = store::attachment_store_path(account_id, folder, uid, &attachment.name);
                if std::fs::write(&path, contents).is_ok() {
                    attachment.path = Some(path);
                }
            }
            attachment
        })
        .collect::<Vec<_>>();

    let timestamp = parsed
        .date()
        .map(|date| date.to_timestamp())
        .filter(|ts| *ts > 0)
        .unwrap_or(fallback_ts);

    Some(Message {
        id: 0,
        account_id,
        folder: folder.to_string(),
        uid,
        from_name,
        from_email,
        to: collect_addrs(parsed.to()),
        cc: collect_addrs(parsed.cc()),
        bcc: collect_addrs(parsed.bcc()),
        subject: parsed.subject().unwrap_or_default().to_string(),
        preview,
        body,
        attachments,
        timestamp,
        is_read,
        is_starred,
        is_draft: folder == "DRAFTS",
        message_id: parsed.message_id().unwrap_or_default().to_string(),
    })
}

fn normalize_body(text: &str) -> String {
    let normalized = text.replace("\r\n", "\n").replace('\r', "\n");
    let mut result = String::with_capacity(normalized.len());
    let mut blank_run = 0;
    for line in normalized.split('\n') {
        let trimmed = line.trim_end();
        if trimmed.is_empty() {
            blank_run += 1;
            if blank_run > 1 {
                continue;
            }
        } else {
            blank_run = 0;
        }
        result.push_str(trimmed);
        result.push('\n');
    }
    result.trim().to_string()
}

fn make_preview(body: &str, max_chars: usize) -> String {
    let collapsed = body.split_whitespace().collect::<Vec<_>>().join(" ");
    let mut preview = String::new();
    for ch in collapsed.chars() {
        if preview.chars().count() >= max_chars {
            preview.push('…');
            break;
        }
        preview.push(ch);
    }
    preview
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &[u8] = b"From: =?UTF-8?B?5rWL6K+V?= <test@example.com>\r\n\
To: me@example.com\r\n\
Subject: =?UTF-8?B?5L2g5aW977yM5LiW55WM?=\r\n\
Date: Fri, 13 Nov 2026 08:00:00 +0800\r\n\
MIME-Version: 1.0\r\n\
Content-Type: text/plain; charset=utf-8\r\n\
\r\n\
Hello from mail.\r\n";

    #[test]
    fn parses_headers_and_body() {
        let message = parse_raw(SAMPLE, 7, "INBOX", 42, false, true, 1).expect("parsed");
        assert_eq!(message.from_email, "test@example.com");
        assert_eq!(message.from_name, "测试");
        assert_eq!(message.subject, "你好，世界");
        assert!(message.body.contains("mail."));
        assert!(message.is_starred);
        assert!(!message.is_read);
        assert_eq!(message.uid, 42);
        assert!(message.timestamp > 0);
    }

    #[test]
    fn collapses_blank_lines() {
        let normalized = normalize_body("a\r\n\r\n\r\n\r\nb\r\n");
        assert_eq!(normalized, "a\n\nb");
    }
}


