//! When a host's tunnel comes back without the user clicking Connect
//! (`RemoteReconnectPolicy.swift`). The owner reports events and carries out
//! the steps; nothing here owns a timer, an ssh process or a clock.
//!
//! Times are seconds from a clock the caller hands in.

use std::collections::BTreeMap;

#[derive(Clone, Debug, PartialEq)]
pub struct Config {
    /// Wait before the nth retry; the last step repeats.
    pub backoff: Vec<f64>,
    pub fast_attempts: usize,
    pub slow_retry: f64,
    pub offline_recheck: f64,
    pub stable_after: f64,
    pub wake_settle: f64,
    pub network_settle: f64,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            backoff: vec![5.0, 15.0, 45.0, 120.0, 300.0],
            fast_attempts: 10,
            slow_retry: 900.0,
            offline_recheck: 300.0,
            stable_after: 120.0,
            wake_settle: 3.0,
            network_settle: 2.0,
        }
    }
}

impl Config {
    /// Wait before the nth (1-based) retry.
    pub fn delay(&self, attempt: i64) -> f64 {
        let first = self.backoff.first().copied().unwrap_or(0.0);
        if attempt < 1 {
            return first;
        }
        let index = ((attempt - 1) as usize).min(self.backoff.len().saturating_sub(1));
        self.backoff.get(index).copied().unwrap_or(first)
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum Phase {
    #[default]
    Idle,
    Connecting,
    Connected { since: f64 },
    RetryPending,
    WaitingForNetwork,
}

/// What the owner does next for one host.
#[derive(Clone, Debug, PartialEq)]
pub enum Step {
    None,
    Connect,
    Retry { delay: f64, attempt: usize },
    RetrySlowly { delay: f64, failures: usize },
    WaitForNetwork { recheck: f64 },
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct HostState {
    wanted: bool,
    phase: Phase,
    failures: usize,
}

#[derive(Clone, Debug, PartialEq)]
pub struct RemoteReconnectPolicy {
    pub config: Config,
    hosts: BTreeMap<String, HostState>,
    /// Unknown counts as online.
    network_satisfied: Option<bool>,
}

impl Default for RemoteReconnectPolicy {
    fn default() -> Self {
        Self::new(Config::default())
    }
}

impl RemoteReconnectPolicy {
    pub fn new(config: Config) -> Self {
        Self {
            config,
            hosts: BTreeMap::new(),
            network_satisfied: None,
        }
    }

    pub fn wants_connection(&self, id: &str) -> bool {
        self.hosts.get(id).is_some_and(|host| host.wanted)
    }

    pub fn phase(&self, id: &str) -> Phase {
        self.hosts.get(id).map(|h| h.phase).unwrap_or(Phase::Idle)
    }

    pub fn failures(&self, id: &str) -> usize {
        self.hosts.get(id).map_or(0, |h| h.failures)
    }

    pub fn network_satisfied(&self) -> Option<bool> {
        self.network_satisfied
    }

    fn is_online(&self) -> bool {
        self.network_satisfied.unwrap_or(true)
    }

    /// Connect in Settings, or on launch with auto-connect: wanted from now
    /// on, with a fresh backoff.
    pub fn user_connected(&mut self, id: &str) -> Step {
        self.hosts.insert(
            id.to_owned(),
            HostState {
                wanted: true,
                phase: Phase::Connecting,
                failures: 0,
            },
        );
        Step::Connect
    }

    pub fn user_disconnected(&mut self, id: &str) {
        self.hosts.remove(id);
    }

    pub fn forget(&mut self, id: &str) {
        self.hosts.remove(id);
    }

    /// A connect stopped before ssh started.
    pub fn connect_aborted(&mut self, id: &str) {
        if let Some(host) = self.hosts.get_mut(id) {
            host.phase = Phase::Idle;
        }
    }

    pub fn tunnel_connected(&mut self, id: &str, now: f64) {
        if let Some(host) = self.hosts.get_mut(id) {
            host.phase = Phase::Connected { since: now };
        }
    }

    /// A tunnel failed to come up, or dropped. Only auto-retry hosts get a
    /// timer: a typo entered by hand is not retried on a clock.
    pub fn tunnel_failed(&mut self, id: &str, auto_retry: bool, now: f64) -> Step {
        let online = self.is_online();
        let Some(host) = self.hosts.get_mut(id) else {
            return Step::None;
        };
        if !host.wanted {
            return Step::None;
        }
        if let Phase::Connected { since } = host.phase {
            if now - since >= self.config.stable_after {
                host.failures = 0;
            }
        }
        if !auto_retry {
            host.phase = Phase::Idle;
            return Step::None;
        }
        if !online {
            host.phase = Phase::WaitingForNetwork;
            return Step::WaitForNetwork {
                recheck: self.config.offline_recheck,
            };
        }
        host.failures += 1;
        host.phase = Phase::RetryPending;
        if host.failures <= self.config.fast_attempts {
            Step::Retry {
                delay: self.config.delay(host.failures as i64),
                attempt: host.failures,
            }
        } else {
            Step::RetrySlowly {
                delay: self.config.slow_retry,
                failures: host.failures,
            }
        }
    }

    /// A retry or offline re-check timer fired.
    pub fn retry_due(&mut self, id: &str) -> Step {
        let online = self.is_online();
        let Some(host) = self.hosts.get_mut(id) else {
            return Step::None;
        };
        if !host.wanted {
            return Step::None;
        }
        match host.phase {
            Phase::RetryPending if !online => {
                host.phase = Phase::WaitingForNetwork;
                Step::WaitForNetwork {
                    recheck: self.config.offline_recheck,
                }
            }
            Phase::RetryPending | Phase::WaitingForNetwork => {
                host.phase = Phase::Connecting;
                Step::Connect
            }
            Phase::Idle | Phase::Connecting | Phase::Connected { .. } => Step::None,
        }
    }

    /// Wake: every wanted host starts a fresh backoff when the owner calls
    /// `recover()` after the network has settled.
    pub fn system_woke(&mut self) {
        for host in self.hosts.values_mut() {
            if host.wanted {
                host.failures = 0;
            }
        }
    }

    /// The first report only says where things stand; only a return from
    /// offline is due a recovery.
    pub fn network_changed(&mut self, satisfied: bool) -> bool {
        let previous = self.network_satisfied;
        self.network_satisfied = Some(satisfied);
        previous.is_some_and(|was_online| satisfied && !was_online)
    }

    /// Bring back every wanted host that is neither up nor connecting.
    pub fn recover(&mut self) -> BTreeMap<String, Step> {
        let mut steps = BTreeMap::new();
        let online = self.is_online();
        for (id, host) in self.hosts.iter_mut() {
            if !host.wanted {
                continue;
            }
            host.failures = 0;
            match host.phase {
                Phase::Connecting | Phase::Connected { .. } => {}
                Phase::Idle | Phase::RetryPending | Phase::WaitingForNetwork => {
                    if online {
                        host.phase = Phase::Connecting;
                        steps.insert(id.clone(), Step::Connect);
                    } else {
                        host.phase = Phase::WaitingForNetwork;
                        steps.insert(
                            id.clone(),
                            Step::WaitForNetwork {
                                recheck: self.config.offline_recheck,
                            },
                        );
                    }
                }
            }
        }
        steps
    }
}
