//! The OS-facing half of remote hosts: run `ssh`, hold the tunnel process and
//! report status. All decisions about what is safe live in `morrow_core::remote`.

pub mod install;
pub mod ssh;
pub mod tunnel;
