use crate::db::{models::*, Database};
use anyhow::Result;
use chrono::{TimeZone, Utc};
use serde_json;

#[derive(Debug, Clone)]
pub struct MailboxStats {
    pub mailbox: Mailbox,
    pub thread_count: u32,
    pub unread_count: u32,
}

#[derive(Debug, Clone)]
pub struct ThreadMessageLocation {
    pub thread_id: String,
    pub account_id: String,
    pub mailbox_id: String,
    pub uid: u32,
}

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
        let accounts = stmt
            .query_map([], |row| {
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

    pub fn list_mailboxes(&self, account_id: &str) -> Result<Vec<Mailbox>> {
        let mut stmt = self.conn.prepare(
            r#"SELECT id, account_id, name, delimiter, flags, uid_validity, uid_next
               FROM mailboxes
               WHERE account_id=?1
               ORDER BY CASE WHEN UPPER(name) = 'INBOX' THEN 0 ELSE 1 END, name COLLATE NOCASE"#,
        )?;
        let mailboxes = stmt
            .query_map([account_id], |row| {
                Ok(Mailbox {
                    id: row.get(0)?,
                    account_id: row.get(1)?,
                    name: row.get(2)?,
                    delimiter: row.get(3)?,
                    flags: serde_json::from_str(&row.get::<_, String>(4)?).unwrap_or_default(),
                    uid_validity: row.get::<_, Option<i64>>(5)?.map(|v| v as u32),
                    uid_next: row.get::<_, Option<i64>>(6)?.map(|v| v as u32),
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(mailboxes)
    }

    pub fn list_mailboxes_with_counts(&self, account_id: &str) -> Result<Vec<MailboxStats>> {
        let mut stmt = self.conn.prepare(
            r#"SELECT mb.id, mb.account_id, mb.name, mb.delimiter, mb.flags, mb.uid_validity, mb.uid_next,
               COUNT(DISTINCT m.thread_id) AS thread_count,
               COUNT(DISTINCT CASE
                 WHEN m.thread_id IS NOT NULL
                  AND instr(COALESCE(m.flags, ''), '\\Seen') = 0
                 THEN m.thread_id
               END) AS unread_count
               FROM mailboxes mb
               LEFT JOIN messages m ON m.mailbox_id = mb.id
               WHERE mb.account_id=?1
               GROUP BY mb.id, mb.account_id, mb.name, mb.delimiter, mb.flags, mb.uid_validity, mb.uid_next
               ORDER BY CASE WHEN UPPER(mb.name) = 'INBOX' THEN 0 ELSE 1 END, mb.name COLLATE NOCASE"#,
        )?;
        let mailboxes = stmt
            .query_map([account_id], |row| {
                Ok(MailboxStats {
                    mailbox: Mailbox {
                        id: row.get(0)?,
                        account_id: row.get(1)?,
                        name: row.get(2)?,
                        delimiter: row.get(3)?,
                        flags: serde_json::from_str(&row.get::<_, String>(4)?).unwrap_or_default(),
                        uid_validity: row.get::<_, Option<i64>>(5)?.map(|v| v as u32),
                        uid_next: row.get::<_, Option<i64>>(6)?.map(|v| v as u32),
                    },
                    thread_count: row.get::<_, i64>(7)? as u32,
                    unread_count: row.get::<_, i64>(8)? as u32,
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(mailboxes)
    }

    pub fn get_mailbox_by_id(&self, account_id: &str, mailbox_id: &str) -> Result<Option<Mailbox>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, account_id, name, delimiter, flags, uid_validity, uid_next
             FROM mailboxes WHERE account_id=?1 AND id=?2",
        )?;
        let mut rows = stmt.query(rusqlite::params![account_id, mailbox_id])?;
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
               has_attachments=excluded.has_attachments,
               triage_score=COALESCE(excluded.triage_score, messages.triage_score),
               ai_summary=COALESCE(excluded.ai_summary, messages.ai_summary),
               synced_at=unixepoch()"#,
            rusqlite::params![
                msg.id,
                msg.account_id,
                msg.mailbox_id,
                msg.uid as i64,
                msg.message_id,
                msg.thread_id,
                msg.subject,
                serde_json::to_string(&msg.from)?,
                serde_json::to_string(&msg.to)?,
                serde_json::to_string(&msg.cc)?,
                msg.date.map(|d| d.timestamp()),
                msg.body_text,
                msg.body_html,
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
               triage_score=COALESCE(excluded.triage_score, threads.triage_score),
               labels=CASE
                 WHEN excluded.labels = '[]' THEN COALESCE(threads.labels, excluded.labels)
                 ELSE excluded.labels
               END,
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

    pub fn list_threads(
        &self,
        account_id: &str,
        mailbox_id: Option<&str>,
        limit: u32,
        offset: u32,
    ) -> Result<Vec<Thread>> {
        let map_thread = |row: &rusqlite::Row<'_>| {
            Ok(Thread {
                id: row.get(0)?,
                account_id: row.get(1)?,
                subject: row.get(2)?,
                participants: serde_json::from_str(&row.get::<_, String>(3).unwrap_or_default())
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
        };

        let limit = limit as i64;
        let offset = offset as i64;
        let threads = if let Some(mailbox_id) = mailbox_id {
            let mut stmt = self.conn.prepare(
                r#"SELECT t.id, t.account_id, t.subject, t.participant_ids, t.message_count,
                   t.unread_count, t.last_date, t.last_from, t.triage_score, t.labels
                   FROM threads t
                   WHERE t.account_id=?1
                     AND EXISTS (
                       SELECT 1 FROM messages m
                       WHERE m.thread_id = t.id AND m.mailbox_id = ?2
                     )
                   ORDER BY t.last_date DESC LIMIT ?3 OFFSET ?4"#,
            )?;
            let rows = stmt.query_map(
                rusqlite::params![account_id, mailbox_id, limit, offset],
                map_thread,
            )?;
            rows.collect::<rusqlite::Result<Vec<_>>>()?
        } else {
            let mut stmt = self.conn.prepare(
                r#"SELECT id, account_id, subject, participant_ids, message_count, unread_count,
                   last_date, last_from, triage_score, labels
                   FROM threads WHERE account_id=?1
                   ORDER BY last_date DESC LIMIT ?2 OFFSET ?3"#,
            )?;
            let rows = stmt.query_map(rusqlite::params![account_id, limit, offset], map_thread)?;
            rows.collect::<rusqlite::Result<Vec<_>>>()?
        };
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

    pub fn update_thread_labels(&self, thread_id: &str, labels: &[String]) -> Result<()> {
        self.conn.execute(
            "UPDATE threads SET labels=?1, updated_at=unixepoch() WHERE id=?2",
            rusqlite::params![serde_json::to_string(labels)?, thread_id],
        )?;
        Ok(())
    }

    pub fn upsert_thread_actions(
        &self,
        thread_id: &str,
        actions: &[ExtractedAction],
    ) -> Result<()> {
        self.conn.execute(
            r#"INSERT INTO thread_actions (thread_id, actions_json)
               VALUES (?1, ?2)
               ON CONFLICT(thread_id) DO UPDATE SET
               actions_json=excluded.actions_json,
               updated_at=unixepoch()"#,
            rusqlite::params![thread_id, serde_json::to_string(actions)?],
        )?;
        Ok(())
    }

    pub fn get_thread_actions(&self, thread_id: &str) -> Result<Vec<ExtractedAction>> {
        let mut stmt = self
            .conn
            .prepare("SELECT actions_json FROM thread_actions WHERE thread_id=?1")?;
        let mut rows = stmt.query([thread_id])?;
        if let Some(row) = rows.next()? {
            let raw: String = row.get(0)?;
            Ok(serde_json::from_str(&raw).unwrap_or_default())
        } else {
            Ok(vec![])
        }
    }

    pub fn get_thread_summary(&self, thread_id: &str) -> Result<Option<String>> {
        let mut stmt = self.conn.prepare(
            r#"SELECT ai_summary
               FROM messages
               WHERE thread_id=?1
                 AND ai_summary IS NOT NULL
                 AND ai_summary != ''
               ORDER BY date DESC
               LIMIT 1"#,
        )?;
        let mut rows = stmt.query([thread_id])?;
        if let Some(row) = rows.next()? {
            let summary: Option<String> = row.get(0)?;
            Ok(summary)
        } else {
            Ok(None)
        }
    }

    pub fn set_thread_read_state(&self, thread_id: &str, read: bool) -> Result<()> {
        let mut stmt = self
            .conn
            .prepare("SELECT id, flags FROM messages WHERE thread_id=?1")?;
        let messages = stmt
            .query_map([thread_id], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;

        for (message_id, flags_raw) in messages {
            let mut flags: Vec<String> = serde_json::from_str(&flags_raw).unwrap_or_default();
            let has_seen = flags.iter().any(|flag| flag == "\\Seen");

            if read && !has_seen {
                flags.push("\\Seen".to_string());
            } else if !read && has_seen {
                flags.retain(|flag| flag != "\\Seen");
            }

            self.conn.execute(
                "UPDATE messages SET flags=?1 WHERE id=?2",
                rusqlite::params![serde_json::to_string(&flags)?, message_id],
            )?;
        }

        self.conn.execute(
            "UPDATE threads SET unread_count=(
                SELECT COUNT(*) FROM messages
                WHERE thread_id=?1 AND instr(COALESCE(flags, ''), '\\Seen') = 0
            ) WHERE id=?1",
            [thread_id],
        )?;
        Ok(())
    }

    pub fn set_threads_read_state(&self, thread_ids: &[String], read: bool) -> Result<usize> {
        for thread_id in thread_ids {
            self.set_thread_read_state(thread_id, read)?;
        }
        Ok(thread_ids.len())
    }

    pub fn get_thread_message_locations(
        &self,
        thread_ids: &[String],
    ) -> Result<Vec<ThreadMessageLocation>> {
        if thread_ids.is_empty() {
            return Ok(vec![]);
        }

        let placeholders = thread_ids
            .iter()
            .enumerate()
            .map(|(i, _)| format!("?{}", i + 1))
            .collect::<Vec<_>>()
            .join(",");
        let sql = format!(
            "SELECT thread_id, account_id, mailbox_id, uid
             FROM messages
             WHERE thread_id IN ({})
             ORDER BY thread_id, account_id, mailbox_id, uid",
            placeholders
        );

        let params: Vec<&dyn rusqlite::ToSql> = thread_ids
            .iter()
            .map(|id| id as &dyn rusqlite::ToSql)
            .collect();
        let mut stmt = self.conn.prepare(&sql)?;
        let rows = stmt.query_map(params.as_slice(), |row| {
            Ok(ThreadMessageLocation {
                thread_id: row.get(0)?,
                account_id: row.get(1)?,
                mailbox_id: row.get(2)?,
                uid: row.get::<_, i64>(3)? as u32,
            })
        })?;

        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }

    pub fn fts_search(
        &self,
        query: &str,
        account_id: &str,
        mailbox_id: Option<&str>,
        limit: u32,
    ) -> Result<Vec<String>> {
        let limit = limit as i64;
        let ids = if let Some(mailbox_id) = mailbox_id {
            let mut stmt = self.conn.prepare(
                r#"SELECT m.thread_id FROM messages m
                   JOIN messages_fts fts ON m.rowid = fts.rowid
                   WHERE fts.messages_fts MATCH ?1 AND m.account_id=?2 AND m.mailbox_id=?3
                   GROUP BY m.thread_id ORDER BY MAX(rank) LIMIT ?4"#,
            )?;
            let rows = stmt.query_map(
                rusqlite::params![query, account_id, mailbox_id, limit],
                |row| row.get::<_, Option<String>>(0),
            )?;
            rows.filter_map(|r| r.ok().flatten()).collect()
        } else {
            let mut stmt = self.conn.prepare(
                r#"SELECT m.thread_id FROM messages m
                   JOIN messages_fts fts ON m.rowid = fts.rowid
                   WHERE fts.messages_fts MATCH ?1 AND m.account_id=?2
                   GROUP BY m.thread_id ORDER BY MAX(rank) LIMIT ?3"#,
            )?;
            let rows = stmt.query_map(rusqlite::params![query, account_id, limit], |row| {
                row.get::<_, Option<String>>(0)
            })?;
            rows.filter_map(|r| r.ok().flatten()).collect()
        };
        Ok(ids)
    }

    pub fn get_threads_by_ids(
        &self,
        ids: &[String],
        mailbox_id: Option<&str>,
    ) -> Result<Vec<Thread>> {
        if ids.is_empty() {
            return Ok(vec![]);
        }
        let placeholders = ids
            .iter()
            .enumerate()
            .map(|(i, _)| format!("?{}", i + 1))
            .collect::<Vec<_>>()
            .join(",");
        let sql = if mailbox_id.is_some() {
            format!(
                "SELECT t.id, t.account_id, t.subject, t.participant_ids, t.message_count, t.unread_count, t.last_date, t.last_from, t.triage_score, t.labels
                 FROM threads t
                 WHERE t.id IN ({})
                   AND EXISTS (
                     SELECT 1 FROM messages m
                     WHERE m.thread_id = t.id AND m.mailbox_id = ?{}
                   )
                 ORDER BY t.last_date DESC",
                placeholders,
                ids.len() + 1
            )
        } else {
            format!(
                "SELECT id, account_id, subject, participant_ids, message_count, unread_count, last_date, last_from, triage_score, labels
                 FROM threads WHERE id IN ({}) ORDER BY last_date DESC",
                placeholders
            )
        };
        let map_thread = |row: &rusqlite::Row<'_>| {
            Ok(Thread {
                id: row.get(0)?,
                account_id: row.get(1)?,
                subject: row.get(2)?,
                participants: serde_json::from_str(&row.get::<_, String>(3).unwrap_or_default())
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
        };

        let mut stmt = self.conn.prepare(&sql)?;
        let threads = if let Some(mailbox_id) = mailbox_id {
            let mut params: Vec<&dyn rusqlite::ToSql> =
                ids.iter().map(|s| s as &dyn rusqlite::ToSql).collect();
            params.push(&mailbox_id);
            let rows = stmt.query_map(params.as_slice(), map_thread)?;
            rows.collect::<rusqlite::Result<Vec<_>>>()?
        } else {
            let params: Vec<&dyn rusqlite::ToSql> =
                ids.iter().map(|s| s as &dyn rusqlite::ToSql).collect();
            let rows = stmt.query_map(params.as_slice(), map_thread)?;
            rows.collect::<rusqlite::Result<Vec<_>>>()?
        };
        Ok(threads)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;

    fn test_db() -> Database {
        let mut conn = Connection::open_in_memory().expect("in-memory db");
        crate::db::schema::run_migrations(&mut conn).expect("migrations");
        Database { conn }
    }

    #[test]
    fn list_mailboxes_returns_inbox_first_then_alphabetical() {
        let db = test_db();
        let account = Account {
            id: "acc1".to_string(),
            name: "Test".to_string(),
            email: "test@example.com".to_string(),
            provider: "generic".to_string(),
            imap_host: "imap.example.com".to_string(),
            imap_port: 993,
            smtp_host: "smtp.example.com".to_string(),
            smtp_port: 465,
        };
        db.upsert_account(&account).expect("account");

        for name in ["Archive", "INBOX", "Sent"] {
            db.upsert_mailbox(&Mailbox {
                id: format!("{}:{}", account.id, name),
                account_id: account.id.clone(),
                name: name.to_string(),
                delimiter: Some("/".to_string()),
                flags: Vec::new(),
                uid_validity: None,
                uid_next: None,
            })
            .expect("mailbox");
        }

        let names: Vec<_> = db
            .list_mailboxes(&account.id)
            .expect("mailboxes")
            .into_iter()
            .map(|mailbox| mailbox.name)
            .collect();

        assert_eq!(names, vec!["INBOX", "Archive", "Sent"]);
    }

    #[test]
    fn list_threads_can_filter_by_mailbox() {
        let db = test_db();
        let account = Account {
            id: "acc1".to_string(),
            name: "Test".to_string(),
            email: "test@example.com".to_string(),
            provider: "generic".to_string(),
            imap_host: "imap.example.com".to_string(),
            imap_port: 993,
            smtp_host: "smtp.example.com".to_string(),
            smtp_port: 465,
        };
        db.upsert_account(&account).expect("account");

        let inbox = Mailbox {
            id: "acc1:INBOX".to_string(),
            account_id: account.id.clone(),
            name: "INBOX".to_string(),
            delimiter: Some("/".to_string()),
            flags: Vec::new(),
            uid_validity: None,
            uid_next: None,
        };
        let archive = Mailbox {
            id: "acc1:Archive".to_string(),
            account_id: account.id.clone(),
            name: "Archive".to_string(),
            delimiter: Some("/".to_string()),
            flags: Vec::new(),
            uid_validity: None,
            uid_next: None,
        };
        db.upsert_mailbox(&inbox).expect("inbox");
        db.upsert_mailbox(&archive).expect("archive");

        let inbox_thread = Thread {
            id: "thread-inbox".to_string(),
            account_id: account.id.clone(),
            subject: Some("Inbox thread".to_string()),
            participants: Vec::new(),
            message_count: 1,
            unread_count: 0,
            last_date: Some(Utc::now()),
            last_from: Some("inbox@example.com".to_string()),
            triage_score: None,
            labels: Vec::new(),
            messages: None,
        };
        let archive_thread = Thread {
            id: "thread-archive".to_string(),
            account_id: account.id.clone(),
            subject: Some("Archive thread".to_string()),
            participants: Vec::new(),
            message_count: 1,
            unread_count: 0,
            last_date: Some(Utc::now()),
            last_from: Some("archive@example.com".to_string()),
            triage_score: None,
            labels: Vec::new(),
            messages: None,
        };
        db.upsert_thread(&inbox_thread).expect("inbox thread");
        db.upsert_thread(&archive_thread).expect("archive thread");

        let build_message = |id: &str, thread_id: &str, mailbox_id: &str| Message {
            id: id.to_string(),
            account_id: account.id.clone(),
            mailbox_id: mailbox_id.to_string(),
            uid: 1,
            message_id: Some(format!("<{}>", id)),
            thread_id: Some(thread_id.to_string()),
            subject: Some(id.to_string()),
            from: Vec::new(),
            to: Vec::new(),
            cc: Vec::new(),
            date: Some(Utc::now()),
            body_text: None,
            body_html: None,
            references_ids: Vec::new(),
            in_reply_to: None,
            flags: Vec::new(),
            has_attachments: false,
            triage_score: None,
            ai_summary: None,
        };
        db.upsert_message(&build_message("msg-inbox", &inbox_thread.id, &inbox.id))
            .expect("inbox message");
        db.upsert_message(&build_message(
            "msg-archive",
            &archive_thread.id,
            &archive.id,
        ))
        .expect("archive message");

        let inbox_threads = db
            .list_threads(&account.id, Some(&inbox.id), 50, 0)
            .expect("filtered inbox threads");
        let archive_threads = db
            .list_threads(&account.id, Some(&archive.id), 50, 0)
            .expect("filtered archive threads");

        assert_eq!(inbox_threads.len(), 1);
        assert_eq!(inbox_threads[0].id, inbox_thread.id);
        assert_eq!(archive_threads.len(), 1);
        assert_eq!(archive_threads[0].id, archive_thread.id);
    }

    #[test]
    fn list_mailboxes_with_counts_returns_unread_badges() {
        let db = test_db();
        let account = Account {
            id: "acc1".to_string(),
            name: "Test".to_string(),
            email: "test@example.com".to_string(),
            provider: "generic".to_string(),
            imap_host: "imap.example.com".to_string(),
            imap_port: 993,
            smtp_host: "smtp.example.com".to_string(),
            smtp_port: 465,
        };
        db.upsert_account(&account).expect("account");

        let inbox = Mailbox {
            id: "acc1:INBOX".to_string(),
            account_id: account.id.clone(),
            name: "INBOX".to_string(),
            delimiter: Some("/".to_string()),
            flags: Vec::new(),
            uid_validity: None,
            uid_next: None,
        };
        db.upsert_mailbox(&inbox).expect("inbox");

        let unread_thread = Thread {
            id: "thread-unread".to_string(),
            account_id: account.id.clone(),
            subject: Some("Unread".to_string()),
            participants: Vec::new(),
            message_count: 1,
            unread_count: 1,
            last_date: Some(Utc::now()),
            last_from: Some("unread@example.com".to_string()),
            triage_score: None,
            labels: Vec::new(),
            messages: None,
        };
        let read_thread = Thread {
            id: "thread-read".to_string(),
            account_id: account.id.clone(),
            subject: Some("Read".to_string()),
            participants: Vec::new(),
            message_count: 1,
            unread_count: 0,
            last_date: Some(Utc::now()),
            last_from: Some("read@example.com".to_string()),
            triage_score: None,
            labels: Vec::new(),
            messages: None,
        };
        db.upsert_thread(&unread_thread).expect("unread thread");
        db.upsert_thread(&read_thread).expect("read thread");

        let unread_message = Message {
            id: "msg-unread".to_string(),
            account_id: account.id.clone(),
            mailbox_id: inbox.id.clone(),
            uid: 1,
            message_id: Some("<msg-unread>".to_string()),
            thread_id: Some(unread_thread.id.clone()),
            subject: Some("Unread".to_string()),
            from: Vec::new(),
            to: Vec::new(),
            cc: Vec::new(),
            date: Some(Utc::now()),
            body_text: None,
            body_html: None,
            references_ids: Vec::new(),
            in_reply_to: None,
            flags: Vec::new(),
            has_attachments: false,
            triage_score: None,
            ai_summary: None,
        };
        let read_message = Message {
            id: "msg-read".to_string(),
            account_id: account.id.clone(),
            mailbox_id: inbox.id.clone(),
            uid: 2,
            message_id: Some("<msg-read>".to_string()),
            thread_id: Some(read_thread.id.clone()),
            subject: Some("Read".to_string()),
            from: Vec::new(),
            to: Vec::new(),
            cc: Vec::new(),
            date: Some(Utc::now()),
            body_text: None,
            body_html: None,
            references_ids: Vec::new(),
            in_reply_to: None,
            flags: vec!["\\Seen".to_string()],
            has_attachments: false,
            triage_score: None,
            ai_summary: None,
        };
        db.upsert_message(&unread_message).expect("unread message");
        db.upsert_message(&read_message).expect("read message");

        let stats = db
            .list_mailboxes_with_counts(&account.id)
            .expect("mailbox stats");

        assert_eq!(stats.len(), 1);
        assert_eq!(stats[0].thread_count, 2);
        assert_eq!(stats[0].unread_count, 1);
    }

    #[test]
    fn get_threads_by_ids_can_filter_by_mailbox() {
        let db = test_db();
        let account = Account {
            id: "acc1".to_string(),
            name: "Test".to_string(),
            email: "test@example.com".to_string(),
            provider: "generic".to_string(),
            imap_host: "imap.example.com".to_string(),
            imap_port: 993,
            smtp_host: "smtp.example.com".to_string(),
            smtp_port: 465,
        };
        db.upsert_account(&account).expect("account");

        let inbox = Mailbox {
            id: "acc1:INBOX".to_string(),
            account_id: account.id.clone(),
            name: "INBOX".to_string(),
            delimiter: Some("/".to_string()),
            flags: Vec::new(),
            uid_validity: None,
            uid_next: None,
        };
        let archive = Mailbox {
            id: "acc1:Archive".to_string(),
            account_id: account.id.clone(),
            name: "Archive".to_string(),
            delimiter: Some("/".to_string()),
            flags: Vec::new(),
            uid_validity: None,
            uid_next: None,
        };
        db.upsert_mailbox(&inbox).expect("inbox");
        db.upsert_mailbox(&archive).expect("archive");

        let inbox_thread = Thread {
            id: "thread-inbox".to_string(),
            account_id: account.id.clone(),
            subject: Some("Inbox".to_string()),
            participants: Vec::new(),
            message_count: 1,
            unread_count: 0,
            last_date: Some(Utc::now()),
            last_from: Some("inbox@example.com".to_string()),
            triage_score: None,
            labels: Vec::new(),
            messages: None,
        };
        let archive_thread = Thread {
            id: "thread-archive".to_string(),
            account_id: account.id.clone(),
            subject: Some("Archive".to_string()),
            participants: Vec::new(),
            message_count: 1,
            unread_count: 0,
            last_date: Some(Utc::now()),
            last_from: Some("archive@example.com".to_string()),
            triage_score: None,
            labels: Vec::new(),
            messages: None,
        };
        db.upsert_thread(&inbox_thread).expect("inbox thread");
        db.upsert_thread(&archive_thread).expect("archive thread");

        for (id, thread_id, mailbox_id, uid) in [
            (
                "msg-inbox",
                inbox_thread.id.as_str(),
                inbox.id.as_str(),
                1_u32,
            ),
            (
                "msg-archive",
                archive_thread.id.as_str(),
                archive.id.as_str(),
                2_u32,
            ),
        ] {
            db.upsert_message(&Message {
                id: id.to_string(),
                account_id: account.id.clone(),
                mailbox_id: mailbox_id.to_string(),
                uid,
                message_id: Some(format!("<{}>", id)),
                thread_id: Some(thread_id.to_string()),
                subject: Some(id.to_string()),
                from: Vec::new(),
                to: Vec::new(),
                cc: Vec::new(),
                date: Some(Utc::now()),
                body_text: None,
                body_html: None,
                references_ids: Vec::new(),
                in_reply_to: None,
                flags: Vec::new(),
                has_attachments: false,
                triage_score: None,
                ai_summary: None,
            })
            .expect("message");
        }

        let ids = vec![inbox_thread.id.clone(), archive_thread.id.clone()];
        let filtered = db
            .get_threads_by_ids(&ids, Some(&archive.id))
            .expect("filtered ids");

        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].id, archive_thread.id);
    }
}
