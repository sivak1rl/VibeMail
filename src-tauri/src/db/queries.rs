use crate::db::{models::*, Database};
use anyhow::Result;
use chrono::{TimeZone, Utc};
use serde_json;

impl Database {
    pub fn upsert_account(&self, account: &Account) -> Result<()> {
        self.conn.execute(
            r#"INSERT INTO accounts (id, name, email, provider, imap_host, imap_port, smtp_host, smtp_port)
               VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
               ON CONFLICT(email) DO UPDATE SET name=excluded.name, provider=excluded.provider,
               imap_host=excluded.imap_host, imap_port=excluded.imap_port,
               smtp_host=excluded.smtp_host, smtp_port=excluded.smtp_port"#,
            rusqlite::params![
                account.id, account.name, account.email, account.provider,
                account.imap_host, account.imap_port as i64,
                account.smtp_host, account.smtp_port as i64
            ],
        )?;
        Ok(())
    }

    pub fn list_accounts(&self) -> Result<Vec<Account>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, name, email, provider, imap_host, imap_port, smtp_host, smtp_port FROM accounts",
        )?;
        let accounts = stmt.query_map([], |row| {
            Ok(Account {
                id: row.get(0)?,
                name: row.get(1)?,
                email: row.get(2)?,
                provider: row.get(3)?,
                imap_host: row.get(4)?,
                imap_port: row.get::<_, i64>(5)? as u16,
                smtp_host: row.get(6)?,
                smtp_port: row.get::<_, i64>(7)? as u16,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(accounts)
    }

    pub fn delete_account(&self, account_id: &str) -> Result<()> {
        self.conn
            .execute("DELETE FROM accounts WHERE id=?1", [account_id])?;
        Ok(())
    }

    pub fn upsert_mailbox(&self, mailbox: &Mailbox) -> Result<()> {
        self.conn.execute(
            r#"INSERT INTO mailboxes (id, account_id, name, delimiter, flags, uid_validity, uid_next)
               VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
               ON CONFLICT(account_id, name) DO UPDATE SET
               flags=excluded.flags, uid_validity=excluded.uid_validity, uid_next=excluded.uid_next"#,
            rusqlite::params![
                mailbox.id, mailbox.account_id, mailbox.name, mailbox.delimiter,
                serde_json::to_string(&mailbox.flags)?,
                mailbox.uid_validity.map(|v| v as i64),
                mailbox.uid_next.map(|v| v as i64),
            ],
        )?;
        Ok(())
    }

    pub fn get_mailbox_by_name(&self, account_id: &str, name: &str) -> Result<Option<Mailbox>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, account_id, name, delimiter, flags, uid_validity, uid_next
             FROM mailboxes WHERE account_id=?1 AND name=?2",
        )?;
        let mut rows = stmt.query(rusqlite::params![account_id, name])?;
        if let Some(row) = rows.next()? {
            Ok(Some(Mailbox {
                id: row.get(0)?,
                account_id: row.get(1)?,
                name: row.get(2)?,
                delimiter: row.get(3)?,
                flags: serde_json::from_str(&row.get::<_, String>(4)?).unwrap_or_default(),
                uid_validity: row.get::<_, Option<i64>>(5)?.map(|v| v as u32),
                uid_next: row.get::<_, Option<i64>>(6)?.map(|v| v as u32),
            }))
        } else {
            Ok(None)
        }
    }

    pub fn upsert_message(&self, msg: &Message) -> Result<()> {
        self.conn.execute(
            r#"INSERT INTO messages
               (id, account_id, mailbox_id, uid, message_id, thread_id, subject, "from", "to", cc,
                date, body_text, body_html, references_ids, in_reply_to, flags, has_attachments,
                triage_score, ai_summary)
               VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,?18,?19)
               ON CONFLICT(account_id, mailbox_id, uid) DO UPDATE SET
               message_id=excluded.message_id, thread_id=excluded.thread_id,
               subject=excluded.subject, "from"=excluded."from", "to"=excluded."to",
               cc=excluded.cc, date=excluded.date, body_text=excluded.body_text,
               body_html=excluded.body_html, references_ids=excluded.references_ids,
               in_reply_to=excluded.in_reply_to, flags=excluded.flags,
               has_attachments=excluded.has_attachments, synced_at=unixepoch()"#,
            rusqlite::params![
                msg.id, msg.account_id, msg.mailbox_id, msg.uid as i64,
                msg.message_id, msg.thread_id, msg.subject,
                serde_json::to_string(&msg.from)?,
                serde_json::to_string(&msg.to)?,
                serde_json::to_string(&msg.cc)?,
                msg.date.map(|d| d.timestamp()),
                msg.body_text, msg.body_html,
                serde_json::to_string(&msg.references_ids)?,
                msg.in_reply_to,
                serde_json::to_string(&msg.flags)?,
                msg.has_attachments as i64,
                msg.triage_score,
                msg.ai_summary,
            ],
        )?;
        Ok(())
    }

    pub fn upsert_thread(&self, thread: &Thread) -> Result<()> {
        self.conn.execute(
            r#"INSERT INTO threads
               (id, account_id, subject, participant_ids, message_count, unread_count, last_date, last_from, triage_score, labels)
               VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10)
               ON CONFLICT(id) DO UPDATE SET
               subject=excluded.subject, participant_ids=excluded.participant_ids,
               message_count=excluded.message_count, unread_count=excluded.unread_count,
               last_date=excluded.last_date, last_from=excluded.last_from,
               triage_score=excluded.triage_score, labels=excluded.labels,
               updated_at=unixepoch()"#,
            rusqlite::params![
                thread.id, thread.account_id, thread.subject,
                serde_json::to_string(&thread.participants)?,
                thread.message_count as i64, thread.unread_count as i64,
                thread.last_date.map(|d| d.timestamp()),
                thread.last_from,
                thread.triage_score,
                serde_json::to_string(&thread.labels)?,
            ],
        )?;
        Ok(())
    }

    pub fn list_threads(&self, account_id: &str, limit: u32, offset: u32) -> Result<Vec<Thread>> {
        let mut stmt = self.conn.prepare(
            r#"SELECT id, account_id, subject, participant_ids, message_count, unread_count,
               last_date, last_from, triage_score, labels
               FROM threads WHERE account_id=?1
               ORDER BY last_date DESC LIMIT ?2 OFFSET ?3"#,
        )?;
        let threads = stmt
            .query_map(rusqlite::params![account_id, limit as i64, offset as i64], |row| {
                Ok(Thread {
                    id: row.get(0)?,
                    account_id: row.get(1)?,
                    subject: row.get(2)?,
                    participants: serde_json::from_str(
                        &row.get::<_, String>(3).unwrap_or_default(),
                    )
                    .unwrap_or_default(),
                    message_count: row.get::<_, i64>(4)? as u32,
                    unread_count: row.get::<_, i64>(5)? as u32,
                    last_date: row
                        .get::<_, Option<i64>>(6)?
                        .map(|ts| Utc.timestamp_opt(ts, 0).unwrap()),
                    last_from: row.get(7)?,
                    triage_score: row.get(8)?,
                    labels: serde_json::from_str(&row.get::<_, String>(9).unwrap_or_default())
                        .unwrap_or_default(),
                    messages: None,
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(threads)
    }

    pub fn get_thread_messages(&self, thread_id: &str) -> Result<Vec<Message>> {
        let mut stmt = self.conn.prepare(
            r#"SELECT id, account_id, mailbox_id, uid, message_id, thread_id, subject,
               "from", "to", cc, date, body_text, body_html, references_ids, in_reply_to,
               flags, has_attachments, triage_score, ai_summary
               FROM messages WHERE thread_id=?1 ORDER BY date ASC"#,
        )?;
        let messages = stmt
            .query_map([thread_id], |row| {
                let parse_addr = |s: String| -> Vec<EmailAddress> {
                    serde_json::from_str(&s).unwrap_or_default()
                };
                Ok(Message {
                    id: row.get(0)?,
                    account_id: row.get(1)?,
                    mailbox_id: row.get(2)?,
                    uid: row.get::<_, i64>(3)? as u32,
                    message_id: row.get(4)?,
                    thread_id: row.get(5)?,
                    subject: row.get(6)?,
                    from: parse_addr(row.get::<_, String>(7).unwrap_or_default()),
                    to: parse_addr(row.get::<_, String>(8).unwrap_or_default()),
                    cc: parse_addr(row.get::<_, String>(9).unwrap_or_default()),
                    date: row
                        .get::<_, Option<i64>>(10)?
                        .map(|ts| Utc.timestamp_opt(ts, 0).unwrap()),
                    body_text: row.get(11)?,
                    body_html: row.get(12)?,
                    references_ids: serde_json::from_str(
                        &row.get::<_, String>(13).unwrap_or_default(),
                    )
                    .unwrap_or_default(),
                    in_reply_to: row.get(14)?,
                    flags: serde_json::from_str(&row.get::<_, String>(15).unwrap_or_default())
                        .unwrap_or_default(),
                    has_attachments: row.get::<_, i64>(16)? != 0,
                    triage_score: row.get(17)?,
                    ai_summary: row.get(18)?,
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(messages)
    }

    pub fn get_ai_config(&self) -> Result<AiConfig> {
        let mut stmt = self.conn.prepare(
            "SELECT provider, base_url, model_triage, model_summary, model_draft, model_extract, model_embed, privacy_mode, enabled FROM ai_config WHERE id=1",
        )?;
        let config = stmt.query_row([], |row| {
            Ok(AiConfig {
                provider: row.get(0)?,
                base_url: row.get(1)?,
                model_triage: row.get(2)?,
                model_summary: row.get(3)?,
                model_draft: row.get(4)?,
                model_extract: row.get(5)?,
                model_embed: row.get(6)?,
                privacy_mode: row.get::<_, i64>(7)? != 0,
                enabled: row.get::<_, i64>(8)? != 0,
            })
        })?;
        Ok(config)
    }

    pub fn set_ai_config(&self, config: &AiConfig) -> Result<()> {
        self.conn.execute(
            r#"INSERT INTO ai_config (id, provider, base_url, model_triage, model_summary, model_draft, model_extract, model_embed, privacy_mode, enabled)
               VALUES (1,?1,?2,?3,?4,?5,?6,?7,?8,?9)
               ON CONFLICT(id) DO UPDATE SET provider=excluded.provider, base_url=excluded.base_url,
               model_triage=excluded.model_triage, model_summary=excluded.model_summary,
               model_draft=excluded.model_draft, model_extract=excluded.model_extract,
               model_embed=excluded.model_embed, privacy_mode=excluded.privacy_mode,
               enabled=excluded.enabled"#,
            rusqlite::params![
                config.provider, config.base_url, config.model_triage, config.model_summary,
                config.model_draft, config.model_extract, config.model_embed,
                config.privacy_mode as i64, config.enabled as i64
            ],
        )?;
        Ok(())
    }

    pub fn update_message_triage(&self, message_id: &str, score: f64) -> Result<()> {
        self.conn.execute(
            "UPDATE messages SET triage_score=?1 WHERE id=?2",
            rusqlite::params![score, message_id],
        )?;
        Ok(())
    }

    pub fn update_thread_summary(&self, thread_id: &str, summary: &str) -> Result<()> {
        self.conn.execute(
            "UPDATE threads SET triage_score=(SELECT AVG(triage_score) FROM messages WHERE thread_id=?1) WHERE id=?1",
            [thread_id],
        )?;
        // Store summary on the most recent message in the thread
        self.conn.execute(
            "UPDATE messages SET ai_summary=?1 WHERE thread_id=?2 AND date=(SELECT MAX(date) FROM messages WHERE thread_id=?2)",
            rusqlite::params![summary, thread_id],
        )?;
        Ok(())
    }

    pub fn fts_search(&self, query: &str, account_id: &str, limit: u32) -> Result<Vec<String>> {
        let mut stmt = self.conn.prepare(
            r#"SELECT m.thread_id FROM messages m
               JOIN messages_fts fts ON m.rowid = fts.rowid
               WHERE fts.messages_fts MATCH ?1 AND m.account_id=?2
               GROUP BY m.thread_id ORDER BY MAX(rank) LIMIT ?3"#,
        )?;
        let ids = stmt
            .query_map(rusqlite::params![query, account_id, limit as i64], |row| {
                row.get::<_, Option<String>>(0)
            })?
            .filter_map(|r| r.ok().flatten())
            .collect();
        Ok(ids)
    }

    pub fn get_threads_by_ids(&self, ids: &[String]) -> Result<Vec<Thread>> {
        if ids.is_empty() {
            return Ok(vec![]);
        }
        let placeholders = ids
            .iter()
            .enumerate()
            .map(|(i, _)| format!("?{}", i + 1))
            .collect::<Vec<_>>()
            .join(",");
        let sql = format!(
            "SELECT id, account_id, subject, participant_ids, message_count, unread_count, last_date, last_from, triage_score, labels FROM threads WHERE id IN ({}) ORDER BY last_date DESC",
            placeholders
        );
        let mut stmt = self.conn.prepare(&sql)?;
        let params: Vec<&dyn rusqlite::ToSql> =
            ids.iter().map(|s| s as &dyn rusqlite::ToSql).collect();
        let threads = stmt
            .query_map(params.as_slice(), |row| {
                Ok(Thread {
                    id: row.get(0)?,
                    account_id: row.get(1)?,
                    subject: row.get(2)?,
                    participants: serde_json::from_str(
                        &row.get::<_, String>(3).unwrap_or_default(),
                    )
                    .unwrap_or_default(),
                    message_count: row.get::<_, i64>(4)? as u32,
                    unread_count: row.get::<_, i64>(5)? as u32,
                    last_date: row
                        .get::<_, Option<i64>>(6)?
                        .map(|ts| chrono::Utc.timestamp_opt(ts, 0).unwrap()),
                    last_from: row.get(7)?,
                    triage_score: row.get(8)?,
                    labels: serde_json::from_str(&row.get::<_, String>(9).unwrap_or_default())
                        .unwrap_or_default(),
                    messages: None,
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(threads)
    }
}
