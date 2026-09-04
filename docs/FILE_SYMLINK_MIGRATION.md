# File-module symbolic-link corrections (alpha)

The native file module now resolves a relative symbolic-link target from the
link's parent directory. Previously `dir/link -> target` could create or change
`target` in the process working directory instead of `dir/target`. Chained links
use the same rule at each step; up to 40 links are supported. Longer chains and
cycles fail. Resolution errors are reported instead of falling back silently.
For file/directory/touch states with `follow: true`, attributes apply to the
resolved target. A dangling final file target can still be created, including
missing parent directories.

With `follow: false`, ownership checks inspect the link and ownership changes
use `lchown`, which updates that link rather than its target. Recursive ownership
and permission checks inspect the same object selected by `follow`. A root
directory link is not traversed when following is disabled. The file state can
manage a dangling link without creating or truncating its target. Symlink modes
remain unchanged when `follow: false`; regular-file mode behavior is unchanged.

Timestamp and SELinux helpers do not support updating the link itself. An explicit
timestamp, touch or SELinux request on a no-follow link now returns an unsupported
error before any mutation, including in check mode. Recursive SELinux requests
with `follow: false` also fail before mutation if the existing tree contains a
link. Enable following only when the intended operation is on the target, or
use a separate tool that explicitly supports attributes on the link itself.
The migration does not add SELinux link support or silently skip that request.

Public module method signatures and the default `follow: true` remain unchanged.
These alpha corrections can change which path receives an update and can reject
previously accepted requests. They address stable local paths; they do not make
multi-step filesystem operations atomic against concurrent path replacement,
add remote transport support, or certify all file-module/Ansible behavior.

Regressions use only fresh temporary files and isolated test child processes.
Ownership controls change group only to a group already permitted to the test
user and only within its temporary directory. Those two controls explicitly
report a skip if no different supplementary group is available; all path/mode,
unsupported-option and check-mode controls are unconditional. No privileged
ownership operation, installed SELinux command or managed host is used.
