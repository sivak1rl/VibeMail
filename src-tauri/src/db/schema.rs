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

    // Gmail All Mail source-of-truth: stores all mailbox IDs a message belongs to,
    // derived from X-GM-LABELS. NULL for messages synced the old way (mailbox_id is canonical).
    let has_inbox_mailboxes: i64 = conn.query_row(
        "SELECT count(*) FROM pragma_table_info('messages') WHERE name='inbox_mailboxes'",
        [],
        |row| row.get(0),
    )?;

    if has_inbox_mailboxes == 0 {
        conn.execute(
            "ALTER TABLE messages ADD COLUMN inbox_mailboxes TEXT",
            [],
        )?;
    }

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
    UNIQUE(account_id, name)
    );


CREATE TABLE IF NOT EXISTS messages (
    id              TEXT PRIMARY KEY,       -- <account_id>:<uid>
    account_id      TEXT NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
    mailbox_id      TEXT NOT NULL REFERENCES mailboxes(id) ON DELETE CASCADE,
    uid             INTEGER NOT NULL,
    message_id      TEXT,                   -- Message-ID header
    thread_id       TEXT,                   -- computed thread grouping
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
    has_attachments INTEGER NOT NULL DEFAULT 0,
    triage_score    REAL,                   -- 0.0–1.0, higher = more important
    ai_summary      TEXT,
    synced_at       INTEGER NOT NULL DEFAULT (unixepoch()),
    UNIQUE(account_id, mailbox_id, uid)
);

CREATE INDEX IF NOT EXISTS idx_messages_thread ON messages(thread_id);
CREATE INDEX IF NOT EXISTS idx_messages_account_date ON messages(account_id, date DESC);
CREATE INDEX IF NOT EXISTS idx_messages_message_id ON messages(message_id);

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
