pub mod error;
pub mod types;

pub use types::*;

use std::ffi::c_void;

// --- 定数定義 ---

const CODE_PAGE_UTF8: u32 = 65_001;
pub const AF_UNSPEC: u32 = 0;
pub const AF_INET: u16 = 2;
pub const GAA_FLAG_INCLUDE_GATEWAYS: u32 = 0x0080;
pub const ERROR_BUFFER_OVERFLOW: u32 = 111;
pub const NO_ERROR: u32 = 0;
pub const IP_REQ_TIMED_OUT: u32 = 11010;

pub const FORMAT_MESSAGE_FROM_SYSTEM: u32 = 0x0000_1000;
pub const FORMAT_MESSAGE_IGNORE_INSERTS: u32 = 0x0000_0200;

pub const IF_TYPE_SOFTWARE_LOOPBACK: u32 = 24;
pub const IF_OPER_STATUS_UP: u32 = 1;

pub const INTERNET_OPEN_TYPE_PRECONFIG: u32 = 0;
pub const INTERNET_OPEN_TYPE_PROXY: u32 = 3;
pub const INTERNET_OPTION_CONNECT_TIMEOUT: u32 = 2;
pub const INTERNET_OPTION_RECEIVE_TIMEOUT: u32 = 6;
pub const INTERNET_OPTION_PROXY: u32 = 38;
pub const INTERNET_FLAG_RELOAD: u32 = 0x8000_0000;
pub const INTERNET_FLAG_NO_CACHE_WRITE: u32 = 0x0400_0000;
pub const HTTP_QUERY_STATUS_CODE: u32 = 19;
pub const HTTP_QUERY_FLAG_NUMBER: u32 = 0x2000_0000;

// --- Windows API 宣言 (Rust 2024 unsafe extern) ---

#[link(name = "kernel32")]
unsafe extern "system" {
    fn SetConsoleCP(wCodePageID: u32) -> i32;
    fn SetConsoleOutputCP(wCodePageID: u32) -> i32;
    fn GetLocalTime(lpSystemTime: *mut SYSTEMTIME);
    fn GetLastError() -> u32;
    pub fn FormatMessageW(
        dwFlags: u32,
        lpSource: *const c_void,
        dwMessageId: u32,
        dwLanguageId: u32,
        lpBuffer: *mut u16,
        nSize: u32,
        Arguments: *mut c_void,
    ) -> u32;
}

#[link(name = "msvcrt")]
unsafe extern "C" {
    fn _getch() -> i32;
}

#[link(name = "iphlpapi")]
unsafe extern "system" {
    pub fn GetAdaptersAddresses(
        family: u32,
        flags: u32,
        reserved: *mut c_void,
        adapter_addresses: *mut IP_ADAPTER_ADDRESSES,
        size_pointer: *mut u32,
    ) -> u32;

    pub fn GetBestInterface(destination_address: u32, best_if_index: *mut u32) -> u32;

    pub fn IcmpCreateFile() -> *mut c_void;
    fn IcmpCloseHandle(icmp_handle: *mut c_void) -> i32;
    pub fn IcmpSendEcho(
        icmp_handle: *mut c_void,
        destination_address: u32,
        request_data: *const c_void,
        request_size: u16,
        request_options: *const IP_OPTION_INFORMATION,
        reply_buffer: *mut c_void,
        reply_size: u32,
        timeout: u32,
    ) -> u32;
}

#[link(name = "wininet")]
unsafe extern "system" {
    pub fn InternetOpenW(
        lpszAgent: *const u16,
        dwAccessType: u32,
        lpszProxy: *const u16,
        lpszProxyBypass: *const u16,
        dwFlags: u32,
    ) -> *mut c_void;

    pub fn InternetSetOptionW(
        hInternet: *mut c_void,
        dwOption: u32,
        lpBuffer: *const c_void,
        dwBufferLength: u32,
    ) -> i32;

    pub fn InternetQueryOptionA(
        hInternet: *mut c_void,
        dwOption: u32,
        lpBuffer: *mut c_void,
        lpdwBufferLength: *mut u32,
    ) -> i32;

    pub fn InternetOpenUrlW(
        hInternet: *mut c_void,
        lpszUrl: *const u16,
        lpszHeaders: *const u16,
        dwHeadersLength: u32,
        dwFlags: u32,
        dwContext: usize,
    ) -> *mut c_void;

    pub fn HttpQueryInfoW(
        hRequest: *mut c_void,
        dwInfoLevel: u32,
        lpvBuffer: *mut c_void,
        lpdwBufferLength: *mut u32,
        lpdwIndex: *mut u32,
    ) -> i32;

    fn InternetCloseHandle(hInternet: *mut c_void) -> i32;
}

// --- RAII ガード ---

const INVALID_HANDLE_VALUE: *mut c_void = -1isize as *mut c_void;

pub struct AutoCloseIcmp(*mut c_void);

impl AutoCloseIcmp {
    pub fn new(handle: *mut c_void) -> Option<Self> {
        (!handle.is_null() && handle != INVALID_HANDLE_VALUE).then(|| Self(handle))
    }

    pub fn as_ptr(&self) -> *mut c_void {
        self.0
    }
}

impl Drop for AutoCloseIcmp {
    fn drop(&mut self) {
        unsafe {
            IcmpCloseHandle(self.0);
        }
    }
}

pub struct AutoCloseInternet(*mut c_void);

impl AutoCloseInternet {
    pub fn new(handle: *mut c_void) -> Option<Self> {
        (!handle.is_null()).then(|| Self(handle))
    }

    pub fn as_ptr(&self) -> *mut c_void {
        self.0
    }
}

impl Drop for AutoCloseInternet {
    fn drop(&mut self) {
        unsafe {
            InternetCloseHandle(self.0);
        }
    }
}

// --- 安全ラッパー（コンソール・エラー取得） ---

/// コンソールの入出力コードページをUTF-8に設定する
pub fn init_console_utf8() {
    unsafe {
        SetConsoleCP(CODE_PAGE_UTF8);
        SetConsoleOutputCP(CODE_PAGE_UTF8);
    }
}

/// 任意のキー入力があるまで待機する
pub fn wait_for_keypress() {
    unsafe {
        _getch();
    }
}

/// 直近のWin32エラーコードを取得する
pub fn get_last_error() -> u32 {
    unsafe { GetLastError() }
}

// --- 文字列・時刻ユーティリティ ---

pub fn to_wide_chars(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

pub unsafe fn pwstr_to_string(ptr: *const u16) -> String {
    if ptr.is_null() {
        return String::new();
    }
    let mut len = 0;
    unsafe {
        while *ptr.add(len) != 0 {
            len += 1;
        }
        let slice = std::slice::from_raw_parts(ptr, len);
        String::from_utf16_lossy(slice)
    }
}

pub fn get_now_system_time() -> SYSTEMTIME {
    let mut st = SYSTEMTIME::default();
    unsafe {
        GetLocalTime(&mut st);
    }
    st
}

pub fn format_system_time(st: &SYSTEMTIME) -> (String, String) {
    let display_str = format!(
        "{:04}-{:02}-{:02} {:02}:{:02}:{:02}",
        st.w_year, st.w_month, st.w_day, st.w_hour, st.w_minute, st.w_second
    );
    let file_str = format!(
        "{:04}{:02}{:02}_{:02}{:02}{:02}",
        st.w_year, st.w_month, st.w_day, st.w_hour, st.w_minute, st.w_second
    );
    (display_str, file_str)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_auto_close_icmp_rejects_invalid_handles() {
        assert!(AutoCloseIcmp::new(std::ptr::null_mut()).is_none());
        assert!(AutoCloseIcmp::new(INVALID_HANDLE_VALUE).is_none());
        assert!(AutoCloseInternet::new(std::ptr::null_mut()).is_none());
    }

    #[test]
    fn test_to_wide_chars_and_pwstr_to_string() {
        let wide = to_wide_chars("NetCheckerテスト");
        assert_eq!(wide.last(), Some(&0));
        let restored = unsafe { pwstr_to_string(wide.as_ptr()) };
        assert_eq!(restored, "NetCheckerテスト");
        assert_eq!(unsafe { pwstr_to_string(std::ptr::null()) }, "");
    }

    #[test]
    fn test_format_system_time() {
        let st = SYSTEMTIME {
            w_year: 2026,
            w_month: 11,
            w_day: 8,
            w_day_of_week: 0,
            w_hour: 9,
            w_minute: 5,
            w_second: 3,
            w_milliseconds: 0,
        };
        let (disp, file) = format_system_time(&st);
        assert_eq!(disp, "2026-11-08 09:05:03");
        assert_eq!(file, "20261108_090503");
    }
}
