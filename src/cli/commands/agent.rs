//! `rustible agent` — build, deploy and inspect the agent binary.
//!
//! The agent is a small binary that runs on a target and answers JSON
//! requests. These subcommands cover its lifecycle: building it for a target
//! triple, copying it to the hosts in an inventory, asking each host what it
//! reports, and stopping a listening agent.

use anyhow::{anyhow, Result};
use std::path::{Path, PathBuf};

use rustible::agent::{AgentBuilder, AgentStatus};
use rustible::connection::{Connection, ConnectionFactory, ExecuteOptions};
use rustible::inventory::Inventory;

use super::CommandContext;
use crate::cli::{AgentBuildArgs, AgentCommand, AgentDeployArgs, AgentStatusArgs, AgentStopArgs};

/// Execute an `agent` subcommand.
pub async fn execute(command: &AgentCommand, ctx: &mut CommandContext) -> Result<i32> {
    match command {
        AgentCommand::Build(args) => build(args, ctx),
        AgentCommand::Deploy(args) => deploy(args, ctx).await,
        AgentCommand::Status(args) => status(args, ctx).await,
        AgentCommand::Stop(args) => stop(args, ctx).await,
    }
}

/// Build the agent binary for a target triple.
fn build(args: &AgentBuildArgs, ctx: &mut CommandContext) -> Result<i32> {
    ctx.output.banner("AGENT BUILD");

    let mut builder = AgentBuilder::new()
        .release(!args.debug)
        .output_dir(args.output.clone())
        .strip(args.strip);
    if let Some(target) = &args.target {
        builder = builder.target(target);
    }

    ctx.output.plan(&format!(
        "Building rustible-agent for {} ({})",
        builder.config().target,
        if args.debug { "debug" } else { "release" }
    ));

    match builder.build() {
        Ok(path) => {
            ctx.output
                .success(&format!("Agent binary: {}", path.display()));
            Ok(0)
        }
        Err(error) => {
            ctx.output.error(&format!("Agent build failed: {}", error));
            Ok(1)
        }
    }
}

/// Copy the agent binary to every host in the inventory.
async fn deploy(args: &AgentDeployArgs, ctx: &mut CommandContext) -> Result<i32> {
    ctx.output.banner("AGENT DEPLOY");

    let binary = match resolve_binary(args, ctx) {
        Ok(binary) => binary,
        Err(error) => {
            ctx.output.error(&error.to_string());
            return Ok(1);
        }
    };

    let (inventory, factory) = match load_targets(ctx) {
        Ok(targets) => targets,
        Err(error) => {
            ctx.output.error(&error.to_string());
            return Ok(1);
        }
    };

    let remote_path = PathBuf::from(&args.remote_path);
    let mut failures = 0;

    for host in hosts_for(&inventory, ctx) {
        let connection = match factory.get_connection(&host).await {
            Ok(connection) => connection,
            Err(error) => {
                ctx.output
                    .error(&format!("{}: cannot connect: {}", host, error));
                failures += 1;
                continue;
            }
        };

        match install(connection.as_ref(), &binary, &remote_path).await {
            Ok(version) => ctx
                .output
                .success(&format!("{}: installed {}", host, version)),
            Err(error) => {
                ctx.output.error(&format!("{}: {}", host, error));
                failures += 1;
            }
        }
    }

    Ok(if failures == 0 { 0 } else { 1 })
}

/// Ask every host what its agent reports.
async fn status(args: &AgentStatusArgs, ctx: &mut CommandContext) -> Result<i32> {
    ctx.output.banner("AGENT STATUS");

    let (inventory, factory) = match load_targets(ctx) {
        Ok(targets) => targets,
        Err(error) => {
            ctx.output.error(&error.to_string());
            return Ok(1);
        }
    };

    let mut missing = 0;
    for host in hosts_for(&inventory, ctx) {
        let connection = match factory.get_connection(&host).await {
            Ok(connection) => connection,
            Err(error) => {
                ctx.output
                    .error(&format!("{}: cannot connect: {}", host, error));
                missing += 1;
                continue;
            }
        };

        match agent_status(connection.as_ref(), DEFAULT_REMOTE_PATH).await {
            Ok(status) => {
                ctx.output.plan(&format!(
                    "{}: version {}, {} task(s) executed, {} running",
                    host, status.version, status.tasks_executed, status.tasks_running
                ));
                if args.detailed {
                    ctx.output.plan(&format!(
                        "    host {} ({} {}), uptime {}s",
                        status.host_info.hostname,
                        status.host_info.os,
                        status.host_info.arch,
                        status.uptime
                    ));
                }
            }
            Err(error) => {
                ctx.output.warning(&format!("{}: {}", host, error));
                missing += 1;
            }
        }
    }

    Ok(if missing == 0 { 0 } else { 1 })
}

/// Stop a listening agent on every host.
async fn stop(args: &AgentStopArgs, ctx: &mut CommandContext) -> Result<i32> {
    ctx.output.banner("AGENT STOP");

    let (inventory, factory) = match load_targets(ctx) {
        Ok(targets) => targets,
        Err(error) => {
            ctx.output.error(&error.to_string());
            return Ok(1);
        }
    };

    let signal = if args.force { "-KILL" } else { "-TERM" };
    let mut failures = 0;

    for host in hosts_for(&inventory, ctx) {
        let connection = match factory.get_connection(&host).await {
            Ok(connection) => connection,
            Err(error) => {
                ctx.output
                    .error(&format!("{}: cannot connect: {}", host, error));
                failures += 1;
                continue;
            }
        };

        // pkill reports 1 when nothing matched, which is not an error here.
        let command = format!("pkill {} -f 'rustible-agent --serve' || true", signal);
        match connection
            .execute(&command, Some(ExecuteOptions::new()))
            .await
        {
            Ok(_) => ctx.output.success(&format!("{}: stop signal sent", host)),
            Err(error) => {
                ctx.output.error(&format!("{}: {}", host, error));
                failures += 1;
            }
        }
    }

    Ok(if failures == 0 { 0 } else { 1 })
}

/// Where `agent deploy` installs the binary unless told otherwise.
const DEFAULT_REMOTE_PATH: &str = "/usr/local/bin/rustible-agent";

/// Decide which local binary to deploy, building it first when asked.
fn resolve_binary(args: &AgentDeployArgs, ctx: &mut CommandContext) -> Result<PathBuf> {
    if let Some(binary) = &args.binary {
        if !binary.is_file() {
            return Err(anyhow!("Agent binary not found: {}", binary.display()));
        }
        return Ok(binary.clone());
    }

    if !args.build {
        return Err(anyhow!(
            "Provide --binary <PATH>, or --build to build one first"
        ));
    }

    let mut builder = AgentBuilder::new();
    if let Some(target) = &args.target {
        builder = builder.target(target);
    }
    ctx.output.plan(&format!(
        "Building rustible-agent for {}",
        builder.config().target
    ));
    builder
        .build()
        .map_err(|error| anyhow!("Agent build failed: {}", error))
}

/// Load the inventory and a transport for its hosts.
fn load_targets(ctx: &CommandContext) -> Result<(Inventory, ConnectionFactory)> {
    let inventory_path = ctx
        .inventory()
        .cloned()
        .ok_or_else(|| anyhow!("An inventory is required (-i/--inventory)"))?;
    let inventory = Inventory::load(&inventory_path).map_err(|error| {
        anyhow!(
            "Failed to load inventory from {}: {}",
            inventory_path.display(),
            error
        )
    })?;

    let factory = ConnectionFactory::with_pool_size(
        super::run::build_connection_config(&inventory, None, None, ctx.timeout),
        ctx.forks.max(1),
    );
    Ok((inventory, factory))
}

/// Hosts the command applies to, honoring `--limit`.
fn hosts_for(inventory: &Inventory, ctx: &CommandContext) -> Vec<String> {
    let pattern = ctx.limit.as_deref().unwrap_or("all");
    inventory
        .get_hosts_for_pattern(pattern)
        .map(|hosts| hosts.into_iter().map(|host| host.name.clone()).collect())
        .unwrap_or_default()
}

/// Upload the binary, make it executable and confirm it runs.
async fn install(
    connection: &(dyn Connection + Send + Sync),
    binary: &Path,
    remote_path: &Path,
) -> Result<String> {
    connection
        .upload(binary, remote_path, None)
        .await
        .map_err(|error| anyhow!("upload failed: {}", error))?;

    let chmod = format!(
        "chmod 0755 {}",
        rustible::utils::shell_escape(&remote_path.to_string_lossy())
    );
    let result = connection
        .execute(&chmod, Some(ExecuteOptions::new()))
        .await
        .map_err(|error| anyhow!("chmod failed: {}", error))?;
    if !result.success {
        return Err(anyhow!("chmod failed: {}", result.stderr.trim()));
    }

    let version = connection
        .execute(
            &format!(
                "{} --version",
                rustible::utils::shell_escape(&remote_path.to_string_lossy())
            ),
            Some(ExecuteOptions::new()),
        )
        .await
        .map_err(|error| anyhow!("the deployed agent did not run: {}", error))?;
    if !version.success {
        return Err(anyhow!(
            "the deployed agent did not run: {}",
            version.stderr.trim()
        ));
    }

    Ok(version.stdout.trim().to_string())
}

/// Read the status the deployed agent reports.
async fn agent_status(
    connection: &(dyn Connection + Send + Sync),
    remote_path: &str,
) -> Result<AgentStatus> {
    let command = format!("{} --status", rustible::utils::shell_escape(remote_path));
    let result = connection
        .execute(&command, Some(ExecuteOptions::new()))
        .await
        .map_err(|error| anyhow!("cannot run the agent: {}", error))?;

    if !result.success {
        return Err(anyhow!(
            "no agent at {} ({})",
            remote_path,
            result.stderr.trim()
        ));
    }

    serde_json::from_str(result.stdout.trim())
        .map_err(|error| anyhow!("unreadable agent status: {}", error))
}
