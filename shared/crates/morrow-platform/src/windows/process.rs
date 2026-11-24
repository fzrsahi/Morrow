//! The process table a jump walks (#187, W-007).
//!
//! `cli_pid` is a shell child, not the window owner: the owning terminal sits
//! somewhere above it. `CreateToolhelp32Snapshot` gives both the parent chain
//! and each process's executable name in one read, so the pure
//! `windows_rules::terminal_pid` can decide which window to raise.

use std::collections::BTreeMap;

use windows::Win32::Foundation::CloseHandle;
use windows::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, PROCESSENTRY32W, Process32FirstW, Process32NextW, TH32CS_SNAPPROCESS,
};

use crate::windows_rules::Parents;

/// (parent chain, executable name by pid), or why the table could not be read.
pub fn table() -> Result<(Parents, BTreeMap<u32, String>), String> {
    unsafe {
        let snapshot = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0).map_err(|e| e.message())?;
        // A snapshot is a kernel handle: close it even if enumeration fails.
        let guard = Snapshot(snapshot);
        let mut entry = PROCESSENTRY32W {
            dwSize: size_of::<PROCESSENTRY32W>() as u32,
            ..Default::default()
        };
        Process32FirstW(snapshot, &mut entry).map_err(|e| e.message())?;
        let mut parents = Parents::new();
        let mut names = BTreeMap::new();
        loop {
            let pid = entry.th32ProcessID;
            if pid != 0 {
                parents.insert(pid, entry.th32ParentProcessID);
                names.insert(pid, exe_name(&entry.szExeFile));
            }
            if Process32NextW(snapshot, &mut entry).is_err() {
                break;
            }
        }
        drop(guard);
        Ok((parents, names))
    }
}

struct Snapshot(windows::Win32::Foundation::HANDLE);

impl Drop for Snapshot {
    fn drop(&mut self) {
        unsafe {
            let _ = CloseHandle(self.0);
        }
    }
}

fn exe_name(raw: &[u16; 260]) -> String {
    let length = raw.iter().position(|unit| *unit == 0).unwrap_or(raw.len());
    String::from_utf16_lossy(&raw[..length])
}
