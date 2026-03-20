# VibeMail — Roadmap & Future Direction

## Current State (MVP)

Working end-to-end: Gmail OAuth sign-in, IMAP sync with graduated batching (newest-first), SQLite storage with FTS5, dark-themed inbox/thread view, compose editor, and AI provider scaffolding (Ollama + OpenAI-compatible). Outlook OAuth and generic IMAP also supported. Gmail syncs exclusively from [Gmail]/All Mail using X-GM-LABELS for folder membership. Email roundup digest available as a standalone window with inline AI actions.

---

## Short Term — Polish the MVP

### Sync & Reliability
- [x] Background auto-sync on a timer (configurable, default 15 minutes)
- [ ] IMAP IDLE push notifications for real-time new mail
- [ ] Retry logic with exponential backoff on transient failures
- [x] Sync multiple mailboxes (Sync All button + background progress)
- [x] Flag sync — mark read/unread, star/flag changes propagated back to server
- [x] Sliding sync window (only fetch mail since last sync)
- [x] Historical mail fetching (targeted older mail fetch from server)
- [x] Gmail All Mail sync — single-pass sync via [Gmail]/All Mail with X-GM-LABELS for folder membership
- [ ] Detect and handle token revocation gracefully (re-prompt OAuth)

### UI/UX
- [x] Keyboard shortcuts (j/k, r reply, a archive, e read, f flag, c compose, / search, ? help)
- [ ] Swipe actions on thread list (archive, delete, snooze)
- [x] Unread count badge in sidebar per mailbox
- [x] Bulk thread selection (checkboxes + shift-select range)
- [x] Tree-like folder selector (Gmail style nesting)
- [x] Lightbox email view (expand to full focused view)
- [x] Two-column Settings UI with navigation hotlinks
- [x] "Pull to refresh" gesture on thread list
- [x] Empty state illustrations (no mail, no search results)
- [x] Loading skeleton improvements during sync
- [x] Background progress indicators (sidebar status + reindex toast)
- [x] Responsive layout — collapsible sidebar, mobile-friendly thread view
- [x] Thread view: collapse/expand individual messages
- [x] Inline image rendering in HTML emails
- [x] Attachment download and preview (PDF, images)
- [x] Unified attachment sidebar panel
- [x] Global error boundaries (catch and report crashes)

### Compose
- [x] Rich text editor (Tiptap — bold, italic, strike, bullet/ordered lists, blockquote, inline code)
- [x] Reply/Reply All/Forward with quoted original message
- [x] Attachment upload via drag-and-drop (browse or drop, shown in list, sent as MIME attachments)
- [x] Draft auto-save to local DB (2s debounce, restore on reopen, delete on send)
- [x] CC/BCC fields
- [x] Contact autocomplete from message history
- [x] Signature configuration per account
- [x] AI compose panel for new/reply/forward modes (draft/generate + proofread with per-change accept/reject)

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
- [x] AI-generated reply suggestions (3 options: brief, detailed, decline)
- [x] Tone selector: professional, casual, friendly
- [x] Context-aware: uses full thread history, not just last message
- [x] "Edit and send" flow — AI drafts, user refines, then sends

### Triage & Priority
- [ ] Auto-triage on sync — score every new thread
- [ ] Focus inbox: separate "Important" and "Other" tabs
- [ ] Custom triage rules (always important: boss@company.com)
- [x] Email roundup digest — AI-generated summary with stats, narrative, and top threads in a dedicated window
- [x] Roundup action buttons — open, reply, summarize, and label threads inline from the roundup panel
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

### Performance & Scale — DB Schema Refactor
- [x] SQLite performance tuning — denormalized message_mailboxes join table, mmap, 32MB cache, NORMAL synchronous
- [x] Add `is_read`/`is_flagged` boolean columns on messages — replace `instr(flags, '\\Seen')` string searches with indexed column lookups
- [x] Batch state updates — replace N+1 per-message flag loops with single UPDATE statements using `is_read`/`is_flagged` columns
- [x] Precompute mailbox counts — add `thread_count`/`unread_count` columns on mailboxes, refresh after sync instead of correlated subqueries on every sidebar render
- [x] Add `thread_mailboxes` join table — indexed `(mailbox_id, thread_id)` to replace correlated EXISTS subquery in list_threads
- [x] Drop `inbox_mailboxes` JSON column — make `message_mailboxes` join table the sole source of truth, eliminate redundant JSON serialization on every upsert
- [x] Classify system folders — add `folder_role` column on mailboxes (`inbox`/`sent`/`trash`/`spam`/`drafts`/`all_mail`), replace 5x UPPER/LIKE pattern matching
- [ ] Batch inserts in persist_batch — multi-row INSERT for message_mailboxes, wrap sync batches in explicit transactions

### Performance & Scale — Other
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
- [x] Proper error boundaries in React (catch panics gracefully)
- [ ] Structured logging with log levels (replace remaining println/eprintln debug statements)
- [ ] Remove Gmail label debug println! statements from imap.rs
- [ ] Rate limiting on token refresh (don't refresh on every IMAP connect)
- [ ] Connection pooling — reuse IMAP sessions across syncs
- [ ] Proper IMAP LOGOUT on app quit

---

## Non-Goals (Intentional)

- **Not a webmail clone** — no browser-based access, desktop-first
- **Not a calendar app** — may extract events, but no built-in calendar UI
- **Not a chat app** — email only, no Slack/Teams integration in core
- **No server component** — fully local, no VibeMail cloud service
