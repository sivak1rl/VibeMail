# Coming Soon: VibeMail Roadmap

We are constantly working to make VibeMail the most intelligent and private email client on the desktop. Here is a look at what's currently in development and what we have planned for the future.

---

## 🚀 Short-Term (Polishing the MVP)
Focusing on core reliability and essential email features.

### Sync & Reliability
- **Real-time Push (IMAP IDLE)**: Get notified of new emails instantly without waiting for the sync timer.
- **Multi-mailbox Sync**: Full support for Sent, Drafts, Archive, and Trash folders.
- **Robust Retries**: Exponential backoff to handle transient network issues gracefully.

### UI/UX Enhancements
- **Keyboard-First Navigation**: `j/k` to move, `r` to reply, `e` to mark read—stay on your keyboard.
- **Mobile-Friendly Layout**: A responsive design with a collapsible sidebar and improved thread views.
- **Rich Media**: Inline image rendering and built-in attachment previews (PDF, Images).

### Advanced Compose
- **Full Rich Text Support**: Bold, italics, links, and lists.
- **Draft Auto-save**: Never lose a message; drafts are saved locally as you type.
- **Smart Autocomplete**: Contact suggestions based on your interaction history.

---

## 🧠 Medium-Term (Advanced AI Features)
Taking "AI-Native" to the next level.

- **Smart Reply Suggestions**: Choose from three AI-generated options (Brief, Detailed, or Decline) and refine them before sending.
- **Focus Inbox**: Automatically separate "Important" threads from "Other" using local triage scoring.
- **Daily Digest**: A "Here's what matters today" summary delivered every morning.
- **Action Extraction**: Automatically surface deadlines, tasks, and meeting times in a dedicated sidebar.

---

## 🌐 Long-Term (The VibeMail Platform)
Scaling the experience and expanding the ecosystem.

- **Unified Inbox**: Manage all your accounts in a single, color-coded view.
- **Privacy First**: Local-only mode where *all* data stays on your device (no cloud AI required).
- **Plugin System**: A Lua or WASM-based API for custom email processing and automation.
- **System Integration**: Tray icons, native OS notifications, and a CLI mode for power users.

---

## 🛠️ Technical Improvements
- **Comprehensive Testing**: Adding unit and integration tests for both Rust and React.
- **Performance at Scale**: Moving to DuckDB for heavy analytics and optimizing search indices.
- **Enhanced Security**: PGP/GPG support for end-to-end encrypted communication.

> [!TIP]
> Want to contribute? Check out our [[Build and Packaging]] guide to get started!
