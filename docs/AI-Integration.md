# AI Integration

VibeMail is designed to be "bring-your-own-AI." It supports both local-first and cloud-based models.

## Local AI (Ollama)
**Ollama** is the recommended provider for maximum privacy.
1. Install [Ollama](https://ollama.com/).
2. Pull a model (e.g., `ollama pull llama3.2`).
3. VibeMail connects to `localhost:11434` by default.

## Cloud AI (OpenAI-Compatible)
Any API that supports the OpenAI chat completion format (e.g., Anthropic, Groq, or OpenAI itself) can be used.
1. Navigate to **Settings > AI Configuration**.
2. Select **OpenAI-compatible** as the provider.
3. Provide the Base URL and your API Key.
4. API Keys are stored securely in the system keychain (via the `keyring` crate).

## AI Tasks
- **Summarization**: Generates a concise summary of long email threads.
- **Drafting**: Proposes replies based on the context of the conversation.
- **Triage**: Assigns importance scores (0-100) based on your preferences.
- **Action Items**: Extracts tasks and dates from emails automatically.
