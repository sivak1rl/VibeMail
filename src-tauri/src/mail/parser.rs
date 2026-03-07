use crate::db::models::{EmailAddress, Message};
use anyhow::Result;
use chrono::{DateTime, Utc};
use mail_parser::{Addr, Address, HeaderValue, MessageParser};

pub fn parse_message(
    raw: &[u8],
    id: &str,
    account_id: &str,
    mailbox_id: &str,
    uid: u32,
) -> Result<Message> {
    let parser = MessageParser::default();
    let parsed = parser
        .parse(raw)
        .ok_or_else(|| anyhow::anyhow!("Failed to parse message"))?;

    let message_id = parsed
        .message_id()
        .map(|s| format!("<{}>", s.trim_matches(['<', '>'])));

    let subject = parsed.subject().map(|s| s.to_string());

    // from/to/cc return Option<&Address<'_>>
    let from = extract_opt_address(parsed.from());
    let to = extract_opt_address(parsed.to());
    let cc = extract_opt_address(parsed.cc());

    let date: Option<DateTime<Utc>> = parsed
        .date()
        .map(|d| DateTime::from_timestamp(d.to_timestamp(), 0).unwrap_or_default());

    // references() and in_reply_to() return &HeaderValue (never Option; Empty when absent)
    let references_ids: Vec<String> = match parsed.references() {
        HeaderValue::Text(t) => vec![normalize_msgid(t)],
        HeaderValue::TextList(list) => list.iter().map(|s| normalize_msgid(s)).collect(),
        _ => vec![],
    };

    let in_reply_to: Option<String> = match parsed.in_reply_to() {
        HeaderValue::Text(t) => Some(normalize_msgid(t)),
        HeaderValue::TextList(list) => list.first().map(|s| normalize_msgid(s)),
        _ => None,
    };

    let body_text = parsed.body_text(0).map(|s| s.to_string());
    let body_html = parsed.body_html(0).map(|s| s.to_string());
    let has_attachments = parsed.attachments().count() > 0;

    Ok(Message {
        id: id.to_string(),
        account_id: account_id.to_string(),
        mailbox_id: mailbox_id.to_string(),
        uid,
        message_id,
        thread_id: None,
        subject,
        from,
        to,
        cc,
        date,
        body_text,
        body_html,
        references_ids,
        in_reply_to,
        flags: Vec::new(),
        has_attachments,
        triage_score: None,
        ai_summary: None,
    })
}

fn normalize_msgid(s: &str) -> String {
    format!("<{}>", s.trim_matches(['<', '>']))
}

fn extract_opt_address(addr: Option<&Address<'_>>) -> Vec<EmailAddress> {
    match addr {
        None => vec![],
        Some(a) => extract_address(a),
    }
}

fn extract_address(addr: &Address<'_>) -> Vec<EmailAddress> {
    match addr {
        Address::List(list) => list.iter().map(addr_to_email).collect(),
        Address::Group(groups) => groups
            .iter()
            .flat_map(|g| g.addresses.iter().map(addr_to_email))
            .collect(),
    }
}

fn addr_to_email(a: &Addr<'_>) -> EmailAddress {
    EmailAddress {
        name: a.name.as_deref().map(|s| s.to_string()),
        email: a.address.as_deref().unwrap_or("").to_string(),
    }
}
