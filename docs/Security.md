# Security & Authentication

Privacy and security are first-class citizens in VibeMail.

## OAuth2 PKCE
For providers like Gmail and Outlook, VibeMail uses the **Proof Key for Code Exchange (PKCE)** flow.
- The app starts a temporary local HTTP server (port `7887`) to receive the redirect callback.
- No client secret is required for public clients, but can be configured in the UI.

## Token Storage
- **Access Tokens**: Kept in memory and refreshed as needed.
- **Refresh Tokens**: Stored securely using the system's native keychain (macOS Keychain, Windows Credential Manager, or Secret Service on Linux) via the `keyring` Rust crate.

## Database Security
- The local SQLite database (`vibemail.db`) is stored in the application's data directory.
- It is recommended to use full-disk encryption (FileVault, BitLocker, LUKS) to protect local data.

## AI Privacy
- **Ollama**: All data stays local; no email content ever leaves your machine.
- **Cloud Providers**: Only the specific thread being analyzed is sent to the API. We do not use your data for training.
