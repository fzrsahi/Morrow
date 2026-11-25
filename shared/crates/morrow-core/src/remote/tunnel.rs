//! The generation rule behind `SSHForwarder.swift` (`TunnelStatus` and its
//! callbacks). The platform owns the ssh process; this contains the decision
//! about which status may be published.

use super::socket::RemoteSocketCheckFailure;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TunnelStatus {
    Disconnected,
    Connecting,
    Connected,
    Failed(String),
}

/// A generation moves on every connect and disconnect. An ssh exit, a stderr
/// line or a socket check from an older attempt is ignored, so it cannot set
/// the status of a newer one.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TunnelState {
    generation: u64,
    status: TunnelStatus,
}

impl Default for TunnelState {
    fn default() -> Self {
        Self {
            generation: 0,
            status: TunnelStatus::Disconnected,
        }
    }
}

impl TunnelState {
    pub fn generation(&self) -> u64 {
        self.generation
    }

    pub fn status(&self) -> &TunnelStatus {
        &self.status
    }

    /// Start a connect. An invalid host fails without ever starting ssh, but
    /// still consumes a generation so a late callback from the old attempt is
    /// discarded.
    pub fn begin_connect(&mut self, invalid_host: bool) -> u64 {
        self.generation = self.generation.wrapping_add(1);
        self.status = if invalid_host {
            TunnelStatus::Failed("invalid host".into())
        } else {
            TunnelStatus::Connecting
        };
        self.generation
    }

    /// Disconnect. Bumping the generation here is what makes an in-flight
    /// cleanup or probe harmless.
    pub fn disconnect(&mut self) -> u64 {
        self.generation = self.generation.wrapping_add(1);
        self.status = TunnelStatus::Disconnected;
        self.generation
    }

    pub fn connected(&mut self, generation: u64) {
        if generation == self.generation && self.status == TunnelStatus::Connecting {
            self.status = TunnelStatus::Connected;
        }
    }

    pub fn failed(&mut self, generation: u64, message: impl Into<String>) {
        if generation == self.generation && self.status != TunnelStatus::Disconnected {
            self.status = TunnelStatus::Failed(message.into());
        }
    }

    pub fn socket_refused(&mut self, generation: u64, problem: &RemoteSocketCheckFailure) {
        self.failed(generation, problem.message_en());
    }

    /// ssh exited. A deliberate disconnect already set `Disconnected`, so the
    /// termination callback from that kill stays quiet.
    pub fn process_exited(&mut self, generation: u64, exit_code: i32) {
        self.failed(generation, format!("ssh exited ({exit_code})"));
    }
}
