//! Conformance with `SSHForwarderTests.swift` and `RemoteHost.swift`.

use morrow_core::remote::host::{self, SshHost};

fn host() -> SshHost {
    SshHost {
        name: "test".into(),
        host: "192.168.1.10".into(),
        user: "alice".into(),
        ..SshHost::default()
    }
}

#[test]
fn cleanup_arguments_basic() {
    let args = host::cleanup_arguments(&host(), "/tmp/codeisland-501.sock");
    assert_eq!(
        args,
        [
            "-o", "BatchMode=yes", "-o", "ConnectTimeout=5", "--", "alice@192.168.1.10",
            "rm", "-f", "--", "/tmp/codeisland-501.sock"
        ]
    );
}

#[test]
fn cleanup_arguments_include_port_and_identity_before_the_target() {
    let mut target = host();
    target.host = "example.com".into();
    target.user = "bob".into();
    target.port = Some(2222);
    target.identity_file = "  /Users/bob/.ssh/id_rsa  ".into();
    let args = host::cleanup_arguments(&target, "/tmp/codeisland-1000.sock");
    assert_eq!(&args[args.len() - 6..], [
        "--", "bob@example.com", "rm", "-f", "--", "/tmp/codeisland-1000.sock"
    ]);
    let i = args.iter().position(|a| a == "-i").unwrap();
    assert_eq!(args[i + 1], "/Users/bob/.ssh/id_rsa");
    assert!(i < args.iter().position(|a| a == "--").unwrap());
}

#[test]
fn cleanup_quotes_the_remote_path_as_one_word() {
    let mut target = host();
    target.host = "10.0.0.1".into();
    target.user = "root".into();
    let path = "/tmp/it's $(touch x) `y`.sock";
    assert_eq!(
        host::cleanup_arguments(&target, path).last().map(String::as_str),
        Some("'/tmp/it'\\''s $(touch x) `y`.sock'")
    );
    assert_eq!(host::shell_word("/tmp/codeisland-501/island.sock"), "/tmp/codeisland-501/island.sock");
    assert_eq!(host::shell_word(""), "''");
}

#[test]
fn tunnel_arguments_forward_the_given_local_socket() {
    let mut target = host();
    target.host = "10.0.0.1".into();
    let local = "/tmp/codeisland-501-hosts/h-0123456789abcdef.sock";
    let args = host::tunnel_arguments(&target, local, "/tmp/codeisland-1000.sock");
    let i = args.iter().position(|a| a == "-R").unwrap();
    assert_eq!(args[i + 1], format!("/tmp/codeisland-1000.sock:{local}"));
    assert_eq!(args.last().map(String::as_str), Some("alice@10.0.0.1"));
    assert!(args.contains(&"ControlMaster=no".to_owned()));
    assert!(args.contains(&"ControlPath=none".to_owned()));
}

#[test]
fn tunnel_mask_follows_agents_run_as_other_users() {
    let mut target = host();
    target.host = "10.0.0.1".into();
    target.user.clear();
    let args = host::tunnel_arguments(&target, "/tmp/l.sock", "/tmp/r.sock");
    assert!(args.contains(&"StreamLocalBindMask=0177".to_owned()));
    assert!(!args.contains(&"StreamLocalBindMask=0000".to_owned()));

    target.agents_run_as_other_users = true;
    let args = host::tunnel_arguments(&target, "/tmp/l.sock", "/tmp/r.sock");
    assert!(args.contains(&"StreamLocalBindMask=0000".to_owned()));
    assert!(!args.contains(&"StreamLocalBindMask=0177".to_owned()));
}

#[test]
fn every_ssh_ends_its_options_before_the_host() {
    let hosts = [
        SshHost { host: "-oProxyCommand=touch /tmp/ci-pwned".into(), ..SshHost::default() },
        SshHost { host: "10.0.0.1".into(), user: "-oProxyCommand=x".into(), ..SshHost::default() },
        SshHost {
            host: "devbox".into(),
            user: "alice".into(),
            port: Some(2222),
            identity_file: "~/.ssh/id".into(),
            ..SshHost::default()
        },
    ];
    for target in &hosts {
        let runs = [
            host::ssh_arguments(target),
            host::cleanup_arguments(target, "/tmp/r.sock"),
            host::tunnel_arguments(target, "/tmp/l.sock", "/tmp/r.sock"),
        ];
        for args in runs {
            let at = args.iter().position(|a| a == &target.ssh_target()).unwrap();
            assert_eq!(args.get(at.wrapping_sub(1)).map(String::as_str), Some("--"));
            assert_eq!(args.iter().position(|a| a == "--"), Some(at - 1));
        }
    }
}

#[test]
fn fields_ssh_would_misread_are_found() {
    let unsafe_cases = [
        (SshHost { host: "-oProxyCommand=touch /tmp/x".into(), ..SshHost::default() }, "-oProxyCommand=touch /tmp/x"),
        (SshHost { host: " -p2222 devbox".into(), ..SshHost::default() }, "-p2222 devbox"),
        (SshHost { host: "dev box".into(), ..SshHost::default() }, "dev box"),
        (SshHost { host: "devbox\nProxyCommand x".into(), ..SshHost::default() }, "devbox\nProxyCommand x"),
        (SshHost { host: "devbox".into(), user: "-lroot".into(), ..SshHost::default() }, "-lroot"),
        (SshHost { host: "devbox".into(), user: "al\u{7}ice".into(), ..SshHost::default() }, "al\u{7}ice"),
        (SshHost { host: "devbox".into(), identity_file: "-oProxyCommand=x".into(), ..SshHost::default() }, "-oProxyCommand=x"),
        (SshHost { host: "devbox".into(), port: Some(0), ..SshHost::default() }, "0"),
        (SshHost { host: "devbox".into(), port: Some(65_536), ..SshHost::default() }, "65536"),
        (SshHost { host: "devbox".into(), port: Some(-22), ..SshHost::default() }, "-22"),
    ];
    for (target, field) in unsafe_cases {
        assert_eq!(target.unsafe_ssh_field().as_deref(), Some(field), "{field}");
    }
    let safe = [
        SshHost { host: "devbox".into(), ..SshHost::default() },
        SshHost { host: " 192.168.1.10 ".into(), user: "alice".into(), port: Some(22), ..SshHost::default() },
        SshHost { host: "fe80::1%en0".into(), user: "AD\\John Smith".into(), port: Some(65_535), ..SshHost::default() },
        SshHost { host: "build-01.example.com".into(), identity_file: "~/My Keys/id_ed25519".into(), ..SshHost::default() },
    ];
    for target in safe {
        assert_eq!(target.unsafe_ssh_field(), None, "{}", target.ssh_target());
    }
}

#[test]
fn python_and_shell_literals_keep_controls_from_the_shell() {
    assert_eq!(host::quoted_literal("a\\b\"c\nd\te"), "\"a\\\\b\\\"c\\nd\\te\"");
    assert_eq!(host::quoted_literal("\u{7}\u{2028}"), "\"\\u0007\\u2028\"");
}
