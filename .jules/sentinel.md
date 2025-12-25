## 2024-05-23 - Command Injection in Template Module
**Vulnerability:** The `template` module was constructing shell commands (`chmod` and `cp`) by directly interpolating the destination filename into the command string.
**Learning:** Even in modules not explicitly designed for shell execution, internal helper commands often use the shell. If these commands take user-provided paths (like `dest`), they are vulnerable to command injection if not sanitized.
**Prevention:** Always use a `shell_escape` function when interpolating variables into shell command strings. Prefer `std::process::Command` with `.arg()` for local execution, but for remote execution over SSH where a string command is often required, strict escaping is mandatory.
