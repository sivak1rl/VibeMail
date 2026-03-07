# Welcome to VibeMail Wiki

VibeMail is an **AI-Native Desktop Email Client** built with [Tauri 2](https://v2.tauri.app/), [Rust](https://www.rust-lang.org/), and [React](https://react.dev/). It brings intelligent automation, local-first search, and privacy-focused email management to your desktop.

## Core Pillars
- **Local-First**: Your emails and search indices are stored on your machine, not in the cloud.
- **AI-Native**: Integrated LLM support for summarization, smart replies, and triage.
- **High Performance**: Built with Rust for efficient memory usage and fast sync.
- **Privacy**: No middleman; the app connects directly to your IMAP/SMTP providers.

## Getting Started
- [[Build and Packaging]]: How to set up your dev environment and build for your OS.
- [[AI Integration]]: Configuring Ollama or OpenAI-compatible endpoints.
- [[Architecture]]: Understanding the Rust backend and React frontend interaction.

## Features
- **Multi-account IMAP/SMTP**: Gmail, Outlook, and generic IMAP support.
- **Semantic Search**: Vector indexing with Tantivy for smarter queries.
- **Smart Threading**: Conversation grouping using the JWZ algorithm.
- **Triage & Insights**: Automatic importance scoring and thread summarization.
