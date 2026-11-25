//! Prepare and check the remote socket, then install hooks over ssh.
//!
//! The files written are produced by `morrow_core::remote::install::plan`, so
//! the remote host gets the same config edits as a local install.

use std::collections::BTreeMap;
use std::thread;
use std::time::Duration;

use morrow_core::remote::host::SshHost;
use morrow_core::remote::install as core_install;
use morrow_core::remote::socket::{self, RemoteSocketCheckFailure};

use super::ssh::{self, SshOutput};

pub struct InstallOutcome {
    pub ok: bool,
    pub message: String,
    pub socket_problem: Option<RemoteSocketCheckFailure>,
}

impl InstallOutcome {
    fn failed(message: impl Into<String>, problem: Option<RemoteSocketCheckFailure>) -> Self {
        Self {
            ok: false,
            message: message.into(),
            socket_problem: problem,
        }
    }
}

/// Make the remote folder the SSH user's alone before a tunnel is opened.
/// Nothing is forwarded when it cannot be vouched for.
pub fn prepare_socket(binary: &str, host: &SshHost) -> Result<String, RemoteSocketCheckFailure> {
    let script = socket::prepare_remote_socket_folder_script(host.agents_run_as_other_users, socket::REMOTE_SOCKET_BASE);
    let probe = ssh::run(binary, host, &socket::python_command(&script), Duration::from_secs(15));
    match probe {
        Ok(result) => socket::prepared_socket_path(
            &result.stdout,
            &result.stderr,
            result.exit_code,
            socket::REMOTE_SOCKET_BASE,
        ),
        Err(message) => Err(RemoteSocketCheckFailure::Unreachable(message)),
    }
}

/// The socket check, retried once when ssh did not run it at all (#82).
fn check_socket(binary: &str, host: &SshHost, remote_socket_path: &str) -> Option<RemoteSocketCheckFailure> {
    let script = socket::verify_remote_socket_script(remote_socket_path, host.agents_run_as_other_users, 20.0);
    let command = socket::python_command(&script);
    for attempt in 1..=2 {
        let check = ssh::run(binary, host, &command, Duration::from_secs(45));
        let result = check.unwrap_or(SshOutput {
            stdout: String::new(),
            stderr: "ssh timed out".into(),
            exit_code: -1,
        });
        if attempt == 2 || socket::has_socket_check_answer(&result.stdout) {
            return socket::socket_problem(
                &result.stdout,
                &result.stderr,
                result.exit_code,
                remote_socket_path,
            );
        }
        thread::sleep(Duration::from_secs(2));
    }
    None
}

/// Verify the socket, read the host's config files and write the planned
/// result. A socket that cannot be vouched for stops the install here.
pub fn install_hooks(
    binary: &str,
    host: &SshHost,
    remote_socket_path: &str,
    hook_source: &str,
    plugin_source: &str,
) -> InstallOutcome {
    if let Some(problem) = check_socket(binary, host, remote_socket_path) {
        return InstallOutcome::failed(problem.message_en(), Some(problem));
    }
    let snapshot_script = socket::python_command(&core_install::snapshot_script());
    let snapshot = match ssh::run(binary, host, &snapshot_script, Duration::from_secs(30)) {
        Ok(result) if result.ok() => core_install::config_snapshot(&result.stdout),
        Ok(result) => return InstallOutcome::failed(result.stderr.trim().to_owned(), None),
        Err(message) => return InstallOutcome::failed(message, None),
    };
    let input = core_install::RemoteInstallInput {
        host,
        remote_socket_path,
        hook_source,
        plugin_source,
        existing: &snapshot,
    };
    let plan = core_install::plan(&input);
    let write_script = socket::python_command(&core_install::install_script(&plan));
    match ssh::run(binary, host, &write_script, Duration::from_secs(30)) {
        Ok(result) if result.ok() => InstallOutcome {
            ok: true,
            message: plan.summary,
            socket_problem: None,
        },
        Ok(result) => InstallOutcome::failed(result.stderr.trim().to_owned(), None),
        Err(message) => InstallOutcome::failed(message, None),
    }
}

/// The host's config files as a map, for callers that want the plan without
/// running the write step. Kept separate so the plan stays testable.
pub fn read_config_files(binary: &str, host: &SshHost) -> Result<BTreeMap<String, String>, String> {
    let script = socket::python_command(&core_install::snapshot_script());
    let result = ssh::run(binary, host, &script, Duration::from_secs(30))?;
    if !result.ok() {
        return Err(result.stderr);
    }
    Ok(core_install::config_snapshot(&result.stdout))
}
