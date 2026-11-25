//! The pure half of remote hosts (`RemoteInstaller.swift`, `SSHForwarder.swift`
//! and `RemoteReconnectPolicy.swift`).
//!
//! Everything here is a value or a string: the shell runs the scripts and the
//! `ssh` binary, and hands the bytes back. No process, socket or clock is read
//! in this module.

pub mod host;
pub mod install;
pub mod reconnect;
pub mod socket;
pub mod tunnel;
