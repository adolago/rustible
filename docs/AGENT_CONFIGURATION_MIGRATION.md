# Agent configuration corrections (alpha)

The persistent agent runtime has no TLS transport implementation. Previously,
`AgentConfig::tls`, `tls_cert` and `tls_key` were accepted but ignored while the
runtime served plain TCP or Unix-socket messages. `AgentRuntime::start` now rejects
`tls: true` or any supplied certificate/key path before creating its working
directory, binding a listener, replacing a socket path or accepting a connection.
This applies even when the TLS flag is false but a certificate/key path is supplied.
The runtime does not read those files or silently downgrade the request.

The constructor and serialized fields are unchanged. The default remains
`tls: false` with no certificate/key. Removing TLS settings is appropriate only
when an unencrypted transport is intended; it does not enable encryption.
`AgentClient` also has no TLS implementation. Its authentication token is not
transport encryption. This patch does not implement or validate a secure remote
agent deployment architecture.

`ExecuteParams::user` and `group` were also ignored, so a request could run as the
agent's own identity despite naming another one. Any supplied user/group value,
including an empty string, now produces an unsupported-identity error before
command construction, process execution or task counters are updated. This shared
guard covers direct `AgentRuntime::execute`, the runtime RPC handler and the
one-shot binary's calls into that method. Existing error types, constructors and
method signatures are retained. Omitting both fields keeps the existing execution
path. Choose the desired process identity outside the agent; this change does not
perform user/group lookup or switch identity.

These are alpha compatibility changes: formerly ignored requests now fail.
The one-shot binary still reports execution errors in its JSON response using its
existing behavior; this patch does not redesign its process exit status. The
connection wrapper currently launches one-shot agent commands through an underlying
transport and supplies no user/group fields. Its separate escalation wrapper is
unchanged. No general privilege-escalation guarantee follows from this repair.

Command timeout, idle timeout and maximum-concurrency fields remain unenforced by
the agent runtime. The client's request-wait timeout is a separate mechanism and
does not prove termination of an agent command. Listener concurrency, request-size
limits, bounded reads, child cleanup and direct-exec quote parsing remain separate
work. SEC-007 is only partially repaired by the identity guard.

The regression checks use synthetic configuration, empty listener addresses and
temporary paths, so neither baseline nor repaired runs open a network listener or
Unix socket. Identity controls use nonexistent synthetic programs or an empty
command; no real command, user/group utility or remote host is executed. They
verify preflight rejection and unchanged positive controls, not live TLS or a
complete agent lifecycle.
