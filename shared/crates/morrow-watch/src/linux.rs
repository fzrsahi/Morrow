//! inotify (#286).
//!
//! **Never executed on the machine this was written on.** It compiles for
//! `x86_64-unknown-linux-gnu`; that is the whole of the evidence, and the fallback
//! arm is what runs where it was written.
//!
//! # Why a watch per directory
//!
//! inotify is not recursive. A store is a tree that grows — one directory per
//! project, one per session — so watching only the root would go quiet the moment
//! the interesting files moved one level down. Watching every directory under the
//! root and adding new ones as they appear is the shape `ReadDirectoryChangesW`
//! gives for free on Windows and inotify makes you write.

#![allow(unsafe_code)] // This is the OS edge: every call here is a syscall.

use std::collections::HashMap;
use std::os::unix::io::RawFd;
use std::path::{Path, PathBuf};
use std::time::Duration;

pub struct Platform {
    fd: RawFd,
    roots: Vec<PathBuf>,
    /// wd → the directory it stands for, so a new subdirectory can be added and a
    /// vanished one forgotten.
    watched: HashMap<i32, PathBuf>,
}

impl Platform {
    pub fn new(roots: &[PathBuf]) -> std::io::Result<Self> {
        // `IN_NONBLOCK` so `read` never waits behind `poll`.
        let fd = unsafe { libc::inotify_init1(libc::IN_NONBLOCK | libc::IN_CLOEXEC) };
        if fd < 0 {
            return Err(std::io::Error::last_os_error());
        }
        let mut platform = Self {
            fd,
            roots: roots.to_vec(),
            watched: HashMap::new(),
        };
        for root in roots {
            platform.watch_tree(root);
        }
        Ok(platform)
    }

    pub fn wait(&mut self, timeout: Duration) -> bool {
        let mut poll_fd = libc::pollfd {
            fd: self.fd,
            events: libc::POLLIN,
            revents: 0,
        };
        let milliseconds = i32::try_from(timeout.as_millis()).unwrap_or(i32::MAX);
        let ready = unsafe { libc::poll(&mut poll_fd, 1, milliseconds) };
        if ready <= 0 {
            return false;
        }
        let mut changed = false;
        // A single read returns as many events as fit; more than one read is only
        // needed when an agent writes faster than this loop drains, and the next
        // tick catches that.
        let mut buffer = [0u8; 8192];
        let read = unsafe {
            libc::read(
                self.fd,
                buffer.as_mut_ptr().cast::<libc::c_void>(),
                buffer.len(),
            )
        };
        if read <= 0 {
            return false;
        }
        let mut offset = 0usize;
        let read = read as usize;
        while offset + std::mem::size_of::<libc::inotify_event>() <= read {
            // The event header is fixed size and the name follows it, so the buffer
            // is read by hand rather than transmuted: a `#[repr]` assumption about
            // libc's struct would be undefined behaviour if it were ever wrong.
            let wd = i32::from_ne_bytes([
                buffer[offset],
                buffer[offset + 1],
                buffer[offset + 2],
                buffer[offset + 3],
            ]);
            let mask = u32::from_ne_bytes([
                buffer[offset + 4],
                buffer[offset + 5],
                buffer[offset + 6],
                buffer[offset + 7],
            ]);
            let name_length = u32::from_ne_bytes([
                buffer[offset + 12],
                buffer[offset + 13],
                buffer[offset + 14],
                buffer[offset + 15],
            ]) as usize;
            offset += std::mem::size_of::<libc::inotify_event>() + name_length;
            if mask & libc::IN_IGNORED != 0 {
                self.watched.remove(&wd);
                continue;
            }
            changed = true;
            // A directory that just appeared is a project or a session: watch it
            // too, or its files are invisible.
            if mask & libc::IN_ISDIR != 0 && mask & (libc::IN_CREATE | libc::IN_MOVED_TO) != 0 {
                if let Some(parent) = self.watched.get(&wd).cloned() {
                    let name_start = offset - name_length;
                    let name = &buffer[name_start..name_start + name_length];
                    let name = name.split(|byte| *byte == 0).next().unwrap_or_default();
                    if let Ok(name) = std::str::from_utf8(name) {
                        if !name.is_empty() {
                            self.watch_tree(&parent.join(name));
                        }
                    }
                }
            }
        }
        changed
    }

    pub fn roots(&self) -> &[PathBuf] {
        &self.roots
    }

    /// Add every directory at or under `root`, one level deep.
    ///
    /// Depth one is deliberate: a store's useful files sit at
    /// `<root>/<project>/<file>`, and walking deeper would spend the scan's budget
    /// on inotify bookkeeping instead.
    fn watch_tree(&mut self, root: &Path) {
        self.watch_one(root);
        let Ok(entries) = std::fs::read_dir(root) else {
            return;
        };
        for entry in entries.flatten() {
            if entry.file_type().map(|kind| kind.is_dir()).unwrap_or(false) {
                self.watch_one(&entry.path());
            }
        }
    }

    fn watch_one(&mut self, directory: &Path) {
        let mask = libc::IN_CREATE
            | libc::IN_CLOSE_WRITE
            | libc::IN_MODIFY
            | libc::IN_MOVED_TO
            | libc::IN_DELETE
            | libc::IN_MOVED_FROM
            | libc::IN_ONLYDIR;
        let name = match std::ffi::CString::new(directory.to_string_lossy().as_bytes()) {
            Ok(name) => name,
            Err(_) => return,
        };
        let wd = unsafe { libc::inotify_add_watch(self.fd, name.as_ptr(), mask) };
        if wd >= 0 {
            self.watched.insert(wd, directory.to_path_buf());
        }
    }
}

impl Drop for Platform {
    fn drop(&mut self) {
        unsafe { libc::close(self.fd) };
    }
}
