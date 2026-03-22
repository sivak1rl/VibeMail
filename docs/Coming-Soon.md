# Roadmap

What's been built, what's next, and the long-term vision for VibeMail.

---

## Completed

### Sync & Reliability
- Background auto-sync on a configurable timer (default 15 minutes)
- IMAP IDLE push notifications for real-time new mail
- Multi-mailbox sync (Sync All + background progress)
- Flag sync — read/unread, star/flag changes propagated back to server
- Sliding sync window (only fetch mail since last sync)
- Historical mail fetching (targeted older mail fetch)
- Gmail All Mail sync — single-pass via `[Gmail]/All Mail` with `X-GM-LABELS`

### UI/UX
- Keyboard shortcuts (j/k, r reply, a archive, e read, f flag, c compose, / search, ? help)
- Unread count badge in sidebar per mailbox
- Bulk thread selection (checkboxes + shift-select range)
- Tree-like folder selector (Gmail-style nesting)
- Lightbox email view (expand to full focused view)
- Two-column Settings UI with navigation hotlinks
- Pull to refresh, empty state illustrations, loading skeletons
- Responsive layout — collapsible sidebar, mobile-friendly thread view
- Thread view: collapse/expand individual messages
- Inline image rendering, attachment download/preview
- Unified attachment sidebar panel
- Global error boundaries

### Compose & Drafts
- Rich text editor (Tiptap — bold, italic, strike, lists, blockquote, code)
- Reply/Reply All/Forward with quoted original
- Attachment upload via drag-and-drop
- Draft auto-save to local DB (2s debounce)
- Draft sync to IMAP Drafts folder on compose close
- Drafts folder browser with inline editing of IMAP-synced drafts
- CC/BCC fields, contact autocomplete
- Signature configuration per account
- AI compose panel (draft/generate + proofread with per-change accept/reject)

### Search
- Search-as-you-type with debounce
- Filters: from, to, has:attachment, is:unread
- Search result highlighting, recent history
- Semantic search (AI-powered vector embeddings)
- Global search (all folders), paginated results

### AI Features
- One-click thread summarization + batch summarize
- Smart reply suggestions (brief, detailed, decline) with tone selector
- Context-aware replies using full thread history
- Action item extraction panel
- Auto-label threads (newsletters, receipts, social, updates)
- Custom categories with user-provided examples
- Email roundup digest with inline AI actions (open, reply, summarize, label)

### Performance
- SQLite tuning: denormalized joins, mmap, 32MB cache, NORMAL sync
- Indexed `is_read`/`is_flagged` columns replacing string searches
- Batch state updates (single UPDATE vs N+1 loops)
- Precomputed mailbox counts
- `thread_mailboxes` join table, `folder_role` classification
- Batch inserts with explicit transactions

---

## Up Next

### Short-Term
- Retry logic with exponential backoff on transient failures
- Token revocation detection (re-prompt OAuth)
- Swipe actions on thread list (archive, delete, snooze)
- Summary caching in DB with invalidation
- Configurable summary length (brief / detailed)

### Medium-Term
- Auto-triage on sync — score every new thread
- Focus inbox: "Important" and "Other" tabs
- Custom triage rules (always important: boss@company.com)
- Snooze: hide thread until a specified time
- Parse deadlines, meeting times, TODOs from email body
- Calendar integration (add extracted events to system calendar)
- Task list view — all extracted actions across threads
- Learn labeling from user behavior

---

## Long-Term Vision

### Multi-Account & Identity
- Unified inbox across all accounts
- Per-account color coding
- Send-as / alias support

### Privacy & Security
- End-to-end encryption (PGP/GPG)
- S/MIME signature verification
- Phishing detection via AI
- Privacy mode — strip tracking pixels, block remote images
- Local-only mode — all data stays on device, no cloud AI

### Platform Expansion
- System tray icon with unread count badge
- Native OS notifications
- Global keyboard shortcut to open/compose
- Auto-start on login
- CLI mode (`vibemail send --to alice@example.com --subject "Hi"`)
- Headless sync daemon

### Plugin System
- Lua or WASM plugin API for custom email processing
- Webhook integrations (Slack, Discord, Telegram)
- Custom AI prompt templates per category
- Zapier/n8n compatible triggers

### Data & Export
- Export mailbox to mbox/eml format
- Import from Thunderbird, Apple Mail, Gmail Takeout
- Analytics dashboard — emails sent/received per day, response times
- Thread timeline visualization

---

## Technical Debt
- Comprehensive test suite (Rust unit tests, React component tests)
- CI pipeline — `cargo test`, `cargo clippy`, `npx tsc --noEmit` on PR
- Replace file-based token store with OS keychain (fix Linux secret-service)
- Structured logging with log levels (replace println/eprintln)
- Rate limiting on token refresh
- Connection pooling — reuse IMAP sessions across syncs
- Proper IMAP LOGOUT on app quit
