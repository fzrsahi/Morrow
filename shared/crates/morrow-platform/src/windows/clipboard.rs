//! The L-009 floor on Windows (#187): when no terminal window can be raised,
//! hand the working path over through the clipboard instead of doing nothing.

use windows::Win32::Foundation::{GlobalFree, HANDLE};
use windows::Win32::System::DataExchange::{
    CloseClipboard, EmptyClipboard, OpenClipboard, SetClipboardData,
};
use windows::Win32::System::Memory::{GlobalAlloc, GlobalLock, GlobalUnlock, GMEM_MOVEABLE};

// Win32's CF_UNICODETEXT. Spelling the number here keeps the WinRT/Ole feature
// out of this crate for one constant.
const CF_UNICODETEXT: u32 = 13;

/// Put `text` on the clipboard, or say why the desktop refused.
pub fn copy(text: &str) -> Result<(), String> {
    let units: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
    unsafe {
        OpenClipboard(None).map_err(|error| error.message())?;
        let _guard = Open;
        EmptyClipboard().map_err(|error| error.message())?;
        let memory = GlobalAlloc(GMEM_MOVEABLE, units.len() * size_of::<u16>())
            .map_err(|error| error.message())?;
        let target = GlobalLock(memory) as *mut u16;
        if target.is_null() {
            let _ = GlobalUnlock(memory);
            return Err("the clipboard would not lock".to_owned());
        }
        std::ptr::copy_nonoverlapping(units.as_ptr(), target, units.len());
        let _ = GlobalUnlock(memory);
        // On success Windows owns the global; on failure we still must free it.
        match SetClipboardData(CF_UNICODETEXT, Some(HANDLE(memory.0))) {
            Ok(_) => Ok(()),
            Err(error) => {
                let _ = GlobalFree(Some(memory));
                Err(error.message())
            }
        }
        // `guard` closes the clipboard on both paths.
    }
}

struct Open;

impl Drop for Open {
    fn drop(&mut self) {
        unsafe {
            let _ = CloseClipboard();
        }
    }
}
