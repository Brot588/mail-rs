use std::path::{Path, PathBuf};

use anyhow::Result;
use rusqlite::{params, Connection, OptionalExtension};

use crate::models::{Account, Attachment, Folder, FolderKind, Message};

pub struct Store {
    conn: Connection,
}

impl Store {
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref().to_path_buf();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).ok();
        }
        let conn = Connection::open(&path)?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "foreign_keys", "ON")?;
        conn.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS accounts (
                id          INTEGER PRIMARY KEY AUTOINCREMENT,
                display_name TEXT NOT NULL DEFAULT '',
                email       TEXT NOT NULL UNIQUE,
                imap_host   TEXT NOT NULL DEFAULT '',
                imap_port   INTEGER NOT NULL DEFAULT 993,
                imap_starttls INTEGER NOT NULL DEFAULT 0,
                smtp_host   TEXT NOT NULL DEFAULT '',
                smtp_port   INTEGER NOT NULL DEFAULT 465,
                smtp_starttls INTEGER NOT NULL DEFAULT 0,
                password    TEXT NOT NULL DEFAULT '',
                is_demo     INTEGER NOT NULL DEFAULT 0
            );

            CREATE TABLE IF NOT EXISTS folders (
                account_id INTEGER NOT NULL,
                key        TEXT NOT NULL,
                name       TEXT NOT NULL,
                kind       TEXT NOT NULL,
                server_name TEXT,
                uid_validity INTEGER NOT NULL DEFAULT 0,
                last_uid   INTEGER NOT NULL DEFAULT 0,
                oldest_uid INTEGER NOT NULL DEFAULT 0,
                PRIMARY KEY (account_id, key)
            );

            CREATE TABLE IF NOT EXISTS messages (
                id          INTEGER PRIMARY KEY AUTOINCREMENT,
                account_id  INTEGER NOT NULL,
                folder      TEXT NOT NULL,
                uid         INTEGER NOT NULL,
                from_name   TEXT NOT NULL DEFAULT '',
                from_email  TEXT NOT NULL DEFAULT '',
                to_addrs    TEXT NOT NULL DEFAULT '',
                cc_addrs    TEXT NOT NULL DEFAULT '',
                bcc_addrs   TEXT NOT NULL DEFAULT '',
                subject     TEXT NOT NULL DEFAULT '',
                preview     TEXT NOT NULL DEFAULT '',
                body        TEXT NOT NULL DEFAULT '',
                attachments TEXT NOT NULL DEFAULT '[]',
                ts          INTEGER NOT NULL,
                is_read     INTEGER NOT NULL DEFAULT 0,
                is_starred  INTEGER NOT NULL DEFAULT 0,
                is_draft    INTEGER NOT NULL DEFAULT 0,
                message_id  TEXT NOT NULL DEFAULT '',
                UNIQUE(account_id, folder, uid)
            );

            CREATE INDEX IF NOT EXISTS idx_messages_folder
                ON messages(account_id, folder, ts DESC);

            CREATE INDEX IF NOT EXISTS idx_messages_message_id
                ON messages(account_id, message_id);

            CREATE TABLE IF NOT EXISTS settings (
                key   TEXT PRIMARY KEY,
                value TEXT NOT NULL
            );
            "#,
        )?;
        // Older databases predate the IMAP folder metadata columns.
        ensure_column(&conn, "folders", "server_name", "TEXT")?;
        ensure_column(
            &conn,
            "folders",
            "uid_validity",
            "INTEGER NOT NULL DEFAULT 0",
        )?;
        ensure_column(&conn, "folders", "last_uid", "INTEGER NOT NULL DEFAULT 0")?;
        ensure_column(&conn, "folders", "oldest_uid", "INTEGER NOT NULL DEFAULT 0")?;
        ensure_column(&conn, "messages", "bcc_addrs", "TEXT NOT NULL DEFAULT ''")?;
        let _ = path;
        Ok(Self { conn })
    }

    pub fn purge_demo_accounts(&self) -> Result<()> {
        self.conn.execute(
            "DELETE FROM messages WHERE account_id IN (SELECT id FROM accounts WHERE is_demo=1)",
            [],
        )?;
        self.conn.execute(
            "DELETE FROM folders WHERE account_id IN (SELECT id FROM accounts WHERE is_demo=1)",
            [],
        )?;
        self.conn.execute("DELETE FROM accounts WHERE is_demo=1", [])?;
        Ok(())
    }

    pub fn accounts(&self) -> Result<Vec<Account>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, display_name, email, imap_host, imap_port, imap_starttls,
                    smtp_host, smtp_port, smtp_starttls, password, is_demo
             FROM accounts ORDER BY is_demo DESC, id ASC",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(Account {
                id: row.get(0)?,
                display_name: row.get(1)?,
                email: row.get(2)?,
                imap_host: row.get(3)?,
                imap_port: row.get::<_, i64>(4)? as u16,
                imap_starttls: row.get::<_, i32>(5)? != 0,
                smtp_host: row.get(6)?,
                smtp_port: row.get::<_, i64>(7)? as u16,
                smtp_starttls: row.get::<_, i32>(8)? != 0,
                password: row.get(9)?,
                is_demo: row.get::<_, i32>(10)? != 0,
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }

    pub fn save_account(&self, account: &Account) -> Result<i64> {
        if account.id > 0 {
            self.conn.execute(
                "UPDATE accounts SET display_name=?1, email=?2, imap_host=?3, imap_port=?4,
                    imap_starttls=?5, smtp_host=?6, smtp_port=?7, smtp_starttls=?8, password=?9,
                    is_demo=?10
                 WHERE id=?11",
                params![
                    account.display_name,
                    account.email,
                    account.imap_host,
                    account.imap_port,
                    account.imap_starttls as i32,
                    account.smtp_host,
                    account.smtp_port,
                    account.smtp_starttls as i32,
                    account.password,
                    account.is_demo as i32,
                    account.id,
                ],
            )?;
            Ok(account.id)
        } else {
            self.conn.execute(
                "INSERT INTO accounts
                 (display_name, email, imap_host, imap_port, imap_starttls,
                  smtp_host, smtp_port, smtp_starttls, password, is_demo)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
                params![
                    account.display_name,
                    account.email,
                    account.imap_host,
                    account.imap_port,
                    account.imap_starttls as i32,
                    account.smtp_host,
                    account.smtp_port,
                    account.smtp_starttls as i32,
                    account.password,
                    account.is_demo as i32,
                ],
            )?;
            Ok(self.conn.last_insert_rowid())
        }
    }

    pub fn delete_account(&self, id: i64) -> Result<()> {
        self.conn
            .execute("DELETE FROM messages WHERE account_id = ?1", params![id])?;
        self.conn
            .execute("DELETE FROM folders WHERE account_id = ?1", params![id])?;
        self.conn
            .execute("DELETE FROM accounts WHERE id = ?1", params![id])?;
        Ok(())
    }

    pub fn save_folder(&self, folder: &Folder) -> Result<()> {
        self.conn.execute(
            "INSERT INTO folders
                (account_id, key, name, kind, server_name, uid_validity, last_uid, oldest_uid)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
             ON CONFLICT(account_id, key) DO UPDATE SET
                name=excluded.name, kind=excluded.kind,
                server_name=excluded.server_name,
                uid_validity=excluded.uid_validity,
                last_uid=excluded.last_uid,
                oldest_uid=excluded.oldest_uid",
            params![
                folder.account_id,
                folder.key,
                folder.name,
                folder.kind.key(),
                folder.server_name,
                folder.uid_validity as i64,
                folder.last_uid as i64,
                folder.oldest_uid as i64,
            ],
        )?;
        Ok(())
    }

    pub fn folders(&self, account_id: i64) -> Result<Vec<Folder>> {
        let mut stmt = self.conn.prepare(
            "SELECT f.key, f.name, f.kind, f.server_name, f.uid_validity, f.last_uid,
                    f.oldest_uid,
                    COALESCE(SUM(CASE WHEN m.is_read = 0 THEN 1 ELSE 0 END), 0) AS unread,
                    COUNT(m.id) AS total
             FROM folders f
             LEFT JOIN messages m
               ON m.account_id = f.account_id
              AND (m.folder = f.key OR f.key = 'STARRED')
              AND (f.key != 'STARRED' OR m.is_starred = 1)
             WHERE f.account_id = ?1
             GROUP BY f.key
             ORDER BY CASE f.kind
                        WHEN 'INBOX' THEN 0 WHEN 'STARRED' THEN 1 WHEN 'SENT' THEN 2
                        WHEN 'DRAFTS' THEN 3 WHEN 'ARCHIVE' THEN 4
                        WHEN 'JUNK' THEN 5 WHEN 'TRASH' THEN 6 ELSE 7 END",
        )?;
        let rows = stmt.query_map(params![account_id], |row| {
            let key: String = row.get(0)?;
            let name: String = row.get(1)?;
            let kind: String = row.get(2)?;
            Ok(Folder {
                account_id,
                key,
                name,
                kind: FolderKind::from_key(&kind),
                server_name: row.get(3)?,
                uid_validity: row.get::<_, i64>(4)? as u32,
                last_uid: row.get::<_, i64>(5)? as u32,
                oldest_uid: row.get::<_, i64>(6)? as u32,
                unread: row.get::<_, i64>(7)? as usize,
                total: row.get::<_, i64>(8)? as usize,
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }

    pub fn contacts(
        &self,
        account_id: i64,
        query: &str,
        limit: usize,
    ) -> Result<Vec<(String, String)>> {
        let pattern = format!("%{}%", query.trim());
        let mut stmt = self.conn.prepare(
            "SELECT from_name, from_email FROM messages
             WHERE account_id=?1 AND from_email != ''
               AND (from_name LIKE ?2 OR from_email LIKE ?2)
             GROUP BY lower(from_email)
             ORDER BY MAX(ts) DESC LIMIT ?3",
        )?;
        let rows = stmt.query_map(params![account_id, pattern, limit as i64], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }

    pub fn search_messages(
        &self,
        account_id: i64,
        query: &str,
        limit: usize,
    ) -> Result<Vec<Message>> {
        const COLUMNS: &str = "id, account_id, folder, uid, from_name, from_email, to_addrs,
                               cc_addrs, bcc_addrs, subject, preview, body, attachments, ts,
                               is_read, is_starred, is_draft, message_id";
        let pattern = format!("%{}%", query.trim());
        let mut stmt = self.conn.prepare(&format!(
            "SELECT {COLUMNS} FROM messages
             WHERE account_id = ?1 AND (
                subject LIKE ?2 OR from_name LIKE ?2 OR from_email LIKE ?2
                OR to_addrs LIKE ?2 OR cc_addrs LIKE ?2 OR bcc_addrs LIKE ?2
                OR preview LIKE ?2 OR body LIKE ?2
             )
             ORDER BY ts DESC LIMIT ?3"
        ))?;
        let rows = stmt.query_map(
            params![account_id, pattern, limit as i64],
            row_to_message,
        )?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }

    #[allow(dead_code)]
    pub fn messages(&self, account_id: i64, folder: &str, limit: usize) -> Result<Vec<Message>> {
        self.messages_page(account_id, folder, limit, 0)
    }

    pub fn messages_page(
        &self,
        account_id: i64,
        folder: &str,
        limit: usize,
        offset: usize,
    ) -> Result<Vec<Message>> {
        const COLUMNS: &str = "id, account_id, folder, uid, from_name, from_email, to_addrs,
                               cc_addrs, bcc_addrs, subject, preview, body, attachments, ts,
                               is_read, is_starred, is_draft, message_id";
        let mut result = Vec::new();
        if folder == "STARRED" {
            let mut stmt = self.conn.prepare(&format!(
                "SELECT {COLUMNS} FROM messages
                 WHERE account_id = ?1 AND is_starred = 1
                 ORDER BY ts DESC LIMIT ?2 OFFSET ?3"
            ))?;
            let rows = stmt.query_map(
                params![account_id, limit as i64, offset as i64],
                row_to_message,
            )?;
            result.extend(rows.collect::<rusqlite::Result<Vec<_>>>()?);
        } else {
            let mut stmt = self.conn.prepare(&format!(
                "SELECT {COLUMNS} FROM messages
                 WHERE account_id = ?1 AND folder = ?2
                 ORDER BY ts DESC LIMIT ?3 OFFSET ?4"
            ))?;
            let rows = stmt.query_map(
                params![account_id, folder, limit as i64, offset as i64],
                row_to_message,
            )?;
            result.extend(rows.collect::<rusqlite::Result<Vec<_>>>()?);
        }
        Ok(result)
    }

    pub fn upsert(&self, message: &Message) -> Result<i64> {
        let attachments = serde_json::to_string(&message.attachments)?;
        self.conn.execute(
            "INSERT INTO messages
                (account_id, folder, uid, from_name, from_email, to_addrs, cc_addrs,
                 bcc_addrs, subject, preview, body, attachments, ts, is_read, is_starred,
                 is_draft, message_id)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17)
             ON CONFLICT(account_id, folder, uid) DO UPDATE SET
                from_name=excluded.from_name, from_email=excluded.from_email,
                to_addrs=excluded.to_addrs, cc_addrs=excluded.cc_addrs,
                bcc_addrs=excluded.bcc_addrs,
                subject=excluded.subject, preview=excluded.preview, body=excluded.body,
                attachments=excluded.attachments, ts=excluded.ts,
                is_read=excluded.is_read, is_starred=excluded.is_starred,
                message_id=excluded.message_id",
            params![
                message.account_id,
                message.folder,
                message.uid,
                message.from_name,
                message.from_email,
                message.to.join(", "),
                message.cc.join(", "),
                message.bcc.join(", "),
                message.subject,
                message.preview,
                message.body,
                attachments,
                message.timestamp,
                message.is_read as i32,
                message.is_starred as i32,
                message.is_draft as i32,
                message.message_id,
            ],
        )?;
        let id: i64 = self.conn.query_row(
            "SELECT id FROM messages WHERE account_id=?1 AND folder=?2 AND uid=?3",
            params![message.account_id, message.folder, message.uid],
            |row| row.get(0),
        )?;
        Ok(id)
    }

    pub fn set_read(&self, id: i64, read: bool) -> Result<()> {
        self.conn.execute(
            "UPDATE messages SET is_read=?1 WHERE id=?2",
            params![read as i32, id],
        )?;
        Ok(())
    }

    pub fn set_starred(&self, id: i64, starred: bool) -> Result<()> {
        self.conn.execute(
            "UPDATE messages SET is_starred=?1 WHERE id=?2",
            params![starred as i32, id],
        )?;
        Ok(())
    }

    pub fn update_folder_oldest(
        &self,
        account_id: i64,
        folder: &str,
        oldest_uid: u32,
    ) -> Result<()> {
        self.conn.execute(
            "UPDATE folders SET oldest_uid=?1 WHERE account_id=?2 AND key=?3",
            params![oldest_uid as i64, account_id, folder],
        )?;
        Ok(())
    }

    pub fn uids(&self, account_id: i64, folder: &str) -> Result<Vec<(i64, u32)>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, uid FROM messages WHERE account_id=?1 AND folder=?2 ORDER BY uid ASC",
        )?;
        let rows = stmt.query_map(params![account_id, folder], |row| {
            Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)? as u32))
        })?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }

    pub fn delete_folder_messages(&self, account_id: i64, folder: &str) -> Result<()> {
        self.conn.execute(
            "DELETE FROM messages WHERE account_id=?1 AND folder=?2",
            params![account_id, folder],
        )?;
        Ok(())
    }

    pub fn message_ref_by_message_id(
        &self,
        account_id: i64,
        message_id: &str,
    ) -> Option<(i64, u32)> {
        if message_id.trim().is_empty() {
            return None;
        }
        self.conn
            .query_row(
                "SELECT id, uid FROM messages WHERE account_id=?1 AND message_id=?2
                 ORDER BY id ASC LIMIT 1",
                params![account_id, message_id],
                |row| {
                    Ok((
                        row.get::<_, i64>(0)?,
                        row.get::<_, i64>(1)? as u32,
                    ))
                },
            )
            .optional()
            .ok()
            .flatten()
    }

    pub fn set_uid(&self, id: i64, uid: u32) -> Result<()> {
        self.conn.execute(
            "UPDATE messages SET uid=?1 WHERE id=?2",
            params![uid as i64, id],
        )?;
        Ok(())
    }

    pub fn update_draft(&self, id: i64, message: &Message) -> Result<bool> {
        let attachments = serde_json::to_string(&message.attachments)?;
        let rows = self.conn.execute(
            "UPDATE messages SET folder='DRAFTS', from_name=?1, from_email=?2,
                to_addrs=?3, cc_addrs=?4, bcc_addrs=?5, subject=?6, preview=?7,
                body=?8, attachments=?9, ts=?10, message_id=?11, is_draft=1
             WHERE id=?12 AND is_draft=1",
            params![
                message.from_name,
                message.from_email,
                message.to.join(", "),
                message.cc.join(", "),
                message.bcc.join(", "),
                message.subject,
                message.preview,
                message.body,
                attachments,
                message.timestamp,
                message.message_id,
                id,
            ],
        )?;
        Ok(rows > 0)
    }

    pub fn move_message(&self, id: i64, folder: &str) -> Result<()> {
        self.conn.execute(
            "UPDATE messages SET folder=?1 WHERE id=?2",
            params![folder, id],
        )?;
        Ok(())
    }

    pub fn delete_message(&self, id: i64) -> Result<()> {
        self.conn
            .execute("DELETE FROM messages WHERE id=?1", params![id])?;
        Ok(())
    }

    /// Which local folder currently holds a message with this Message-ID.
    ///
    /// Used by IMAP sync to avoid re-importing mails the user archived or
    /// deleted locally: the server still returns them in INBOX, but a local
    /// copy already exists in another folder.
    pub fn folder_for_message_id(&self, account_id: i64, message_id: &str) -> Option<String> {
        if message_id.trim().is_empty() {
            return None;
        }
        self.conn
            .query_row(
                "SELECT folder FROM messages WHERE account_id=?1 AND message_id=?2 LIMIT 1",
                params![account_id, message_id],
                |row| row.get(0),
            )
            .optional()
            .ok()
            .flatten()
    }

    /// Next unused uid for locally created messages (sent mail, drafts).
    ///
    /// Uids must only be unique inside an account/folder; using a timestamp
    /// would collide when two messages are created in the same second.
    pub fn next_local_uid(&self, account_id: i64, folder: &str) -> u32 {
        let max: i64 = self
            .conn
            .query_row(
                "SELECT COALESCE(MAX(uid), 0) FROM messages WHERE account_id=?1 AND folder=?2",
                params![account_id, folder],
                |row| row.get(0),
            )
            .unwrap_or(0);
        (max.max(0) as u64 + 1).min(u32::MAX as u64) as u32
    }

    pub fn set_setting(&self, key: &str, value: &str) -> Result<()> {
        self.conn.execute(
            "INSERT INTO settings (key, value) VALUES (?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value=excluded.value",
            params![key, value],
        )?;
        Ok(())
    }

    pub fn setting(&self, key: &str) -> Result<Option<String>> {
        Ok(self
            .conn
            .query_row("SELECT value FROM settings WHERE key=?1", params![key], |row| {
                row.get(0)
            })
            .optional()?)
    }

    pub fn setting_i64(&self, key: &str) -> Option<i64> {
        self.setting(key)
            .ok()
            .flatten()
            .and_then(|value| value.parse().ok())
    }

}

fn row_to_message(row: &rusqlite::Row<'_>) -> rusqlite::Result<Message> {
    let attachments: String = row.get(12)?;
    let to_addrs: String = row.get(6)?;
    let cc_addrs: String = row.get(7)?;
    let bcc_addrs: String = row.get(8)?;
    let account_id: i64 = row.get(1)?;
    let folder: String = row.get(2)?;
    let uid = row.get::<_, i64>(3)? as u32;

    // `Attachment::path` is a cache detail and is not stored in the database.
    // Recover it from the deterministic cache location so attachments stay
    // downloadable after a restart.
    let mut attachments: Vec<Attachment> =
        serde_json::from_str(&attachments).unwrap_or_default();
    for attachment in &mut attachments {
        let missing = attachment
            .path
            .as_ref()
            .is_none_or(|path| !path.is_file());
        if missing {
            let candidate = attachment_store_path(account_id, &folder, uid, &attachment.name);
            if candidate.is_file() {
                attachment.path = Some(candidate);
            } else if let Some(file_name) = attachment
                .path
                .as_ref()
                .and_then(|path| path.file_name())
            {
                // The database may have been copied to another machine or the
                // portable data folder may have moved; try the same file name
                // inside the current attachment directory.
                let moved = attachments_dir().join(file_name);
                attachment.path = moved.is_file().then_some(moved);
            } else {
                attachment.path = None;
            }
        }
    }

    Ok(Message {
        id: row.get(0)?,
        account_id,
        folder,
        uid,
        from_name: row.get(4)?,
        from_email: row.get(5)?,
        to: split_addrs(&to_addrs),
        cc: split_addrs(&cc_addrs),
        bcc: split_addrs(&bcc_addrs),
        subject: row.get(9)?,
        preview: row.get(10)?,
        body: row.get(11)?,
        attachments,
        timestamp: row.get(13)?,
        is_read: row.get::<_, i32>(14)? != 0,
        is_starred: row.get::<_, i32>(15)? != 0,
        is_draft: row.get::<_, i32>(16)? != 0,
        message_id: row.get(17)?,
    })
}

pub fn split_addrs(raw: &str) -> Vec<String> {
    raw.split(',')
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .collect()
}

/// Directory used for the database and cached attachments.
///
/// Prefers `%APPDATA%\mail`, and falls back to a portable `mail-data`
/// folder next to the executable when that location is not writable.
pub fn data_dir() -> PathBuf {
    static CACHED: std::sync::OnceLock<PathBuf> = std::sync::OnceLock::new();
    CACHED
        .get_or_init(|| {
            for base in ["APPDATA", "LOCALAPPDATA"] {
                if let Ok(root) = std::env::var(base) {
                    let candidate = PathBuf::from(root).join("mail");
                    if ensure_dir_writable(&candidate) {
                        return candidate;
                    }
                }
            }
            let exe_dir = std::env::current_exe()
                .ok()
                .and_then(|path| path.parent().map(|parent| parent.to_path_buf()))
                .unwrap_or_else(|| PathBuf::from("."));
            let candidate = exe_dir.join("mail-data");
            if ensure_dir_writable(&candidate) {
                return candidate;
            }
            PathBuf::from("mail-data")
        })
        .clone()
}

fn ensure_dir_writable(dir: &Path) -> bool {
    if std::fs::create_dir_all(dir).is_err() {
        return false;
    }
    let probe = dir.join(".write-probe");
    let ok = std::fs::write(&probe, b"ok").is_ok();
    let _ = std::fs::remove_file(&probe);
    ok
}

pub fn db_path() -> PathBuf {
    data_dir().join("mail.db")
}

pub fn attachments_dir() -> PathBuf {
    let dir = data_dir().join("attachments");
    std::fs::create_dir_all(&dir).ok();
    dir
}

pub fn attachment_store_path(account_id: i64, folder: &str, uid: u32, name: &str) -> PathBuf {
    let safe_name: String = name
        .chars()
        .map(|c| if c.is_alphanumeric() || c == '.' || c == '-' || c == '_' { c } else { '_' })
        .collect();
    let safe_folder: String = folder
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '_' })
        .collect();
    attachments_dir().join(format!("{account_id}-{safe_folder}-{uid}-{safe_name}"))
}




fn ensure_column(conn: &Connection, table: &str, column: &str, ddl: &str) -> Result<()> {
    let mut stmt = conn.prepare(&format!("PRAGMA table_info({table})"))?;
    let exists = stmt
        .query_map([], |row| row.get::<_, String>(1))?
        .filter_map(|name| name.ok())
        .any(|name| name.eq_ignore_ascii_case(column));
    if !exists {
        conn.execute(
            &format!("ALTER TABLE {table} ADD COLUMN {column} {ddl}"),
            [],
        )?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::Attachment;

    fn temp_store() -> Store {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("target")
            .join("test-data");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join(format!("mail-test-{}-{}.db", std::process::id(), nanos));
        Store::open(path).expect("open test store")
    }

    fn test_account(store: &Store) -> i64 {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let account = Account {
            id: 0,
            display_name: "Tester".into(),
            email: format!("tester-{nanos}@mail.app"),
            imap_host: "imap.mail.app".into(),
            imap_port: 993,
            imap_starttls: false,
            smtp_host: "smtp.mail.app".into(),
            smtp_port: 465,
            smtp_starttls: false,
            password: String::new(),
            is_demo: false,
        };
        store.save_account(&account).unwrap()
    }

    fn test_message(account_id: i64, folder: &str, uid: u32, starred: bool, read: bool) -> Message {
        Message {
            id: 0,
            account_id,
            folder: folder.into(),
            uid,
            from_name: "Lin".into(),
            from_email: "lin@mail.app".into(),
            to: vec!["me@mail.app".into()],
            cc: vec![],
            bcc: vec![],
            subject: "hello".into(),
            preview: "hello".into(),
            body: "hello".into(),
            attachments: vec![],
            timestamp: 1_700_000_000,
            is_read: read,
            is_starred: starred,
            is_draft: false,
            message_id: format!("<{uid}@mail.app>"),
        }
    }

    #[test]
    fn restores_cached_attachment_paths() {
        let store = temp_store();
        let account_id = test_account(&store);
        let uid = 765_432u32;
        let name = "report.pdf";
        let cached = attachment_store_path(account_id, "INBOX", uid, name);
        std::fs::create_dir_all(cached.parent().unwrap()).unwrap();
        std::fs::write(&cached, b"pdf").unwrap();

        let mut message = test_message(account_id, "INBOX", uid, false, false);
        message.attachments = vec![Attachment {
            name: name.into(),
            size: 3,
            mime: "application/pdf".into(),
            path: None,
        }];
        store.upsert(&message).unwrap();

        let loaded = store.messages(account_id, "INBOX", 10).unwrap();
        assert_eq!(loaded.len(), 1);
        assert_eq!(
            loaded[0].attachments[0].path.as_deref(),
            Some(cached.as_path())
        );
        let _ = std::fs::remove_file(cached);
    }

    #[test]
    fn local_uids_are_unique_and_monotonic() {
        let store = temp_store();
        let account_id = test_account(&store);
        assert_eq!(store.next_local_uid(account_id, "SENT"), 1);
        store
            .upsert(&test_message(account_id, "SENT", 41, false, true))
            .unwrap();
        assert_eq!(store.next_local_uid(account_id, "SENT"), 42);
        assert_eq!(store.next_local_uid(account_id, "DRAFTS"), 1);
    }

    #[test]
    fn messages_page_returns_offsets() {
        let store = temp_store();
        let account_id = test_account(&store);
        for (index, uid) in [1u32, 2, 3].into_iter().enumerate() {
            let mut message = test_message(account_id, "INBOX", uid, false, true);
            message.timestamp = index as i64 + 1;
            store.upsert(&message).unwrap();
        }
        let first = store.messages_page(account_id, "INBOX", 2, 0).unwrap();
        let second = store.messages_page(account_id, "INBOX", 2, 2).unwrap();
        assert_eq!(first.len(), 2);
        assert_eq!(second.len(), 1);
        assert_eq!(first[0].uid, 3);
        assert_eq!(second[0].uid, 1);
    }

    #[test]
    fn finds_message_placement_by_message_id() {
        let store = temp_store();
        let account_id = test_account(&store);
        let message = test_message(account_id, "ARCHIVE", 5, false, true);
        let message_id = message.message_id.clone();
        store.upsert(&message).unwrap();
        assert_eq!(
            store.folder_for_message_id(account_id, &message_id).as_deref(),
            Some("ARCHIVE")
        );
        assert!(store
            .folder_for_message_id(account_id, "<missing@mail.app>")
            .is_none());
    }

    #[test]
    fn folders_track_unread_and_starred_counts() {
        let store = temp_store();
        let account_id = test_account(&store);
        for kind in [FolderKind::Inbox, FolderKind::Starred] {
            store
                .save_folder(&Folder {
                    account_id,
                    key: kind.key().into(),
                    name: kind.key().into(),
                    kind,
                    unread: 0,
                    total: 0,
                    server_name: None,
                    uid_validity: 0,
                    last_uid: 0,
                    oldest_uid: 0,
                })
                .unwrap();
        }
        store
            .upsert(&test_message(account_id, "INBOX", 1, true, false))
            .unwrap();
        store
            .upsert(&test_message(account_id, "INBOX", 2, false, true))
            .unwrap();

        let folders = store.folders(account_id).unwrap();
        let inbox = folders.iter().find(|folder| folder.key == "INBOX").unwrap();
        let starred = folders.iter().find(|folder| folder.key == "STARRED").unwrap();
        assert_eq!((inbox.unread, inbox.total), (1, 2));
        assert_eq!(starred.total, 1);
    }
}


#[cfg(test)]
mod attachment_move_tests {
    use super::*;
    use crate::models::Attachment;

    #[test]
    fn attachment_path_survives_folder_move() {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("target")
            .join("test-data");
        std::fs::create_dir_all(&dir).unwrap();
        let store =
            Store::open(dir.join(format!("mail-move-{}-{nanos}.db", std::process::id())))
                .unwrap();

        let account = Account {
            id: 0,
            display_name: "T".into(),
            email: format!("move-{nanos}@mail.app"),
            imap_host: String::new(),
            imap_port: 993,
            imap_starttls: false,
            smtp_host: String::new(),
            smtp_port: 465,
            smtp_starttls: false,
            password: String::new(),
            is_demo: false,
        };
        let account_id = store.save_account(&account).unwrap();
        let cached = attachment_store_path(account_id, "INBOX", 77, "a.txt");
        std::fs::create_dir_all(cached.parent().unwrap()).unwrap();
        std::fs::write(&cached, b"x").unwrap();

        let message = Message {
            id: 0,
            account_id,
            folder: "INBOX".into(),
            uid: 77,
            from_name: "A".into(),
            from_email: "a@mail.app".into(),
            to: vec![],
            cc: vec![],
            bcc: vec![],
            subject: "s".into(),
            preview: "p".into(),
            body: "b".into(),
            attachments: vec![Attachment {
                name: "a.txt".into(),
                size: 1,
                mime: "text/plain".into(),
                path: Some(cached.clone()),
            }],
            timestamp: 1,
            is_read: false,
            is_starred: false,
            is_draft: false,
            message_id: "<77@mail.app>".into(),
        };
        let id = store.upsert(&message).unwrap();
        store.move_message(id, "ARCHIVE").unwrap();
        let loaded = store.messages(account_id, "ARCHIVE", 10).unwrap();
        assert_eq!(
            loaded[0].attachments[0].path.as_deref(),
            Some(cached.as_path())
        );
        let _ = std::fs::remove_file(cached);
    }
}
