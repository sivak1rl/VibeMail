# Configuration & Settings

VibeMail provides a centralized settings page to manage your accounts, AI providers, synchronization behavior, and custom categorization rules.

---

## 1. Account Management
In the **Accounts** section, you can see all linked IMAP/SMTP accounts.
- **Adding Accounts**: Handled via the initial setup or the "Add Account" flow (OAuth2 or Password).
- **Removing Accounts**: Clicking "Remove" will delete the account metadata, local email cache, and search indices for that specific account. It does **not** delete emails from the server.

---

## 2. AI Provider Configuration
VibeMail's "AI-Native" features depend on a configured LLM provider.

### Providers
- **Local Ollama (Default)**: Best for privacy. It connects to a local Ollama instance (usually `http://localhost:11434`). No API key is required.
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
- **Privacy Mode**: When enabled, VibeMail strips real names and specific email addresses from the content before sending it to the AI API. It replaces them with generic placeholders (e.g., `[Sender]`).
- **Enable AI Features**: A global toggle to turn off all AI-related IPC calls.

---

## 4. Sync & Automation
- **Auto-sync interval**: How often (in minutes) the app should poll your IMAP server for new messages. Set to `0` to disable background syncing.
- **Auto-labeling**: If enabled, VibeMail will automatically run the "Triage" AI task on every new unread thread immediately after a sync completes.
- **History fetch duration (days)**: Configures how far back the app looks when you request "older emails" (default 30).
- **Max emails to download**: Limits the number of messages fetched in a single history request to prevent backend throttling.

---

## 5. Custom Categories
You can define up to 12 custom categories to help the AI organize your inbox.
- **Category Name**: A unique identifier (e.g., "Invoices", "Travel", "Open Source").
- **Examples**: Providing 3-5 short examples (e.g., "Flight confirmation for Paris") helps the LLM understand the context of the category.
- **How it works**: These categories are injected into the system prompt when the AI performs the `categorize_threads` task.

---

## 6. Danger Zone
Located at the bottom of the Settings page for critical maintenance tasks.
- **Wipe All Local Data**: Deletes all local messages, threads, mailboxes, and AI indices. This is useful for clearing cache or starting fresh.
- **Reset Database Schema**: An optional step during a wipe that completely drops and recreates the SQLite tables. Recommended if you encounter "ON CONFLICT" or schema-related errors.
- **Preservation**: Note that account credentials are **not** removed during a wipe, but automatic syncing is disabled for safety.

---

## 7. Technical Details: How settings are stored
- **AI Config**: Stored in the `ai_config` table in the local SQLite database.
- **User Preferences**: (Sync interval, labels, etc.) are stored in the browser's `localStorage` via the `zustand` store.
- **Sensitive Data**: API keys and OAuth refresh tokens are stored in the **System Keychain** using the Rust `keyring` crate.
