use anyhow::Result;
use rusqlite::Connection;

pub fn run_migrations(conn: &mut Connection) -> Result<()> {
    conn.execute_batch(SCHEMA)?;

    // Manual migrations for schema updates
    let has_is_flagged: i64 = conn.query_row(
        "SELECT count(*) FROM pragma_table_info('threads') WHERE name='is_flagged'",
        [],
        |row| row.get(0),
    )?;

    if has_is_flagged == 0 {
        conn.execute(
            "ALTER TABLE threads ADD COLUMN is_flagged INTEGER NOT NULL DEFAULT 0",
            [],
        )?;
    }

    let has_has_attachments: i64 = conn.query_row(
        "SELECT count(*) FROM pragma_table_info('threads') WHERE name='has_attachments'",
        [],
        |row| row.get(0),
    )?;

    if has_has_attachments == 0 {
        conn.execute(
            "ALTER TABLE threads ADD COLUMN has_attachments INTEGER NOT NULL DEFAULT 0",
            [],
        )?;
    }

    let has_last_synced_at: i64 = conn.query_row(
        "SELECT count(*) FROM pragma_table_info('mailboxes') WHERE name='last_synced_at'",
        [],
        |row| row.get(0),
    )?;

    if has_last_synced_at == 0 {
        conn.execute(
            "ALTER TABLE mailboxes ADD COLUMN last_synced_at INTEGER",
            [],
        )?;
    }

    // Rename mailbox_id → source_mailbox_id on messages table.
    // This column is the IMAP source mailbox (UID namespace), NOT logical folder membership.
    let has_source_mailbox_id: i64 = conn.query_row(
        "SELECT count(*) FROM pragma_table_info('messages') WHERE name='source_mailbox_id'",
        [],
        |row| row.get(0),
    )?;
    if has_source_mailbox_id == 0 {
        let has_old_mailbox_id: i64 = conn.query_row(
            "SELECT count(*) FROM pragma_table_info('messages') WHERE name='mailbox_id'",
            [],
            |row| row.get(0),
        )?;
        if has_old_mailbox_id > 0 {
            conn.execute("ALTER TABLE messages RENAME COLUMN mailbox_id TO source_mailbox_id", [])?;
        }
    }

    // Drop the legacy inbox_mailboxes JSON column (replaced by message_mailboxes join table).
    let has_inbox_mailboxes: i64 = conn.query_row(
        "SELECT count(*) FROM pragma_table_info('messages') WHERE name='inbox_mailboxes'",
        [],
        |row| row.get(0),
    )?;
    if has_inbox_mailboxes > 0 {
        conn.execute("ALTER TABLE messages DROP COLUMN inbox_mailboxes", [])?;
    }

    // is_read / is_flagged: denormalized booleans for fast filtering.
    // Replaces instr(flags, '\\Seen') string searches in hot-path queries.
    let has_is_read: i64 = conn.query_row(
        "SELECT count(*) FROM pragma_table_info('messages') WHERE name='is_read'",
        [],
        |row| row.get(0),
    )?;
    if has_is_read == 0 {
        conn.execute_batch(
            "ALTER TABLE messages ADD COLUMN is_read INTEGER NOT NULL DEFAULT 0;
             ALTER TABLE messages ADD COLUMN is_flagged INTEGER NOT NULL DEFAULT 0;",
        )?;
        // Backfill from existing flags JSON
        conn.execute(
            "UPDATE messages SET is_read = 1 WHERE instr(COALESCE(flags, ''), '\\Seen') > 0",
            [],
        )?;
        conn.execute(
            "UPDATE messages SET is_flagged = 1 WHERE instr(COALESCE(flags, ''), '\\Flagged') > 0",
            [],
        )?;
    }

    // Denormalized join table for fast mailbox→message lookups.
    // Replaces json_each(inbox_mailboxes) in hot-path queries.
    conn.execute_batch(
        r#"
        CREATE TABLE IF NOT EXISTS message_mailboxes (
            message_id  TEXT NOT NULL REFERENCES messages(id) ON DELETE CASCADE,
            mailbox_id  TEXT NOT NULL,
            PRIMARY KEY (message_id, mailbox_id)
        );
        CREATE INDEX IF NOT EXISTS idx_msgmb_mailbox_msg ON message_mailboxes(mailbox_id, message_id);
        "#,
    )?;

    // Backfill: ensure every message has at least its source mailbox in message_mailboxes.
    conn.execute(
        r#"INSERT OR IGNORE INTO message_mailboxes (message_id, mailbox_id)
           SELECT id, source_mailbox_id FROM messages"#,
        [],
    )?;

    // Denormalized thread↔mailbox join table for fast thread-by-mailbox lookups.
    // Replaces EXISTS(SELECT 1 FROM messages JOIN message_mailboxes ...) in hot-path queries.
    conn.execute_batch(
        r#"
        CREATE TABLE IF NOT EXISTS thread_mailboxes (
            thread_id   TEXT NOT NULL REFERENCES threads(id) ON DELETE CASCADE,
            mailbox_id  TEXT NOT NULL,
            PRIMARY KEY (thread_id, mailbox_id)
        );
        CREATE INDEX IF NOT EXISTS idx_thrmb_mailbox_thread ON thread_mailboxes(mailbox_id, thread_id);
        "#,
    )?;

    // Precomputed mailbox counts: avoid expensive correlated subqueries
    // on every sidebar render.
    let has_thread_count: i64 = conn.query_row(
        "SELECT count(*) FROM pragma_table_info('mailboxes') WHERE name='thread_count'",
        [],
        |row| row.get(0),
    )?;
    if has_thread_count == 0 {
        conn.execute_batch(
            "ALTER TABLE mailboxes ADD COLUMN thread_count INTEGER NOT NULL DEFAULT 0;
             ALTER TABLE mailboxes ADD COLUMN unread_count INTEGER NOT NULL DEFAULT 0;",
        )?;
    }

    // Classify system folders: add folder_role for fast filtering without LIKE patterns.
    let has_folder_role: i64 = conn.query_row(
        "SELECT count(*) FROM pragma_table_info('mailboxes') WHERE name='folder_role'",
        [],
        |row| row.get(0),
    )?;
    if has_folder_role == 0 {
        conn.execute(
            "ALTER TABLE mailboxes ADD COLUMN folder_role TEXT",
            [],
        )?;
        // Backfill from folder names
        conn.execute_batch(
            "UPDATE mailboxes SET folder_role = 'inbox' WHERE UPPER(name) = 'INBOX';
             UPDATE mailboxes SET folder_role = 'sent' WHERE UPPER(name) LIKE '%SENT%' AND folder_role IS NULL;
             UPDATE mailboxes SET folder_role = 'drafts' WHERE UPPER(name) LIKE '%DRAFT%' AND folder_role IS NULL;
             UPDATE mailboxes SET folder_role = 'trash' WHERE UPPER(name) LIKE '%TRASH%' AND folder_role IS NULL;
             UPDATE mailboxes SET folder_role = 'spam' WHERE (UPPER(name) LIKE '%SPAM%' OR UPPER(name) LIKE '%JUNK%') AND folder_role IS NULL;
             UPDATE mailboxes SET folder_role = 'all_mail' WHERE UPPER(name) LIKE '%ALL MAIL%' AND folder_role IS NULL;
             UPDATE mailboxes SET folder_role = 'starred' WHERE UPPER(name) LIKE '%STAR%' AND folder_role IS NULL;
             UPDATE mailboxes SET folder_role = 'important' WHERE UPPER(name) LIKE '%IMPORTANT%' AND folder_role IS NULL;",
        )?;
    }

    // Auto-cleanup: delete orphaned threads when all their messages are gone.
    conn.execute_batch(
        r#"
        CREATE TRIGGER IF NOT EXISTS trg_delete_orphan_thread
        AFTER DELETE ON messages
        FOR EACH ROW
        WHEN OLD.thread_id IS NOT NULL
             AND NOT EXISTS (SELECT 1 FROM messages WHERE thread_id = OLD.thread_id)
        BEGIN
            DELETE FROM threads WHERE id = OLD.thread_id;
        END;
        "#,
    )?;

    Ok(())
}

const SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS accounts (
    id          TEXT PRIMARY KEY,
    name        TEXT NOT NULL,
    email       TEXT NOT NULL UNIQUE,
    provider    TEXT NOT NULL,  -- 'gmail' | 'outlook' | 'generic'
    imap_host   TEXT NOT NULL,
    imap_port   INTEGER NOT NULL,
    smtp_host   TEXT NOT NULL,
    smtp_port   INTEGER NOT NULL,
    created_at  INTEGER NOT NULL DEFAULT (unixepoch())
);

CREATE TABLE IF NOT EXISTS mailboxes (
    id          TEXT PRIMARY KEY,
    account_id  TEXT NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
    name        TEXT NOT NULL,
    delimiter   TEXT,
    flags           TEXT NOT NULL,          -- JSON array
    uid_validity    INTEGER,
    uid_next        INTEGER,
    last_synced_at  INTEGER,
    thread_count    INTEGER NOT NULL DEFAULT 0,
    unread_count    INTEGER NOT NULL DEFAULT 0,
    folder_role     TEXT,               -- inbox|sent|drafts|trash|spam|all_mail|starred|important
    UNIQUE(account_id, name)
    );


CREATE TABLE IF NOT EXISTS messages (
    id              TEXT PRIMARY KEY,       -- <account_id>:<uid>
    account_id      TEXT NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
    source_mailbox_id TEXT NOT NULL REFERENCES mailboxes(id) ON DELETE CASCADE,  -- IMAP source (UID namespace)
    uid             INTEGER NOT NULL,
    message_id      TEXT,                   -- Message-ID header
    thread_id       TEXT REFERENCES threads(id) ON DELETE SET NULL,
    subject         TEXT,
    "from"          TEXT,                   -- JSON: [{name, email}]
    "to"            TEXT,                   -- JSON: [{name, email}]
    cc              TEXT,                   -- JSON: [{name, email}]
    date            INTEGER,                -- Unix timestamp
    body_text       TEXT,
    body_html       TEXT,
    references_ids  TEXT,                   -- JSON array of Message-IDs
    in_reply_to     TEXT,
    flags           TEXT,                   -- JSON array: \Seen, \Flagged, etc.
    is_read         INTEGER NOT NULL DEFAULT 0,
    is_flagged      INTEGER NOT NULL DEFAULT 0,
    has_attachments INTEGER NOT NULL DEFAULT 0,
    triage_score    REAL,                   -- 0.0–1.0, higher = more important
    ai_summary      TEXT,
    synced_at       INTEGER NOT NULL DEFAULT (unixepoch()),
    UNIQUE(account_id, source_mailbox_id, uid)
);

CREATE INDEX IF NOT EXISTS idx_messages_thread ON messages(thread_id);
CREATE INDEX IF NOT EXISTS idx_messages_account_date ON messages(account_id, date DESC);
CREATE INDEX IF NOT EXISTS idx_messages_message_id ON messages(message_id);
CREATE INDEX IF NOT EXISTS idx_messages_thread_read ON messages(thread_id, is_read);
CREATE INDEX IF NOT EXISTS idx_messages_thread_flagged ON messages(thread_id, is_flagged);

CREATE TABLE IF NOT EXISTS threads (
    id              TEXT PRIMARY KEY,
    account_id      TEXT NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
    subject         TEXT,
    participant_ids TEXT,                   -- JSON array of email addresses
    message_count   INTEGER NOT NULL DEFAULT 1,
    unread_count    INTEGER NOT NULL DEFAULT 0,
    is_flagged      INTEGER NOT NULL DEFAULT 0,
    has_attachments INTEGER NOT NULL DEFAULT 0,
    last_date       INTEGER,
    last_from       TEXT,
    triage_score    REAL,
    labels          TEXT,                   -- JSON array
    updated_at      INTEGER NOT NULL DEFAULT (unixepoch())
);

CREATE INDEX IF NOT EXISTS idx_threads_account_date ON threads(account_id, last_date DESC);

CREATE TABLE IF NOT EXISTS attachments (
    id          TEXT PRIMARY KEY,
    message_id  TEXT NOT NULL REFERENCES messages(id) ON DELETE CASCADE,
    filename    TEXT,
    content_type TEXT,
    size        INTEGER,
    data        BLOB,                       -- only stored for small attachments
    UNIQUE(message_id, filename, size)
);

CREATE TABLE IF NOT EXISTS ai_config (
    id          INTEGER PRIMARY KEY CHECK (id = 1),
    provider    TEXT NOT NULL DEFAULT 'ollama',  -- 'ollama' | 'openai_compat'
    base_url    TEXT NOT NULL DEFAULT 'http://localhost:11434',
    model_triage TEXT NOT NULL DEFAULT 'llama3.2:3b',
    model_summary TEXT NOT NULL DEFAULT 'llama3.1:8b',
    model_draft TEXT NOT NULL DEFAULT 'llama3.1:8b',
    model_extract TEXT NOT NULL DEFAULT 'llama3.2:3b',
    model_embed TEXT NOT NULL DEFAULT 'nomic-embed-text',
    privacy_mode INTEGER NOT NULL DEFAULT 0,    -- 1 = strip PII before sending
    enabled     INTEGER NOT NULL DEFAULT 1
);

-- Seed default AI config
INSERT OR IGNORE INTO ai_config (id) VALUES (1);

CREATE TABLE IF NOT EXISTS ai_log (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    feature     TEXT NOT NULL,
    provider    TEXT NOT NULL,
    model       TEXT NOT NULL,
    prompt_tokens INTEGER,
    completion_tokens INTEGER,
    created_at  INTEGER NOT NULL DEFAULT (unixepoch())
);

CREATE TABLE IF NOT EXISTS thread_actions (
    thread_id    TEXT PRIMARY KEY REFERENCES threads(id) ON DELETE CASCADE,
    actions_json TEXT NOT NULL,
    updated_at   INTEGER NOT NULL DEFAULT (unixepoch())
);

CREATE TABLE IF NOT EXISTS thread_embeddings (
    thread_id    TEXT PRIMARY KEY REFERENCES threads(id) ON DELETE CASCADE,
    model        TEXT NOT NULL,
    embedding    BLOB NOT NULL,             -- Float32 array
    updated_at   INTEGER NOT NULL DEFAULT (unixepoch())
);

CREATE INDEX IF NOT EXISTS idx_embeddings_model ON thread_embeddings(model);

CREATE TABLE IF NOT EXISTS drafts (
    id          TEXT PRIMARY KEY,
    account_id  TEXT,
    mode        TEXT NOT NULL DEFAULT 'new',
    to_addrs    TEXT NOT NULL DEFAULT '',
    cc_addrs    TEXT NOT NULL DEFAULT '',
    bcc_addrs   TEXT NOT NULL DEFAULT '',
    subject     TEXT NOT NULL DEFAULT '',
    body_text   TEXT NOT NULL DEFAULT '',
    body_html   TEXT,
    in_reply_to TEXT,
    thread_id   TEXT,
    created_at  INTEGER NOT NULL DEFAULT (unixepoch()),
    updated_at  INTEGER NOT NULL DEFAULT (unixepoch())
);

-- Full-text search
CREATE VIRTUAL TABLE IF NOT EXISTS messages_fts USING fts5(
    subject,
    body_text,
    sender,
    content='messages',
    content_rowid='rowid'
);

CREATE TRIGGER IF NOT EXISTS messages_fts_insert AFTER INSERT ON messages BEGIN
    INSERT INTO messages_fts(rowid, subject, body_text, sender)
    VALUES (new.rowid, new.subject, new.body_text, new."from");
END;

CREATE TRIGGER IF NOT EXISTS messages_fts_update AFTER UPDATE ON messages BEGIN
    INSERT INTO messages_fts(messages_fts, rowid, subject, body_text, sender)
    VALUES ('delete', old.rowid, old.subject, old.body_text, old."from");
    INSERT INTO messages_fts(rowid, subject, body_text, sender)
    VALUES (new.rowid, new.subject, new.body_text, new."from");
END;

CREATE TRIGGER IF NOT EXISTS messages_fts_delete AFTER DELETE ON messages BEGIN
    INSERT INTO messages_fts(messages_fts, rowid, subject, body_text, sender)
    VALUES ('delete', old.rowid, old.subject, old.body_text, old."from");
END;
"#;
