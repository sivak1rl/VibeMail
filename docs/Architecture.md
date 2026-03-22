# Architecture

VibeMail follows the **Tauri 2** architecture, splitting responsibilities between a high-performance Rust backend and a modern React frontend.

## The Backend (Rust)

The core logic resides in `src-tauri/src/`.

### Modules

| Module | Purpose |
|--------|---------|
| `mail/imap.rs` | IMAP protocol via `async-imap` with XOAuth2. Handles fetch, flag sync, search, and APPEND for draft sync. |
| `mail/smtp.rs` | SMTP sending via `lettre` with OAuth2 and password auth. |
| `mail/parser.rs` | MIME parsing via `mail-parser` — extracts body, headers, attachments. |
| `mail/threading.rs` | JWZ algorithm for conversation threading based on `Message-ID`, `In-Reply-To`, and `References` headers. |
| `mail/sync.rs` | `SyncManager` — coordinates concurrent background sync tasks, tracks per-mailbox state. |
| `mail/idle.rs` | `IdleManager` — maintains persistent IMAP IDLE connections for real-time push notifications. |
| `db/` | SQLite via `rusqlite` — `schema.rs` (tables + idempotent migrations), `models.rs` (structs), `queries.rs` (CRUD). |
| `ai/router.rs` | Task-to-provider routing based on user configuration. |
| `ai/ollama.rs` | Local Ollama provider (chat completions + embeddings). |
| `ai/openai_compat.rs` | OpenAI-compatible cloud provider (BYOK). |
| `ai/stream.rs` | Token streaming to frontend via Tauri events. |
| `ai/tools.rs` | System prompts and output parsing for AI tasks. |
| `search/index.rs` | Tantivy full-text indexing + manual cosine similarity for semantic search. |
| `auth/oauth.rs` | OAuth2 PKCE flow with local HTTP listener on port 7887. |
| `auth/keychain.rs` | File-based token store at `~/.local/share/com.vibemail.app/tokens.json`. |
| `commands/` | Tauri IPC handlers: `accounts`, `imap`, `smtp`, `ai`, `search`, `drafts`, `general`. |

### Shared State

The Rust backend manages four global states injected via Tauri's `State<'_>`:
- `Arc<Mutex<Database>>` — SQLite connection
- `Arc<Mutex<SearchIndex>>` — Tantivy index
- `Arc<AiRouter>` — AI provider router (immutable after init)
- `Arc<Mutex<SyncManager>>` — Background sync tracker

## The Frontend (React)

The UI is a single-page application built with React 18 and Vite.

### Core Layers

| Layer | Key Files |
|-------|-----------|
| **Pages** | `Inbox.tsx` (3-pane layout: sidebar + list + thread), `Settings.tsx`, `AccountSetup.tsx` |
| **Stores** | `threads.ts` (sync/pagination/selection), `ai.ts` (streaming summaries/drafts), `search.ts` (FTS + semantic), `accounts.ts`, `mailboxes.ts`, `drafts.ts`, `preferences.ts` (localStorage-persisted) |
| **Components** | `InboxList/` (thread list), `DraftList/` (local drafts browser), `ThreadView/` (message viewer with sandboxed iframe), `SearchBar/`, `AIPanel/`, `Compose/` |

### Core Concepts
- **Zustand**: Lightweight global state management (accounts, threads, AI status, drafts).
- **CSS Modules**: Component-level styling (`.module.css`) with global CSS variables in `src/index.css`.
- **Tauri IPC**: Frontend invokes Rust commands via `invoke()`, ensuring heavy lifting stays off the UI thread.
- **Routing**: State-based in `App.tsx` (`Page = "inbox" | "settings" | "setup"`), not URL-based.

## Data Flow

1. **Sync**: Rust fetches emails via IMAP, parses them, and stores metadata in SQLite. Threads are built using the JWZ algorithm.
2. **IDLE Push**: `IdleManager` maintains persistent IMAP connections that notify the frontend of new mail in real-time.
3. **Indexing**: Content is simultaneously indexed in Tantivy for full-text and semantic search.
4. **UI Update**: Frontend polls or is notified of new messages and updates the Zustand store.
5. **AI Analysis**: When requested, the Rust backend routes thread content to the configured AI provider and streams results back to the UI.
6. **Drafts**: Auto-saved locally (2s debounce), synced to IMAP Drafts folder on compose close, cleaned up on send.
