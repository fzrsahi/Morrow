//! One SSH host and the argument vectors built from it (`RemoteHost.swift`,
//! `SSHForwarder.swift`). Every vector ends its options with `--` before the
//! host, so a host that looks like an option cannot become one.

use serde::{Deserialize, Serialize};

/// A host from Settings → Remote. `port` stays a signed integer here so an
/// out-of-range port can be reported as the bad field instead of silently
/// clamping when a saved host is decoded.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SshHost {
    pub id: String,
    pub name: String,
    pub host: String,
    pub user: String,
    pub port: Option<i64>,
    pub identity_file: String,
    pub auto_connect: bool,
    pub auth_socket: String,
    pub cwd_filter: String,
    pub agents_run_as_other_users: bool,
}

impl SshHost {
    /// The first field ssh would read as something it is not, or `None`.
    pub fn unsafe_ssh_field(&self) -> Option<String> {
        if let Some(port) = self.port {
            if !(1..=65_535).contains(&port) {
                return Some(port.to_string());
            }
        }
        let fields: [(&str, bool); 3] = [
            (&self.host, false),
            (&self.user, true),
            (&self.identity_file, true),
        ];
        for (value, may_hold_spaces) in fields {
            let trimmed = value.trim();
            let unsafe_field = trimmed.starts_with('-')
                || trimmed.chars().any(|c| {
                    c.is_control() || (!may_hold_spaces && c.is_whitespace())
                });
            if unsafe_field {
                return Some(trimmed.to_owned());
            }
        }
        None
    }

    pub fn ssh_target(&self) -> String {
        let user = self.user.trim();
        let host = self.host.trim();
        if user.is_empty() {
            host.to_owned()
        } else {
            format!("{user}@{host}")
        }
    }

    /// The old shared path, still the default in install scripts when no
    /// prepared path is handed in.
    pub fn remote_socket_path(&self) -> &'static str {
        "/tmp/codeisland.sock"
    }

    /// The uid of the SSH user, for a host whose agents run as other users.
    /// Empty when the path is not one this app prepared.
    pub fn socket_owner_uid(&self, remote_socket_path: &str) -> String {
        if !self.agents_run_as_other_users {
            return String::new();
        }
        super::socket::login_uid(remote_socket_path).unwrap_or_default()
    }

    fn common_options(out: &mut Vec<String>) {
        out.push("-o".into());
        out.push("BatchMode=yes".into());
    }

    fn host_options(&self, out: &mut Vec<String>) {
        if let Some(port) = self.port {
            out.push("-p".into());
            out.push(port.to_string());
        }
        let identity = self.identity_file.trim();
        if !identity.is_empty() {
            out.push("-i".into());
            out.push(identity.to_owned());
        }
    }
}

/// Arguments for one short-lived command over ssh, up to and including the
/// host. The remote command is appended by the caller.
pub fn ssh_arguments(host: &SshHost) -> Vec<String> {
    let mut args = Vec::new();
    SshHost::common_options(&mut args);
    args.extend([
        "-o".into(),
        "ConnectTimeout=8".into(),
        "-o".into(),
        "ServerAliveInterval=15".into(),
        "-o".into(),
        "ServerAliveCountMax=2".into(),
    ]);
    host.host_options(&mut args);
    args.push("--".into());
    args.push(host.ssh_target());
    args
}

/// Remove a stale remote socket left by a dropped tunnel (#206).
pub fn cleanup_arguments(host: &SshHost, remote_socket_path: &str) -> Vec<String> {
    let mut args = Vec::new();
    SshHost::common_options(&mut args);
    args.extend(["-o".into(), "ConnectTimeout=5".into()]);
    host.host_options(&mut args);
    args.push("--".into());
    args.push(host.ssh_target());
    args.extend([
        "rm".into(),
        "-f".into(),
        "--".into(),
        shell_word(remote_socket_path),
    ]);
    args
}

/// The long-lived `-R` forward. A dedicated connection is forced: a shared
/// ControlMaster would take the forward and let `ssh -N` exit at once (#190).
pub fn tunnel_arguments(host: &SshHost, local_socket_path: &str, remote_socket_path: &str) -> Vec<String> {
    let mut args = vec![
        "-N".into(),
        "-T".into(),
        "-o".into(),
        "BatchMode=yes".into(),
        "-o".into(),
        "ExitOnForwardFailure=yes".into(),
        "-o".into(),
        "ServerAliveInterval=15".into(),
        "-o".into(),
        "ServerAliveCountMax=2".into(),
        "-o".into(),
        "StreamLocalBindUnlink=yes".into(),
        "-o".into(),
        format!(
            "StreamLocalBindMask={}",
            if host.agents_run_as_other_users { "0000" } else { "0177" }
        ),
        "-o".into(),
        "ControlMaster=no".into(),
        "-o".into(),
        "ControlPath=none".into(),
    ];
    host.host_options(&mut args);
    args.push("-R".into());
    args.push(format!("{remote_socket_path}:{local_socket_path}"));
    args.push("--".into());
    args.push(host.ssh_target());
    args
}

/// `value` as a double-quoted Python or JavaScript literal. Control and line
/// separator characters are escaped, so a host name cannot end the literal or
/// its line.
pub fn quoted_literal(value: &str) -> String {
    let mut out = String::with_capacity(value.len() + 2);
    out.push('"');
    for c in value.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c
                if c.is_control()
                    || matches!(c, '\u{2028}' | '\u{2029}') =>
            {
                out.push_str(&format!("\\u{:04x}", c as u32));
            }
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// `value` as one word for the remote shell. Anything the shell reads is put
/// in single quotes, with each embedded quote spelled `'\''`.
pub fn shell_word(value: &str) -> String {
    let plain = |c: char| c.is_ascii_alphanumeric() || "@%+=:,./_-".contains(c);
    if !value.is_empty() && value.chars().all(plain) {
        return value.to_owned();
    }
    format!("'{}'", value.replace('\'', "'\\''"))
}
