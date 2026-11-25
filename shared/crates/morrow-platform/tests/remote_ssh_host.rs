//! An optional real-host check for issue #233. Run it with
//! `scripts/ci/remote-ssh-host-test.sh`; it stays ignored in normal CI.

use std::env;

use morrow_core::remote::host::SshHost;
use morrow_platform::remote::install;

fn env_host() -> SshHost {
    let host = env::var("MORROW_TEST_SSH_HOST").expect("set MORROW_TEST_SSH_HOST to an sshd host");
    let user = env::var("MORROW_TEST_SSH_USER").unwrap_or_else(|_| "test".into());
    let port = env::var("MORROW_TEST_SSH_PORT")
        .ok()
        .map(|value| value.parse::<i64>().expect("a numeric port"));
    let identity = env::var("MORROW_TEST_SSH_KEY").unwrap_or_default();
    SshHost {
        host,
        user,
        port,
        identity_file: identity,
        ..SshHost::default()
    }
}

#[test]
#[ignore = "needs a host running sshd; see scripts/ci/remote-ssh-host-test.sh"]
fn a_real_ssh_host_prepares_a_safe_folder_and_reads_its_configs() {
    let host = env_host();
    let socket_path = install::prepare_socket("ssh", &host).expect("the folder check must run on the host");
    assert!(socket_path.starts_with("/tmp/codeisland-"), "{socket_path}");
    assert!(socket_path.ends_with("/island.sock"), "{socket_path}");

    let files = install::read_config_files("ssh", &host).expect("the config snapshot must run on the host");
    for path in files.keys() {
        assert!(
            morrow_core::remote::install::config_paths().contains(&path.as_str()),
            "an unplanned file was read: {path}"
        );
    }
}
