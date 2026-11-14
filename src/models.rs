use std::time::{SystemTime, UNIX_EPOCH};

use chrono::Datelike;
use serde::{Deserialize, Serialize};

/// A mail account configured in the app.
#[derive(Clone, Debug)]
pub struct Account {
    pub id: i64,
    pub display_name: String,
    pub email: String,
    pub imap_host: String,
    pub imap_port: u16,
    pub imap_starttls: bool,
    pub smtp_host: String,
    pub smtp_port: u16,
    pub smtp_starttls: bool,
    pub password: String,
    pub is_demo: bool,
}

impl Account {
    pub fn empty() -> Self {
        Self {
            id: 0,
            display_name: "费雪".into(),
            email: "fei.xue@mail.app".into(),
            imap_host: "imap.mail.app".into(),
            imap_port: 993,
            imap_starttls: false,
            smtp_host: "smtp.mail.app".into(),
            smtp_port: 465,
            smtp_starttls: false,
            password: String::new(),
            is_demo: false,
        }
    }

    pub fn avatar_text(&self) -> String {
        let source = if self.display_name.trim().is_empty() {
            &self.email
        } else {
            &self.display_name
        };
        source.chars().take(1).collect::<String>().to_uppercase()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FolderKind {
    Inbox,
    Starred,
    Sent,
    Drafts,
    Archive,
    Trash,
    Junk,
    Custom,
}

impl FolderKind {
    pub fn key(self) -> &'static str {
        match self {
            FolderKind::Inbox => "INBOX",
            FolderKind::Starred => "STARRED",
            FolderKind::Sent => "SENT",
            FolderKind::Drafts => "DRAFTS",
            FolderKind::Archive => "ARCHIVE",
            FolderKind::Trash => "TRASH",
            FolderKind::Junk => "JUNK",
            FolderKind::Custom => "CUSTOM",
        }
    }

    pub fn from_key(key: &str) -> Self {
        match key {
            "INBOX" => FolderKind::Inbox,
            "STARRED" => FolderKind::Starred,
            "SENT" => FolderKind::Sent,
            "DRAFTS" => FolderKind::Drafts,
            "ARCHIVE" => FolderKind::Archive,
            "TRASH" => FolderKind::Trash,
            "JUNK" => FolderKind::Junk,
            _ => FolderKind::Custom,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            FolderKind::Inbox => "收件箱",
            FolderKind::Starred => "星标邮件",
            FolderKind::Sent => "已发送",
            FolderKind::Drafts => "草稿箱",
            FolderKind::Archive => "归档",
            FolderKind::Trash => "已删除",
            FolderKind::Junk => "垃圾邮件",
            FolderKind::Custom => "文件夹",
        }
    }

    pub fn icon(self) -> &'static str {
        match self {
            FolderKind::Inbox => "inbox",
            FolderKind::Starred => "star",
            FolderKind::Sent => "send",
            FolderKind::Drafts => "file",
            FolderKind::Archive => "archive",
            FolderKind::Trash => "trash",
            FolderKind::Junk => "tag",
            FolderKind::Custom => "folder",
        }
    }
}

#[derive(Clone, Debug)]
pub struct Folder {
    pub account_id: i64,
    pub key: String,
    pub name: String,
    pub kind: FolderKind,
    pub unread: usize,
    #[allow(dead_code)]
    pub total: usize,
    /// Actual IMAP mailbox name (e.g. `&XfJT0ZAB-` for NetEase Sent).
    /// `None` means the folder exists only locally.
    pub server_name: Option<String>,
    /// IMAP UIDVALIDITY of the mailbox; 0 until first sync.
    pub uid_validity: u32,
    /// Highest UID already processed by incremental sync.
    pub last_uid: u32,
    /// Lowest server UID fetched so far; 0 means unknown.
    pub oldest_uid: u32,
}

impl Folder {
    pub fn label(&self) -> &str {
        if self.kind == FolderKind::Custom {
            &self.name
        } else {
            self.kind.label()
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Attachment {
    pub name: String,
    pub size: u64,
    pub mime: String,
    #[serde(default)]
    pub path: Option<std::path::PathBuf>,
}

impl Attachment {
    pub fn size_label(&self) -> String {
        human_size(self.size)
    }
}

pub fn human_size(bytes: u64) -> String {
    const UNITS: [&str; 4] = ["B", "KB", "MB", "GB"];
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit < UNITS.len() - 1 {
        value /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{} {}", bytes, UNITS[0])
    } else {
        format!("{:.1} {}", value, UNITS[unit])
    }
}

#[derive(Clone, Debug)]
pub struct Message {
    pub id: i64,
    pub account_id: i64,
    pub folder: String,
    pub uid: u32,
    pub from_name: String,
    pub from_email: String,
    pub to: Vec<String>,
    pub cc: Vec<String>,
    pub bcc: Vec<String>,
    pub subject: String,
    pub preview: String,
    pub body: String,
    pub attachments: Vec<Attachment>,
    pub timestamp: i64,
    pub is_read: bool,
    pub is_starred: bool,
    pub is_draft: bool,
    pub message_id: String,
}

impl Message {
    pub fn sender_label(&self) -> String {
        if self.from_name.trim().is_empty() {
            self.from_email.clone()
        } else {
            self.from_name.clone()
        }
    }

    pub fn subject_or_placeholder(&self) -> &str {
        if self.subject.trim().is_empty() {
            "(无主题)"
        } else {
            &self.subject
        }
    }

    pub fn time_label(&self) -> String {
        let Some(dt) = chrono::DateTime::from_timestamp(self.timestamp, 0)
            .map(|dt| dt.with_timezone(&chrono::Local))
        else {
            return String::new();
        };
        let now = chrono::Local::now();
        let today = now.date_naive();
        let date = dt.date_naive();
        if date == today {
            dt.format("%H:%M").to_string()
        } else if (today - date).num_days() == 1 {
            "昨天".to_string()
        } else if date.year() == today.year() {
            dt.format("%m月%d日").to_string()
        } else {
            dt.format("%Y/%m/%d").to_string()
        }
    }

    pub fn full_date_label(&self) -> String {
        chrono::DateTime::from_timestamp(self.timestamp, 0)
            .map(|dt| {
                dt.with_timezone(&chrono::Local)
                    .format("%Y年%m月%d日 %H:%M")
                    .to_string()
            })
            .unwrap_or_default()
    }

    pub fn has_attachments(&self) -> bool {
        !self.attachments.is_empty()
    }

    pub fn reply_subject(&self) -> String {
        let subject = self.subject_or_placeholder();
        if subject.to_lowercase().starts_with("re:") {
            subject.to_string()
        } else {
            format!("Re: {subject}")
        }
    }

    pub fn forward_subject(&self) -> String {
        let subject = self.subject_or_placeholder();
        if subject.to_lowercase().starts_with("fwd:") {
            subject.to_string()
        } else {
            format!("Fwd: {subject}")
        }
    }
}

pub fn now_ts() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// Trim a string to roughly `max_px` pixels using an estimate per character.
///
/// GPUI lays text out without hard-clipping, so list rows pre-ellipsize instead.
pub fn ellipsize(text: &str, font_size: f32, max_px: f32) -> String {
    let mut out = String::new();
    let mut width = 0.0;
    for ch in text.chars() {
        if ch == '\n' || ch == '\r' || ch == '\t' {
            continue;
        }
        let advance = if ch.is_ascii() {
            font_size * 0.56
        } else {
            font_size
        };
        if width + advance > max_px {
            out.push('…');
            break;
        }
        width += advance;
        out.push(ch);
    }
    out
}

/// All cached mail from one contact, oldest first.
///
/// Used by the "聚合" list mode, where the message list shows one row per
/// contact and the reading pane shows the whole conversation.
#[derive(Clone, Debug)]
pub struct Thread {
    pub key: String,
    pub sender_name: String,
    pub sender_email: String,
    pub messages: Vec<Message>,
}

impl Thread {
    pub fn latest(&self) -> &Message {
        self.messages.last().expect("thread always has a message")
    }

    pub fn count(&self) -> usize {
        self.messages.len()
    }

    pub fn unread(&self) -> usize {
        self.messages.iter().filter(|message| !message.is_read).count()
    }

    pub fn starred(&self) -> bool {
        self.messages.iter().any(|message| message.is_starred)
    }

    pub fn has_attachments(&self) -> bool {
        self.messages.iter().any(|message| message.has_attachments())
    }

    pub fn subject(&self) -> &str {
        self.latest().subject_or_placeholder()
    }
}

pub fn build_threads(messages: &[Message]) -> Vec<Thread> {    use std::collections::HashMap;

    let mut order: Vec<String> = Vec::new();
    let mut grouped: HashMap<String, Vec<Message>> = HashMap::new();
    for message in messages {
        let key = if message.from_email.trim().is_empty() {
            message.sender_label().to_lowercase()
        } else {
            message.from_email.trim().to_lowercase()
        };
        if !grouped.contains_key(&key) {
            order.push(key.clone());
        }
        grouped.entry(key).or_default().push(message.clone());
    }

    let mut threads: Vec<Thread> = order
        .into_iter()
        .filter_map(|key| {
            let mut messages = grouped.remove(&key)?;
            messages.sort_by_key(|message| message.timestamp);
            let latest = messages.last()?;
            Some(Thread {
                key,
                sender_name: latest.sender_label(),
                sender_email: latest.from_email.clone(),
                messages,
            })
        })
        .collect();
    threads.sort_by(|a, b| b.latest().timestamp.cmp(&a.latest().timestamp));
    threads
}

/// Message payload prepared for sending through SMTP.
#[derive(Clone, Debug)]
pub struct Outgoing {
    pub from_name: String,
    pub from_email: String,
    pub to: Vec<String>,
    pub cc: Vec<String>,
    pub bcc: Vec<String>,
    pub subject: String,
    pub body: String,
    pub in_reply_to: Option<String>,
    pub message_id: String,
    pub attachments: Vec<Attachment>,
}

/// Deterministic pleasant colour for an avatar based on an email address.
pub fn avatar_color(seed: &str) -> u32 {
    const PALETTE: [u32; 10] = [
        0x6E8BFF, 0x9B7BFF, 0xF2779C, 0xF0A05A, 0x3FB6A8, 0x4FA8E8, 0x86B94A, 0xD96BA8, 0x8C7BF7,
        0x5BB9C9,
    ];
    let mut hash: u32 = 2166136261;
    for byte in seed.bytes() {
        hash ^= byte as u32;
        hash = hash.wrapping_mul(16777619);
    }
    PALETTE[(hash as usize) % PALETTE.len()]
}


#[cfg(test)]
mod tests {
    use super::*;

    fn message(id: i64, email: &str, name: &str, ts: i64) -> Message {
        Message {
            id,
            account_id: 1,
            folder: "INBOX".into(),
            uid: id as u32,
            from_name: name.into(),
            from_email: email.into(),
            to: vec!["me@mail.app".into()],
            cc: vec![],
            bcc: vec![],
            subject: format!("subject {id}"),
            preview: "preview".into(),
            body: "body".into(),
            attachments: vec![],
            timestamp: ts,
            is_read: false,
            is_starred: false,
            is_draft: false,
            message_id: format!("<{id}@mail.app>"),
        }
    }

    #[test]
    fn groups_messages_by_sender() {
        let messages = vec![
            message(1, "lin@mail.app", "林晓", 100),
            message(2, "other@mail.app", "Other", 200),
            message(3, "lin@mail.app", "林晓", 300),
        ];
        let threads = build_threads(&messages);
        assert_eq!(threads.len(), 2);
        assert_eq!(threads[0].sender_email, "lin@mail.app");
        assert_eq!(threads[0].count(), 2);
        assert_eq!(threads[0].latest().id, 3);
        assert_eq!(threads[0].messages[0].id, 1);
        assert_eq!(threads[1].sender_email, "other@mail.app");
    }

    #[test]
    fn ellipsize_respects_width() {
        let short = ellipsize("hello", 12.0, 100.0);
        assert_eq!(short, "hello");
        let long = ellipsize("这是一封很长的中文主题行", 12.0, 60.0);
        assert!(long.ends_with('…'));
        assert!(long.chars().count() < 12);
    }
}
