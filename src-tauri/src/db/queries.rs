use crate::db::{models::*, Database};
use anyhow::Result;
use chrono::{DateTime, TimeZone, Utc};
use serde_json;
use std::collections::HashMap;

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
               ON CONFLICT(id) DO UPDATE SET 
               name=excluded.name, 
               email=excluded.email,
               provider=excluded.provider,
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
            r#"INSERT INTO mailboxes (id, account_id, name, delimiter, flags, uid_validity, uid_next, last_synced_at)
               VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
               ON CONFLICT(id) DO UPDATE SET
               flags=excluded.flags, 
               delimiter=excluded.delimiter,
               uid_validity=excluded.uid_validity, 
               uid_next=excluded.uid_next,
               last_synced_at=excluded.last_synced_at"#,
            rusqlite::params![
                mailbox.id, mailbox.account_id, mailbox.name, mailbox.delimiter,
                serde_json::to_string(&mailbox.flags)?,
                mailbox.uid_validity.map(|v| v as i64),
                mailbox.uid_next.map(|v| v as i64),
                mailbox.last_synced_at.map(|d| d.timestamp()),
            ],
        )?;
        Ok(())
    }

    pub fn get_mailbox_by_name(&self, account_id: &str, name: &str) -> Result<Option<Mailbox>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, account_id, name, delimiter, flags, uid_validity, uid_next, last_synced_at
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
                last_synced_at: row
                    .get::<_, Option<i64>>(7)?
                    .map(|ts| Utc.timestamp_opt(ts, 0).unwrap()),
            }))
        } else {
            Ok(None)
        }
    }

    pub fn list_mailboxes(&self, account_id: &str) -> Result<Vec<Mailbox>> {
        let mut stmt = self.conn.prepare(
            r#"SELECT id, account_id, name, delimiter, flags, uid_validity, uid_next, last_synced_at
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
                    last_synced_at: row
                        .get::<_, Option<i64>>(7)?
                        .map(|ts| Utc.timestamp_opt(ts, 0).unwrap()),
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(mailboxes)
    }

    pub fn list_mailboxes_with_counts(&self, account_id: &str) -> Result<Vec<MailboxStats>> {
        // Check both legacy (mailbox_id) and new Gmail All Mail (inbox_mailboxes) paths.
        let mut stmt = self.conn.prepare(
            r#"SELECT mb.id, mb.account_id, mb.name, mb.delimiter, mb.flags, mb.uid_validity, mb.uid_next, mb.last_synced_at,
               (SELECT COUNT(DISTINCT m.thread_id)
                FROM messages m
                WHERE m.account_id = mb.account_id
                  AND m.thread_id IS NOT NULL
                  AND (
                    (m.inbox_mailboxes IS NULL AND m.mailbox_id = mb.id)
                    OR (m.inbox_mailboxes IS NOT NULL AND EXISTS (
                      SELECT 1 FROM json_each(m.inbox_mailboxes) WHERE value = mb.id
                    ))
                  )
               ) AS thread_count,
               (SELECT COUNT(DISTINCT m.thread_id)
                FROM messages m
                WHERE m.account_id = mb.account_id
                  AND m.thread_id IS NOT NULL
                  AND instr(COALESCE(m.flags, ''), '\Seen') = 0
                  AND (
                    (m.inbox_mailboxes IS NULL AND m.mailbox_id = mb.id)
                    OR (m.inbox_mailboxes IS NOT NULL AND EXISTS (
                      SELECT 1 FROM json_each(m.inbox_mailboxes) WHERE value = mb.id
                    ))
                  )
               ) AS unread_count
               FROM mailboxes mb
               WHERE mb.account_id=?1
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
                        last_synced_at: row
                            .get::<_, Option<i64>>(7)?
                            .map(|ts| Utc.timestamp_opt(ts, 0).unwrap()),
                    },
                    thread_count: row.get::<_, i64>(8)? as u32,
                    unread_count: row.get::<_, i64>(9)? as u32,
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(mailboxes)
    }

    pub fn get_mailbox_by_id(&self, account_id: &str, mailbox_id: &str) -> Result<Option<Mailbox>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, account_id, name, delimiter, flags, uid_validity, uid_next, last_synced_at
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
                last_synced_at: row
                    .get::<_, Option<i64>>(7)?
                    .map(|ts| Utc.timestamp_opt(ts, 0).unwrap()),
            }))
        } else {
            Ok(None)
        }
    }

    pub fn get_mailbox_oldest_date(&self, mailbox_id: &str) -> Result<Option<DateTime<Utc>>> {
        let mut stmt = self
            .conn
            .prepare("SELECT MIN(date) FROM messages WHERE mailbox_id=?1")?;
        let date: Option<i64> = stmt.query_row([mailbox_id], |row| row.get(0))?;
        Ok(date.map(|ts| Utc.timestamp_opt(ts, 0).unwrap()))
    }

    pub fn upsert_message(&self, msg: &Message) -> Result<()> {
        let inbox_mailboxes_json = if msg.inbox_mailboxes.is_empty() {
            None
        } else {
            Some(serde_json::to_string(&msg.inbox_mailboxes)?)
        };
        self.conn.execute(
            r#"INSERT INTO messages
               (id, account_id, mailbox_id, uid, message_id, thread_id, subject, "from", "to", cc,
                date, body_text, body_html, references_ids, in_reply_to, flags, has_attachments,
                triage_score, ai_summary, inbox_mailboxes)
               VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,?18,?19,?20)
               ON CONFLICT(id) DO UPDATE SET
               message_id=excluded.message_id, thread_id=excluded.thread_id,
               subject=excluded.subject, "from"=excluded."from", "to"=excluded."to",
               cc=excluded.cc, date=excluded.date, body_text=excluded.body_text,
               body_html=excluded.body_html, references_ids=excluded.references_ids,
               in_reply_to=excluded.in_reply_to, flags=excluded.flags,
               has_attachments=excluded.has_attachments,
               triage_score=COALESCE(excluded.triage_score, messages.triage_score),
               ai_summary=COALESCE(excluded.ai_summary, messages.ai_summary),
               inbox_mailboxes=COALESCE(excluded.inbox_mailboxes, messages.inbox_mailboxes),
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
                inbox_mailboxes_json,
            ],
        )?;
        Ok(())
    }

    pub fn upsert_thread(&self, thread: &Thread) -> Result<()> {
        self.conn.execute(
            r#"INSERT INTO threads
               (id, account_id, subject, participant_ids, message_count, unread_count, is_flagged, has_attachments, last_date, last_from, triage_score, labels)
               VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12)
               ON CONFLICT(id) DO UPDATE SET
               subject=excluded.subject,
               participant_ids=excluded.participant_ids,
               message_count=excluded.message_count,
               unread_count=excluded.unread_count,
               is_flagged=excluded.is_flagged,
               has_attachments=excluded.has_attachments,
               last_date=excluded.last_date,
               last_from=excluded.last_from,
               triage_score=excluded.triage_score,
               labels=excluded.labels,
               updated_at=unixepoch()"#,
            rusqlite::params![
                thread.id, thread.account_id, thread.subject,
                serde_json::to_string(&thread.participants)?,
                thread.message_count as i64, thread.unread_count as i64,
                thread.is_flagged as i64,
                thread.has_attachments as i64,
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
                is_flagged: row.get::<_, i64>(6)? != 0,
                has_attachments: row.get::<_, i64>(7)? != 0,
                last_date: row
                    .get::<_, Option<i64>>(8)?
                    .map(|ts| Utc.timestamp_opt(ts, 0).unwrap()),
                last_from: row.get(9)?,
                triage_score: row.get(10)?,
                labels: serde_json::from_str(&row.get::<_, String>(11).unwrap_or_default())
                    .unwrap_or_default(),
                messages: None,
            })
        };

        let limit = limit as i64;
        let offset = offset as i64;
        let threads = if let Some(mailbox_id) = mailbox_id {
            let mut stmt = self.conn.prepare(
                r#"SELECT t.id, t.account_id, t.subject, t.participant_ids, t.message_count,
                   t.unread_count, t.is_flagged, t.has_attachments, t.last_date, t.last_from, t.triage_score, t.labels
                   FROM threads t
                   WHERE t.account_id=?1
                     AND EXISTS (
                       SELECT 1 FROM messages m
                       WHERE m.thread_id = t.id
                         AND (
                           (m.inbox_mailboxes IS NULL AND m.mailbox_id = ?2)
                           OR (m.inbox_mailboxes IS NOT NULL AND EXISTS (
                             SELECT 1 FROM json_each(m.inbox_mailboxes) WHERE value = ?2
                           ))
                         )
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
                   is_flagged, has_attachments, last_date, last_from, triage_score, labels
                   FROM threads WHERE account_id=?1
                   ORDER BY last_date DESC LIMIT ?2 OFFSET ?3"#,
            )?;
            let rows = stmt.query_map(rusqlite::params![account_id, limit, offset], map_thread)?;
            rows.collect::<rusqlite::Result<Vec<_>>>()?
        };
        Ok(threads)
    }

    pub fn get_threads_in_window(
        &self,
        account_id: &str,
        since_timestamp: i64,
        limit: u32,
    ) -> Result<Vec<Thread>> {
        let limit = limit as i64;
        // Exclude threads that only exist in system folders (Trash, Spam, All Mail).
        // Handles both legacy (mailbox_id join) and Gmail All Mail (inbox_mailboxes) paths.
        let mut stmt = self.conn.prepare(
            r#"SELECT id, account_id, subject, participant_ids, message_count, unread_count,
               is_flagged, has_attachments, last_date, last_from, triage_score, labels
               FROM threads WHERE account_id=?1 AND last_date >= ?2
               AND EXISTS (
                 SELECT 1 FROM messages m
                 WHERE m.thread_id = threads.id
                   AND (
                     (m.inbox_mailboxes IS NULL AND EXISTS (
                       SELECT 1 FROM mailboxes mb WHERE mb.id = m.mailbox_id
                         AND UPPER(mb.name) NOT LIKE '%TRASH%'
                         AND UPPER(mb.name) NOT LIKE '%SPAM%'
                         AND UPPER(mb.name) NOT LIKE '%JUNK%'
                         AND UPPER(mb.name) NOT LIKE '%ALL MAIL%'
                         AND UPPER(mb.name) NOT LIKE '%DRAFT%'
                     ))
                     OR (m.inbox_mailboxes IS NOT NULL AND EXISTS (
                       SELECT 1 FROM json_each(m.inbox_mailboxes) WHERE
                         UPPER(value) NOT LIKE '%TRASH%'
                         AND UPPER(value) NOT LIKE '%SPAM%'
                         AND UPPER(value) NOT LIKE '%JUNK%'
                         AND UPPER(value) NOT LIKE '%ALL MAIL%'
                         AND UPPER(value) NOT LIKE '%DRAFT%'
                     ))
                   )
               )
               ORDER BY COALESCE(triage_score, 0.5) DESC, last_date DESC LIMIT ?3"#,
        )?;
        let threads = stmt
            .query_map(
                rusqlite::params![account_id, since_timestamp, limit],
                |row| {
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
                        is_flagged: row.get::<_, i64>(6)? != 0,
                        has_attachments: row.get::<_, i64>(7)? != 0,
                        last_date: row
                            .get::<_, Option<i64>>(8)?
                            .map(|ts| Utc.timestamp_opt(ts, 0).unwrap()),
                        last_from: row.get(9)?,
                        triage_score: row.get(10)?,
                        labels: serde_json::from_str(
                            &row.get::<_, String>(11).unwrap_or_default(),
                        )
                        .unwrap_or_default(),
                        messages: None,
                    })
                },
            )?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(threads)
    }

    pub fn get_roundup_stats(
        &self,
        account_id: &str,
        since_timestamp: i64,
    ) -> Result<(usize, usize, usize)> {
        // Shared filter: exclude threads that only exist in system folders.
        // Handles both legacy (mailbox_id join) and Gmail All Mail (inbox_mailboxes) paths.
        let system_folder_filter = r#"
            AND EXISTS (
              SELECT 1 FROM messages m
              WHERE m.thread_id = t.id
                AND (
                  (m.inbox_mailboxes IS NULL AND EXISTS (
                    SELECT 1 FROM mailboxes mb WHERE mb.id = m.mailbox_id
                      AND UPPER(mb.name) NOT LIKE '%TRASH%'
                      AND UPPER(mb.name) NOT LIKE '%SPAM%'
                      AND UPPER(mb.name) NOT LIKE '%JUNK%'
                      AND UPPER(mb.name) NOT LIKE '%ALL MAIL%'
                      AND UPPER(mb.name) NOT LIKE '%DRAFT%'
                  ))
                  OR (m.inbox_mailboxes IS NOT NULL AND EXISTS (
                    SELECT 1 FROM json_each(m.inbox_mailboxes) WHERE
                      UPPER(value) NOT LIKE '%TRASH%'
                      AND UPPER(value) NOT LIKE '%SPAM%'
                      AND UPPER(value) NOT LIKE '%JUNK%'
                      AND UPPER(value) NOT LIKE '%ALL MAIL%'
                      AND UPPER(value) NOT LIKE '%DRAFT%'
                  ))
                )
            )"#;

        let total: i64 = self.conn.query_row(
            &format!(
                "SELECT COUNT(*) FROM threads t WHERE t.account_id=?1 AND t.last_date >= ?2 {}",
                system_folder_filter
            ),
            rusqlite::params![account_id, since_timestamp],
            |row| row.get(0),
        )?;

        let unread: i64 = self.conn.query_row(
            &format!(
                "SELECT COUNT(*) FROM threads t WHERE t.account_id=?1 AND t.last_date >= ?2 AND t.unread_count > 0 {}",
                system_folder_filter
            ),
            rusqlite::params![account_id, since_timestamp],
            |row| row.get(0),
        )?;

        let action_items: i64 = self.conn.query_row(
            &format!(
                r#"SELECT COUNT(*) FROM thread_actions ta
                   JOIN threads t ON ta.thread_id = t.id
                   WHERE t.account_id=?1 AND t.last_date >= ?2
                     AND ta.actions_json != '[]' AND ta.actions_json != ''
                   {}"#,
                system_folder_filter
            ),
            rusqlite::params![account_id, since_timestamp],
            |row| row.get(0),
        )?;

        Ok((total as usize, unread as usize, action_items as usize))
    }

    pub fn get_thread_messages(&self, thread_id: &str) -> Result<Vec<Message>> {
        let mut stmt = self.conn.prepare(
            r#"SELECT id, account_id, mailbox_id, uid, message_id, thread_id, subject,
               "from", "to", cc, date, body_text, body_html, references_ids, in_reply_to,
               flags, has_attachments, triage_score, ai_summary, inbox_mailboxes
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
                    inbox_mailboxes: row
                        .get::<_, Option<String>>(19)?
                        .as_deref()
                        .and_then(|s| serde_json::from_str(s).ok())
                        .unwrap_or_default(),
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

    /// Returns a map of message_id header → thread_id for all known messages matching
    /// the given Message-ID headers. Used to detect cross-mailbox duplicates (e.g. Gmail labels).
    pub fn get_thread_ids_by_message_ids(
        &self,
        message_ids: &[String],
    ) -> Result<HashMap<String, String>> {
        if message_ids.is_empty() {
            return Ok(HashMap::new());
        }
        let placeholders = message_ids
            .iter()
            .enumerate()
            .map(|(i, _)| format!("?{}", i + 1))
            .collect::<Vec<_>>()
            .join(",");
        let sql = format!(
            "SELECT message_id, thread_id FROM messages
             WHERE message_id IN ({}) AND message_id IS NOT NULL AND thread_id IS NOT NULL",
            placeholders
        );
        let mut stmt = self.conn.prepare(&sql)?;
        let map = stmt
            .query_map(rusqlite::params_from_iter(message_ids.iter()), |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })?
            .collect::<rusqlite::Result<HashMap<_, _>>>()?;
        Ok(map)
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

    pub fn set_thread_flagged_state(&self, thread_id: &str, flagged: bool) -> Result<()> {
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
            let has_flagged = flags.iter().any(|flag| flag == "\\Flagged");

            if flagged && !has_flagged {
                flags.push("\\Flagged".to_string());
            } else if !flagged && has_flagged {
                flags.retain(|flag| flag != "\\Flagged");
            }

            self.conn.execute(
                "UPDATE messages SET flags=?1 WHERE id=?2",
                rusqlite::params![serde_json::to_string(&flags)?, message_id],
            )?;
        }

        self.conn.execute(
            "UPDATE threads SET is_flagged=?1 WHERE id=?2",
            rusqlite::params![flagged as i64, thread_id],
        )?;
        Ok(())
    }

    pub fn set_threads_flagged_state(&self, thread_ids: &[String], flagged: bool) -> Result<usize> {
        for thread_id in thread_ids {
            self.set_thread_flagged_state(thread_id, flagged)?;
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
        offset: u32,
    ) -> Result<Vec<String>> {
        let (fts_query, filters) = parse_query(query);
        let limit = limit as i64;
        let offset = offset as i64;

        let mut sql = "SELECT m.thread_id FROM messages m ".to_string();
        if !fts_query.is_empty() {
            sql.push_str("JOIN messages_fts fts ON m.rowid = fts.rowid ");
        }
        sql.push_str("WHERE m.account_id = ? ");

        let mut params: Vec<rusqlite::types::Value> = vec![account_id.to_string().into()];

        if let Some(mid) = mailbox_id {
            sql.push_str("AND m.mailbox_id = ? ");
            params.push(mid.to_string().into());
        }

        if !fts_query.is_empty() {
            sql.push_str("AND fts.messages_fts MATCH ? ");
            params.push(fts_query.clone().into());
        }

        if let Some(from) = filters.from {
            sql.push_str("AND m.\"from\" LIKE ? ");
            params.push(format!("%{}%", from).into());
        }

        if let Some(to) = filters.to {
            sql.push_str("AND m.\"to\" LIKE ? ");
            params.push(format!("%{}%", to).into());
        }

        if let Some(unread) = filters.unread {
            if unread {
                sql.push_str("AND instr(COALESCE(m.flags, ''), '\\Seen') = 0 ");
            } else {
                sql.push_str("AND instr(COALESCE(m.flags, ''), '\\Seen') > 0 ");
            }
        }

        if let Some(has_attachment) = filters.has_attachment {
            sql.push_str("AND m.has_attachments = ? ");
            params.push((if has_attachment { 1 } else { 0 }).into());
        }

        sql.push_str("GROUP BY m.thread_id ");

        if !fts_query.is_empty() {
            sql.push_str("ORDER BY MAX(rank) LIMIT ? OFFSET ?");
        } else {
            sql.push_str("ORDER BY MAX(m.date) DESC LIMIT ? OFFSET ?");
        }

        params.push(limit.into());
        params.push(offset.into());

        let mut stmt = self.conn.prepare(&sql)?;
        println!(">>> FTS SQL: {}", sql);
        println!(">>> FTS Params: {:?}", params);
        let rows = stmt.query_map(rusqlite::params_from_iter(params), |row| {
            row.get::<_, Option<String>>(0)
        })?;

        Ok(rows.filter_map(|r| r.ok().flatten()).collect())
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

        let sql = if let Some(_mid) = mailbox_id {
            format!(
                "SELECT DISTINCT t.id, t.account_id, t.subject, t.participant_ids, t.message_count, 
                        t.unread_count, t.is_flagged, t.has_attachments, t.last_date, t.last_from, 
                        t.triage_score, t.labels
                 FROM threads t
                 JOIN messages m ON t.id = m.thread_id
                 WHERE t.id IN ({}) AND m.mailbox_id = ?{}
                 ORDER BY t.last_date DESC",
                placeholders,
                ids.len() + 1
            )
        } else {
            format!(
                "SELECT id, account_id, subject, participant_ids, message_count, unread_count, 
                        is_flagged, has_attachments, last_date, last_from, triage_score, labels
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
                is_flagged: row.get::<_, i64>(6)? != 0,
                has_attachments: row.get::<_, i64>(7)? != 0,
                last_date: row
                    .get::<_, Option<i64>>(8)?
                    .map(|ts| chrono::Utc.timestamp_opt(ts, 0).unwrap()),
                last_from: row.get(9)?,
                triage_score: row.get(10)?,
                labels: serde_json::from_str(&row.get::<_, String>(11).unwrap_or_default())
                    .unwrap_or_default(),
                messages: None,
            })
        };

        let mut stmt = self.conn.prepare(&sql)?;

        let rows = if let Some(mid) = mailbox_id {
            let mut params: Vec<&dyn rusqlite::ToSql> =
                ids.iter().map(|s| s as &dyn rusqlite::ToSql).collect();
            params.push(&mid as &dyn rusqlite::ToSql);
            stmt.query_map(params.as_slice(), map_thread)?
        } else {
            let params: Vec<&dyn rusqlite::ToSql> =
                ids.iter().map(|s| s as &dyn rusqlite::ToSql).collect();
            stmt.query_map(params.as_slice(), map_thread)?
        };

        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }

    pub fn get_message_attachments(&self, message_id: &str) -> Result<Vec<Attachment>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, message_id, filename, content_type, size FROM attachments WHERE message_id=?1",
        )?;
        let rows = stmt.query_map([message_id], |row| {
            Ok(Attachment {
                id: row.get(0)?,
                message_id: row.get(1)?,
                filename: row.get(2)?,
                content_type: row.get(3)?,
                size: row.get::<_, i64>(4)? as u32,
                data: None,
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }

    pub fn get_messages_missing_attachments(&self, mailbox_id: &str) -> Result<Vec<u32>> {
        let mut stmt = self.conn.prepare(
            "SELECT uid FROM messages 
             WHERE mailbox_id=?1 AND has_attachments=1
             AND id NOT IN (SELECT DISTINCT message_id FROM attachments)",
        )?;
        let rows = stmt.query_map([mailbox_id], |row| row.get::<_, i64>(0))?;
        Ok(rows.filter_map(|r| r.ok()).map(|u| u as u32).collect())
    }

    pub fn get_thread_attachments(&self, thread_id: &str) -> Result<Vec<Attachment>> {
        let mut stmt = self.conn.prepare(
            "SELECT a.id, a.message_id, a.filename, a.content_type, a.size 
             FROM attachments a
             JOIN messages m ON a.message_id = m.id
             WHERE m.thread_id=?1
             GROUP BY a.filename, a.size",
        )?;
        let rows = stmt.query_map([thread_id], |row| {
            Ok(Attachment {
                id: row.get(0)?,
                message_id: row.get(1)?,
                filename: row.get(2)?,
                content_type: row.get(3)?,
                size: row.get::<_, i64>(4)? as u32,
                data: None,
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }

    pub fn get_attachment_by_id(&self, id: &str) -> Result<Option<Attachment>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, message_id, filename, content_type, size, data FROM attachments WHERE id=?1",
        )?;
        let mut rows = stmt.query([id])?;
        if let Some(row) = rows.next()? {
            Ok(Some(Attachment {
                id: row.get(0)?,
                message_id: row.get(1)?,
                filename: row.get(2)?,
                content_type: row.get(3)?,
                size: row.get::<_, i64>(4)? as u32,
                data: row.get(5)?,
            }))
        } else {
            Ok(None)
        }
    }

    pub fn upsert_attachment(&self, att: &Attachment) -> Result<()> {
        self.conn.execute(
            "INSERT INTO attachments (id, message_id, filename, content_type, size, data)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)
             ON CONFLICT(message_id, filename, size) DO UPDATE SET
             id=excluded.id, content_type=excluded.content_type, data=excluded.data",
            rusqlite::params![
                att.id,
                att.message_id,
                att.filename,
                att.content_type,
                att.size as i64,
                att.data
            ],
        )?;
        Ok(())
    }

    pub fn delete_message_attachments(&self, message_id: &str) -> Result<()> {
        self.conn
            .execute("DELETE FROM attachments WHERE message_id=?1", [message_id])?;
        Ok(())
    }

    pub fn get_counts(&self) -> Result<HashMap<String, i64>> {
        let mut counts = HashMap::new();
        counts.insert(
            "accounts".to_string(),
            self.conn
                .query_row("SELECT count(*) FROM accounts", [], |r| r.get(0))?,
        );
        counts.insert(
            "mailboxes".to_string(),
            self.conn
                .query_row("SELECT count(*) FROM mailboxes", [], |r| r.get(0))?,
        );
        counts.insert(
            "threads".to_string(),
            self.conn
                .query_row("SELECT count(*) FROM threads", [], |r| r.get(0))?,
        );
        counts.insert(
            "messages".to_string(),
            self.conn
                .query_row("SELECT count(*) FROM messages", [], |r| r.get(0))?,
        );
        counts.insert(
            "attachments".to_string(),
            self.conn
                .query_row("SELECT count(*) FROM attachments", [], |r| r.get(0))?,
        );
        Ok(counts)
    }

    pub fn delete_all_attachments(&self) -> Result<()> {
        self.conn.execute("DELETE FROM attachments", [])?;
        Ok(())
    }

    pub fn wipe_data(&self) -> Result<()> {
        self.conn.execute("DELETE FROM attachments", [])?;
        self.conn.execute("DELETE FROM thread_actions", [])?;
        self.conn.execute("DELETE FROM thread_embeddings", [])?;
        self.conn.execute("DELETE FROM messages", [])?;
        self.conn.execute("DELETE FROM threads", [])?;
        self.conn.execute("DELETE FROM mailboxes", [])?;
        // Preservation of accounts table as requested
        Ok(())
    }

    pub fn drop_tables(&mut self) -> Result<()> {
        // We drop in reverse order of dependencies
        self.conn.execute("DROP TABLE IF EXISTS attachments", [])?;
        self.conn.execute("DROP TABLE IF EXISTS messages_fts", [])?;
        self.conn
            .execute("DROP TABLE IF EXISTS thread_embeddings", [])?;
        self.conn
            .execute("DROP TABLE IF EXISTS thread_actions", [])?;
        self.conn.execute("DROP TABLE IF EXISTS messages", [])?;
        self.conn.execute("DROP TABLE IF EXISTS threads", [])?;
        self.conn.execute("DROP TABLE IF EXISTS mailboxes", [])?;

        // Re-run migrations to recreate them
        crate::db::schema::run_migrations(&mut self.conn)?;
        Ok(())
    }

    pub fn upsert_thread_embedding(
        &self,
        thread_id: &str,
        model: &str,
        embedding: &[f32],
    ) -> Result<()> {
        let blob: Vec<u8> = embedding
            .iter()
            .flat_map(|f| f.to_ne_bytes().to_vec())
            .collect();

        self.conn.execute(
            "INSERT INTO thread_embeddings (thread_id, model, embedding, updated_at)
             VALUES (?1, ?2, ?3, unixepoch())
             ON CONFLICT(thread_id) DO UPDATE SET
             model=excluded.model, embedding=excluded.embedding, updated_at=unixepoch()",
            rusqlite::params![thread_id, model, blob],
        )?;
        Ok(())
    }

    pub fn semantic_search(
        &self,
        account_id: &str,
        query_embedding: &[f32],
        model: &str,
        limit: usize,
        offset: usize,
    ) -> Result<Vec<(String, f32)>> {
        let mut stmt = self.conn.prepare(
            "SELECT e.thread_id, e.embedding
             FROM thread_embeddings e
             JOIN threads t ON e.thread_id = t.id
             WHERE t.account_id = ?1 AND e.model = ?2",
        )?;

        let matches = stmt.query_map(rusqlite::params![account_id, model], |row| {
            let thread_id: String = row.get(0)?;
            let blob: Vec<u8> = row.get(1)?;

            // Convert Vec<u8> to Vec<f32> safely
            let embedding: Vec<f32> = blob
                .chunks_exact(4)
                .map(|chunk| {
                    let mut array = [0u8; 4];
                    array.copy_from_slice(chunk);
                    f32::from_ne_bytes(array)
                })
                .collect();

            let similarity = cosine_similarity(query_embedding, &embedding);
            Ok((thread_id, similarity))
        })?;

        let mut results: Vec<(String, f32)> = matches.collect::<rusqlite::Result<Vec<_>>>()?;

        // Filter out zero similarity (likely error or empty embeddings)
        results.retain(|(_, sim)| *sim > 0.0);

        results.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        if !results.is_empty() {
            tracing::info!(
                "Top semantic match: {} (score: {:.4})",
                results[0].0,
                results[0].1
            );
        }

        // Apply offset and limit manually for now since we sort in memory
        let start = offset.min(results.len());
        let end = (offset + limit).min(results.len());
        Ok(results[start..end].to_vec())
    }

    // ── Draft auto-save ───────────────────────────────────────────────────────

    pub fn save_draft(&self, draft: &Draft) -> Result<()> {
        self.conn.execute(
            r#"INSERT INTO drafts (id, account_id, mode, to_addrs, cc_addrs, bcc_addrs, subject,
                                   body_text, body_html, in_reply_to, thread_id, updated_at)
               VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, unixepoch())
               ON CONFLICT(id) DO UPDATE SET
                 account_id=excluded.account_id, mode=excluded.mode,
                 to_addrs=excluded.to_addrs, cc_addrs=excluded.cc_addrs,
                 bcc_addrs=excluded.bcc_addrs, subject=excluded.subject,
                 body_text=excluded.body_text, body_html=excluded.body_html,
                 in_reply_to=excluded.in_reply_to, thread_id=excluded.thread_id,
                 updated_at=unixepoch()"#,
            rusqlite::params![
                draft.id, draft.account_id, draft.mode,
                draft.to_addrs, draft.cc_addrs, draft.bcc_addrs,
                draft.subject, draft.body_text, draft.body_html,
                draft.in_reply_to, draft.thread_id,
            ],
        )?;
        Ok(())
    }

    pub fn get_draft(&self, id: &str) -> Result<Option<Draft>> {
        let mut stmt = self.conn.prepare(
            r#"SELECT id, account_id, mode, to_addrs, cc_addrs, bcc_addrs, subject,
                      body_text, body_html, in_reply_to, thread_id, updated_at
               FROM drafts WHERE id=?1"#,
        )?;
        let mut rows = stmt.query([id])?;
        if let Some(row) = rows.next()? {
            Ok(Some(Draft {
                id: row.get(0)?,
                account_id: row.get(1)?,
                mode: row.get(2)?,
                to_addrs: row.get(3)?,
                cc_addrs: row.get(4)?,
                bcc_addrs: row.get(5)?,
                subject: row.get(6)?,
                body_text: row.get(7)?,
                body_html: row.get(8)?,
                in_reply_to: row.get(9)?,
                thread_id: row.get(10)?,
                updated_at: row.get(11)?,
            }))
        } else {
            Ok(None)
        }
    }

    pub fn delete_draft(&self, id: &str) -> Result<()> {
        self.conn.execute("DELETE FROM drafts WHERE id=?1", [id])?;
        Ok(())
    }
}

fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() {
        tracing::warn!(
            "Cosine similarity length mismatch: query {} vs stored {}. Reindexing may be required.",
            a.len(),
            b.len()
        );
        return 0.0;
    }
    let mut dot = 0.0;
    let mut norm_a = 0.0;
    let mut norm_b = 0.0;
    for i in 0..a.len() {
        dot += a[i] * b[i];
        norm_a += a[i] * a[i];
        norm_b += b[i] * b[i];
    }
    if norm_a == 0.0 || norm_b == 0.0 {
        return 0.0;
    }
    dot / (norm_a.sqrt() * norm_b.sqrt())
}

#[derive(Default)]
struct SearchFilters {
    from: Option<String>,
    to: Option<String>,
    unread: Option<bool>,
    has_attachment: Option<bool>,
}

fn parse_query(query: &str) -> (String, SearchFilters) {
    let mut fts_parts = Vec::new();
    let mut filters = SearchFilters::default();

    for part in query.split_whitespace() {
        if let Some(from) = part.strip_prefix("from:") {
            filters.from = Some(from.to_string());
        } else if let Some(to) = part.strip_prefix("to:") {
            filters.to = Some(to.to_string());
        } else if let Some(is) = part.strip_prefix("is:") {
            match is {
                "unread" => filters.unread = Some(true),
                "read" => filters.unread = Some(false),
                _ => fts_parts.push(part),
            }
        } else if let Some(has) = part.strip_prefix("has:") {
            if has == "attachment" {
                filters.has_attachment = Some(true);
            } else {
                fts_parts.push(part);
            }
        } else {
            fts_parts.push(part);
        }
    }

    (fts_parts.join(" "), filters)
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
                last_synced_at: None,
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
            last_synced_at: None,
        };
        let archive = Mailbox {
            id: "acc1:Archive".to_string(),
            account_id: account.id.clone(),
            name: "Archive".to_string(),
            delimiter: Some("/".to_string()),
            flags: Vec::new(),
            uid_validity: None,
            uid_next: None,
            last_synced_at: None,
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
            is_flagged: false,
            has_attachments: false,
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
            is_flagged: false,
            has_attachments: false,
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
            inbox_mailboxes: Vec::new(),
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
            last_synced_at: None,
        };
        db.upsert_mailbox(&inbox).expect("inbox");

        let unread_thread = Thread {
            id: "thread-unread".to_string(),
            account_id: account.id.clone(),
            subject: Some("Unread".to_string()),
            participants: Vec::new(),
            message_count: 1,
            unread_count: 1,
            is_flagged: false,
            has_attachments: false,
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
            is_flagged: false,
            has_attachments: false,
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
            inbox_mailboxes: Vec::new(),
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
            inbox_mailboxes: Vec::new(),
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
            last_synced_at: None,
        };
        let archive = Mailbox {
            id: "acc1:Archive".to_string(),
            account_id: account.id.clone(),
            name: "Archive".to_string(),
            delimiter: Some("/".to_string()),
            flags: Vec::new(),
            uid_validity: None,
            uid_next: None,
            last_synced_at: None,
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
            is_flagged: false,
            has_attachments: false,
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
            is_flagged: false,
            has_attachments: false,
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
                inbox_mailboxes: Vec::new(),
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
