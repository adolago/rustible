# Rustible

## Build Commands
- `cargo check` - Fast typecheck; `cargo build` - Build project
- `cargo test -- --test-threads=1` - Run tests in a disposable environment (CI runs single-threaded)
- `cargo clippy --all-targets -- -D warnings` - Lint using the CI target scope
- `cargo fmt --all -- --check` - Format check

Canonical feature/implementation status lives in `docs/FEATURE_STATUS.md`.
Broad module and integration suites can execute commands and change filesystem or service state. Use an isolated disposable environment; inspect a focused test before running it on the development host.

## Important Rules
- Do what has been asked; nothing more, nothing less.
- NEVER create files unless they're absolutely necessary for achieving your goal.
- ALWAYS prefer editing an existing file to creating a new one.
- NEVER proactively create documentation files (*.md) or README files. Only create documentation files if explicitly requested by the User.
