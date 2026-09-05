# Native file and transfer safety changes

These alpha fixes change unsafe or falsely successful behavior. They do not
establish full Ansible compatibility, cross-platform support, or safe execution
of every native module.

## Before and after

- Copy `directory_mode` previously changed existing ancestors, potentially up
  to filesystem root. It now affects only directories this operation creates.
  Set permissions on existing directories with a separate explicit task.
- File/content replacement previously wrote new bytes before restricting an
  existing file's permissions. Copy and the shared file-writing utility now
  restrict access before writing; broader permissions and special mode bits
  apply only after the complete write. Writes are still not crash-atomic.
- Archive output must now be outside its source. Equal paths, existing source
  inode aliases, and destinations inside source directories fail before output
  creation. Move the archive destination to a sibling directory. Enumeration
  and compressor-finalization failures no longer allow source deletion, and
  single-file archives retain the source basename. Complete archive output is
  staged privately and published by replacing its directory entry, so an output
  hardlinked to a source-directory member cannot truncate that member. Existing
  output permissions are retained; new archives have private mode0600. Source
  removal occurs only after successful publication. Set broader archive access
  with a separate explicit permissions task if needed.
- Archive entries named `.unarchive_marker` at the extraction root are now
  rejected because that name is reserved for Rustible metadata. Repack archives
  that contain it. Metadata replacement does not follow a previous marker link.
- ZIP extraction on Unix resolves member directories without following symlinks
  and replaces file directory entries atomically. Existing file symlinks and
  hardlinks are replaced, not written through; linked outside files are unchanged.
  Existing symlink directories cause failure. ZIP extraction on other platforms
  fails explicitly until an equivalent safe implementation is available.
- Local `get_url` now writes verified bytes, respects `force: false`, applies
  custom header mappings, and enforces the 100MiB limit on streamed bytes even
  without Content-Length. Local owner/group changes are explicitly unsupported.
  Local replacement is staged in the destination directory; the destination
  parent must already exist. The synchronous network module still expects an
  available Tokio runtime. Separate executor-level transport guards are not
  changed by this module-only patch.
- Scripts request mode0700 at upload, check chmod success, and attempt cleanup
  after both success and failure. Script privilege escalation is rejected until
  its identity and private-file access contract is implemented. Keep such jobs
  on a tool with verified escalation. Cleanup failure after successful execution
  is an error, not a successful task.

Archive/ZIP/download atomic replacement creates a new inode; existing custom ACLs,
extended attributes, and ownership are not promised to survive. Tar extraction
continues to use the tar library's member handling. These changes do not certify
extraction into directories concurrently controlled by another user, rollback,
real SSH/cloud/Windows/HPC behavior, or remote-backend permission guarantees.

## Verification boundary

`tests/diligence_module_safety_tests.rs` uses private temporary directories,
loopback HTTP, and a mock script connection. Its copy-ancestor regression runs
in a child process with a temporary working directory and relative paths, so
the failing baseline cannot chmod host ancestors. Permission-order tests use
temporary FIFOs, bounded child lifetimes, and explicit child cleanup.
The archive hardlink regression also limits child file size, address space,
CPU time, and wall-clock lifetime, so a broken self-copy cannot grow without bound.

Run only this focused suite until legacy broad suites have been isolated in
disposable infrastructure. A cached-dependency exact-source harness is useful
early evidence but is not a replacement for normal Cargo, CI, independent
review, or actual transport verification.
