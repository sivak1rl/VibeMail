# Build and Packaging

VibeMail requires both Node.js and Rust toolchains.

## System Dependencies

### macOS
- Xcode Command Line Tools: `xcode-select --install`

### Linux (Ubuntu/Debian)
```bash
sudo apt install libgtk-3-dev libwebkit2gtk-4.1-dev libappindicator3-dev librsvg2-dev patchelf
```

## Development
```bash
# 1. Install JS dependencies
npm install

# 2. Run in dev mode
npm run tauri dev
```

## Production Build
To generate a platform-specific installer (DMG, AppImage, MSI):
```bash
npm run tauri build
```

The output will be located in `src-tauri/target/release/bundle/`.

## Manual Installation
- **Linux**: The `.AppImage` can be run directly after `chmod +x`.
- **macOS**: Drag `VibeMail.app` to your Applications folder.
- **Windows**: Run the `.msi` installer.
