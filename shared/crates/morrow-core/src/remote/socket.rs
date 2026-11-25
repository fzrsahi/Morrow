//! The remote socket's security checks, ported from `RemoteInstaller.swift`.
//!
//! These scripts run on the host as the SSH user before and after the tunnel.
//! Parsing their answers is pure: the shell hands in stdout, stderr and the
//! exit status.

use base64::Engine;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::host::quoted_literal;

pub const REMOTE_SOCKET_BASE: &str = "/tmp";
const SOCKET_FOLDER_MARKER: &str = "CODEISLAND-SOCKET-FOLDER ";
const SOCKET_CHECK_MARKER: &str = "CODEISLAND-SOCKET-CHECK ";

/// What stops the island's socket on a remote host from being one only the SSH
/// user can reach.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum RemoteSocketCheckFailure {
    /// ssh itself did not run the check: unreachable host, refused login, no
    /// python3. The string is ssh's own words.
    Unreachable(String),
    /// The folder the socket goes in is not, and cannot be made, the SSH
    /// user's alone.
    FolderUnsafe { path: String, detail: String },
    /// The socket the tunnel made is not the SSH user's, in that folder, or
    /// could not be given the mode the host needs.
    SocketUnverified { path: String, detail: String },
}

impl RemoteSocketCheckFailure {
    /// The Settings → Remote message, with the path and the detail named.
    /// The format is the English string table entry for the failure.
    pub fn message_en(&self) -> String {
        match self {
            Self::Unreachable(detail) => detail.clone(),
            Self::FolderUnsafe { path, detail } => format!(
                "Morrow's folder on the host, {path}, can't be made the SSH user's alone ({detail}), so nothing is forwarded there."
            ),
            Self::SocketUnverified { path, detail } => format!(
                "Morrow's socket on the host, {path}, couldn't be verified ({detail}), so its tunnel was closed."
            ),
        }
    }

    pub fn path(&self) -> Option<&str> {
        match self {
            Self::Unreachable(_) => None,
            Self::FolderUnsafe { path, .. } | Self::SocketUnverified { path, .. } => Some(path),
        }
    }
}

impl std::fmt::Display for RemoteSocketCheckFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message_en())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RemoteCommandResult {
    pub stdout: String,
    pub stderr: String,
    pub exit_code: i32,
}

impl RemoteCommandResult {
    pub fn ok(&self) -> bool {
        self.exit_code == 0
    }
}

pub fn remote_socket_folder(uid: &str, base: &str) -> String {
    format!("{base}/codeisland-{uid}")
}

pub fn remote_socket_path(uid: &str, base: &str) -> String {
    format!("{}/island.sock", remote_socket_folder(uid, base))
}

fn is_uid(value: &str) -> bool {
    !value.is_empty() && value.bytes().all(|b| b.is_ascii_digit())
}

/// The SSH user's uid, read back from a socket path made here.
pub fn login_uid(path: &str) -> Option<String> {
    let prefix = format!("{REMOTE_SOCKET_BASE}/codeisland-");
    let suffix = "/island.sock";
    let uid = path.strip_prefix(&prefix)?.strip_suffix(suffix)?;
    if !uid.is_empty() && is_uid(uid) {
        Some(uid.to_owned())
    } else {
        None
    }
}

/// A command any login shell runs as the same python3: base64 carries the
/// script, and there is nothing in it for the shell to read.
pub fn python_command(script: &str) -> String {
    let encoded = base64::engine::general_purpose::STANDARD.encode(script.as_bytes());
    format!("python3 -c \"import base64;exec(base64.b64decode('{encoded}'))\"")
}

/// Run on the host before the tunnel: make the socket's folder the SSH
/// user's alone (0711 only where other users' agents must pass through), in a
/// base no other user can move things out of.
pub fn prepare_remote_socket_folder_script(agents_run_as_other_users: bool, base: &str) -> String {
    let script = r#"
import errno, json, os, stat, sys

base = __BASE__
shared = __SHARED__
uid = os.getuid()
folder = base + "/codeisland-%d" % uid
mode = 0o711 if shared else 0o700

def answer(problem=None):
    print(__FOLDER_MARKER__ + json.dumps({"uid": uid, "folder": folder, "problem": problem}))
    sys.exit(0)

try:
    parent = os.stat(base)
except OSError as e:
    answer(base + ": " + e.strerror)
if parent.st_uid not in (0, uid) or (parent.st_mode & 0o022 and not parent.st_mode & stat.S_ISVTX):
    answer(base + " lets other users move what is in it")
try:
    os.mkdir(folder, 0o700)
except OSError as e:
    if e.errno != errno.EEXIST:
        answer(e.strerror)
try:
    fd = os.open(folder, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW)
except OSError as e:
    if e.errno in (errno.ELOOP, errno.ENOTDIR):
        answer("not a folder (a symlink or a file)")
    answer(e.strerror)
info = os.fstat(fd)
if not stat.S_ISDIR(info.st_mode):
    answer("not a folder")
if info.st_uid != uid:
    answer("it belongs to uid %d" % info.st_uid)
if stat.S_IMODE(info.st_mode) != mode:
    try:
        os.fchmod(fd, mode)
    except OSError as e:
        answer("its mode cannot be set: " + e.strerror)
answer()
"#;
    script
        .replace("__BASE__", &quoted_literal(base))
        .replace("__SHARED__", if agents_run_as_other_users { "True" } else { "False" })
        .replace("__FOLDER_MARKER__", &quoted_literal(SOCKET_FOLDER_MARKER))
        .trim_start_matches('\n')
        .to_owned()
}

/// Run on the host once the tunnel is up: vouch for the socket in that folder
/// before anything is installed. A socket sshd has not made yet is waited for.
pub fn verify_remote_socket_script(
    socket_path: &str,
    agents_run_as_other_users: bool,
    wait_for_socket: f64,
) -> String {
    let script = r#"
import errno, json, os, stat, sys, time

path = __PATH__
shared = __SHARED__
wait_for_socket = __WAIT__
uid = os.getuid()
folder, name = os.path.split(path)
folder_mode = 0o711 if shared else 0o700

def answer(problem=None):
    print(__CHECK_MARKER__ + json.dumps({"ok": problem is None, "problem": problem}))
    sys.exit(0)

def socket_info(fd, wait=0):
    deadline = time.time() + wait
    while True:
        try:
            return os.stat(name, dir_fd=fd, follow_symlinks=False)
        except OSError as e:
            if e.errno != errno.ENOENT or time.time() >= deadline:
                answer("the socket: " + e.strerror)
        time.sleep(0.2)

def set_shared_mode(fd, checked):
    try:
        os.chmod(name, 0o666, dir_fd=fd, follow_symlinks=False)
        return
    except (NotImplementedError, ValueError):
        pass
    except OSError as e:
        if e.errno not in (errno.ENOTSUP, errno.EOPNOTSUPP, errno.EINVAL):
            answer("the socket's mode cannot be set: " + e.strerror)
    if hasattr(os, "O_PATH"):
        try:
            handle = os.open(name, os.O_PATH | os.O_NOFOLLOW, dir_fd=fd)
        except OSError as e:
            answer("the socket: " + e.strerror)
        try:
            now = os.fstat(handle)
            if (now.st_dev, now.st_ino) != (checked.st_dev, checked.st_ino):
                answer("the socket changed while it was checked")
            os.chmod("/proc/self/fd/%d" % handle, 0o666)
            return
        except OSError:
            pass
        finally:
            os.close(handle)
    now = socket_info(fd)
    if (now.st_dev, now.st_ino) != (checked.st_dev, checked.st_ino) or not stat.S_ISSOCK(now.st_mode):
        answer("the socket changed while it was checked")
    try:
        os.chmod(name, 0o666, dir_fd=fd)
    except OSError as e:
        answer("the socket's mode cannot be set: " + e.strerror)

try:
    fd = os.open(folder, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW)
except OSError as e:
    answer(folder + ": " + e.strerror)
info = os.fstat(fd)
if info.st_uid != uid:
    answer(folder + " belongs to uid %d" % info.st_uid)
if stat.S_IMODE(info.st_mode) != folder_mode:
    answer(folder + " has mode %o" % stat.S_IMODE(info.st_mode))
found = socket_info(fd, wait_for_socket)
if not stat.S_ISSOCK(found.st_mode):
    answer("not a socket")
if found.st_uid != uid:
    answer("the socket belongs to uid %d" % found.st_uid)
if shared and stat.S_IMODE(found.st_mode) != 0o666:
    set_shared_mode(fd, found)
answer()
"#;
    script
        .replace("__PATH__", &quoted_literal(socket_path))
        .replace("__SHARED__", if agents_run_as_other_users { "True" } else { "False" })
        .replace("__WAIT__", &wait_for_socket.max(0.0).to_string())
        .replace("__CHECK_MARKER__", &quoted_literal(SOCKET_CHECK_MARKER))
        .trim_start_matches('\n')
        .to_owned()
}

/// The JSON object on the last answer line of stdout. Anything a login shell
/// prints first is skipped.
fn marked_answer(output: &str, marker: &str) -> Option<Value> {
    let line = output.lines().rev().find(|line| line.starts_with(marker))?;
    let value: Value = serde_json::from_str(line[marker.len()..].trim()).ok()?;
    value.as_object()?;
    Some(value)
}

fn command_failure(error_output: &str, exit_code: i32) -> String {
    let trimmed = error_output.trim();
    if trimmed.is_empty() {
        format!("ssh exited ({exit_code})")
    } else {
        trimmed
            .chars()
            .rev()
            .take(300)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect()
    }
}

/// What the folder probe found, ported from `preparedSocketPath`.
pub fn prepared_socket_path(
    output: &str,
    error_output: &str,
    exit_code: i32,
    base: &str,
) -> Result<String, RemoteSocketCheckFailure> {
    let Some(answer) = marked_answer(output, SOCKET_FOLDER_MARKER) else {
        return Err(RemoteSocketCheckFailure::Unreachable(command_failure(
            error_output, exit_code,
        )));
    };
    let uid = answer.get("uid").and_then(Value::as_i64).unwrap_or(-1);
    let folder = answer.get("folder").and_then(Value::as_str).unwrap_or("");
    let expected = remote_socket_folder(&uid.to_string(), base);
    if uid < 0 || folder != expected {
        let tail: String = output.chars().rev().take(200).collect::<Vec<_>>().into_iter().rev().collect();
        return Err(RemoteSocketCheckFailure::Unreachable(format!(
            "Unexpected answer from the host: {tail}"
        )));
    }
    if let Some(problem) = answer.get("problem").and_then(Value::as_str) {
        return Err(RemoteSocketCheckFailure::FolderUnsafe {
            path: folder.to_owned(),
            detail: problem.to_owned(),
        });
    }
    Ok(remote_socket_path(&uid.to_string(), base))
}

/// What the socket check found: `None` means it is vouched for.
pub fn socket_problem(
    output: &str,
    error_output: &str,
    exit_code: i32,
    socket_path: &str,
) -> Option<RemoteSocketCheckFailure> {
    let Some(answer) = marked_answer(output, SOCKET_CHECK_MARKER) else {
        return Some(RemoteSocketCheckFailure::SocketUnverified {
            path: socket_path.to_owned(),
            detail: command_failure(error_output, exit_code),
        });
    };
    if answer.get("ok").and_then(Value::as_bool) == Some(true) {
        return None;
    }
    Some(RemoteSocketCheckFailure::SocketUnverified {
        path: socket_path.to_owned(),
        detail: answer
            .get("problem")
            .and_then(Value::as_str)
            .unwrap_or("unknown")
            .to_owned(),
    })
}

/// Whether a check's answer reached stdout. A dropped ssh connection is tried
/// once more before the socket counts as unverified.
pub fn has_socket_check_answer(output: &str) -> bool {
    marked_answer(output, SOCKET_CHECK_MARKER).is_some()
}
