# Configuration & Settings

VibeMail provides a centralized settings page to manage your accounts, AI providers, synchronization behavior, and custom categorization rules.

---

## 1. Account Management
In the **Accounts** section, you can see all linked IMAP/SMTP accounts.
- **Adding Accounts**: Handled via the initial setup or the "Add Account" flow (OAuth2 or Password).
- **Removing Accounts**: Clicking "Remove" will delete the account metadata, local email cache, and search indices for that specific account. It does **not** delete emails from the server.

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

At startup, the AccountSetup form accepts Client ID/Secret fields.

---

## 2. AI Provider Configuration
VibeMail's "AI-Native" features depend on a configured LLM provider.

### Providers
- **Local Ollama (Default)**: Best for privacy. Connects to a local Ollama instance (usually `http://localhost:11434`). No API key required.
- **OpenAI-Compatible (BYOK)**: Supports any API following the OpenAI spec (OpenAI, Anthropic via proxy, Groq, Mistral, etc.).
  - **API Key**: Keys are stored securely in your operating system's **Keychain** (macOS), **Credential Manager** (Windows), or **Secret Service** (Linux). They are never saved in the local database or frontend storage.

### Model Assignments
You can specify different models for different tasks to balance speed and quality:
- **Triage / Fast tasks**: Used for importance scoring and basic categorization. (e.g., `llama3.2:1b` or `gpt-4o-mini`).
- **Summarization**: Used to condense long threads. (e.g., `llama3.1:8b`).
- **Reply Drafting**: Used for generating high-quality email responses.
- **Action Extraction**: Used to find tasks and dates within text.
- **Embeddings**: Used for semantic search indexing (e.g., `nomic-embed-text`).

---

## 3. Privacy Settings
- **Privacy Mode**: When enabled, VibeMail strips real names and specific email addresses from content before sending it to the AI API, replacing them with generic placeholders (e.g., `[Sender]`).
- **Enable AI Features**: A global toggle to turn off all AI-related IPC calls.

---

## 4. Sync & Automation
- **Auto-sync interval**: How often (in minutes) the app polls your IMAP server for new messages. Set to `0` to disable background syncing.
- **IMAP IDLE**: Real-time push notifications for new mail, maintained as persistent connections per account.
- **Auto-labeling**: If enabled, VibeMail automatically runs the "Triage" AI task on every new unread thread after sync.
- **Auto-mark-as-read**: Optionally mark threads as read when opened in the thread view.
- **History fetch duration (days)**: How far back the app looks when you request "older emails" (default 30).
- **Max emails to download**: Limits messages fetched in a single history request to prevent backend throttling.

---

## 5. Drafts
- **Auto-save**: Drafts are saved to the local database automatically with a 2-second debounce as you type.
- **IMAP sync**: On compose close, drafts are synced to the IMAP server's Drafts folder (handles Gmail's `[Gmail]/Drafts` automatically).
- **Cleanup**: When a draft is sent, it is deleted from both the local database and the IMAP Drafts folder.
- **Drafts folder**: Browse both local auto-saved drafts and IMAP-synced drafts in the dedicated Drafts folder view.

---

## 6. Custom Categories
You can define up to 12 custom categories to help the AI organize your inbox.
- **Category Name**: A unique identifier (e.g., "Invoices", "Travel", "Open Source").
- **Examples**: Providing 3-5 short examples (e.g., "Flight confirmation for Paris") helps the LLM understand the context of the category.
- **How it works**: These categories are injected into the system prompt when the AI performs the `categorize_threads` task.

---

## 7. Danger Zone
Located at the bottom of the Settings page for critical maintenance tasks.
- **Wipe All Local Data**: Deletes all local messages, threads, mailboxes, and AI indices. Useful for clearing cache or starting fresh.
- **Reset Database Schema**: Drops and recreates SQLite tables. Recommended if you encounter "ON CONFLICT" or schema-related errors.
- **Preservation**: Account credentials are **not** removed during a wipe, but automatic syncing is disabled for safety.

---

## 8. Technical Details: How settings are stored
- **AI Config**: Stored in the `ai_config` table in the local SQLite database.
- **User Preferences**: (Sync interval, labels, etc.) stored in the browser's `localStorage` via the `zustand` store.
- **Sensitive Data**: API keys and OAuth refresh tokens stored in the **System Keychain** using the Rust `keyring` crate.
- **Token Storage Paths**:
  - **macOS/Linux**: `~/.local/share/com.vibemail.app/tokens.json`
  - **Windows**: `%APPDATA%\com.vibemail.app\tokens.json`
