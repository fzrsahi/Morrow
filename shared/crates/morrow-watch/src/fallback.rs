//! The fallback: poll the roots' own modification times.
//!
//! This is the only arm this port can actually run, and it is deliberately not a
//! stub — it reports the same answer as the platform arms, at the timer's
//! granularity. A directory's mtime moves when an entry is added or removed, so a
//! new session file is noticed; an append **inside** an existing file is not, which
//! is why the shell still ticks on its own clock.

use std::path::PathBuf;
use std::time::{Duration, SystemTime};

pub struct Platform {
    roots: Vec<PathBuf>,
    seen: Vec<Option<SystemTime>>,
}

impl Platform {
    pub fn new(roots: &[PathBuf]) -> std::io::Result<Self> {
        let seen = roots.iter().map(|root| stamp(root)).collect();
        Ok(Self {
            roots: roots.to_vec(),
            seen,
        })
    }

    pub fn wait(&mut self, timeout: Duration) -> bool {
        // Not a sleep loop: one comparison per root, so the caller's own clock is
        // what paces it.
        std::thread::sleep(timeout.min(Duration::from_millis(50)));
        let mut changed = false;
        for (root, seen) in self.roots.iter().zip(self.seen.iter_mut()) {
            let now = stamp(root);
            if now != *seen {
                *seen = now;
                changed = true;
            }
        }
        changed
    }

    pub fn roots(&self) -> &[PathBuf] {
        &self.roots
    }
}

fn stamp(path: &std::path::Path) -> Option<SystemTime> {
    std::fs::metadata(path)
        .and_then(|metadata| metadata.modified())
        .ok()
}
