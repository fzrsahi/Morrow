//! The generation rule from `SSHForwarderTests.swift`: a late callback from
//! an old connect must never become the new host's status.

use morrow_core::remote::socket::RemoteSocketCheckFailure;
use morrow_core::remote::tunnel::{TunnelState, TunnelStatus};

#[test]
fn a_connection_reports_connecting_then_connected() {
    let mut state = TunnelState::default();
    let generation = state.begin_connect(false);
    assert_eq!(state.status(), &TunnelStatus::Connecting);
    state.connected(generation);
    assert_eq!(state.status(), &TunnelStatus::Connected);
}

#[test]
fn an_invalid_host_fails_without_a_process() {
    let mut state = TunnelState::default();
    let generation = state.begin_connect(true);
    state.connected(generation);
    assert_eq!(state.status(), &TunnelStatus::Failed("invalid host".into()));
}

#[test]
fn a_disconnect_during_a_probe_stops_the_old_connect() {
    let mut state = TunnelState::default();
    let old = state.begin_connect(false);
    state.disconnect();
    state.connected(old);
    state.failed(old, "ssh exited (1)");
    assert_eq!(state.status(), &TunnelStatus::Disconnected);
}

#[test]
fn only_the_newer_of_two_overlapping_connects_sets_status() {
    let mut state = TunnelState::default();
    let old = state.begin_connect(false);
    let new = state.begin_connect(false);
    state.failed(old, "old ssh exited (1)");
    assert_eq!(state.status(), &TunnelStatus::Connecting);
    state.connected(new);
    assert_eq!(state.status(), &TunnelStatus::Connected);
}

#[test]
fn an_exit_after_a_deliberate_disconnect_stays_quiet() {
    let mut state = TunnelState::default();
    let generation = state.begin_connect(false);
    state.disconnect();
    state.process_exited(generation, 0);
    assert_eq!(state.status(), &TunnelStatus::Disconnected);
}

#[test]
fn a_refused_socket_names_the_path_and_its_detail() {
    let mut state = TunnelState::default();
    let generation = state.begin_connect(false);
    let problem = RemoteSocketCheckFailure::SocketUnverified {
        path: "/tmp/codeisland-1000/island.sock".into(),
        detail: "not a socket".into(),
    };
    state.socket_refused(generation, &problem);
    assert_eq!(
        state.status(),
        &TunnelStatus::Failed(problem.message_en())
    );
}
