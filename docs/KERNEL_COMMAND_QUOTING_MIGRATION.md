# Kernel workflow command quoting

Kernel boot-entry lookup and GRUB selection now quote complete inner scripts
before passing them to `bash -lc`. Previously, independently quoted values were
inserted inside another single-quoted script. Ordinary entry names containing
spaces could be truncated, and punctuation could change shell interpretation.

GRUB lookup passes the expected release in a dedicated environment variable and
reads it through AWK's ENVIRON map. It is no longer inserted into AWK program
text or interpreted as an AWK escaped assignment. Systemd-boot lookup uses fixed
string grep matching, so release punctuation is literal rather than a regular
expression. Existing case-insensitive substring matching is retained.

No request schema changes. GRUB one-shot and persistent-default operations retain
their existing fallback utilities; systemd-boot setting commands are unchanged.
This corrects argument transport, not every bootloader semantic. Leading-option
entry identifiers, submenu discovery, configuration-file variants, operation
failure/recovery, privilege escalation and actual boot success remain separate
verification concerns. No real boot command or configuration change is a test.

Focused tests capture production commands with an inert connection, inspect the
outer quoting and evaluate inner scripts with a clean environment and only shell
stub functions. A separate AWK case uses an owned temporary configuration file;
no test reads `/boot` or connects to a host. Real bootloader/platform integration
remains unverified.
