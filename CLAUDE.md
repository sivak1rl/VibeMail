# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

VibeMail is an AI-native desktop email client built with **Tauri 2** (Rust backend + React frontend). It provides local IMAP/SMTP sync, provider-agnostic AI integration (Ollama + OpenAI-compatible), full-text + semantic search, and JWZ email threading.

## Build & Development Commands

```bash
# Development
npm install                          # Install Node dependencies
npm run tauri dev                    # Run full app (Vite + Tauri + hot-reload)

# Frontend only
npm run dev                          # Vite dev server on http://localhost:1420
npm run type-check                   # TypeScript check (tsc --noEmit)
npm run lint                         # ESLint for src/**/*.{ts,tsx}
npm run build                        # Production frontend build (tsc + vite)

# Backend only (run from src-tauri/)
cd src-tauri && cargo check          # Quick compilation check
cd src-tauri && cargo build          # Full build
cd src-tauri && cargo test           # Run Rust tests
cd src-tauri && cargo clippy -- -D warnings  # Lint with all warnings as errors
cd src-tauri && cargo fmt --check    # Format check

# Production
npm run tauri build                  # Build release binaries
npm run tauri build -- --no-bundle   # CI-like build without packaging
```

Debug logging: `RUST_LOG=vibemail=debug npm run tauri dev`

## Architecture

### IPC Bridge Pattern (Frontend ↔ Backend)

The frontend communicates with Rust via `invoke()` calls to Tauri commands. This is the core integration seam:

- **Frontend**: Zustand stores (`src/stores/`) call `invoke<ResponseType>("command_name", { request: {...} })`
- **Backend**: Commands in `src-tauri/src/commands/` are `#[tauri::command] async fn` handlers that receive `State<'_, Arc<Mutex<T>>>` for shared state
- **Streaming**: Long-running AI operations stream tokens to the frontend via `app.emit("event_name", payload)`, listened to with `listen<T>("event_name", callback)` in stores
- **Error boundary**: Commands return `Result<T, String>` — internal `anyhow::Result` is converted via `.map_err(|e| e.to_string())` at the command boundary

Command registration happens in `src-tauri/src/lib.rs` via `tauri::generate_handler![]`. When adding new commands, they must be registered there.

### Backend Modules (`src-tauri/src/`)

| Module | Purpose |
|--------|---------|
| `commands/` | Tauri IPC handlers: `accounts`, `imap`, `smtp`, `ai`, `search`, `drafts`, `general` |
| `db/` | SQLite via rusqlite — `schema.rs` (tables + migrations), `models.rs` (structs), `queries.rs` (CRUD) |
| `mail/` | `imap.rs` (async-imap with XOAuth2 + IDLE), `smtp.rs` (lettre), `parser.rs` (mail-parser), `threading.rs` (JWZ algorithm), `idle.rs` (IMAP IDLE push notifications), `sync.rs` (state tracker) |
| `ai/` | `provider.rs` (AiProvider trait), `router.rs` (task→provider routing), `ollama.rs`, `openai_compat.rs`, `stream.rs` (token streaming to frontend), `tools.rs` (output parsing + system prompts) |
| `auth/` | `oauth.rs` (PKCE flow on port 7887), `keychain.rs` (file-based token store at `~/.local/share/com.vibemail.app/tokens.json`) |
| `search/` | `index.rs` (Tantivy full-text search) + FTS5 virtual table in SQLite |

### Frontend Structure (`src/`)

| Layer | Key Files |
|-------|-----------|
| **Pages** | `Inbox.tsx` (3-pane layout: sidebar + list + thread), `Settings.tsx`, `AccountSetup.tsx` |
| **Stores** | `threads.ts` (sync/pagination/selection), `ai.ts` (streaming summaries/drafts), `search.ts` (FTS + semantic), `drafts.ts` (local + IMAP draft management), `accounts.ts`, `mailboxes.ts`, `preferences.ts` (localStorage-persisted) |
| **Components** | `InboxList/` (thread list), `DraftList/` (local drafts), `ThreadView/` (message viewer with sandboxed iframe), `SearchBar/`, `AIPanel/`, `Compose/` |

Routing is state-based in `App.tsx` (`Page = "inbox" | "settings" | "setup"`), not URL-based.

### Shared State Model

The Rust backend manages five global states injected via `State<'_>`:
- `Arc<Mutex<Database>>` — SQLite connection
- `Arc<Mutex<SearchIndex>>` — Tantivy index
- `Arc<AiRouter>` — AI provider router (immutable after init)
- `Arc<Mutex<SyncManager>>` — background sync tracker
- `Arc<Mutex<IdleManager>>` — IMAP IDLE push notification manager

### Database

SQLite with manual migrations in `db/schema.rs` using `run_migrations()`. Migrations are idempotent (check column existence via `pragma_table_info()` before ALTER TABLE). Key tables: `accounts`, `mailboxes`, `messages`, `threads`, `attachments`, `ai_config`, `thread_embeddings`, `messages_fts` (FTS5 virtual table).

Array fields (flags, participants, labels) are stored as JSON strings.

## Conventions

- **Commit messages**: Conventional prefixes (`feat:`, `fix:`, `docs:`, `chore:`)
- **TypeScript**: Strict mode, 2-space indent, `PascalCase.tsx` for components, `camelCase` for stores/hooks
- **Rust**: `rustfmt` + clippy with `-D warnings`, `snake_case` functions, `PascalCase` structs
- **Styling**: CSS Modules (`.module.css`) with global CSS variables defined in `src/index.css` (dark theme with `--color-*` vars)
- **Path alias**: `@/*` maps to `./src/*` in TypeScript (configured in tsconfig.json + vite.config.ts)
