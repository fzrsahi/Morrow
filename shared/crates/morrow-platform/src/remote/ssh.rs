//! Run the system `ssh` binary. No SSH library and no new dependency: the
//! command vectors come from `morrow_core::remote::host`.

use std::io::{BufRead, BufReader, Read};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

use morrow_core::remote::host::{self, SshHost};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SshOutput {
    pub stdout: String,
    pub stderr: String,
    pub exit_code: i32,
}

impl SshOutput {
    pub fn ok(&self) -> bool {
        self.exit_code == 0
    }
}

/// Run one command on the host and collect both streams. The child is killed
/// at `timeout`; ssh's own ConnectTimeout is the shorter of the two for a
/// connection attempt.
pub fn run(
    binary: &str,
    host: &SshHost,
    command: &str,
    timeout: Duration,
) -> Result<SshOutput, String> {
    if host.ssh_target().is_empty() || host.unsafe_ssh_field().is_some() {
        return Err("invalid host".into());
    }
    let mut args = host::ssh_arguments(host);
    args.push(command.to_owned());
    run_args(binary, &args, timeout)
}

pub fn run_args(binary: &str, args: &[String], timeout: Duration) -> Result<SshOutput, String> {
    let mut child = Command::new(binary)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("ssh launch failed: {e}"))?;
    let mut stdout = child.stdout.take().ok_or("ssh launch failed: no stdout")?;
    let mut stderr = child.stderr.take().ok_or("ssh launch failed: no stderr")?;

    let (tx_out, rx_out) = mpsc::channel();
    let (tx_err, rx_err) = mpsc::channel();
    thread::spawn(move || {
        let mut bytes = Vec::new();
        let _ = stdout.read_to_end(&mut bytes);
        let _ = tx_out.send(String::from_utf8_lossy(&bytes).into_owned());
    });
    thread::spawn(move || {
        let mut bytes = Vec::new();
        let _ = stderr.read_to_end(&mut bytes);
        let _ = tx_err.send(String::from_utf8_lossy(&bytes).into_owned());
    });

    let deadline = Instant::now() + timeout;
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                return Ok(SshOutput {
                    stdout: rx_out.recv().unwrap_or_default(),
                    stderr: rx_err.recv().unwrap_or_default(),
                    exit_code: status.code().unwrap_or(-1),
                });
            }
            Ok(None) if Instant::now() >= deadline => {
                let _ = child.kill();
                let _ = child.wait();
                let _ = rx_out.recv_timeout(Duration::from_secs(1));
                let _ = rx_err.recv_timeout(Duration::from_secs(1));
                return Err("ssh timed out".into());
            }
            Ok(None) => thread::sleep(Duration::from_millis(20)),
            Err(e) => return Err(format!("ssh wait failed: {e}")),
        }
    }
}

pub struct TunnelProcess {
    pub child: Child,
    /// The first non-empty stderr line, sent by the reader thread. The thread
    /// exits at EOF, so no readability handler keeps a closed pipe alive.
    pub stderr: mpsc::Receiver<String>,
}

/// Start the long-lived `-R` forward. Stdout is discarded; stderr is read once
/// so a dropped tunnel can report ssh's own words without spinning on a pipe.
pub fn spawn_tunnel(
    binary: &str,
    host: &SshHost,
    local_socket_path: &str,
    remote_socket_path: &str,
) -> Result<TunnelProcess, String> {
    let args = host::tunnel_arguments(host, local_socket_path, remote_socket_path);
    let mut child = Command::new(binary)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("ssh launch failed: {e}"))?;
    let stderr = child.stderr.take().ok_or("ssh launch failed: no stderr")?;
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        for line in BufReader::new(stderr).lines().map_while(Result::ok) {
            let message = line.trim();
            if !message.is_empty() {
                let _ = tx.send(message.to_owned());
                break;
            }
        }
    });
    Ok(TunnelProcess { child, stderr: rx })
}
