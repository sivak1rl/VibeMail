# Build and Packaging

VibeMail requires both Node.js and Rust toolchains.

## System Dependencies

### macOS
```bash
xcode-select --install
```

### Linux (Ubuntu/Debian)
```bash
sudo apt install \
  libgtk-3-dev \
  libwebkit2gtk-4.1-dev \
  libappindicator3-dev \
  librsvg2-dev \
  patchelf
```

### Linux (Fedora/RHEL)
```bash
sudo dnf install \
  gtk3-devel \
  webkit2gtk3-devel \
  libappindicator-gtk3-devel \
  librsvg2-devel
```

### Windows
- Visual Studio 2019+ with C++ workload, or
- `cargo install cargo-vs-code` for MSVC setup

### Development Tools
- **Node.js 18+** (for React + Vite)
- **Rust 1.70+** (for Tauri + async-imap)
- **npm** or **yarn**

---

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

### Environment Variables
```bash
# Enable debug logging (Rust backend)
RUST_LOG=vibemail=debug npm run tauri dev

# Custom token storage path (must be set before app starts)
export OUTLOOKR_DATA_DIR=/custom/path
```

---

## Development Commands

```bash
# Start dev environment
npm run tauri dev

# Check code (no build)
cargo check
npx tsc --noEmit

# Lint
cd src-tauri && cargo clippy -- -D warnings
npm run lint

# Format
cargo fmt
npm run format  # if defined in package.json

# Run tests
cargo test
npm run test
```

---

## Production Build

```bash
npm run tauri build
```

Outputs to `src-tauri/target/release/bundle/`:

| OS | Format | Location |
|----|--------|----------|
| macOS | `.app` + `.dmg` | `bundle/dmg/` |
| Linux | `.AppImage` + `.deb` | `bundle/appimage/`, `bundle/deb/` |
| Windows | `.msi` | `bundle/msi/` |

---

## Packaging & Distribution

### macOS

The `.app` bundle is created automatically. To distribute:

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

**AppImage** (single-file portable):
```bash
chmod +x vibemail_*.AppImage
./vibemail_*.AppImage

# Or install system-wide:
sudo cp vibemail_*.AppImage /usr/local/bin/vibemail
```

**.deb package** (Debian/Ubuntu):
```bash
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

```bash
# Double-click the .msi or install via command line:
msiexec /i VibeMail_*.msi
```

To customize the installer, edit `src-tauri/tauri.conf.json` → `bundle` section (icon, license text, install directory).

### Command-Line Usage

After installation:
```bash
# macOS/Linux
vibemail
# or: /usr/local/bin/vibemail

# Windows
VibeMail.exe
```
