# VibeMail — Roadmap & Future Direction

## Current State (MVP)

Working end-to-end: Gmail OAuth sign-in, IMAP sync with graduated batching (newest-first), SQLite storage with FTS5, dark-themed inbox/thread view, compose editor, and AI provider scaffolding (Ollama + OpenAI-compatible). Outlook OAuth and generic IMAP also supported.

---

## Short Term — Polish the MVP

### Sync & Reliability
- [x] Background auto-sync on a timer (configurable, default 15 minutes)
- [ ] IMAP IDLE push notifications for real-time new mail
- [ ] Retry logic with exponential backoff on transient failures
- [x] Sync multiple mailboxes (Sync All button + background progress)
- [x] Flag sync — mark read/unread, star/flag changes propagated back to server
- [ ] Detect and handle token revocation gracefully (re-prompt OAuth)

### UI/UX
- [ ] Keyboard shortcuts (j/k navigation, r reply, a archive, e mark read)
- [ ] Swipe actions on thread list (archive, delete, snooze)
- [x] Unread count badge in sidebar per mailbox
- [x] Bulk thread selection (checkboxes + shift-select range)
- [x] Tree-like folder selector (Gmail style nesting)
- [x] Lightbox email view (expand to full focused view)
- [x] Two-column Settings UI with navigation hotlinks
- [ ] "Pull to refresh" gesture on thread list
- [ ] Empty state illustrations (no mail, no search results)
- [ ] Loading skeleton improvements during sync
- [x] Background progress indicators (sidebar status + reindex toast)
- [x] Responsive layout — collapsible sidebar, mobile-friendly thread view
- [x] Thread view: collapse/expand individual messages
- [x] Inline image rendering in HTML emails
- [x] Attachment download and preview (PDF, images)

### Compose
- [ ] Rich text editor (bold, italic, links, lists)
- [ ] Reply/Reply All/Forward with quoted original message
- [ ] Attachment upload via drag-and-drop
- [ ] Draft auto-save to local DB
- [ ] CC/BCC fields
- [ ] Contact autocomplete from message history
- [ ] Signature configuration per account

### Search
- [x] Search-as-you-type with debounce
- [x] Search filters: from, to, has:attachment, is:unread
- [x] Search result highlighting
- [x] Recent search history
- [x] Semantic Search (AI-powered meaningful search)
- [x] Global search (search all folders at once)
- [x] Paginated search results (infinite scroll)

---

## Medium Term — AI Features

### Summarization
- [x] One-click thread summary (already scaffolded, needs UI wiring)
- [x] Batch summarize — summarize all unread threads at once
- [ ] Summary caching — store in DB, invalidate when new messages arrive
- [ ] Configurable summary length (brief / detailed)

### Smart Reply
- [ ] AI-generated reply suggestions (3 options: brief, detailed, decline)
- [ ] Tone selector: professional, casual, friendly
- [ ] Context-aware: uses full thread history, not just last message
- [ ] "Edit and send" flow — AI drafts, user refines, then sends

### Triage & Priority
- [ ] Auto-triage on sync — score every new thread
- [ ] Focus inbox: separate "Important" and "Other" tabs
- [ ] Custom triage rules (always important: boss@company.com)
- [ ] Daily digest — "Here's what matters today" summary
- [ ] Snooze: hide thread until a specified time

### Action Extraction
- [ ] Parse deadlines, meeting times, TODOs from email body
- [x] Surface action items in a dedicated panel
- [ ] Calendar integration (add extracted events to system calendar)
- [ ] Task list view — all extracted actions across threads

### Smart Categorization
- [x] Auto-label threads (newsletters, receipts, social, updates)
- [ ] Learn from user behavior (what they read vs. archive immediately)
- [x] Custom categories with user-provided examples

---

## Long Term — Platform

### Multi-Account & Identity
- [ ] Unified inbox across all accounts
- [ ] Per-account color coding
- [ ] Send-as / alias support
- [ ] Account-specific signatures and settings

### Collaboration
- [ ] Shared mailbox support
- [ ] Internal notes on threads (never sent, local only)
- [ ] "@mention" teammates on threads (if shared mailbox)

### Privacy & Security
- [ ] End-to-end encryption support (PGP/GPG)
- [ ] S/MIME signature verification
- [ ] Phishing detection via AI
- [ ] Privacy mode — strip tracking pixels, block remote images by default
- [ ] Local-only mode — all data stays on device, no cloud AI

### Performance & Scale
- [ ] SQLite WAL2 or move to DuckDB for analytics-heavy queries
- [ ] Incremental Tantivy index updates (currently rebuilds)
- [ ] Message body lazy-loading (fetch on demand, not on sync)
- [ ] Attachment storage with deduplication
- [ ] Database compaction and cleanup of old messages

### Platform Expansion
- [ ] System tray icon with unread count badge
- [ ] Native OS notifications (new mail, action reminders)
- [ ] Global keyboard shortcut to open/compose
- [ ] Auto-start on login (optional)
- [ ] CLI mode — `vibemail send --to alice@example.com --subject "Hi"`
- [ ] Headless sync daemon — runs in background, UI optional

### Plugin / Extension System
- [ ] Lua or WASM plugin API for custom email processing
- [ ] Webhook integrations (Slack, Discord, Telegram on new mail)
- [ ] Custom AI prompt templates per category
- [ ] Zapier/n8n compatible triggers

### Data & Export
- [ ] Export mailbox to mbox/eml format
- [ ] Import from Thunderbird, Apple Mail, Gmail Takeout
- [ ] Analytics dashboard — emails sent/received per day, response times
- [ ] Thread timeline visualization

---

## Technical Debt

- [ ] Add comprehensive test suite (Rust unit tests, React component tests)
- [ ] CI pipeline — run `cargo test`, `cargo clippy`, `npx tsc --noEmit` on PR
- [ ] Replace file-based token store with OS keychain (fix Linux secret-service detection)
- [ ] Remove unused `StartOAuthRequest` struct
- [x] Remove unused `list_mailboxes` function or wire it into UI
- [ ] Proper error boundaries in React (catch panics gracefully)
- [ ] Structured logging with log levels (replace remaining eprintln)
- [ ] Rate limiting on token refresh (don't refresh on every IMAP connect)
- [ ] Connection pooling — reuse IMAP sessions across syncs
- [ ] Proper IMAP LOGOUT on app quit

---

## Non-Goals (Intentional)

- **Not a webmail clone** — no browser-based access, desktop-first
- **Not a calendar app** — may extract events, but no built-in calendar UI
- **Not a chat app** — email only, no Slack/Teams integration in core
- **No server component** — fully local, no VibeMail cloud service
