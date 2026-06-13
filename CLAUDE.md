# Rustible

## Build Commands
- `cargo check` - Fast typecheck; `cargo build` - Build project
- `cargo test -- --test-threads=1` - Run tests (CI runs single-threaded)
- `cargo clippy --lib --bins -- -D warnings` - Lint
- `cargo fmt --all -- --check` - Format check

Canonical feature/implementation status lives in `docs/FEATURE_STATUS.md`.

## Important Rules
- Do what has been asked; nothing more, nothing less.
- NEVER create files unless they're absolutely necessary for achieving your goal.
- ALWAYS prefer editing an existing file to creating a new one.
- NEVER proactively create documentation files (*.md) or README files. Only create documentation files if explicitly requested by the User.
