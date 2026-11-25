//! Conformance with `RemoteReconnectPolicyTests.swift`.

use std::collections::BTreeMap;

use morrow_core::remote::reconnect::{Config, RemoteReconnectPolicy as Policy, Step};

fn failing(failures: usize, online: Option<bool>) -> Policy {
    let mut policy = Policy::default();
    if let Some(online) = online {
        policy.network_changed(online);
    }
    policy.user_connected("a");
    for n in 1..=failures {
        policy.tunnel_failed("a", true, n as f64);
        if n < failures {
            assert_eq!(policy.retry_due("a"), Step::Connect);
        }
    }
    policy
}

#[test]
fn backoff_follows_the_table_and_clamps() {
    let config = Config::default();
    for (attempt, delay) in [(1, 5.0), (2, 15.0), (3, 45.0), (4, 120.0), (5, 300.0), (6, 300.0), (100, 300.0)] {
        assert_eq!(config.delay(attempt), delay);
    }
    assert_eq!(config.delay(0), 5.0);
    assert_eq!(config.delay(-1), 5.0);
}

#[test]
fn retries_slow_down_instead_of_giving_up() {
    let mut policy = Policy::default();
    policy.user_connected("a");
    let mut steps = Vec::new();
    for n in 1..=12 {
        steps.push(policy.tunnel_failed("a", true, n as f64));
        assert_eq!(policy.phase("a"), morrow_core::remote::reconnect::Phase::RetryPending);
        assert_eq!(policy.retry_due("a"), Step::Connect);
    }
    assert_eq!(steps[0], Step::Retry { delay: 5.0, attempt: 1 });
    assert_eq!(steps[4], Step::Retry { delay: 300.0, attempt: 5 });
    assert_eq!(steps[9], Step::Retry { delay: 300.0, attempt: 10 });
    assert_eq!(steps[10], Step::RetrySlowly { delay: 900.0, failures: 11 });
    assert_eq!(steps[11], Step::RetrySlowly { delay: 900.0, failures: 12 });
}

#[test]
fn quick_drops_keep_counting_and_a_stable_tunnel_starts_over() {
    let mut policy = Policy::default();
    policy.user_connected("a");
    policy.tunnel_connected("a", 0.0);
    assert_eq!(policy.tunnel_failed("a", true, 1.0), Step::Retry { delay: 5.0, attempt: 1 });
    assert_eq!(policy.retry_due("a"), Step::Connect);
    policy.tunnel_connected("a", 10.0);
    assert_eq!(policy.tunnel_failed("a", true, 85.0), Step::Retry { delay: 15.0, attempt: 2 });
    policy.tunnel_connected("a", 100.0);
    assert_eq!(policy.tunnel_failed("a", true, 220.0), Step::Retry { delay: 5.0, attempt: 1 });
}

#[test]
fn wake_after_the_retries_ran_out_reconnects() {
    let mut policy = failing(10, Some(true));
    assert_eq!(policy.retry_due("a"), Step::Connect);
    assert_eq!(policy.tunnel_failed("a", true, 11.0), Step::RetrySlowly { delay: 900.0, failures: 11 });
    policy.system_woke();
    assert_eq!(policy.recover(), BTreeMap::from([("a".into(), Step::Connect)]));
    assert_eq!(policy.tunnel_failed("a", true, 20.0), Step::Retry { delay: 5.0, attempt: 1 });
}

#[test]
fn wake_while_a_retry_is_pending_connects_now() {
    let mut policy = failing(5, Some(true));
    assert_eq!(policy.failures("a"), 5);
    policy.system_woke();
    assert_eq!(policy.failures("a"), 0);
    assert_eq!(policy.recover(), BTreeMap::from([("a".into(), Step::Connect)]));
    assert_eq!(policy.retry_due("a"), Step::None);
}

#[test]
fn offline_at_wake_waits_for_the_network() {
    let mut policy = failing(3, Some(true));
    assert!(!policy.network_changed(false));
    policy.system_woke();
    assert_eq!(
        policy.recover(),
        BTreeMap::from([("a".into(), Step::WaitForNetwork { recheck: 300.0 })])
    );
    assert!(policy.network_changed(true));
    assert_eq!(policy.recover(), BTreeMap::from([("a".into(), Step::Connect)]));
    assert_eq!(policy.tunnel_failed("a", true, 40.0), Step::Retry { delay: 5.0, attempt: 1 });
}

#[test]
fn a_connected_host_is_left_alone_at_wake() {
    let mut policy = Policy::default();
    policy.user_connected("a");
    policy.tunnel_connected("a", 1800000000.0);
    policy.system_woke();
    assert!(policy.recover().is_empty());
    assert_eq!(policy.tunnel_failed("a", true, 1800000000.0 + 8.0 * 3600.0), Step::Retry { delay: 5.0, attempt: 1 });
}

#[test]
fn a_manual_host_comes_back_on_wake_but_not_on_a_timer() {
    let mut policy = Policy::default();
    policy.user_connected("m");
    policy.tunnel_connected("m", 0.0);
    assert_eq!(policy.tunnel_failed("m", false, 600.0), Step::None);
    assert!(policy.wants_connection("m"));
    policy.system_woke();
    assert_eq!(policy.recover(), BTreeMap::from([("m".into(), Step::Connect)]));
    assert_eq!(policy.tunnel_failed("m", false, 700.0), Step::None);
}

#[test]
fn user_disconnect_stays_down_and_removed_hosts_are_forgotten() {
    let mut policy = failing(2, Some(true));
    policy.user_disconnected("a");
    assert!(!policy.wants_connection("a"));
    policy.system_woke();
    assert!(policy.recover().is_empty());
    assert_eq!(policy.tunnel_failed("a", true, 1.0), Step::None);

    let mut policy = failing(2, Some(true));
    policy.forget("a");
    assert_eq!(policy.retry_due("a"), Step::None);
    assert!(policy.recover().is_empty());
}

#[test]
fn connecting_host_is_not_started_twice() {
    let mut policy = Policy::default();
    policy.network_changed(true);
    policy.user_connected("a");
    policy.user_connected("b");
    policy.tunnel_failed("b", true, 0.0);
    policy.system_woke();
    assert_eq!(policy.recover(), BTreeMap::from([("b".into(), Step::Connect)]));
    policy.network_changed(false);
    policy.network_changed(true);
    policy.system_woke();
    assert!(policy.recover().is_empty());
    assert_eq!(policy.retry_due("a"), Step::None);
    assert_eq!(policy.retry_due("b"), Step::None);
}

#[test]
fn only_the_network_coming_back_triggers_recovery() {
    let mut policy = Policy::default();
    assert_eq!(policy.network_satisfied(), None);
    assert!(!policy.network_changed(true));
    assert!(!policy.network_changed(true));
    assert!(!policy.network_changed(false));
    assert!(policy.network_changed(true));
}

#[test]
fn failures_while_offline_do_not_use_up_attempts() {
    let mut policy = failing(2, Some(true));
    assert!(!policy.network_changed(false));
    assert_eq!(policy.retry_due("a"), Step::WaitForNetwork { recheck: 300.0 });
    for n in 1..=3 {
        assert_eq!(policy.retry_due("a"), Step::Connect);
        assert_eq!(
            policy.tunnel_failed("a", true, (100 * n) as f64),
            Step::WaitForNetwork { recheck: 300.0 }
        );
    }
    assert_eq!(policy.failures("a"), 2);
    assert!(policy.network_changed(true));
    assert_eq!(policy.recover(), BTreeMap::from([("a".into(), Step::Connect)]));
    assert_eq!(policy.tunnel_failed("a", true, 500.0), Step::Retry { delay: 5.0, attempt: 1 });
}

#[test]
fn unknown_network_counts_as_online() {
    let policy = failing(1, None);
    assert_eq!(policy.phase("a"), morrow_core::remote::reconnect::Phase::RetryPending);
}
