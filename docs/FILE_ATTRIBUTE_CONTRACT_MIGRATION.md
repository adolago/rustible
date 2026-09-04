# File attribute contract corrections

## Local line and block permissions

On Unix, `lineinfile` and `blockinfile` now apply an explicit `mode` when an
existing file already has the requested content or absent state. Previously,
both modules returned unchanged before checking permissions.

Check mode reports the pending permission change without applying it. Apply
mode changes permissions without rewriting file contents, normalizing line
endings, adding a trailing newline or changing the file's modification time.
A second run with the same mode reports unchanged. Mode-only changes do not
create content backups or content diffs. Existing combined content/mode updates
and requests without `mode` retain their prior behavior.

The existing path-based symlink-following behavior is retained. Metadata checks
and chmod are separate operations; this is not an atomic file-identity guarantee
against concurrent path replacement. This correction does not address ownership,
non-Unix permission support, the separate content-write permission-order issue,
or back-reference matching/replacement semantics. The existing early no-match
back-reference return remains a no-op, including its mode handling.

## Remote line permission limits

When remote `lineinfile` finds that content already matches and `mode` was
requested, it now returns an explicit unsupported error before backup or upload,
including in check mode. The transport interface does not provide a verified,
portable attribute-only update. Re-uploading identical bytes with a creation mode
does not reliably update existing SFTP file permissions. Even an already-correct
remote mode cannot currently be confirmed by this module, so these requests are
rejected instead of reported as unchanged.

Remote requests without `mode` keep their existing behavior. Remote updates that
also change content still use the existing transfer path; their mode enforcement
is not repaired or certified here. No new remote command, generic transport stat
call or transport implementation is introduced. `blockinfile` still uses its
local implementation; this change does not establish remote support for it.

## Local stat output

Local `stat.mode` now contains only the permission and special bits, formatted as
four octal digits: for example, a regular file with mode 0644 reports `"0644"`
instead of `"100644"`. Directory and symlink type bits are also excluded; sticky,
setuid and setgid bits remain part of the mode. Consumers that parsed the old
type-prefixed string should use the dedicated file-type fields instead.

With `follow: false`, a symlink reports its raw stored target in `lnk_source`,
including relative and dangling targets. With `follow: true`, metadata describes
the target and does not include `lnk_source`; a dangling target remains absent.
This aligns the local mode/link fields with the intended remote stat contract.
It does not repair remote command error handling, locale assumptions or other
local metadata error handling.

Verification uses temporary local files and an in-memory remote transport that
never runs commands. It does not establish behavior on a real remote host or a
complete cross-platform filesystem guarantee.
