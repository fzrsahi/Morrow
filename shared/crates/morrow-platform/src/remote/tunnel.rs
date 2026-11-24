//! A held ssh `-R` tunnel with the Swift forwarder's generation rule.
//!
//! `connect()` and `disconnect()` are synchronous. The caller drives `poll()`
//! from its event loop to notice an exited ssh, and reads status changes with
//! `take_status_change()`. No runtime is pulled into the workspace for this.

use std::time::Duration;

use morrow_core::remote::host::{self, SshHost};
use morrow_core::remote::tunnel::{TunnelState, TunnelStatus};

use super::install;
use super::ssh::{self, TunnelProcess};

pub struct RemoteTunnel {
    binary: String,
    state: TunnelState,
    process: Option<TunnelProcess>,
    hook_source: String,
    plugin_source: String,
    published: Option<TunnelStatus>,
    changed: Option<TunnelStatus>,
}

impl Default for RemoteTunnel {
    fn default() -> Self {
        Self::new()
    }
}

impl RemoteTunnel {
    pub fn new() -> Self {
        Self {
            binary: "ssh".into(),
            state: TunnelState::default(),
            process: None,
            hook_source: String::new(),
            plugin_source: String::new(),
            published: None,
            changed: None,
        }
    }

    /// Tests can point this at a stand-in for `ssh`; production uses the one
    /// on PATH, as on macOS and Linux.
    pub fn with_binary(binary: impl Into<String>) -> Self {
        Self {
            binary: binary.into(),
            ..Self::new()
        }
    }

    pub fn with_resources(mut self, hook_source: impl Into<String>, plugin_source: impl Into<String>) -> Self {
        self.hook_source = hook_source.into();
        self.plugin_source = plugin_source.into();
        self
    }

    pub fn status(&self) -> &TunnelStatus {
        self.state.status()
    }

    /// The changed status since the last read, for the Settings card.
    pub fn take_status_change(&mut self) -> Option<TunnelStatus> {
        self.changed.take()
    }

    fn publish(&mut self) {
        let status = self.state.status().clone();
        if self.published.as_ref() != Some(&status) {
            self.published = Some(status.clone());
            self.changed = Some(status);
        }
    }

    /// Clear a stale socket, start the tunnel, then vouch for the new socket
    /// and install hooks. A refused socket means no install and no live
    /// tunnel: the child is terminated before this returns.
    pub fn connect(&mut self, host: &SshHost, local_socket_path: &str) {
        let invalid = host.ssh_target().is_empty() || host.unsafe_ssh_field().is_some();
        let generation = self.state.begin_connect(invalid);
        self.publish();
        if invalid {
            return;
        }

        let socket_path = match install::prepare_socket(&self.binary, host) {
            Ok(path) => path,
            Err(problem) => {
                self.state.socket_refused(generation, &problem);
                self.publish();
                return;
            }
        };

        let cleanup = host::cleanup_arguments(host, &socket_path);
        let _ = ssh::run_args(&self.binary, &cleanup, Duration::from_secs(5));

        match ssh::spawn_tunnel(&self.binary, host, local_socket_path, &socket_path) {
            Ok(process) => self.process = Some(process),
            Err(message) => {
                self.state.failed(generation, message);
                self.publish();
                return;
            }
        }

        let outcome = install::install_hooks(
            &self.binary,
            host,
            &socket_path,
            &self.hook_source,
            &self.plugin_source,
        );
        if outcome.ok {
            self.state.connected(generation);
        } else {
            self.stop_process();
            if let Some(problem) = outcome.socket_problem {
                self.state.socket_refused(generation, &problem);
            } else {
                self.state.failed(generation, outcome.message);
            }
        }
        self.publish();
    }

    pub fn disconnect(&mut self) {
        self.state.disconnect();
        self.stop_process();
        self.publish();
    }

    /// Notice an ssh exit or its first stderr line. A late event from a
    /// superseded connect is ignored by `TunnelState`.
    pub fn poll(&mut self) {
        let generation = self.state.generation();
        let mut exited: Option<i32> = None;
        let mut stderr_message = None;
        if let Some(process) = self.process.as_mut() {
            match process.child.try_wait() {
                Ok(Some(status)) => exited = Some(status.code().unwrap_or(-1)),
                Ok(None) => stderr_message = process.stderr.try_recv().ok(),
                Err(_) => exited = Some(-1),
            }
        }
        if let Some(code) = exited {
            self.process.take();
            self.state.process_exited(generation, code);
            self.publish();
        } else if let Some(message) = stderr_message {
            self.state.failed(generation, message);
            self.publish();
        }
    }

    fn stop_process(&mut self) {
        if let Some(mut process) = self.process.take() {
            let _ = process.child.kill();
            let _ = process.child.wait();
        }
    }
}
