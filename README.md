# VibeMail — AI-Native Email Client

<p align="center">
  <img src="./logo_transparent.png" alt="VibeMail logo" width="320" />
</p>

An intelligent desktop email client built with Tauri 2 (Rust + React) featuring local IMAP/SMTP sync, provider-agnostic AI integration (Ollama + OpenAI-compatible), full-text + semantic search, and automatic email threading.

## Features

- **Multi-account IMAP/SMTP**: Gmail, Outlook, and generic IMAP servers with OAuth2 PKCE or password auth.
- **Background Synchronization**:
  - Non-blocking sync runs in the background.
  - Real-time progress indicators in the sidebar.
  - "Sliding Window" sync: only fetches mail since your last successful update.
  - **IMAP IDLE**: Real-time new mail push notifications for instant updates.
- **Advanced Search**:
  - **Keyword Search**: Powered by SQLite FTS5.
  - **Semantic Search**: Meaning-based search using vector embeddings (Ollama/OpenAI).
  - **History & Filters**: Persistent search history and Gmail-style filters (`from:`, `is:unread`, etc.).
- **Drafts Management**:
  - Auto-save drafts to local DB (2s debounce).
  - Sync drafts to IMAP Drafts folder on compose close.
  - Browse local and server-synced drafts in dedicated Drafts folder.
  - Edit IMAP-synced drafts inline in the preview pane.
  - Automatic cleanup when sending.
- **AI-Powered Insights**:
  - Intelligent thread summarization.
  - Smart reply drafting with tone selector.
  - Action item extraction and triage scoring.
  - **Intelligent Labeling**: Automatic categorization that avoids redundant processing.
  - Email roundup digest with inline AI actions.
- **Robust Attachment Handling**:
  - Unified attachment sidebar for entire threads.
  - One-click "Open" in system-default applications.
  - Image previews and thumbnails.
- **Modern Responsive UI**:
  - Collapsible sidebar and adaptive layout for different window sizes.
  - Lightbox-style focused email view.
  - Loading skeletons and "Pull to Refresh" support.
  - Dark theme with intuitive keyboard shortcuts.
- **Danger Zone**: Wipe local cache and reset database schemas without losing account credentials.

## Architecture

```
Tauri 2 (Desktop Shell)
├── Rust Backend (src-tauri/src/)
│   ├── IMAP: async-imap with non-blocking background tasks
│   ├── DB: SQLite with automated schema migrations
│   ├── Search: Tantivy full-text + Manual vector similarity
│   ├── AI: Router supporting Task-specific models and providers
│   └── OS Integration: Native keychain for tokens and file openers for attachments
└── React Frontend (src/)
    ├── State: Zustand with persistent storage
    ├── UI: React 18 with Error Boundaries and CSS Modules
    └── Transitions: Smooth animations for Lightbox and Sidebar
```

## Build Requirements

### System Dependencies

**macOS:**
```bash
# WebKit development headers
xcode-select --install
```

**Linux (Ubuntu/Debian):**
```bash
sudo apt install \
  libgtk-3-dev \
  libwebkit2gtk-4.1-dev \
  libappindicator3-dev \
  librsvg2-dev \
  patchelf
```

**Linux (Fedora/RHEL):**
```bash
sudo dnf install \
  gtk3-devel \
  webkit2gtk3-devel \
  libappindicator-gtk3-devel \
  librsvg2-devel
```

**Windows:**
- Visual Studio 2019+ with C++ workload, or
- `cargo install cargo-vs-code` for MSVC setup

### Development Tools

- **Node.js 18+** (for React + Vite)
- **Rust 1.70+** (for Tauri + async-imap)
- **npm** or **yarn**

## Development Setup

### 1. Clone & Install

```bash
git clone <your-repo-url>
cd vibemail
npm install
```

### 2. Build Rust Backend (Optional)

```bash
cd src-tauri
cargo build
cd ..
```

### 3. Run Dev Server

```bash
npm run tauri dev
```

This starts:
- Vite dev server on `http://localhost:1420`
- Tauri Rust backend with hot-reload watching
- Desktop window with auto-refresh

### 4. Build for Production

```bash
npm run tauri build
```

Outputs to `src-tauri/target/release/` (binary depends on OS):
- **macOS**: `VibeMail.app` (dmg installer)
- **Linux**: `vibemail` (AppImage)
- **Windows**: `VibeMail.exe` (msi installer)

## Configuration

### OAuth Credentials

For Gmail/Outlook integration, create OAuth applications:

**Gmail (Google Cloud Console):**
1. Create project → APIs & Services → Credentials
2. Create OAuth 2.0 Client ID → Desktop app
3. Copy Client ID and Secret
4. Register redirect URI: `http://localhost:7887/oauth/callback`

**Outlook (Azure AD):**
1. Azure Portal → App registrations → New registration
2. Mobile and desktop applications → Add platform
3. Custom redirect URI: `http://localhost:7887/oauth/callback`

At startup, AccountSetup form accepts Client ID/Secret fields.

### AI Configuration

**Local (Ollama - Default):**
```bash
# Install Ollama from https://ollama.ai
ollama pull llama3.2:3b    # Fast, lightweight
# or
ollama pull llama3.1:8b    # More capable
ollama serve               # Runs on localhost:11434
```

**Cloud (OpenAI-compatible):**
- In Settings, switch provider to "OpenAI-compatible"
- Enter API endpoint (e.g., `https://api.openai.com/v1`)
- Enter API key (stored securely in local file store)

### Token Storage

Credentials are stored in:
- **macOS/Linux**: `~/.local/share/com.vibemail.app/tokens.json`
- **Windows**: `%APPDATA%\com.vibemail.app\tokens.json`

Never commit this file. `.gitignore` excludes it by default.

## Packaging as System Application

### macOS

The `.app` bundle is created automatically by `npm run tauri build`. To distribute:

1. **Notarize** (required for macOS 11+):
   ```bash
   xcrun altool --notarize-app \
     -f VibeMail.dmg \
     -t osx \
     -u <apple-id> \
     -p <app-password>
   ```

2. **Create DMG installer** (auto-generated):
   ```bash
   hdiutil create -volname "VibeMail" -srcfolder "src-tauri/target/release/bundle/dmg" -ov -format UDZO VibeMail.dmg
   ```

### Linux

**Create AppImage** (single-file portable):
```bash
# Already created by `npm run tauri build`
# Located at: src-tauri/target/release/bundle/appimage/vibemail_*.AppImage

# Make executable:
chmod +x vibemail_*.AppImage

# Or install system-wide:
sudo cp vibemail_*.AppImage /usr/local/bin/vibemail
```

**Create .deb package** (Debian/Ubuntu):
```bash
# Already created by `npm run tauri build`
# Located at: src-tauri/target/release/bundle/deb/vibemail_*.deb

sudo apt install ./vibemail_*.deb
```

**Manual Desktop Entry** (for custom installation):

Create `~/.local/share/applications/vibemail.desktop`:
```ini
[Desktop Entry]
Name=VibeMail
Exec=/usr/local/bin/vibemail
Type=Application
Icon=vibemail
Categories=Mail;
```

### Windows

The `.msi` installer is auto-generated:
```bash
# Located at: src-tauri/target/release/bundle/msi/VibeMail_*.msi
# Double-click to install
# Or via command line:
msiexec /i VibeMail_*.msi
```

To customize installer:
- Edit `src-tauri/tauri.conf.json` → `bundle` section
- Modify icon, license text, install directory

## Command-Line Usage

After installation, run from terminal:

```bash
# macOS/Linux
/usr/local/bin/vibemail

# Or if in PATH:
vibemail

# Windows
VibeMail.exe
```

## Environment Variables

Optional env vars (useful for scripting):

```bash
# Enable debug logging (Rust backend)
RUST_LOG=vibemail=debug npm run tauri dev

# Custom token storage path
# (Must be set before app starts)
export OUTLOOKR_DATA_DIR=/custom/path
```

## Troubleshooting

### "Port 7887 already in use"
Another app is using the OAuth redirect port. Kill it:
```bash
lsof -i :7887 | grep -v PID | awk '{print $2}' | xargs kill -9
```

### IMAP connection timeout (Linux)
IPv6 may be broken in your environment. The app prefers IPv4, but if it still fails:
```bash
# Disable IPv6 for testing:
echo 1 | sudo tee /proc/sys/net/ipv6/conf/all/disable_ipv6
```

### "No tokens for <email>; re-auth required"
OAuth token has expired. Click sync or restart the app to trigger refresh.

### Ollama not responding
Ensure Ollama is running:
```bash
# Check if service is up
curl http://localhost:11434/api/tags

# Or start it:
ollama serve
```

### Search not working
Ensure Tantivy index was created during sync. Restart the app if needed.

## Development Commands

```bash
# Start dev environment
npm run tauri dev

# Check code (no build)
cargo check
npx tsc --noEmit

# Build release binaries
npm run tauri build

# Format code
cargo fmt
npm run format  # if defined in package.json

# Run tests (when implemented)
cargo test
npm run test
```

## Project Structure

```
vibemail/
├── src/                     # React frontend
│   ├── pages/              # Inbox, Settings, AccountSetup
│   ├── components/         # UI components (InboxList, ThreadView, etc)
│   ├── stores/             # Zustand state (accounts, threads, AI, search)
│   └── App.tsx
├── src-tauri/              # Rust backend
│   ├── src/
│   │   ├── db/            # SQLite schema & queries
│   │   ├── auth/          # OAuth + token storage
│   │   ├── mail/          # IMAP + SMTP + threading
│   │   ├── ai/            # Provider trait + Ollama + OpenAI
│   │   ├── search/        # Tantivy indexing
│   │   └── commands/      # Tauri IPC handlers
│   ├── Cargo.toml
│   └── tauri.conf.json
├── package.json
├── Cargo.toml
└── README.md
```

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
- You ARE allowed to keep the name VibeMail in your fork (e.g., VibeMail-Community or [YourName]’s VibeMail).
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
