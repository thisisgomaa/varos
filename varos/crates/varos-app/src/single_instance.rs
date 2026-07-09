use std::{
    ffi::{OsStr, OsString},
    path::{Path, PathBuf},
};

pub const MUTEX_NAME: &str = "Varos_SingleInstance_Mutex";
pub const WINDOW_CLASS_NAME: &str = "Varos_Main_Window";

pub fn file_arg_from_env() -> Option<PathBuf> {
    first_file_arg(std::env::args_os().skip(1))
}

pub(crate) fn first_file_arg<I, S>(args: I) -> Option<PathBuf>
where
    I: IntoIterator<Item = S>,
    S: Into<OsString>,
{
    let mut args = args.into_iter();
    while let Some(arg) = args.next() {
        let arg = arg.into();
        if arg == OsStr::new("--") {
            return args.next().map(|p| {
                let p: OsString = p.into();
                PathBuf::from(p)
            });
        }
        if arg == OsStr::new("--dump-tool-icons") || arg == OsStr::new("--preview") {
            let _ = args.next();
            continue;
        }
        if arg.to_string_lossy().starts_with("--") {
            continue;
        }
        return Some(PathBuf::from(arg));
    }
    None
}

#[cfg(windows)]
mod platform {
    use super::{MUTEX_NAME, WINDOW_CLASS_NAME};
    use std::{
        ffi::OsString,
        os::windows::ffi::{OsStrExt, OsStringExt},
        path::{Path, PathBuf},
        sync::Mutex,
        thread,
        time::{Duration, Instant},
    };
    use windows::{
        core::PCWSTR,
        Win32::{
            Foundation::{CloseHandle, GetLastError, ERROR_ALREADY_EXISTS, HANDLE, HWND, LPARAM, LRESULT, WPARAM},
            System::{DataExchange::COPYDATASTRUCT, Threading::CreateMutexW},
            UI::{
                Shell::{DefSubclassProc, RemoveWindowSubclass, SetWindowSubclass},
                WindowsAndMessaging::{
                    FindWindowW, PostMessageW, SendMessageTimeoutW, SetForegroundWindow, ShowWindow, SMTO_ABORTIFHUNG,
                    SMTO_BLOCK, SW_RESTORE, WM_COPYDATA, WM_NCDESTROY, WM_NULL,
                },
            },
        },
    };

    const COPYDATA_FILE_PATH: usize = 0x5652_5346; // "VRSF"
    const COPYDATA_TIMEOUT_MS: u32 = 2_000;
    const FIND_WINDOW_TIMEOUT: Duration = Duration::from_secs(2);
    const FIND_WINDOW_POLL: Duration = Duration::from_millis(50);
    const SUBCLASS_ID: usize = 2;

    static PENDING_FILE_OPENS: Mutex<Vec<PathBuf>> = Mutex::new(Vec::new());

    pub struct SingleInstanceGuard {
        mutex: Option<HANDLE>,
    }

    impl Drop for SingleInstanceGuard {
        fn drop(&mut self) {
            if let Some(mutex) = self.mutex.take() {
                // SAFETY: `mutex` was returned by a successful CreateMutexW call and is owned by this guard.
                unsafe {
                    let _ = CloseHandle(mutex);
                }
            }
        }
    }

    pub fn acquire_or_forward(file_arg: Option<&Path>) -> Option<SingleInstanceGuard> {
        let name = wide_null(MUTEX_NAME);
        // SAFETY: The name buffer is NUL-terminated and lives for this call. We pass no security
        // attributes and do not request initial ownership; object existence is the instance guard.
        let mutex = unsafe { CreateMutexW(None, false, PCWSTR(name.as_ptr())) }.ok()?;
        // SAFETY: GetLastError is read immediately after CreateMutexW, whose success path documents
        // ERROR_ALREADY_EXISTS when the named mutex already existed.
        let already_exists = unsafe { GetLastError() == ERROR_ALREADY_EXISTS };
        if already_exists {
            if let Some(hwnd) = find_existing_window_with_retry() {
                focus_window(hwnd);
                if let Some(path) = file_arg {
                    send_file_path(hwnd, path);
                }
            }
            // SAFETY: `mutex` is the handle CreateMutexW returned to this second process.
            unsafe {
                let _ = CloseHandle(mutex);
            }
            return None;
        }
        Some(SingleInstanceGuard { mutex: Some(mutex) })
    }

    pub fn install_file_open_handler(hwnd: isize) -> bool {
        // SAFETY: `hwnd` is the raw HWND extracted from winit for the live main window. The subclass
        // callback is a static function and uses only process-global state.
        unsafe { SetWindowSubclass(HWND(hwnd as *mut _), Some(file_open_subclass), SUBCLASS_ID, 0).as_bool() }
    }

    pub fn take_pending_file_paths() -> Vec<PathBuf> {
        match PENDING_FILE_OPENS.lock() {
            Ok(mut pending) => std::mem::take(&mut *pending),
            Err(poisoned) => {
                let mut pending = poisoned.into_inner();
                std::mem::take(&mut *pending)
            }
        }
    }

    unsafe extern "system" fn file_open_subclass(
        hwnd: HWND,
        msg: u32,
        wparam: WPARAM,
        lparam: LPARAM,
        _id: usize,
        _data: usize,
    ) -> LRESULT {
        match msg {
            WM_COPYDATA => {
                if receive_file_path(lparam) {
                    // SAFETY: Posting WM_NULL to the same live HWND only wakes the winit message loop;
                    // it carries no pointers or ownership.
                    unsafe {
                        let _ = PostMessageW(Some(hwnd), WM_NULL, WPARAM(0), LPARAM(0));
                    }
                    return LRESULT(1);
                }
            }
            WM_NCDESTROY => {
                // SAFETY: This removes the exact subclass installed by install_file_open_handler.
                unsafe {
                    let _ = RemoveWindowSubclass(hwnd, Some(file_open_subclass), SUBCLASS_ID);
                }
            }
            _ => {}
        }
        // SAFETY: Messages not handled here continue through the standard subclass chain.
        unsafe { DefSubclassProc(hwnd, msg, wparam, lparam) }
    }

    fn receive_file_path(lparam: LPARAM) -> bool {
        if lparam.0 == 0 {
            return false;
        }
        // SAFETY: For WM_COPYDATA, lParam points to a COPYDATASTRUCT valid for the duration of the
        // SendMessage call. We validate the tag, byte count and data pointer before copying the UTF-16
        // payload into an owned PathBuf.
        let path = unsafe {
            let cds = &*(lparam.0 as *const COPYDATASTRUCT);
            if cds.dwData != COPYDATA_FILE_PATH
                || cds.lpData.is_null()
                || cds.cbData < 2
                || !cds.cbData.is_multiple_of(2)
            {
                return false;
            }
            let words = cds.cbData as usize / 2;
            let mut raw = std::slice::from_raw_parts(cds.lpData as *const u16, words);
            if raw.last() == Some(&0) {
                raw = &raw[..raw.len() - 1];
            }
            if raw.is_empty() {
                return false;
            }
            PathBuf::from(OsString::from_wide(raw))
        };
        match PENDING_FILE_OPENS.lock() {
            Ok(mut pending) => pending.push(path),
            Err(poisoned) => poisoned.into_inner().push(path),
        }
        true
    }

    fn find_existing_window_with_retry() -> Option<HWND> {
        let deadline = Instant::now() + FIND_WINDOW_TIMEOUT;
        loop {
            if let Some(hwnd) = find_existing_window() {
                return Some(hwnd);
            }
            if Instant::now() >= deadline {
                return None;
            }
            thread::sleep(FIND_WINDOW_POLL);
        }
    }

    fn find_existing_window() -> Option<HWND> {
        let class_name = wide_null(WINDOW_CLASS_NAME);
        // SAFETY: The class-name buffer is NUL-terminated and lives for this call; a null window title
        // means "match any title" because Varos titles include the current document and tool.
        unsafe { FindWindowW(PCWSTR(class_name.as_ptr()), PCWSTR::null()).ok() }
    }

    fn focus_window(hwnd: HWND) {
        // SAFETY: `hwnd` came from FindWindowW for our main window class. Restoring and foregrounding
        // may fail due to normal foreground-lock rules; failures are harmless for the guard.
        unsafe {
            let _ = ShowWindow(hwnd, SW_RESTORE);
            let _ = SetForegroundWindow(hwnd);
        }
    }

    fn send_file_path(hwnd: HWND, path: &Path) {
        let path = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
        let wide: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
        let cds = COPYDATASTRUCT {
            dwData: COPYDATA_FILE_PATH,
            cbData: (wide.len() * std::mem::size_of::<u16>()) as u32,
            lpData: wide.as_ptr() as *mut _,
        };
        let mut result = 0usize;
        // SAFETY: WM_COPYDATA is sent synchronously; `wide` and `cds` stay alive until the call returns.
        // The timeout keeps a hung first instance from hanging this second process indefinitely.
        unsafe {
            let _ = SendMessageTimeoutW(
                hwnd,
                WM_COPYDATA,
                WPARAM(0),
                LPARAM(&cds as *const _ as isize),
                SMTO_ABORTIFHUNG | SMTO_BLOCK,
                COPYDATA_TIMEOUT_MS,
                Some(&mut result),
            );
        }
    }

    fn wide_null(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(Some(0)).collect()
    }
}

#[cfg(windows)]
pub(crate) type SingleInstanceGuard = platform::SingleInstanceGuard;

#[cfg(windows)]
pub(crate) fn acquire_or_forward(file_arg: Option<&Path>) -> Option<SingleInstanceGuard> {
    platform::acquire_or_forward(file_arg)
}

#[cfg(windows)]
pub(crate) fn install_file_open_handler(hwnd: isize) -> bool {
    platform::install_file_open_handler(hwnd)
}

#[cfg(windows)]
pub(crate) fn take_pending_file_paths() -> Vec<PathBuf> {
    platform::take_pending_file_paths()
}

#[cfg(not(windows))]
pub struct SingleInstanceGuard;

#[cfg(not(windows))]
pub fn acquire_or_forward(_file_arg: Option<&Path>) -> Option<SingleInstanceGuard> {
    Some(SingleInstanceGuard)
}

#[cfg(not(windows))]
pub fn install_file_open_handler(_hwnd: isize) -> bool {
    false
}

#[cfg(not(windows))]
pub fn take_pending_file_paths() -> Vec<PathBuf> {
    Vec::new()
}

#[cfg(test)]
mod tests {
    use super::first_file_arg;
    use std::{ffi::OsString, path::PathBuf};

    #[test]
    fn first_file_arg_ignores_flags() {
        let args = [OsString::from("--ignored"), OsString::from("C:\\work\\one.vrs")];
        assert_eq!(first_file_arg(args), Some(PathBuf::from("C:\\work\\one.vrs")));
    }

    #[test]
    fn first_file_arg_skips_known_flag_values() {
        let args = [OsString::from("--preview"), OsString::from("assets"), OsString::from("D:\\docs\\two.vrs")];
        assert_eq!(first_file_arg(args), Some(PathBuf::from("D:\\docs\\two.vrs")));
    }
}
