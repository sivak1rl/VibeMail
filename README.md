# Outlookr — AI-Native Email Client

An intelligent desktop email client built with Tauri 2 (Rust + React) featuring local IMAP/SMTP sync, provider-agnostic AI integration (Ollama + OpenAI-compatible), full-text + semantic search, and automatic email threading.

## Features

- **Multi-account IMAP/SMTP**: Gmail, Outlook, and generic IMAP servers with OAuth2 PKCE or password auth
- **OAuth2 PKCE Flow**: Seamless Gmail/Outlook authentication with automatic token refresh
- **AI-Powered Insights**:
  - Thread summarization
  - Draft smart replies
  - Extract actionable items
  - Triage scoring for email importance
- **Flexible AI Backends**: Local Ollama (default) or bring-your-own OpenAI-compatible endpoint
- **Full-Text + Semantic Search**: FTS5 keyword search + Tantivy vector indexing
- **Smart Threading**: JWZ algorithm groups conversations by Message-ID/References
- **Graduated Sync**: Initial fetch loads newest emails first in batches (10→25→50→100→200→500)
- **Dark Theme UI**: Modern React + CSS Modules with infinite scroll

## Architecture

```
Tauri 2 (Desktop Shell)
├── Rust Backend (src-tauri/src/)
│   ├── IMAP: async-imap 0.10 with tokio async runtime
│   ├── SMTP: lettre 0.11 for message sending
│   ├── DB: SQLite with WAL + FTS5 virtual tables
│   ├── Search: Tantivy 0.22 full-text index
│   ├── Auth: OAuth PKCE with local HTTP redirect listener
│   └── AI: Provider trait with Ollama + OpenAI-compatible
└── React Frontend (src/)
    ├── Zustand stores (accounts, threads, AI, search)
    ├── CSS Modules (dark theme)
    └── Tauri IPC commands for all operations
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
cd outlookr
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
- **macOS**: `Outlookr.app` (dmg installer)
- **Linux**: `outlookr` (AppImage)
- **Windows**: `Outlookr.exe` (msi installer)

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
- **macOS/Linux**: `~/.local/share/com.outlookr.app/tokens.json`
- **Windows**: `%APPDATA%\com.outlookr.app\tokens.json`

Never commit this file. `.gitignore` excludes it by default.

## Packaging as System Application

### macOS

The `.app` bundle is created automatically by `npm run tauri build`. To distribute:

1. **Notarize** (required for macOS 11+):
   ```bash
   xcrun altool --notarize-app \
     -f Outlookr.dmg \
     -t osx \
     -u <apple-id> \
     -p <app-password>
   ```

2. **Create DMG installer** (auto-generated):
   ```bash
   hdiutil create -volname "Outlookr" -srcfolder "src-tauri/target/release/bundle/dmg" -ov -format UDZO Outlookr.dmg
   ```

### Linux

**Create AppImage** (single-file portable):
```bash
# Already created by `npm run tauri build`
# Located at: src-tauri/target/release/bundle/appimage/outlookr_*.AppImage

# Make executable:
chmod +x outlookr_*.AppImage

# Or install system-wide:
sudo cp outlookr_*.AppImage /usr/local/bin/outlookr
```

**Create .deb package** (Debian/Ubuntu):
```bash
# Already created by `npm run tauri build`
# Located at: src-tauri/target/release/bundle/deb/outlookr_*.deb

sudo apt install ./outlookr_*.deb
```

**Manual Desktop Entry** (for custom installation):

Create `~/.local/share/applications/outlookr.desktop`:
```ini
[Desktop Entry]
Name=Outlookr
Exec=/usr/local/bin/outlookr
Type=Application
Icon=outlookr
Categories=Mail;
```

### Windows

The `.msi` installer is auto-generated:
```bash
# Located at: src-tauri/target/release/bundle/msi/Outlookr_*.msi
# Double-click to install
# Or via command line:
msiexec /i Outlookr_*.msi
```

To customize installer:
- Edit `src-tauri/tauri.conf.json` → `bundle` section
- Modify icon, license text, install directory

## Command-Line Usage

After installation, run from terminal:

```bash
# macOS/Linux
/usr/local/bin/outlookr

# Or if in PATH:
outlookr

# Windows
Outlookr.exe
```

## Environment Variables

Optional env vars (useful for scripting):

```bash
# Enable debug logging (Rust backend)
RUST_LOG=outlookr=debug npm run tauri dev

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
outlookr/
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

## License

[Choose: MIT, Apache 2.0, GPL-3.0, etc.]

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
