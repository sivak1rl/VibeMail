# VibeMail — Roadmap & Future Direction

## Current State

Working end-to-end: Gmail OAuth sign-in, IMAP sync with graduated batching (newest-first), SQLite storage with FTS5, dark-themed inbox/thread view, compose editor, and AI provider scaffolding (Ollama + OpenAI-compatible). Outlook OAuth and generic IMAP also supported. Gmail syncs exclusively from [Gmail]/All Mail using X-GM-LABELS for folder membership. Email roundup digest available as a standalone window with inline AI actions.

### What's Built

- **Sync**: Background auto-sync, IMAP IDLE push, multi-mailbox sync, flag sync, sliding window, historical fetch, Gmail All Mail single-pass sync
- **UI/UX**: Keyboard shortcuts, bulk selection, tree folder selector, lightbox view, responsive layout, collapsible sidebar, pull-to-refresh, empty states, loading skeletons, error boundaries
- **Compose & Drafts**: Rich text (Tiptap), reply/reply all/forward, drag-and-drop attachments, CC/BCC, contact autocomplete, signatures, auto-save drafts (local + IMAP sync), drafts folder browser with inline editing, AI compose panel with proofread
- **Search**: Search-as-you-type, filters (from/to/has:attachment/is:unread), highlighting, history, semantic search, global search, paginated results
- **AI**: Thread summarization (single + batch), smart reply with tone selector, context-aware replies, action item extraction, auto-labeling, custom categories, email roundup digest with inline actions
- **Performance**: Denormalized joins, indexed boolean columns, batch updates, precomputed mailbox counts, thread_mailboxes join table, folder_role classification, batch inserts with transactions

---

## Short Term — Polish the MVP

### Sync & Reliability
- [ ] Retry logic with exponential backoff on transient failures
- [ ] Detect and handle token revocation gracefully (re-prompt OAuth)

### AI
- [ ] Summary caching — store in DB, invalidate when new messages arrive
- [ ] Configurable summary length (brief / detailed)

---

## Medium Term — AI & Triage

### Triage & Priority
- [ ] Auto-triage on sync — score every new thread
- [ ] Focus inbox: separate "Important" and "Other" tabs
- [ ] Custom triage rules (always important: boss@company.com)

### Action Extraction
- [ ] Parse deadlines, meeting times, TODOs from email body
- [ ] Task list view — all extracted actions across threads

---

## Long Term — Platform

### Multi-Account & Identity
- [ ] Unified inbox across all accounts
- [ ] Per-account color coding
- [ ] Send-as / alias support

### Privacy & Security
- [ ] End-to-end encryption support (PGP/GPG)
- [ ] S/MIME signature verification
- [ ] Phishing detection via AI
- [ ] Privacy mode — strip tracking pixels, block remote images by default
- [ ] Local-only mode — all data stays on device, no cloud AI

### Performance & Scale
- [ ] Incremental Tantivy index updates (currently rebuilds)
- [ ] Message body lazy-loading (fetch on demand, not on sync)
- [ ] Connection pooling — reuse IMAP sessions across syncs
- [ ] Proper IMAP LOGOUT on app quit

### Platform Expansion
- [ ] System tray icon with unread count badge
- [ ] Native OS notifications (new mail, action reminders)
- [ ] Global keyboard shortcut to open/compose
- [ ] Auto-start on login (optional)
- [ ] Snooze: hide thread until a specified time

### Plugin / Extension System
- [ ] Lua or WASM plugin API for custom email processing
- [ ] Webhook integrations (Slack, Discord, Telegram on new mail)
- [ ] Custom AI prompt templates per category

### Data & Export
- [ ] Export mailbox to mbox/eml format
- [ ] Import from Thunderbird, Apple Mail, Gmail Takeout
- [ ] Analytics dashboard — emails sent/received per day, response times

---

## Technical Debt

- [ ] Replace file-based token store with OS keychain (fix Linux secret-service detection)
- [ ] Structured logging — migrate 31 println!/eprintln! statements to `tracing` crate (already imported)
- [ ] Rate limiting on token refresh (don't refresh on every IMAP connect)
- [ ] Add comprehensive test suite (Rust unit tests, React component tests — currently only 5 Rust tests, zero React tests)

---

## Non-Goals (Intentional)

- **Not a webmail clone** — no browser-based access, desktop-first
- **Not a calendar app** — may extract events, but no built-in calendar UI
- **Not a chat app** — email only, no Slack/Teams integration in core
- **No server component** — fully local, no VibeMail cloud service
