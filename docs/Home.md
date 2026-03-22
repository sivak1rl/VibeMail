# Welcome to VibeMail Wiki

VibeMail is an **AI-Native Desktop Email Client** built with [Tauri 2](https://v2.tauri.app/), [Rust](https://www.rust-lang.org/), and [React](https://react.dev/). It brings intelligent automation, local-first search, and privacy-focused email management to your desktop.

## Core Pillars
- **Local-First**: Your emails and search indices are stored on your machine, not in the cloud.
- **AI-Native**: Integrated LLM support for summarization, smart replies, triage, and intelligent labeling.
- **High Performance**: Built with Rust for efficient memory usage and fast sync.
- **Privacy**: No middleman; the app connects directly to your IMAP/SMTP providers.

## Getting Started
- [[Build and Packaging]]: System dependencies, dev setup, and production builds.
- [[Configuration]]: Accounts, AI providers, sync settings, and custom categories.
- [[AI Integration]]: Configuring Ollama or OpenAI-compatible endpoints.
- [[Architecture]]: Understanding the Rust backend and React frontend interaction.
- [[Security]]: OAuth2, token storage, and AI privacy.
- [[Troubleshooting]]: Common issues and fixes.
- [[Coming Soon]]: Roadmap — what's done and what's planned.

## Features
- **Multi-account IMAP/SMTP**: Gmail, Outlook, and generic IMAP with OAuth2 PKCE or password auth.
- **Real-time Sync**: Background sync with IMAP IDLE push notifications and sliding window fetch.
- **Advanced Search**: FTS5 keyword search, semantic vector search, Gmail-style filters, and persistent history.
- **Drafts Management**: Auto-save, IMAP Drafts folder sync, inline editing, automatic cleanup on send.
- **AI-Powered Insights**: Thread summaries, smart replies with tone selector, action items, triage scoring, intelligent labeling, and email roundup digest.
- **Attachment Sidebar**: Unified access to all thread files with image previews and one-click open.
- **Compose**: Rich text editor, drag-and-drop attachments, CC/BCC, contact autocomplete, signatures, and AI-assisted drafting.
- **Responsive Design**: Collapsible sidebar, lightbox email view, keyboard shortcuts, and dark theme.
