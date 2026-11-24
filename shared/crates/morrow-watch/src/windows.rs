//! `ReadDirectoryChangesW` (#286).
//!
//! **Never executed on the machine this was written on.** It compiles for
//! `x86_64-pc-windows-msvc` with `cargo xwin`; that is the whole of the evidence,
//! and the fallback arm is what runs where it was written.
//!
//! One handle per root with `bWatchSubtree`, which is the whole difference from the
//! Linux arm: Windows walks the tree for us, so no per-directory bookkeeping.

#![allow(unsafe_code)] // This is the OS edge: every call here is a syscall.

use std::os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle};
use std::path::PathBuf;
use std::time::Duration;

use windows_sys::Win32::Foundation::{ERROR_IO_PENDING, HANDLE};
use windows_sys::Win32::Storage::FileSystem::{
    FILE_FLAG_BACKUP_SEMANTICS, FILE_LIST_DIRECTORY, FILE_NOTIFY_CHANGE_DIR_NAME,
    FILE_NOTIFY_CHANGE_FILE_NAME, FILE_NOTIFY_CHANGE_LAST_WRITE, FILE_NOTIFY_CHANGE_SIZE,
    FILE_SHARE_DELETE, FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING, ReadDirectoryChangesW,
};
use windows_sys::Win32::System::IO::OVERLAPPED;
use windows_sys::Win32::System::Threading::CreateEventW;

pub struct Platform {
    roots: Vec<PathBuf>,
    directories: Vec<Directory>,
}

struct Directory {
    handle: OwnedHandle,
    event: OwnedHandle,
}

impl Platform {
    pub fn new(roots: &[PathBuf]) -> std::io::Result<Self> {
        let mut directories = Vec::new();
        let mut watched = Vec::new();
        for root in roots {
            match Directory::open(root) {
                Ok(directory) => {
                    directories.push(directory);
                    watched.push(root.clone());
                }
                // A root we cannot open is a store we cannot watch; the timer
                // covers it, so this is not an error.
                Err(_) => continue,
            }
        }
        Ok(Self {
            roots: watched,
            directories,
        })
    }

    pub fn wait(&mut self, timeout: Duration) -> bool {
        if self.directories.is_empty() {
            std::thread::sleep(timeout);
            return false;
        }
        let milliseconds = u32::try_from(timeout.as_millis()).unwrap_or(u32::MAX);
        // Waiting on the change events rather than the handles: the overlapped read
        // signals the event, and an empty buffer afterwards means "nothing this
        // arm can name", which is still a change worth asking the policy about.
        let handles: Vec<HANDLE> = self
            .directories
            .iter()
            .map(|directory| directory.event.as_raw_handle() as HANDLE)
            .collect();
        let waited = unsafe {
            windows_sys::Win32::System::Threading::WaitForMultipleObjects(
                handles.len() as u32,
                handles.as_ptr(),
                0,
                milliseconds,
            )
        };
        if waited == windows_sys::Win32::Foundation::WAIT_TIMEOUT {
            return false;
        }
        // One root moved; draining every root keeps the next wait honest, because
        // an undrained overlapped read never signals again.
        for directory in &self.directories {
            directory.drain();
        }
        true
    }

    pub fn roots(&self) -> &[PathBuf] {
        &self.roots
    }
}

impl Directory {
    fn open(root: &std::path::Path) -> std::io::Result<Self> {
        use std::os::windows::ffi::OsStrExt;
        let wide: Vec<u16> = root.as_os_str().encode_wide().chain([0]).collect();
        let handle = unsafe {
            windows_sys::Win32::Storage::FileSystem::CreateFileW(
                wide.as_ptr(),
                FILE_LIST_DIRECTORY,
                FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
                std::ptr::null(),
                OPEN_EXISTING,
                FILE_FLAG_BACKUP_SEMANTICS,
                std::ptr::null_mut(),
            )
        };
        if handle == windows_sys::Win32::Foundation::INVALID_HANDLE_VALUE {
            return Err(std::io::Error::last_os_error());
        }
        // `FromRawHandle` rather than `from_raw_handle`: the handle must be closed,
        // and a leaked one per root would outlive the shell.
        let handle = unsafe { OwnedHandle::from_raw_handle(handle) };
        let event = unsafe { CreateEventW(std::ptr::null(), 1, 0, std::ptr::null()) };
        if event.is_null() {
            return Err(std::io::Error::last_os_error());
        }
        let event = unsafe { OwnedHandle::from_raw_handle(event) };
        let directory = Self { handle, event };
        directory.begin()?;
        Ok(directory)
    }

    /// Start an overlapped read. The result is deliberately ignored: what matters
    /// is that the event is armed, and `drain` is where the answer is read.
    fn begin(&self) -> std::io::Result<()> {
        let mut overlapped = self.overlapped();
        let mut buffer = [0u8; 4096];
        let started = unsafe {
            ReadDirectoryChangesW(
                self.handle.as_raw_handle() as HANDLE,
                buffer.as_mut_ptr().cast(),
                buffer.len() as u32,
                1,
                FILE_NOTIFY_CHANGE_FILE_NAME
                    | FILE_NOTIFY_CHANGE_DIR_NAME
                    | FILE_NOTIFY_CHANGE_LAST_WRITE
                    | FILE_NOTIFY_CHANGE_SIZE,
                std::ptr::null_mut(),
                &mut overlapped,
                None,
            )
        };
        if started == 0 {
            let error = std::io::Error::last_os_error();
            // ERROR_IO_PENDING is the success path for an overlapped call.
            if error.raw_os_error() != Some(ERROR_IO_PENDING as i32) {
                return Err(error);
            }
        }
        Ok(())
    }

    /// Re-arm after the event fired.
    fn drain(&self) {
        let _ = self.begin();
    }

    fn overlapped(&self) -> OVERLAPPED {
        let mut overlapped: OVERLAPPED = unsafe { std::mem::zeroed() };
        overlapped.hEvent = self.event.as_raw_handle() as HANDLE;
        overlapped
    }
}
