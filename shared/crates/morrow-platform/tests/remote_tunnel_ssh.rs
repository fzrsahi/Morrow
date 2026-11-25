//! Generation and refusal behaviour with a stand-in for the system `ssh`.

#![cfg(unix)]

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use morrow_core::remote::host::SshHost;
use morrow_core::remote::tunnel::TunnelStatus;
use morrow_platform::remote::tunnel::RemoteTunnel;

fn fake_ssh(body: &str) -> (PathBuf, PathBuf) {
    let nanos = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().subsec_nanos();
    let dir = PathBuf::from(format!("/tmp/morrow-ssh-{}-{nanos}", std::process::id()));
    fs::create_dir(&dir).unwrap();
    let binary = dir.join("ssh");
    fs::write(&binary, format!("#!/bin/sh\n{body}\n")).unwrap();
    fs::set_permissions(&binary, fs::Permissions::from_mode(0o755)).unwrap();
    (binary, dir)
}

fn uid() -> String {
    let output = Command::new("id").arg("-u").output().unwrap();
    String::from_utf8_lossy(&output.stdout).trim().to_owned()
}

fn host() -> SshHost {
    SshHost {
        id: "h-01".into(),
        name: "devbox".into(),
        host: "10.0.0.1".into(),
        user: "alice".into(),
        ..SshHost::default()
    }
}

const GOOD: &str = r#"
for arg in "$@"; do
    if [ "$arg" = "-N" ]; then exec sleep 30; fi
done
cat <<'EOF'
CODEISLAND-SOCKET-FOLDER {"uid": UID, "folder": "/tmp/codeisland-UID", "problem": null}
CODEISLAND-SOCKET-CHECK {"ok": true, "problem": null}
CODEISLAND-CONFIG {}
EOF
exit 0
"#;

#[test]
fn an_invalid_host_starts_no_ssh() {
    let (binary, dir) = fake_ssh("touch \"$(dirname \"$0\")/ran\"\nexit 0\n");
    let mut target = host();
    target.host = "-oProxyCommand=touch /tmp/ci-pwned".into();
    let mut tunnel = RemoteTunnel::with_binary(binary.to_str().unwrap());
    tunnel.connect(&target, "/tmp/l.sock");
    assert_eq!(tunnel.status(), &TunnelStatus::Failed("invalid host".into()));
    assert!(!dir.join("ran").exists());
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn an_unsafe_folder_stops_before_the_tunnel_and_the_install() {
    let uid = uid();
    let body = format!(
        "cat <<'EOF'\nCODEISLAND-SOCKET-FOLDER {{\"uid\": {uid}, \"folder\": \"/tmp/codeisland-{uid}\", \"problem\": \"it belongs to uid 0\"}}\nEOF\n"
    );
    let (binary, dir) = fake_ssh(&body);
    let mut tunnel = RemoteTunnel::with_binary(binary.to_str().unwrap());
    tunnel.connect(&host(), "/tmp/l.sock");
    let status = tunnel.status().clone();
    let TunnelStatus::Failed(message) = status else {
        panic!("an unsafe folder connected: {status:?}");
    };
    assert!(message.contains(&format!("/tmp/codeisland-{uid}")), "{message}");
    assert!(message.contains("it belongs to uid 0"), "{message}");
    assert_eq!(
        tunnel.take_status_change(),
        Some(TunnelStatus::Failed(message.clone()))
    );
    tunnel.poll();
    assert_eq!(tunnel.status(), &TunnelStatus::Failed(message.clone()));
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn a_good_host_connects_and_disconnect_publishes_the_new_generation() {
    let body = GOOD.replace("UID", &uid());
    let (binary, dir) = fake_ssh(&body);
    let mut tunnel = RemoteTunnel::with_binary(binary.to_str().unwrap());
    tunnel.connect(&host(), "/tmp/l.sock");
    assert_eq!(tunnel.status(), &TunnelStatus::Connected);
    tunnel.poll();
    assert_eq!(tunnel.status(), &TunnelStatus::Connected);
    assert_eq!(tunnel.take_status_change(), Some(TunnelStatus::Connected));

    tunnel.disconnect();
    assert_eq!(tunnel.status(), &TunnelStatus::Disconnected);
    assert_eq!(tunnel.take_status_change(), Some(TunnelStatus::Disconnected));
    tunnel.poll();
    assert_eq!(tunnel.take_status_change(), None);
    fs::remove_dir_all(dir).unwrap();
}
