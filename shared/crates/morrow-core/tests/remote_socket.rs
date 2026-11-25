//! Conformance with the answer-parsing fixtures in
//! `RemoteSocketFolderTests.swift`.

use morrow_core::remote::socket::{
    RemoteSocketCheckFailure, login_uid, prepared_socket_path, remote_socket_path, socket_problem,
};

#[test]
fn the_folder_answer_is_read_from_its_own_line() {
    let line = r#"CODEISLAND-SOCKET-FOLDER {"uid": 1000, "folder": "/tmp/codeisland-1000", "problem": null}"#;
    assert_eq!(
        prepared_socket_path(&format!("Welcome to devbox\n{line}\n"), "", 0, "/tmp"),
        Ok("/tmp/codeisland-1000/island.sock".into())
    );
}

#[test]
fn an_answer_that_names_another_folder_is_not_taken() {
    let elsewhere = r#"CODEISLAND-SOCKET-FOLDER {"uid": 1000, "folder": "/home/x", "problem": null}"#;
    assert!(matches!(
        prepared_socket_path(elsewhere, "", 0, "/tmp"),
        Err(RemoteSocketCheckFailure::Unreachable(_))
    ));
}

#[test]
fn ssh_failure_is_reported_in_sshs_own_words() {
    assert_eq!(
        prepared_socket_path("", "ssh: connect to host devbox port 22: Connection refused\n", 255, "/tmp"),
        Err(RemoteSocketCheckFailure::Unreachable(
            "ssh: connect to host devbox port 22: Connection refused".into()
        ))
    );
    assert_eq!(
        prepared_socket_path("", "", 255, "/tmp"),
        Err(RemoteSocketCheckFailure::Unreachable("ssh exited (255)".into()))
    );
}

#[test]
fn an_unsafe_folder_is_named_with_its_detail() {
    let answer = r#"CODEISLAND-SOCKET-FOLDER {"uid": 1000, "folder": "/tmp/codeisland-1000", "problem": "it belongs to uid 0"}"#;
    let problem = prepared_socket_path(answer, "", 0, "/tmp").unwrap_err();
    assert_eq!(
        problem,
        RemoteSocketCheckFailure::FolderUnsafe {
            path: "/tmp/codeisland-1000".into(),
            detail: "it belongs to uid 0".into(),
        }
    );
    let message = problem.message_en();
    assert!(message.contains("/tmp/codeisland-1000"));
    assert!(message.contains("it belongs to uid 0"));
}

#[test]
fn socket_check_maps_ok_problem_and_no_answer() {
    let ok = r#"CODEISLAND-SOCKET-CHECK {"ok": true, "problem": null}"#;
    assert_eq!(socket_problem(ok, "", 0, "/tmp/codeisland-1000/island.sock"), None);

    let refused = r#"CODEISLAND-SOCKET-CHECK {"ok": false, "problem": "not a socket"}"#;
    assert_eq!(
        socket_problem(refused, "", 0, "/tmp/codeisland-1000/island.sock"),
        Some(RemoteSocketCheckFailure::SocketUnverified {
            path: "/tmp/codeisland-1000/island.sock".into(),
            detail: "not a socket".into(),
        })
    );

    assert_eq!(
        socket_problem("", "Connection closed\n", 255, "/tmp/r.sock"),
        Some(RemoteSocketCheckFailure::SocketUnverified {
            path: "/tmp/r.sock".into(),
            detail: "Connection closed".into(),
        })
    );
}

#[test]
fn the_ssh_users_uid_is_read_back_from_the_socket_path() {
    assert_eq!(remote_socket_path("1000", "/tmp"), "/tmp/codeisland-1000/island.sock");
    assert_eq!(login_uid("/tmp/codeisland-1000/island.sock").as_deref(), Some("1000"));
    for path in [
        "/tmp/codeisland.sock",
        "/tmp/codeisland-1000.sock",
        "/tmp/codeisland-/island.sock",
        "/tmp/codeisland-10a0/island.sock",
        "/var/codeisland-1000/island.sock",
    ] {
        assert_eq!(login_uid(path), None, "{path}");
    }
}
