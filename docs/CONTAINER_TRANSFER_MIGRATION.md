# Docker and Podman transfer corrections (alpha)

DockerConnection and PodmanConnection now keep transfer paths and owner/group
values as literal command operands. Shell helpers quote the complete values;
utility option boundaries prevent a filename or ownership value from becoming an
option. Public method signatures and default TransferOptions remain unchanged.

All remote transfer and file-query paths now use the container root as their
base. For example, `file` means `/file` and the literal filename `-` means `/-`.
Previously cp used that root base while chmod/chown and queries could use the
container process working directory and therefore reach a different file. This
follows the documented path rules of [Docker cp](https://docs.docker.com/reference/cli/docker/container/cp/)
and [Podman cp](https://docs.podman.io/en/stable/markdown/podman-cp.1.html).
Relative local cp operands are made explicit with `./`, so `file:name` stays a
local filename and `-` stays a file rather than selecting a tar stream. These
special cp meanings are described in the same official command references.

Path normalization is lexical only. Existing `..`, symlinks and trailing path
components are resolved by the container tools/filesystem, without host-side
canonicalization or a containment guarantee. A transfer to an existing directory
keeps the underlying cp directory behavior; this patch does not redesign file
versus directory targeting or follow-symlink semantics.

Empty paths, NUL and paths that cannot be represented as UTF-8 now fail before
transfer CLI calls or destination-directory creation. Owner/group fields reject
empty values, NUL and `:` inside an individual field before copying. Use the
separate owner and group fields; numeric IDs and other literal values remain
available subject to the container utility's own rules. No host identity lookup
or ownership change is performed by validation.

Requested mkdir, copy, chmod and chown failures now fail the transfer and stop
later stages. Previously a nonzero metadata command could still produce overall
success. A failed operation may already have copied data or created a directory;
no rollback or atomic replacement is added. File queries accept only a successful
command with the expected true/false marker, so transport failures and invalid
query responses are errors rather than false results.

This is a partial SEC004 repair for these two transports. It does not repair
Kubernetes, SSM or SSH transfer paths. Existing limitations remain: backup is
ignored; mode changes occur after copy; binary download_content still passes
through lossy UTF-8 command output; timeout and child cleanup do not guarantee
termination of container work; Docker Compose targeting/options and general
container-identifier validation are unchanged. This is not a guarantee of secure
secret transfer or behavior on every container utility implementation.

Focused Unix tests use a temporary fake CLI that records argv and returns
synthetic outcomes. Recorded remote strings are parsed for argument boundaries
and are never evaluated, forwarded or sent to a daemon. The tests verify failure
ordering and preflight behavior; no real Docker/Podman daemon, chown command,
network target or container is exercised.
