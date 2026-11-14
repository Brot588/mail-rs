use anyhow::{anyhow, Result};
use imap::types::{Flag, NameAttribute};
use native_tls::TlsStream;
use std::net::{TcpStream, ToSocketAddrs};
use std::time::Duration;

use crate::mail::parse::parse_raw;
use crate::models::{now_ts, Account, FolderKind, Message};

type Session = imap::Session<TlsStream<TcpStream>>;

/// A server mailbox that maps to one of the app's standard local folders.
#[derive(Clone, Debug)]
pub struct RemoteFolder {
    pub key: String,
    pub kind: FolderKind,
    pub server_name: String,
}

#[derive(Clone, Debug)]
pub struct FolderState {
    pub key: String,
    pub uid_validity: u32,
    pub last_uid: u32,
    pub oldest_uid: u32,
    pub cached_uids: Vec<u32>,
}

#[derive(Clone, Debug)]
pub struct FolderSync {
    pub key: String,
    pub server_name: String,
    pub uid_validity: u32,
    pub last_uid: u32,
    pub oldest_uid: u32,
    pub total: usize,
    pub reset: bool,
    pub messages: Vec<Message>,
    /// Flags for messages that were already cached locally.
    pub flags: Vec<(u32, bool, bool)>,
    /// Every UID currently on the server, used to prune deleted messages.
    pub server_uids: Vec<u32>,
}

pub struct AccountSync {
    pub folders: Vec<FolderSync>,
}

fn has_attribute(attributes: &[NameAttribute<'_>], wanted: &str) -> bool {
    attributes.iter().any(|attribute| match attribute {
        NameAttribute::Custom(value) => value.eq_ignore_ascii_case(wanted),
        _ => false,
    })
}

fn classify_mailbox(name: &str, attributes: &[NameAttribute<'_>]) -> Option<FolderKind> {
    if attributes
        .iter()
        .any(|attribute| matches!(attribute, NameAttribute::NoSelect))
    {
        return None;
    }
    let lower = name.to_ascii_lowercase();
    if lower == "inbox" || has_attribute(attributes, "\\Inbox") {
        Some(FolderKind::Inbox)
    } else if has_attribute(attributes, "\\Sent") || lower.contains("sent") {
        Some(FolderKind::Sent)
    } else if has_attribute(attributes, "\\Drafts") || lower.contains("draft") {
        Some(FolderKind::Drafts)
    } else if has_attribute(attributes, "\\Trash")
        || lower.contains("trash")
        || lower.contains("deleted")
        || name.contains("已删除")
    {
        Some(FolderKind::Trash)
    } else if has_attribute(attributes, "\\Junk")
        || lower.contains("junk")
        || lower.contains("spam")
        || name.contains("垃圾")
    {
        Some(FolderKind::Junk)
    } else if has_attribute(attributes, "\\Archive")
        || lower.contains("archive")
        || name.contains("归档")
    {
        Some(FolderKind::Archive)
    } else {
        None
    }
}

fn fetch_flags(session: &mut Session, uids: &[u32]) -> Vec<(u32, bool, bool)> {
    if uids.is_empty() {
        return Vec::new();
    }
    let set = uids
        .iter()
        .map(|uid| uid.to_string())
        .collect::<Vec<_>>()
        .join(",");
    let Ok(fetches) = session.uid_fetch(&set, "(UID FLAGS)") else {
        return Vec::new();
    };
    let mut flags = Vec::new();
    for fetch in fetches.iter() {
        let Some(uid) = fetch.uid else {
            continue;
        };
        let read = fetch.flags().iter().any(|flag| matches!(flag, Flag::Seen));
        let starred = fetch
            .flags()
            .iter()
            .any(|flag| matches!(flag, Flag::Flagged));
        flags.push((uid, read, starred));
    }
    flags
}

fn fetch_messages(
    session: &mut Session,
    uids: &[u32],
    account_id: i64,
    folder_key: &str,
) -> Vec<Message> {
    if uids.is_empty() {
        return Vec::new();
    }
    let set = uids
        .iter()
        .map(|uid| uid.to_string())
        .collect::<Vec<_>>()
        .join(",");
    let Ok(fetches) = session.uid_fetch(&set, "(UID FLAGS INTERNALDATE BODY.PEEK[])") else {
        return Vec::new();
    };
    let mut messages = Vec::new();
    for fetch in fetches.iter() {
        let uid = fetch.uid.unwrap_or(0);
        let is_read = fetch.flags().iter().any(|flag| matches!(flag, Flag::Seen));
        let is_starred = fetch
            .flags()
            .iter()
            .any(|flag| matches!(flag, Flag::Flagged));
        let fallback = fetch
            .internal_date()
            .map(|date| date.timestamp())
            .unwrap_or_else(now_ts);
        if let Some(raw) = fetch.body() {
            if let Some(message) = parse_raw(
                raw,
                account_id,
                folder_key,
                uid,
                is_read,
                is_starred,
                fallback,
            ) {
                messages.push(message);
            }
        }
    }
    messages
}

/// Synchronise every standard folder (INBOX/Sent/Drafts/Trash/Junk/Archive)
/// with one IMAP connection.
pub fn sync_account(
    account: &Account,
    states: &[FolderState],
    batch_limit: usize,
) -> Result<AccountSync> {
    let mut session = login(account)?;
    let names = session.list(None, Some("*"))?;
    let mut discovered: Vec<RemoteFolder> = Vec::new();
    for name in names.iter() {
        let Some(kind) = classify_mailbox(name.name(), name.attributes()) else {
            continue;
        };
        let key = kind.key().to_string();
        if discovered.iter().any(|folder| folder.key == key) {
            continue;
        }
        discovered.push(RemoteFolder {
            key,
            kind,
            server_name: name.name().to_string(),
        });
    }
    if !discovered.iter().any(|folder| folder.key == "INBOX") {
        discovered.insert(
            0,
            RemoteFolder {
                key: "INBOX".to_string(),
                kind: FolderKind::Inbox,
                server_name: "INBOX".to_string(),
            },
        );
    }
    discovered.sort_by_key(|folder| match folder.kind {
        FolderKind::Inbox => 0,
        FolderKind::Starred => 1,
        FolderKind::Sent => 2,
        FolderKind::Drafts => 3,
        FolderKind::Archive => 4,
        FolderKind::Junk => 5,
        FolderKind::Trash => 6,
        FolderKind::Custom => 7,
    });

    let mut folders = Vec::new();
    for remote in &discovered {
        let state = states.iter().find(|state| state.key == remote.key);
        let Ok(mailbox) = session.select(&remote.server_name) else {
            continue;
        };
        let uid_validity = mailbox.uid_validity.unwrap_or(0);
        let mut server_uids: Vec<u32> = session
            .uid_search("ALL")
            .map(|set| set.into_iter().collect())
            .unwrap_or_default();
        server_uids.sort_unstable();
        let reset = state
            .map(|state| state.uid_validity == 0 || state.uid_validity != uid_validity)
            .unwrap_or(true);
        let fetch_uids: Vec<u32> = if reset {
            server_uids
                .iter()
                .rev()
                .take(batch_limit)
                .rev()
                .copied()
                .collect()
        } else {
            let last_uid = state.map(|state| state.last_uid).unwrap_or(0);
            server_uids
                .iter()
                .filter(|uid| **uid > last_uid)
                .take(batch_limit)
                .copied()
                .collect()
        };
        let flags = if reset {
            Vec::new()
        } else {
            fetch_flags(
                &mut session,
                &state.map(|state| state.cached_uids.clone()).unwrap_or_default(),
            )
        };
        let messages = fetch_messages(
            &mut session,
            &fetch_uids,
            account.id,
            &remote.key,
        );
        let last_uid = if reset {
            server_uids.last().copied().unwrap_or(0)
        } else {
            fetch_uids
                .iter()
                .max()
                .copied()
                .unwrap_or_else(|| state.map(|state| state.last_uid).unwrap_or(0))
        };
        let oldest_uid = if reset {
            fetch_uids.first().copied().unwrap_or(0)
        } else {
            state
                .map(|state| state.oldest_uid)
                .filter(|uid| *uid > 0)
                .unwrap_or_else(|| fetch_uids.first().copied().unwrap_or(0))
        };
        folders.push(FolderSync {
            key: remote.key.clone(),
            server_name: remote.server_name.clone(),
            uid_validity,
            last_uid,
            oldest_uid,
            total: server_uids.len(),
            reset,
            messages,
            flags,
            server_uids,
        });
    }
    let _ = session.logout();
    Ok(AccountSync { folders })
}

fn connect(account: &Account) -> Result<imap::Client<TlsStream<TcpStream>>> {
    let address = format!("{}:{}", account.imap_host, account.imap_port)
        .to_socket_addrs()
        .map_err(|err| anyhow!("无法解析 IMAP 服务器地址：{err}"))?
        .next()
        .ok_or_else(|| anyhow!("无法解析 IMAP 服务器地址"))?;
    let tcp = TcpStream::connect_timeout(&address, Duration::from_secs(15))
        .map_err(|err| anyhow!("连接 IMAP 服务器失败：{err}"))?;
    tcp.set_read_timeout(Some(Duration::from_secs(90))).ok();
    tcp.set_write_timeout(Some(Duration::from_secs(30))).ok();

    let tls = native_tls::TlsConnector::builder()
        .build()
        .map_err(|err| anyhow!("初始化 TLS 失败：{err}"))?;

    if account.imap_starttls {
        let mut client = imap::Client::new(tcp);
        client
            .read_greeting()
            .map_err(|err| anyhow!("读取 IMAP 服务器问候失败：{err}"))?;
        client
            .secure(account.imap_host.as_str(), &tls)
            .map_err(|err| anyhow!("STARTTLS 握手失败：{err}"))
    } else {
        let stream = tls
            .connect(account.imap_host.as_str(), tcp)
            .map_err(|err| anyhow!("TLS 握手失败：{err}"))?;
        let mut client = imap::Client::new(stream);
        client
            .read_greeting()
            .map_err(|err| anyhow!("读取 IMAP 服务器问候失败：{err}"))?;
        Ok(client)
    }
}

fn login(account: &Account) -> Result<Session> {
    let client = connect(account)?;
    let mut session = client
        .login(&account.email, &account.password)
        .map_err(|(err, _)| anyhow!("登录失败：{err}（请确认邮箱地址与授权码）"))?;

    // NetEase mailboxes (163/126/yeah.net) reject IMAP clients that do not
    // identify themselves with the non-standard ID command.
    let host = account.imap_host.to_ascii_lowercase();
    if host.contains("163.com") || host.contains("126.com") || host.contains("yeah.net") {
        let _ = session.run_command_and_check_ok(
            "ID (\"name\" \"mail\" \"version\" \"0.2.0\" \"vendor\" \"mail\")",
        );
    }
    Ok(session)
}

/// Fetch one page of messages older than `before_uid` from a server folder.
pub fn fetch_older(
    account: &Account,
    folder_key: &str,
    server_name: &str,
    before_uid: u32,
    limit: usize,
) -> Result<(Vec<Message>, u32)> {
    let mut session = login(account)?;
    session.select(server_name)?;
    let mut uids: Vec<u32> = session
        .uid_search("ALL")
        .map(|set| set.into_iter().collect())
        .unwrap_or_default();
    uids.sort_unstable();
    let selected: Vec<u32> = if before_uid == 0 {
        uids.iter().rev().take(limit).rev().copied().collect()
    } else {
        let mut older: Vec<u32> = uids
            .iter()
            .copied()
            .filter(|uid| *uid < before_uid)
            .collect();
        let keep_from = older.len().saturating_sub(limit);
        older.split_off(keep_from)
    };
    let messages = fetch_messages(&mut session, &selected, account.id, folder_key);
    let oldest = selected.first().copied().unwrap_or(0);
    let _ = session.logout();
    Ok((messages, oldest))
}

/// Append a locally composed RFC 822 message to a server mailbox.
pub fn append_message(
    account: &Account,
    mailbox: &str,
    raw: &[u8],
    draft: bool,
) -> Result<()> {
    let mut session = login(account)?;
    let mut flags = vec![Flag::Seen];
    if draft {
        flags.push(Flag::Draft);
    }
    session.append_with_flags(mailbox, raw, &flags)?;
    let _ = session.logout();
    Ok(())
}

/// Move one message to another server mailbox. Falls back to COPY + DELETE
/// for providers that do not implement the MOVE extension (e.g. some 163
/// accounts).
pub fn move_remote(
    account: &Account,
    source_mailbox: &str,
    uid: u32,
    target_mailbox: &str,
) -> Result<()> {
    let mut session = login(account)?;
    session.select(source_mailbox)?;
    let uid_set = uid.to_string();
    if session.uid_mv(&uid_set, target_mailbox).is_err() {
        session.uid_copy(&uid_set, target_mailbox)?;
        session.uid_store(&uid_set, "+FLAGS.SILENT (\\Deleted)")?;
        if session.uid_expunge(&uid_set).is_err() {
            let _ = session.expunge();
        }
    }
    let _ = session.logout();
    Ok(())
}

/// Delete any message in `mailbox` whose Message-ID matches.
pub fn delete_by_message_id(
    account: &Account,
    mailbox: &str,
    message_id: &str,
) -> Result<()> {
    if message_id.trim().is_empty() {
        return Ok(());
    }
    let mut session = login(account)?;
    session.select(mailbox)?;
    let query = format!(
        "HEADER Message-ID \"{}\"",
        message_id.replace('"', "")
    );
    let uids = session.uid_search(query)?;
    if uids.is_empty() {
        let _ = session.logout();
        return Ok(());
    }
    let uid_set = uids
        .iter()
        .map(|uid| uid.to_string())
        .collect::<Vec<_>>()
        .join(",");
    session.uid_store(&uid_set, "+FLAGS.SILENT (\\Deleted)")?;
    if session.uid_expunge(&uid_set).is_err() {
        let _ = session.expunge();
    }
    let _ = session.logout();
    Ok(())
}

/// Permanently remove one message from a server mailbox.
pub fn delete_remote(account: &Account, mailbox: &str, uid: u32) -> Result<()> {
    let mut session = login(account)?;
    session.select(mailbox)?;
    let uid_set = uid.to_string();
    session.uid_store(&uid_set, "+FLAGS.SILENT (\\Deleted)")?;
    if session.uid_expunge(&uid_set).is_err() {
        let _ = session.expunge();
    }
    let _ = session.logout();
    Ok(())
}

/// Update the Flagged flag on several messages with one connection.
pub fn set_flagged_batch(
    account: &Account,
    mailbox: &str,
    uids: &[u32],
    flagged: bool,
) -> Result<()> {
    if uids.is_empty() {
        return Ok(());
    }
    let mut session = login(account)?;
    session.select(mailbox)?;
    let op = if flagged { "+" } else { "-" };
    for uid in uids {
        session.uid_store(uid.to_string(), format!("{op}FLAGS.SILENT (\\Flagged)"))?;
    }
    let _ = session.logout();
    Ok(())
}

/// Update the Seen flag on several messages with one connection.
pub fn set_seen_batch(account: &Account, mailbox: &str, uids: &[u32], seen: bool) -> Result<()> {
    if uids.is_empty() {
        return Ok(());
    }
    let mut session = login(account)?;
    session.select(mailbox)?;
    let op = if seen { "+" } else { "-" };
    for uid in uids {
        session.uid_store(uid.to_string(), format!("{op}FLAGS.SILENT (\\Seen)"))?;
    }
    let _ = session.logout();
    Ok(())
}

/// Update a system flag (Seen/Flagged) on the server. Best effort.
pub fn set_system_flag(
    account: &Account,
    mailbox: &str,
    uid: u32,
    seen: Option<bool>,
    flagged: Option<bool>,
) -> Result<()> {
    let mut session = login(account)?;
    session.select(mailbox)?;
    if let Some(seen) = seen {
        let op = if seen { "+" } else { "-" };
        session.uid_store(uid.to_string(), format!("{op}FLAGS.SILENT (\\Seen)"))?;
    }
    if let Some(flagged) = flagged {
        let op = if flagged { "+" } else { "-" };
        session.uid_store(uid.to_string(), format!("{op}FLAGS.SILENT (\\Flagged)"))?;
    }
    let _ = session.logout();
    Ok(())
}




#[cfg(test)]
mod tests {
    use super::classify_mailbox;
    use crate::models::FolderKind;
    use imap::types::NameAttribute;
    use std::borrow::Cow;

    #[test]
    fn classifies_standard_mailbox_names() {
        assert_eq!(classify_mailbox("INBOX", &[]), Some(FolderKind::Inbox));
        assert_eq!(
            classify_mailbox(
                "&XfJT0ZAB-",
                &[NameAttribute::Custom(Cow::Borrowed("\\Sent"))]
            ),
            Some(FolderKind::Sent)
        );
        assert_eq!(
            classify_mailbox("Drafts", &[]),
            Some(FolderKind::Drafts)
        );
        assert_eq!(
            classify_mailbox("Deleted Messages", &[]),
            Some(FolderKind::Trash)
        );
        assert_eq!(
            classify_mailbox(
                "Junk",
                &[NameAttribute::Custom(Cow::Borrowed("\\Junk"))]
            ),
            Some(FolderKind::Junk)
        );
        assert_eq!(
            classify_mailbox(
                "Archive",
                &[NameAttribute::Custom(Cow::Borrowed("\\Archive"))]
            ),
            Some(FolderKind::Archive)
        );
        assert_eq!(classify_mailbox("Some Custom", &[]), None);
        assert_eq!(
            classify_mailbox("Parent", &[NameAttribute::NoSelect]),
            None
        );
    }
}
