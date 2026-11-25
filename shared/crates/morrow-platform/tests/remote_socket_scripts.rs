//! The Python scripts run here the way `RemoteSocketFolderTests.swift` runs
//! them on the Mac: with this machine's python3, under a throwaway base in
//! /tmp. Pure parsing lives in core; this file only supplies the host.

#![cfg(unix)]

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use morrow_core::remote::host::quoted_literal;
use morrow_core::remote::socket::{
    RemoteSocketCheckFailure, prepare_remote_socket_folder_script, prepared_socket_path,
    python_command, remote_socket_folder, socket_problem, verify_remote_socket_script,
};

struct Host {
    base: PathBuf,
}

impl Host {
    fn new() -> Self {
        let nanos = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().subsec_nanos();
        let base = PathBuf::from(format!("/tmp/morrow-rs-{}-{nanos}", std::process::id()));
        fs::create_dir(&base).unwrap();
        Self { base }
    }

    fn str(&self) -> &str {
        self.base.to_str().unwrap()
    }

    fn folder(&self, uid: &str) -> PathBuf {
        PathBuf::from(remote_socket_folder(uid, self.str()))
    }

    fn run(&self, program: &str) -> (String, String, i32) {
        let output = Command::new("/bin/sh")
            .args(["-c", &python_command(program)])
            .output()
            .unwrap();
        (
            String::from_utf8_lossy(&output.stdout).into_owned(),
            String::from_utf8_lossy(&output.stderr).into_owned(),
            output.status.code().unwrap_or(-1),
        )
    }

    fn prepare(&self, shared: bool, fake_uid: Option<i64>) -> Result<String, RemoteSocketCheckFailure> {
        let mut program = String::new();
        if let Some(uid) = fake_uid {
            program.push_str(&format!("import os\nos.getuid = lambda: {uid}\n"));
        }
        program.push_str(&prepare_remote_socket_folder_script(shared, self.str()));
        let (out, err, code) = self.run(&program);
        prepared_socket_path(&out, &err, code, self.str())
    }

    fn verify(&self, path: &str, shared: bool, prelude: &str) -> Option<RemoteSocketCheckFailure> {
        let program = format!(
            "{prelude}{}",
            verify_remote_socket_script(path, shared, 0.5)
        );
        let (out, err, code) = self.run(&program);
        socket_problem(&out, &err, code, path)
    }

    fn leave_socket(&self, path: &Path) {
        let program = format!(
            "import os, socket\npath = {}\nos.makedirs(os.path.dirname(path), exist_ok=True)\ns = socket.socket(socket.AF_UNIX)\ns.bind(path)\n",
            quoted_literal(path.to_str().unwrap())
        );
        let (_, err, code) = self.run(&program);
        assert_eq!(code, 0, "{err}");
    }
}

impl Drop for Host {
    fn drop(&mut self) {
        for entry in fs::read_dir(&self.base).into_iter().flatten().flatten() {
            let _ = fs::set_permissions(entry.path(), fs::Permissions::from_mode(0o700));
        }
        let _ = fs::remove_dir_all(&self.base);
    }
}

fn mode(path: &std::path::Path) -> u32 {
    fs::metadata(path).unwrap().permissions().mode() & 0o7777
}

fn uid() -> String {
    // The script patches os.getuid for the fake-uid cases; this is the real
    // uid for naming the folder and for the ownership assertion.
    let output = Command::new("id").arg("-u").output().unwrap();
    String::from_utf8_lossy(&output.stdout).trim().to_owned()
}

#[test]
fn the_folder_is_made_the_ssh_users_alone() {
    let host = Host::new();
    let folder = host.folder(&uid());
    let path = host.prepare(false, None).unwrap();
    assert_eq!(path, format!("{}/island.sock", folder.display()));
    assert_eq!(mode(&folder), 0o700);
    assert_eq!(host.prepare(false, None).unwrap(), path);
}

#[test]
fn other_users_agents_may_only_pass_through() {
    let host = Host::new();
    let folder = host.folder(&uid());
    host.prepare(true, None).unwrap();
    assert_eq!(mode(&folder), 0o711);
    host.prepare(false, None).unwrap();
    assert_eq!(mode(&folder), 0o700);
}

#[test]
fn an_open_folder_is_narrowed_but_a_symlink_or_file_is_refused() {
    let host = Host::new();
    let folder = host.folder(&uid());
    fs::create_dir(&folder).unwrap();
    fs::set_permissions(&folder, fs::Permissions::from_mode(0o777)).unwrap();
    assert!(host.prepare(false, None).is_ok());
    assert_eq!(mode(&folder), 0o700);

    fs::remove_dir(&folder).unwrap();
    let elsewhere = host.base.join("elsewhere");
    fs::create_dir(&elsewhere).unwrap();
    std::os::unix::fs::symlink(&elsewhere, &folder).unwrap();
    assert!(matches!(
        host.prepare(false, None),
        Err(RemoteSocketCheckFailure::FolderUnsafe { .. })
    ));
    assert_eq!(mode(&elsewhere), 0o755);
    fs::remove_file(&folder).unwrap();
    fs::write(&folder, b"").unwrap();
    assert!(matches!(
        host.prepare(false, None),
        Err(RemoteSocketCheckFailure::FolderUnsafe { .. })
    ));
}

#[test]
fn a_base_others_may_move_folders_out_of_is_refused() {
    let host = Host::new();
    fs::set_permissions(&host.base, fs::Permissions::from_mode(0o777)).unwrap();
    let problem = host.prepare(false, None).unwrap_err();
    match problem {
        RemoteSocketCheckFailure::FolderUnsafe { detail, .. } => assert!(detail.contains("move"), "{detail}"),
        other => panic!("a base without the sticky bit was used: {other:?}"),
    }
    fs::set_permissions(&host.base, fs::Permissions::from_mode(0o1777)).unwrap();
    assert!(host.prepare(false, None).is_ok());
}

#[test]
fn only_a_socket_in_the_folder_is_vouched_for() {
    let host = Host::new();
    let path = host.prepare(true, None).unwrap();
    assert!(matches!(
        host.verify(&path, true, ""),
        Some(RemoteSocketCheckFailure::SocketUnverified { .. })
    ));

    host.leave_socket(&PathBuf::from(&path));
    assert_eq!(host.verify(&path, true, ""), None);

    let target = host.base.join("target.sock");
    host.leave_socket(&target);
    let target_path = target.to_str().unwrap();
    fs::set_permissions(&target, fs::Permissions::from_mode(0o600)).unwrap();
    let program = format!(
        "import os\nos.unlink({})\nos.symlink({}, {})\n",
        quoted_literal(&path),
        quoted_literal(target_path),
        quoted_literal(&path)
    );
    assert_eq!(host.run(&program).2, 0);
    assert_eq!(
        host.verify(&path, true, ""),
        Some(RemoteSocketCheckFailure::SocketUnverified {
            path,
            detail: "not a socket".into(),
        })
    );
    assert_eq!(mode(&target), 0o600);
}

#[test]
fn a_folder_opened_up_since_fails_the_check() {
    let host = Host::new();
    let path = host.prepare(false, None).unwrap();
    host.leave_socket(&PathBuf::from(&path));
    let folder = host.folder(&uid());
    fs::set_permissions(&folder, fs::Permissions::from_mode(0o755)).unwrap();
    match host.verify(&path, false, "") {
        Some(RemoteSocketCheckFailure::SocketUnverified { detail, .. }) => {
            assert!(detail.contains("mode 755"), "{detail}")
        }
        other => panic!("a folder others may read was vouched for: {other:?}"),
    }
}
