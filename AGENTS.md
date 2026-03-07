# Repository Guidelines

## Project Structure & Module Organization
- `src/` contains the React + TypeScript UI.
- `src/components/` holds feature components (`InboxList`, `ThreadView`, `Compose`, `AIPanel`).
- `src/pages/` contains top-level screens (`Inbox`, `Settings`, `AccountSetup`).
- `src/stores/` contains Zustand stores and Tauri IPC interactions.
- `src-tauri/` is the Rust backend:
  - `src/commands/` for `tauri::command` handlers.
  - `src/mail/`, `src/auth/`, `src/db/`, `src/search/`, `src/ai/` for domain logic.
- `.github/workflows/ci.yml` defines required checks and is the CI source of truth.

## Build, Test, and Development Commands
```bash
npm ci                               # Install Node deps from lockfile
npm run tauri dev                    # Run desktop app in development
npm run build                        # Frontend production build (includes tsc)
npm run type-check                   # TypeScript checks only
npm run lint                         # ESLint for TS/TSX
cd src-tauri && cargo fmt --check    # Rust formatting check
cd src-tauri && cargo clippy -- -D warnings
cd src-tauri && cargo test           # Rust unit/integration tests
npm run tauri build -- --no-bundle   # CI-like cross-platform build step
```

## Coding Style & Naming Conventions
- TypeScript/TSX uses 2-space indentation and strict compiler settings.
- React component/page files use `PascalCase.tsx`; hooks and store APIs use `camelCase` (for example, `useAccountStore`).
- Keep CSS in `*.module.css` files next to related UI files.
- Rust code follows `rustfmt`; prefer `snake_case` for modules/functions and `PascalCase` for structs/enums.
- Run linting and formatting locally before pushing.

## Testing Guidelines
- Backend tests use Rust’s built-in test framework (`cargo test`) under `src-tauri`.
- Frontend automated tests are not configured yet; include manual verification steps for UI changes.
- Prioritize regression tests for backend changes in `db`, `mail`, `search`, and `ai`.
- No minimum coverage percentage is enforced today; new logic should include tests when practical.

## Commit & Pull Request Guidelines
- Use concise, imperative commit messages, typically with conventional prefixes seen in history: `feat:`, `fix:`, `docs:`, `chore:`.
- Keep each commit scoped to one logical change.
- PRs should include:
  - What changed and why.
  - How it was tested (commands run and/or manual steps).
  - Screenshots or short recordings for UI updates.
  - Linked issue/task IDs and any config/migration notes.
