# Architecture

VibeMail follows the **Tauri 2** architecture, splitting responsibilities between a high-performance Rust backend and a modern React frontend.

## The Backend (Rust)
The core logic resides in `src-tauri/src/`.

### Modules
- **`mail/`**: Handles the IMAP protocol, MIME parsing, and SMTP sending. Includes the `SyncManager` for coordinating concurrent background tasks.
- **`db/`**: A SQLite database (via `rusqlite`) stores account configurations, message metadata, and AI settings. Supports automated schema migrations.
- **`ai/`**: A provider-agnostic router that handles local (Ollama) and remote (OpenAI) LLM requests, including vector embeddings for search.
- **`search/`**: Hybrid search using **Tantivy** for keywords and manual cosine similarity for semantic matches.
- **`auth/`**: Manages OAuth2 PKCE flows and secure token storage in the system keychain.

## The Frontend (React)
The UI is a single-page application built with React and Vite.

### Core Concepts
- **Zustand**: Used for lightweight global state management (accounts, threads, AI status).
- **CSS Modules**: Component-level styling for a clean, maintainable UI.
- **Tauri IPC**: The frontend invokes Rust commands via `invoke()`, ensuring heavy lifting stays off the UI thread.

## Data Flow
1. **Sync**: Rust fetches emails via IMAP, parses them, and stores metadata in SQLite.
2. **Indexing**: Content is simultaneously indexed in Tantivy for search.
3. **UI Update**: Frontend polls or is notified of new messages and updates the Zustand store.
4. **AI Analysis**: When requested, the Rust backend routes specific thread content to the configured AI provider and returns insights to the UI.
