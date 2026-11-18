use std::ffi::{CStr, c_void};
use std::ptr;
use std::time::Instant;

use crate::ffi::error::get_win32_error_detail;
use crate::ffi::types::INTERNET_PROXY_INFO;
use crate::ffi::{
    AutoCloseInternet, HTTP_QUERY_FLAG_NUMBER, HTTP_QUERY_STATUS_CODE, HttpQueryInfoW,
    INTERNET_FLAG_NO_CACHE_WRITE, INTERNET_FLAG_RELOAD, INTERNET_OPEN_TYPE_PRECONFIG,
    INTERNET_OPEN_TYPE_PROXY, INTERNET_OPTION_CONNECT_TIMEOUT, INTERNET_OPTION_PROXY,
    INTERNET_OPTION_RECEIVE_TIMEOUT, InternetOpenUrlW, InternetOpenW, InternetQueryOptionA,
    InternetSetOptionW, get_last_error, to_wide_chars,
};

use super::{FallbackOutcome, execute_with_fallback};

const fn ascii_to_wide<const N: usize>(bytes: &[u8; N]) -> [u16; N] {
    let mut out = [0u16; N];
    let mut i = 0;
    while i < N {
        out[i] = bytes[i] as u16;
        i += 1;
    }
    out
}

const USER_AGENT_WIDE: [u16; 15] = ascii_to_wide(b"NetChecker/1.0\0");
const PROXY_ENV_VARS: [&str; 4] = ["HTTPS_PROXY", "https_proxy", "HTTP_PROXY", "http_proxy"];

/// 環境変数候補イテレータから有効なプロキシ設定値を抽出する純粋関数
fn find_non_empty_proxy(mut candidates: impl Iterator<Item = Option<String>>) -> Option<String> {
    candidates.find_map(|opt| {
        let val = opt?;
        let trimmed = val.trim();
        (!trimmed.is_empty()).then(|| trimmed.to_string())
    })
}

/// Windowsシステム（WinINet）または環境変数のプロキシ設定を検出する
pub fn detect_system_proxy() -> Option<String> {
    let mut buf_len: u32 = 0;
    unsafe {
        InternetQueryOptionA(
            ptr::null_mut(),
            INTERNET_OPTION_PROXY,
            ptr::null_mut(),
            &mut buf_len,
        );
    }

    if buf_len >= std::mem::size_of::<INTERNET_PROXY_INFO>() as u32 && buf_len <= 8192 {
        let mut buffer: Vec<usize> =
            vec![0; (buf_len as usize).div_ceil(std::mem::size_of::<usize>())];
        let ok = unsafe {
            InternetQueryOptionA(
                ptr::null_mut(),
                INTERNET_OPTION_PROXY,
                buffer.as_mut_ptr() as *mut c_void,
                &mut buf_len,
            )
        };
        if ok != 0 {
            let info = unsafe { &*(buffer.as_ptr() as *const INTERNET_PROXY_INFO) };
            if info.dw_access_type == INTERNET_OPEN_TYPE_PROXY && !info.lpsz_proxy.is_null() {
                let c_str = unsafe { CStr::from_ptr(info.lpsz_proxy) };
                let lossy = c_str.to_string_lossy();
                let trimmed = lossy.trim();
                if !trimmed.is_empty() {
                    return Some(trimmed.to_string());
                }
            }
        }
    }

    find_non_empty_proxy(
        PROXY_ENV_VARS
            .into_iter()
            .map(|key| std::env::var(key).ok()),
    )
}

fn check_single_web_access(url: &str, timeout_ms: u32) -> Result<(u32, u64), (String, u64)> {
    let start = Instant::now();
    let url_wide = to_wide_chars(url);

    let Some(guard_internet) = AutoCloseInternet::new(unsafe {
        InternetOpenW(
            USER_AGENT_WIDE.as_ptr(),
            INTERNET_OPEN_TYPE_PRECONFIG,
            ptr::null(),
            ptr::null(),
            0,
        )
    }) else {
        let err_code = get_last_error();
        let elapsed = start.elapsed().as_millis() as u64;
        let err_detail = get_win32_error_detail(err_code);
        return Err((format!("InternetOpen初期化失敗 ({err_detail})"), elapsed));
    };

    let timeout_size = std::mem::size_of::<u32>() as u32;
    unsafe {
        InternetSetOptionW(
            guard_internet.as_ptr(),
            INTERNET_OPTION_CONNECT_TIMEOUT,
            &timeout_ms as *const u32 as *const c_void,
            timeout_size,
        );
        InternetSetOptionW(
            guard_internet.as_ptr(),
            INTERNET_OPTION_RECEIVE_TIMEOUT,
            &timeout_ms as *const u32 as *const c_void,
            timeout_size,
        );
    }

    let raw_url = unsafe {
        InternetOpenUrlW(
            guard_internet.as_ptr(),
            url_wide.as_ptr(),
            ptr::null(),
            0,
            INTERNET_FLAG_RELOAD | INTERNET_FLAG_NO_CACHE_WRITE,
            0,
        )
    };

    let elapsed = start.elapsed().as_millis() as u64;

    let Some(guard_url) = AutoCloseInternet::new(raw_url) else {
        let err_code = get_last_error();
        let err_detail = get_win32_error_detail(err_code);
        return Err((format!("接続失敗 ({err_detail})"), elapsed));
    };

    let mut status_code: u32 = 0;
    let mut code_size = std::mem::size_of::<u32>() as u32;
    let mut index: u32 = 0;

    let success = unsafe {
        HttpQueryInfoW(
            guard_url.as_ptr(),
            HTTP_QUERY_STATUS_CODE | HTTP_QUERY_FLAG_NUMBER,
            &mut status_code as *mut u32 as *mut c_void,
            &mut code_size,
            &mut index,
        )
    };

    if success != 0 && (200..=399).contains(&status_code) {
        Ok((status_code, elapsed))
    } else if success != 0 {
        Err((format!("予期しないHTTPステータス: {status_code}"), elapsed))
    } else {
        let err_code = get_last_error();
        let err_detail = get_win32_error_detail(err_code);
        Err((format!("HTTPステータス取得失敗 ({err_detail})"), elapsed))
    }
}

/// ステップ5: Webアクセス (HTTP/HTTPS) 検証
pub fn check_web_access(
    primary_url: &str,
    secondary_url: Option<&str>,
    timeout_ms: u32,
    proxy_info: Option<&str>,
) -> (bool, String, u64) {
    let format_proxy_suffix = || match proxy_info {
        Some(p) => format!(" [プロキシ設定あり: {p}]"),
        None => String::new(),
    };

    match execute_with_fallback(primary_url, secondary_url, |url| {
        check_single_web_access(url, timeout_ms)
    }) {
        FallbackOutcome::PrimaryOk {
            val: status,
            elapsed_ms,
        } => (
            true,
            format!("HTTP/HTTPS通信 正常 (ステータス: {status})"),
            elapsed_ms,
        ),
        FallbackOutcome::SecondaryOk {
            val: status,
            elapsed_ms,
        } => (
            true,
            format!(
                "HTTP/HTTPS通信 正常 (ステータス: {status}) [フォールバック: {primary_url}接続失敗]"
            ),
            elapsed_ms,
        ),
        FallbackOutcome::FailedBoth {
            primary_err,
            secondary_err,
            elapsed_ms,
        } => {
            let proxy_suffix = format_proxy_suffix();
            (
                false,
                format!(
                    "接続失敗 (プライマリ: {primary_err} / セカンダリ: {secondary_err}){proxy_suffix}"
                ),
                elapsed_ms,
            )
        }
        FallbackOutcome::FailedSingle { err, elapsed_ms } => {
            let proxy_suffix = format_proxy_suffix();
            (false, format!("{err}{proxy_suffix}"), elapsed_ms)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_find_non_empty_proxy() {
        let none_list = [None, Some("   ".to_string()), None];
        assert_eq!(find_non_empty_proxy(none_list.into_iter()), None);

        let some_list = [
            None,
            Some("  http://proxy.corp.local:8080  ".to_string()),
            Some("http://fallback:8080".to_string()),
        ];
        assert_eq!(
            find_non_empty_proxy(some_list.into_iter()),
            Some("http://proxy.corp.local:8080".to_string())
        );
    }
}
