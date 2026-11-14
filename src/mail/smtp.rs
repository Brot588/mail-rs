use std::time::Duration;

use anyhow::{anyhow, Result};
use lettre::message::header::{ContentDisposition, ContentType};
use lettre::message::{Mailbox, Message as SmtpMessage, MultiPart, SinglePart};
use lettre::transport::smtp::authentication::Credentials;
use lettre::transport::smtp::SmtpTransport;
use lettre::Transport;

use crate::models::{Account, Outgoing};

fn mailbox(raw: &str) -> Result<Mailbox> {
    let raw = raw.trim();
    if raw.is_empty() {
        return Err(anyhow!("收件人地址为空"));
    }
    // Accept both `Name <addr@host>` and a bare address.
    let (name, address) = if let (Some(start), Some(end)) = (raw.rfind('<'), raw.rfind('>')) {
        if start < end {
            let name = raw[..start].trim().trim_matches('"').to_string();
            (Some(name).filter(|name| !name.is_empty()), &raw[start + 1..end])
        } else {
            (None, raw)
        }
    } else {
        (None, raw)
    };
    let address = address
        .trim()
        .parse()
        .map_err(|_| anyhow!("无法识别的邮箱地址：{address}"))?;
    Ok(Mailbox::new(name, address))
}

pub(crate) fn build_message(mail: &Outgoing) -> Result<SmtpMessage> {
    let mut message = SmtpMessage::builder()
        .from(mailbox(&format!("{} <{}>", mail.from_name, mail.from_email))?)
        .subject(&mail.subject)
        .header(ContentType::TEXT_PLAIN);

    if mail.to.is_empty() {
        return Err(anyhow!("请至少填写一位收件人"));
    }
    for recipient in &mail.to {
        message = message.to(mailbox(recipient)?);
    }
    for recipient in &mail.cc {
        message = message.cc(mailbox(recipient)?);
    }
    for recipient in &mail.bcc {
        message = message.bcc(mailbox(recipient)?);
    }
    if !mail.message_id.trim().is_empty() {
        message = message.message_id(Some(mail.message_id.clone()));
    }
    if let Some(in_reply_to) = &mail.in_reply_to {
        if !in_reply_to.is_empty() {
            message = message.in_reply_to(in_reply_to.clone());
        }
    }

    let message = if mail.attachments.is_empty() {
        message
            .body(mail.body.clone())
            .map_err(|err| anyhow!("构建邮件失败：{err}"))?
    } else {
        let mut mixed = MultiPart::mixed().singlepart(SinglePart::plain(mail.body.clone()));
        for attachment in &mail.attachments {
            let Some(path) = &attachment.path else {
                continue;
            };
            let bytes = std::fs::read(path)
                .map_err(|err| anyhow!("读取附件 {} 失败：{err}", attachment.name))?;
            let content_type = attachment
                .mime
                .parse::<ContentType>()
                .unwrap_or(ContentType::TEXT_PLAIN);
            let part = SinglePart::builder()
                .header(content_type)
                .header(ContentDisposition::attachment(&attachment.name))
                .body(bytes);
            mixed = mixed.singlepart(part);
        }
        message
            .multipart(mixed)
            .map_err(|err| anyhow!("构建带附件的邮件失败：{err}"))?
    };

    Ok(message)
}

/// Send a plain-text message over SMTP and return its raw RFC 822 bytes
/// (used for IMAP APPEND to the Sent folder).
pub fn send(account: &Account, mail: &Outgoing) -> Result<Vec<u8>> {
    let mut builder = if account.smtp_starttls {
        SmtpTransport::starttls_relay(&account.smtp_host)
    } else {
        SmtpTransport::relay(&account.smtp_host)
    }
    .map_err(|err| anyhow!("初始化 SMTP 传输失败：{err}"))?;

    builder = builder
        .port(account.smtp_port)
        .timeout(Some(Duration::from_secs(60)))
        .credentials(Credentials::new(
            account.email.clone(),
            account.password.clone(),
        ));
    let mailer = builder.build();

    let message = build_message(mail)?;

    mailer
        .send(&message)
        .map_err(|err| anyhow!("发送失败：{err}"))?;
    Ok(message.formatted())
}





#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::Attachment;

    #[test]
    fn builds_multipart_with_attachment_and_hidden_bcc() {
        let dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("target")
            .join("test-data");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("smtp-attachment-test.txt");
        std::fs::write(&path, b"attachment body").unwrap();
        let mail = Outgoing {
            from_name: "Tester".into(),
            from_email: "tester@mail.app".into(),
            to: vec!["to@mail.app".into()],
            cc: vec![],
            bcc: vec!["hidden@mail.app".into()],
            subject: "with attachment".into(),
            body: "hello body".into(),
            in_reply_to: None,
            message_id: "<test-message@mail.app>".into(),
            attachments: vec![Attachment {
                name: "smtp-attachment-test.txt".into(),
                size: 15,
                mime: "text/plain".into(),
                path: Some(path.clone()),
            }],
        };
        let message = build_message(&mail).unwrap();
        let raw = String::from_utf8_lossy(&message.formatted()).to_string();
        assert!(raw.contains("hello body"));
        assert!(raw.contains("smtp-attachment-test.txt"));
        assert!(raw.contains("attachment body"));
        assert!(!raw.contains("hidden@mail.app"));
        let _ = std::fs::remove_file(path);
    }
}

