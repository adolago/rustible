# Russh Crate Research - SSH Implementation in Rust

**Date:** 2025-12-22
**Purpose:** Research the russh crate for SSH connectivity, command execution, and file transfer capabilities

---

## Table of Contents

1. [Overview](#overview)
2. [Key Features](#key-features)
3. [Architecture & Design](#architecture--design)
4. [Establishing Connections](#establishing-connections)
5. [Authentication Methods](#authentication-methods)
6. [Executing Commands](#executing-commands)
7. [File Transfer (SFTP)](#file-transfer-sftp)
8. [Companion Crates](#companion-crates)
9. [Code Examples](#code-examples)
10. [Projects Using Russh](#projects-using-russh)
11. [Comparison with Alternatives](#comparison-with-alternatives)
12. [References](#references)

---

## Overview

**Russh** is a low-level Tokio SSH2 client and server implementation written in Rust. It is a fork of Thrussh by Pierre-Étienne Meunier and provides a fully asynchronous, protocol-level SSH implementation.

- **Crate:** [russh](https://crates.io/crates/russh)
- **Documentation:** [docs.rs/russh](https://docs.rs/russh/latest/russh/)
- **Repository:** [github.com/Eugeny/russh](https://github.com/Eugeny/russh)
- **Latest Version:** 0.54.3 (as of research date)
- **Downloads:** High popularity with active maintenance

### Key Characteristics

- **Async-first:** Built on Tokio for asynchronous I/O operations
- **Low-level:** Protocol-level implementation requiring manual trait implementation
- **Pure Rust:** No C dependencies (unlike ssh2 crate which depends on libssh2)
- **Both client and server:** Can be used for both SSH client and server applications

---

## Key Features

### Crypto Backend Requirement

**CRITICAL:** Russh requires enabling at least one crypto backend feature:
- `aws-lc-rs`
- `ring`

The crate fails to compile when both are disabled because a crypto backend is required.

### Protocol Support

- **SSH Protocol Version:** SSH2
- **SFTP Version:** Version 3 (most popular/widely supported)
- **Channel-based:** Supports multiple parallel requests in a single connection
- **Tunneling:** SSH tunneling support via `russh-config` crate

### Supported Algorithms

**Host Keys & Public Key Auth:**
- `ssh-ed25519`
- `rsa-sha2-256`
- `rsa-sha2-512`
- `ssh-rsa`
- ECDSA variants

**Authentication Methods:**
- Password authentication
- Public key authentication
- Keyboard-interactive authentication
- None authentication
- OpenSSH certificates

**Symmetric Encryption & MAC:**
- Chacha20-Poly1305 (recommended by maintainers)
- Other modern cryptographic primitives

### Design Philosophy

The maintainers explicitly state they do NOT aim to implement all possible cryptographic algorithms published since SSH's initial release. They recommend:
- **Public Key Crypto:** Ed25519
- **Symmetric Crypto & MAC:** Chacha20-Poly1305

This design reduces technical debt and focuses on modern, secure primitives.

---

## Architecture & Design

### How Russh Works

If we exclude the key exchange and authentication phases (handled by Russh behind the scenes), the SSH protocol is relatively simple:

1. **Channels:** Clients and servers open channels (just integers) to handle multiple requests in parallel in a single connection
2. **Channel Opening:** Client obtains a `ChannelId` by calling `channel_open_…` methods on `client::Connection`
3. **Execution:** Client sends exec requests and data to the server via the channel
4. **Event Loop:** Async message handling for bidirectional communication

### Client Handler Pattern

Russh does NOT provide a direct SSH client. Instead, it provides the `russh::client::Handler` trait that must be implemented in your own struct. This trait defines handlers for SSH session features.

**Key Points:**
- Implementation of `check_server_key` method is **mandatory**
- The host key is provided as `russh_keys::key::PublicKey`
- Return `Ok(true)` for successful verification, `Ok(false)` to abort
- Clients handle both synchronous operations (commands) and asynchronous events (unsolicited server messages)

### Connection Lifecycle

```
1. Implement client::Handler trait
2. Create client::Config
3. Call russh::client::connect()
4. Authenticate (password, publickey, etc.)
5. Open channels (channel_open_session, etc.)
6. Execute commands or transfer files
7. Close channels
8. Disconnect session
```

---

## Establishing Connections

### Basic Connection Pattern

```rust
use russh::client;
use russh::keys::*;
use std::sync::Arc;

struct Client {}

#[async_trait]
impl client::Handler for Client {
    type Error = russh::Error;

    async fn check_server_key(
        &mut self,
        _server_public_key: &ssh_key::PublicKey,
    ) -> Result<bool, Self::Error> {
        // IMPORTANT: In production, verify the host key!
        // This example accepts all keys (INSECURE)
        Ok(true)
    }
}

#[tokio::main]
async fn main() {
    let config = russh::client::Config::default();
    let config = Arc::new(config);
    let sh = Client {};

    let mut session = russh::client::connect(config, ("127.0.0.1", 22), sh)
        .await
        .unwrap();

    // Continue with authentication...
}
```

### Host Key Verification

By default, the russh client will **NOT** connect to any host. Implementation of host authenticity check through the `check_server_key` method is mandatory. This may be tricky for newcomers but ensures security.

**Best Practices:**
- Verify the host key against known hosts
- Implement proper key fingerprint checking
- Never blindly return `Ok(true)` in production code

### Session Wrapper Pattern

A common pattern is to create a `Session` struct as a convenience wrapper:

```rust
pub struct Session {
    session: client::Handle<Client>,
}
```

This provides a higher-level interface for managing the connection lifecycle.

---

## Authentication Methods

Russh supports multiple authentication methods. After establishing a connection, you must authenticate before executing commands.

### Password Authentication

```rust
let auth_result = session.authenticate_password(username, password).await?;
```

### Public Key Authentication

Public key authentication involves:
1. Loading the private key using `russh-keys`
2. Authenticating with the key

**Key Points:**
- The public key is provided by the client, not pulled from a file
- Russh verifies key ownership (authentication)
- You decide whether to authorize the key (authorization)
- The `auth_publickey` method is called when a client tries to authenticate

### Loading Keys with russh-keys

The `russh-keys` crate provides methods to:
- Load secret keys from files
- Decipher encrypted keys with passwords
- Interact with SSH agents
- Handle key formats

Example pattern:
```rust
use russh_keys::*;

// Load a secret key, deciphering it with password if necessary
let key = russh_keys::load_secret_key(key_path, password)?;
```

### Keyboard-Interactive Authentication

Russh supports keyboard-interactive authentication for systems requiring challenge-response authentication.

### SSH Agent Support

The `russh_agent` crate provides asynchronous ssh-agent client implementation:
- Start an SSH agent server
- Connect with a client
- Decipher encrypted private keys
- Send keys to the agent
- Request the agent to sign data

### Dual Authentication

Some servers require multiple authentication methods (e.g., `AuthenticationMethods publickey,password` in OpenSSH config). Russh supports this but may require careful implementation.

---

## Executing Commands

### Basic Command Execution

The standard pattern for executing a command:

```rust
// 1. Open a session channel
let mut channel = session.channel_open_session().await?;

// 2. Execute the command
channel.exec(true, "ls -la").await?;

// 3. Read the output
let mut output = Vec::new();
let mut exit_code = None;

while let Some(msg) = channel.wait().await {
    match msg {
        russh::ChannelMsg::Data { ref data } => {
            output.write_all(data).unwrap();
        }
        russh::ChannelMsg::ExitStatus { exit_status } => {
            exit_code = Some(exit_status);
        }
        _ => {}
    }
}

// 4. Close the session properly
session
    .disconnect(Disconnect::ByApplication, "", "English")
    .await?;
```

### Channel Methods

Key methods on `client::Connection`:
- `channel_open_session()` - Opens a session channel
- `exec()` - Executes a command on the channel
- `data()` - Sends data to the command's stdin
- `wait()` - Waits for messages from the server

### Message Types

`ChannelMsg` variants include:
- `Data { data }` - Standard output data
- `ExtendedData { data, ext }` - Extended data (stderr)
- `ExitStatus { exit_status }` - Command exit code
- `Eof` - End of file
- And others...

### Interactive Sessions

For interactive PTY sessions, russh provides additional channel methods and the ability to handle terminal resize events, input/output, etc.

Example locations in repository:
- `russh/examples/remote_shell_call.rs`
- `russh/examples/client_exec_simple.rs`
- `russh/examples/client_exec_interactive.rs`

### russh-process Extension

The `russh-process` crate provides convenience extension traits:
- `HandleProcessExt` trait
- `channel_open_exec_spawn()` method
- `channel_open_exec_output()` method

These wrap the channel opening and command execution pattern for easier use.

---

## File Transfer (SFTP)

### russh-sftp Crate

**russh-sftp** provides server-side and client-side SFTP subsystem support for Russh.

- **Crate:** [russh-sftp](https://crates.io/crates/russh-sftp)
- **Documentation:** [docs.rs/russh-sftp](https://docs.rs/russh-sftp)
- **Repository:** [github.com/AspectUnk/russh-sftp](https://github.com/AspectUnk/russh-sftp)
- **Downloads:** 707,730 all-time downloads, 150,959 recent downloads

### Key Features

1. **std::fs-like API:** High-level API similar to Rust's standard filesystem API
2. **Async I/O:** Full async support for file operations
3. **SFTP v3:** Implemented according to version 3 specifications (most popular)
4. **Raw and High-Level:** Provides both low-level `RawSftpSession` and high-level abstractions

### Basic SFTP Pattern

```rust
use russh_sftp::client::SftpSession;
use russh_sftp::protocol::OpenFlags;
use tokio::io::{AsyncReadExt, AsyncSeekExt, AsyncWriteExt};

// After establishing SSH connection and authenticating...

// Create SFTP session
let sftp = SftpSession::new(channel).await?;

// File operations similar to std::fs
// - Read files
// - Write files
// - List directories
// - Create/delete files and directories
// - Get file metadata
```

### File Operations

The russh-sftp API abstracts the nuances and flaws of the SFTP protocol, providing:
- **File reading/writing** with async I/O traits
- **Directory operations** (list, create, remove)
- **Metadata operations** (stat, chmod, chown, etc.)
- **Symlink operations**

### Standard Communication

`RawSftpSession` provides methods for:
- Sending packets
- Receiving packets
- Low-level protocol control

### Examples

Official examples in the russh repository:
- `russh/examples/sftp_client.rs` - Client-side file transfer
- `russh/examples/sftp_server.rs` - Server-side SFTP implementation

### Alternative: rusftp

**rusftp** is an alternative SFTP library based on russh:
- **Crate:** [rusftp](https://crates.io/crates/rusftp)
- **Repository:** [github.com/aneoconsulting/rusftp](https://github.com/aneoconsulting/rusftp)

Key features:
- Pure Rust async SFTP client
- Cloneable `SftpClient` for concurrent operations
- Can be used behind shared references
- Supports multiple concurrent SFTP requests, even from multiple threads

---

## Companion Crates

### russh-keys

**Purpose:** SSH key management

**Features:**
- Load secret keys from files
- Decipher encrypted keys with passwords
- Deal with SSH agents
- Support various key formats
- Key generation and manipulation

**Documentation:** [docs.rs/russh-keys](https://docs.rs/russh-keys)

### russh-sftp

**Purpose:** SFTP subsystem for file transfer

**Features:**
- Client and server SFTP implementations
- High-level std::fs-like API
- Async I/O support
- SFTP v3 protocol

**Documentation:** [docs.rs/russh-sftp](https://docs.rs/russh-sftp)

### russh_agent

**Purpose:** SSH agent client

**Features:**
- Asynchronous ssh-agent client
- Key management via agent
- Signature requests

**Documentation:** [docs.rs/russh-agent](https://docs.rs/russh-agent)

### russh-config

**Purpose:** SSH configuration and tunneling

**Features:**
- SSH tunnel implementation
- ProxyCommand support (like OpenSSH)
- `Stream::tcp_connect` method
- `Stream::proxy_command` method

**Use Case:** Easy way to implement SSH tunnels and proxy commands

### russh-process

**Purpose:** Process execution helpers

**Features:**
- `HandleProcessExt` trait
- Simplified command execution wrappers
- Convenience methods for common patterns

---

## Code Examples

### Complete Client Example

```rust
use russh::client;
use russh::keys::*;
use std::sync::Arc;
use anyhow::Result;

struct Client;

#[async_trait::async_trait]
impl client::Handler for Client {
    type Error = anyhow::Error;

    async fn check_server_key(
        &mut self,
        server_public_key: &russh::keys::PublicKey,
    ) -> Result<bool, Self::Error> {
        // In production: verify against known_hosts
        println!("Server key: {:?}", server_public_key);
        Ok(true)
    }

    async fn data(
        &mut self,
        channel: ChannelId,
        data: &[u8],
        _session: &mut client::Session,
    ) -> Result<(), Self::Error> {
        println!("Data on channel {:?}: {} bytes", channel, data.len());
        Ok(())
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    // Configure client
    let config = russh::client::Config::default();
    let config = Arc::new(config);

    // Create handler
    let handler = Client;

    // Connect
    let mut session = russh::client::connect(
        config,
        ("example.com", 22),
        handler
    ).await?;

    // Authenticate
    let auth_res = session.authenticate_password("username", "password").await?;
    if !auth_res {
        return Err(anyhow::anyhow!("Authentication failed"));
    }

    // Open channel and execute command
    let mut channel = session.channel_open_session().await?;
    channel.exec(true, "whoami").await?;

    // Read output
    let mut output = Vec::new();
    let mut exit_code = None;

    while let Some(msg) = channel.wait().await {
        match msg {
            russh::ChannelMsg::Data { ref data } => {
                output.extend_from_slice(data);
            }
            russh::ChannelMsg::ExitStatus { exit_status } => {
                exit_code = Some(exit_status);
            }
            _ => {}
        }
    }

    println!("Output: {}", String::from_utf8_lossy(&output));
    println!("Exit code: {:?}", exit_code);

    // Disconnect
    session.disconnect(
        russh::Disconnect::ByApplication,
        "",
        "English"
    ).await?;

    Ok(())
}
```

### SFTP File Upload Example

```rust
use russh_sftp::client::SftpSession;
use russh_sftp::protocol::OpenFlags;
use tokio::io::AsyncWriteExt;

// After SSH connection and authentication...

// Open SFTP subsystem
let channel = session.channel_open_session().await?;
channel.request_subsystem(true, "sftp").await?;
let sftp = SftpSession::new(channel).await?;

// Upload a file
let mut remote_file = sftp.open_with_flags(
    "/remote/path/file.txt",
    OpenFlags::WRITE | OpenFlags::CREATE | OpenFlags::TRUNCATE
).await?;

let content = b"Hello, SFTP!";
remote_file.write_all(content).await?;
remote_file.shutdown().await?;
```

### Key-Based Authentication Example

```rust
use russh_keys::*;

// Load private key
let key_path = "/home/user/.ssh/id_ed25519";
let key = russh_keys::load_secret_key(key_path, None)?;

// Authenticate with key
let auth_res = session.authenticate_publickey("username", Arc::new(key)).await?;
```

---

## Projects Using Russh

### Sandhole

**Description:** Expose HTTP/SSH/TCP services through SSH port forwarding. A reverse proxy that works with an OpenSSH client.

**Russh Usage:**
- `russh::server` for reverse forwarding connections
- Local forwarding tunnels
- ratatui-based admin interface

### Motor OS

**Description:** A new Rust-based operating system for VMs.

**Russh Usage:**
- `russh::server` as the base for its SSH Server implementation

### termscp

**Description:** Feature-rich terminal file transfer and explorer with support for SCP/SFTP/FTP/Kube/S3/WebDAV.

**Note:** Currently uses ssh2, but developers are considering migrating to russh for a pure Rust implementation.

---

## Comparison with Alternatives

### russh vs ssh2

| Feature | russh | ssh2 |
|---------|-------|------|
| **Implementation** | Pure Rust | FFI bindings to libssh2 (C) |
| **Dependencies** | No C dependencies | Requires libssh2 |
| **Async** | Native Tokio async | Blocking (requires wrappers like async-ssh2-tokio) |
| **Level** | Low-level protocol | Higher-level API |
| **Client & Server** | Both supported | Client only |
| **Maintenance** | Active fork (Eugeny/russh) | Stable, maintained |

### async-ssh2-tokio

**Description:** An asynchronous, easy-to-use high-level SSH client library built on top of russh.

**Use Case:** If you want a simpler API without implementing traits, async-ssh2-tokio provides a higher-level wrapper around russh with:
- Simple connection methods
- Easy command execution
- Stdout/stderr/exit code retrieval
- Password, key file, and SSH agent auth

**Powered by:** russh

### When to Use Russh Directly

- You need server-side SSH functionality
- You want maximum control over the SSH protocol
- You need pure Rust without C dependencies
- You want to implement custom SSH features
- Performance is critical (no FFI overhead)

### When to Use Alternatives

- **async-ssh2-tokio:** You want a simple, high-level client API
- **ssh2:** You need a battle-tested library and don't mind C dependencies
- **Other libraries:** Specific protocol features or compatibility requirements

---

## References

### Official Documentation

- [russh crate page](https://crates.io/crates/russh)
- [russh API documentation](https://docs.rs/russh/latest/russh/)
- [russh GitHub repository](https://github.com/Eugeny/russh)
- [russh-keys documentation](https://docs.rs/russh-keys)
- [russh-sftp documentation](https://docs.rs/russh-sftp)

### Examples

- [Simple client exec](https://github.com/Eugeny/russh/blob/main/russh/examples/client_exec_simple.rs)
- [Interactive client exec](https://github.com/Eugeny/russh/blob/main/russh/examples/client_exec_interactive.rs)
- [Remote shell call](https://github.com/Eugeny/russh/blob/master/russh/examples/remote_shell_call.rs)
- [SFTP client](https://github.com/warp-tech/russh/blob/main/russh/examples/sftp_client.rs)
- [SFTP server](https://github.com/Eugeny/russh/blob/main/russh/examples/sftp_server.rs)

### Community Resources

- [Rust Users Forum: How to run a command on remote system through SSH?](https://users.rust-lang.org/t/how-to-run-a-command-on-remote-system-through-ssh/83325)
- [Rust Users Forum: Running multiple SSH client sessions using russh](https://users.rust-lang.org/t/running-multiple-ssh-client-sessions-using-russh/123513)
- [SSH port forwarding from within Rust code](https://dev.to/bbkr/ssh-port-forwarding-from-within-rust-code-5an)
- [A journey into File Transfer Protocols in Rust](https://blog.veeso.dev/blog/en/a-journey-into-file-transfer-protocols-in-rust/)

### GitHub Issues & Discussions

- [Dual Authentication (Key + Password) Issue #456](https://github.com/Eugeny/russh/issues/456)
- [Authentication with public keys Discussion #304](https://github.com/Eugeny/russh/discussions/304)
- [How to serve basic shell request Issue #162](https://github.com/Eugeny/russh/issues/162)

---

## Summary & Recommendations

### For Rustible Integration

**Pros of using russh:**
1. **Pure Rust** - No C dependencies, better for cross-compilation and security auditing
2. **Async-first** - Aligns with Rustible's async-first architecture (Tokio)
3. **Full control** - Low-level access to SSH protocol for custom features
4. **Active maintenance** - Well-maintained fork with regular updates
5. **SFTP support** - russh-sftp provides file transfer capabilities
6. **Both client & server** - Could enable future server features if needed

**Cons:**
1. **Low-level** - Requires implementing `client::Handler` trait (more boilerplate)
2. **Complexity** - Protocol-level implementation means more code to maintain
3. **Learning curve** - Less documentation than mature alternatives

**Recommendation:**
- **Direct russh use** is suitable for Rustible given the project's async-first architecture and desire for full control
- Consider creating a higher-level abstraction within Rustible's connection layer to simplify usage
- Use russh-sftp for file transfer operations
- Implement proper host key verification (don't blindly accept keys)
- Start with simple command execution, then add SFTP and advanced features

### Quick Start Checklist

1. Add dependencies to `Cargo.toml`:
   ```toml
   russh = { version = "0.54", features = ["aws-lc-rs"] }
   russh-keys = "0.49"
   russh-sftp = "2.0"
   ```

2. Implement `client::Handler` trait for your SSH client

3. Create connection configuration with appropriate crypto settings

4. Implement authentication (password, key-based, or both)

5. Build channel management for command execution

6. Add SFTP support for file transfers

7. Implement proper error handling and connection pooling

8. Add host key verification for security

---

**Research completed:** 2025-12-22
**Researched by:** Claude (Sonnet 4.5)
