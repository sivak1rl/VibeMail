# VibeMail — AI-Native Email Client

<p align="center">
  <img src="./logo_transparent.png" alt="VibeMail logo" width="320" />
</p>

An intelligent desktop email client built with Tauri 2 (Rust + React) featuring local IMAP/SMTP sync, provider-agnostic AI integration (Ollama + OpenAI-compatible), full-text + semantic search, and automatic email threading.

## Features

- **Multi-account IMAP/SMTP**: Gmail, Outlook, and generic IMAP servers with OAuth2 PKCE or password auth.
- **Background Synchronization**: Non-blocking sync with IMAP IDLE push notifications, sliding window fetch, and real-time progress indicators.
- **Advanced Search**: SQLite FTS5 keyword search, semantic vector search, persistent history, and Gmail-style filters (`from:`, `is:unread`, etc.).
- **Drafts Management**: Auto-save to local DB, IMAP Drafts folder sync, inline editing of server-synced drafts, automatic cleanup on send.
- **AI-Powered Insights**: Thread summarization, smart reply drafting with tone selector, action item extraction, triage scoring, intelligent labeling, and email roundup digest.
- **Robust Attachments**: Unified thread attachment sidebar, one-click open, image previews and thumbnails.
- **Modern Responsive UI**: Collapsible sidebar, lightbox email view, loading skeletons, pull-to-refresh, dark theme, keyboard shortcuts.
- **Danger Zone**: Wipe local cache and reset database schemas without losing account credentials.

## Quick Start

```bash
# Install dependencies
npm install

# Run development environment (Vite + Tauri + hot-reload)
npm run tauri dev

# Build production binaries
npm run tauri build
```

**Prerequisites**: Node.js 18+, Rust 1.70+. See [Build and Packaging](docs/Build-and-Packaging.md) for system dependencies per OS.

## Architecture

```
Tauri 2 (Desktop Shell)
├── Rust Backend (src-tauri/src/)
│   ├── IMAP: async-imap with non-blocking background tasks + IDLE push
│   ├── DB: SQLite with automated schema migrations
│   ├── Search: Tantivy full-text + vector similarity for semantic search
│   ├── AI: Provider-agnostic router with task-specific model assignments
│   ├── Drafts: Local auto-save + IMAP Drafts folder sync
│   └── OS Integration: Native keychain for tokens, file openers for attachments
└── React Frontend (src/)
    ├── State: Zustand with persistent storage
    ├── UI: React 18 with Error Boundaries and CSS Modules
    └── Transitions: Smooth animations for Lightbox and Sidebar
```

See [Architecture](docs/Architecture.md) for a detailed breakdown of backend modules and frontend layers.

## Project Structure

```
vibemail/
├── src/                     # React frontend
│   ├── pages/              # Inbox, Settings, AccountSetup
│   ├── components/         # UI components (InboxList, ThreadView, DraftList, etc)
│   ├── stores/             # Zustand state (accounts, threads, AI, search, drafts)
│   └── App.tsx
├── src-tauri/              # Rust backend
│   ├── src/
│   │   ├── db/            # SQLite schema & queries
│   │   ├── auth/          # OAuth + token storage
│   │   ├── mail/          # IMAP + SMTP + threading + drafts
│   │   ├── ai/            # Provider trait + Ollama + OpenAI
│   │   ├── search/        # Tantivy indexing
│   │   └── commands/      # Tauri IPC handlers
│   ├── Cargo.toml
│   └── tauri.conf.json
├── docs/                    # Documentation wiki
├── package.json
├── Cargo.toml
└── README.md
```

## Documentation

| Guide | Description |
|-------|-------------|
| [Build and Packaging](docs/Build-and-Packaging.md) | System dependencies, dev setup, production builds, and OS-specific packaging |
| [Configuration](docs/Configuration.md) | Accounts, AI providers, sync settings, OAuth credentials, custom categories |
| [Architecture](docs/Architecture.md) | Backend modules, frontend layers, data flow, and shared state model |
| [AI Integration](docs/AI-Integration.md) | Ollama and OpenAI-compatible setup, AI task types |
| [Security](docs/Security.md) | OAuth2 PKCE, token storage, database security, AI privacy |
| [Troubleshooting](docs/Troubleshooting.md) | Common issues and fixes |
| [Roadmap](docs/Coming-Soon.md) | What's done, what's next, and long-term vision |

## Key Dependencies

### Frontend (React)
- `@tauri-apps/api` — Tauri IPC bridge
- `zustand` — State management
- `date-fns` — Date formatting
- `vite` — Build tool

### Backend (Rust)
- `tauri` v2 — Desktop framework
- `tokio` — Async runtime
- `async-imap` v0.10 — IMAP protocol
- `lettre` v0.11 — SMTP sending
- `rusqlite` v0.32 — SQLite bindings (bundled)
- `mail-parser` v0.9 — MIME parsing
- `tantivy` v0.22 — Full-text search
- `serde_json` — JSON serialization

## License & Trademarks

This project is dual-licensed under the MIT and Apache 2.0 licenses.

### Trademarks

VibeMail™ and the VibeMail logo are trademarks of Rich Sivak. While the code is open-source, I want to keep the VibeMail brand recognizable.

If you fork this project:
- You ARE allowed to keep the name VibeMail in your fork (e.g., VibeMail-Community or [YourName]'s VibeMail).
- You MUST provide attribution and a link back to this original repository.
- Please do not use the name in a way that suggests I officially endorse your version.

## Contributing

Pull requests welcome! Please:
1. Fork the repo
2. Create feature branch (`git checkout -b feature/xyz`)
3. Commit with clear messages
4. Push and open PR with description

## Support

- Report bugs via GitHub Issues
- For questions, see Discussions tab
- Star the repo if you find it useful!
