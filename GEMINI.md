# VibeMail — AI-Native Email Client

## Project Overview
VibeMail is a high-performance, AI-native desktop email client built with **Tauri 2**, **Rust**, and **React**. It is designed to provide a modern email experience with integrated AI capabilities, local-first search, and cross-platform support.

### Key Technologies
- **Frontend**: React, TypeScript, Vite, Zustand (state management), CSS Modules.
- **Backend**: Rust, Tauri 2, Tokio (async runtime).
- **Database**: SQLite (via `rusqlite`) for local storage of accounts, threads, and AI configurations.
- **Search**: Tantivy for full-text and semantic search indexing.
- **Email Protocols**: `async-imap` for fetching and `lettre` for sending.
- **AI Integration**: Provider-agnostic router supporting **Ollama** (local) and **OpenAI-compatible** endpoints (cloud/BYOK).

---

## Building and Running

### Development
```bash
# Install dependencies
npm install

# Run the development environment (starts Vite and Tauri)
npm run tauri dev
```

### Build & Release
```bash
# Build production-ready binaries
npm run tauri build
```

### Testing and Linting
```bash
# Lint the frontend code
npm run lint

# Type-check the frontend code
npm run type-check

# Check the Rust backend (from project root)
cd src-tauri && cargo check
```

---

## Project Structure & Architecture

### Backend (`src-tauri/src/`)
- `ai/`: Implements the AI router and providers (Ollama, OpenAI).
- `auth/`: Handles OAuth2 PKCE flows and keychain integration for secure token storage.
- `commands/`: Contains the Tauri IPC command handlers exposed to the frontend (accounts, imap, smtp, ai, search, drafts, general).
- `db/`: Manages SQLite schema, models, and queries.
- `mail/`: Core email logic including IMAP sync, SMTP sending, IMAP IDLE push notifications, conversation threading (JWZ algorithm), and draft management.
- `search/`: Tantivy-based indexing for messages.

### Frontend (`src/`)
- `components/`: UI components (e.g., `InboxList`, `ThreadView`, `AIPanel`).
- `pages/`: Main views (e.g., `Inbox`, `Settings`, `AccountSetup`).
- `stores/`: Zustand stores for global state (accounts, threads, AI config, etc.).
- `App.tsx`: Main application entry point handling routing and initial load.

---

## Development Conventions

- **State Management**: Use **Zustand** stores for global application state. Avoid prop drilling where possible.
- **IPC (Inter-Process Communication)**: Frontend communicates with the Rust backend via `invoke` calls to registered commands in `src-tauri/src/commands/`.
- **Styling**: Use **CSS Modules** (`.module.css`) for component-specific styles to ensure scoping.
- **AI Task Routing**: AI features are routed through `src-tauri/src/ai/router.rs`, which selects the provider and model based on user configuration.
- **Error Handling**: Use `anyhow` or `thiserror` in Rust for robust error propagation.
- **Database Access**: All database operations should go through the `Database` struct in `src-tauri/src/db/mod.rs`.
- **OAuth**: Authentication is handled via a local HTTP listener on port `7887` for redirect callbacks.
- **Drafts**: Local drafts are auto-saved to the `drafts` table (2s debounce), synced to IMAP Drafts folder on compose close, and managed via the `drafts` store and `DraftList` component.

---

## Important Files
- `src-tauri/tauri.conf.json`: Tauri configuration (capabilities, bundle settings, etc.).
- `src-tauri/src/lib.rs`: Entry point for the Rust backend and command registration.
- `src-tauri/src/db/schema.rs`: SQLite database schema definitions.
- `src/stores/`: Contains the logic for syncing frontend state with backend data.
- `README.md`: Detailed setup and troubleshooting guide.
