# API playbook path visibility

Absolute playbook paths outside every configured search root now return the same
HTTP404 response whether the path exists or is missing. Earlier code inspected
the outside path and returned403 for an existing object, which disclosed that
filesystem fact to an authenticated caller.

Clients should treat404 as unavailable or inaccessible and must not use the API
to determine whether an outside path exists. Valid paths under configured roots
retain their current behavior. Explicit parent-directory traversal and detected
symlinks escaping a configured root remain forbidden.

The correction removes the outside-path existence check. It does not change
authentication, inventory path overrides, configured-root authority, symlink race
handling or the broader filesystem authorization model. In-process regression
tests use only temporary files and never create jobs or listeners.
