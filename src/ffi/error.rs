use super::{
    FORMAT_MESSAGE_FROM_SYSTEM, FORMAT_MESSAGE_IGNORE_INSERTS, FormatMessageW, IP_REQ_TIMED_OUT,
};
use std::ptr;

/// ICMP / IP (11000〜11050系) の共通説明文を返す
fn get_ip_status_desc(status: u32) -> Option<&'static str> {
    match status {
        0 => Some("成功 (IP_SUCCESS)"),
        11002 => Some("宛先ネットワーク到達不能 (IP_DEST_NET_UNREACHABLE)"),
        11003 => Some("宛先ホスト到達不能 (IP_DEST_HOST_UNREACHABLE)"),
        IP_REQ_TIMED_OUT => Some("要求タイムアウト (IP_REQ_TIMED_OUT)"),
        11013 => Some("TTL有効期限切れ (IP_TTL_EXPIRED_TRANSIT)"),
        _ => None,
    }
}

/// ICMP/IP・WinINet・Winsock の既知エラー解説を統合検索する
fn get_known_error_desc(code: u32) -> Option<&'static str> {
    if let Some(desc) = get_ip_status_desc(code) {
        return Some(desc);
    }

    match code {
        // WinINet (12000 - 12174)
        12002 => Some("要求タイムアウト (ERROR_INTERNET_TIMEOUT)"),
        12005 => Some("URL書式不正 (ERROR_INTERNET_INVALID_URL)"),
        12007 => Some("サーバー名解決失敗 (ERROR_INTERNET_NAME_NOT_RESOLVED)"),
        12029 => Some("サーバー接続失敗 (ERROR_INTERNET_CANNOT_CONNECT)"),
        12030 => Some("接続切断 (ERROR_INTERNET_CONNECTION_ABORTED)"),
        12031 => Some("接続リセット (ERROR_INTERNET_CONNECTION_RESET)"),
        12045 => Some("無効な認証機関 (ERROR_INTERNET_INVALID_CA)"),
        12157 => Some("TLS/SSLハンドシェイク失敗 (ERROR_INTERNET_SECURITY_CHANNEL_ERROR)"),
        // WSA / Winsock (10000 - 10099)
        10051 => Some("ネットワーク到達不能 (WSAENETUNREACH)"),
        10054 => Some("接続強制切断 (WSAECONNRESET)"),
        10060 => Some("接続タイムアウト (WSAETIMEDOUT)"),
        10061 => Some("接続拒否 (WSAECONNREFUSED)"),
        10065 => Some("ホスト到達不能 (WSAEHOSTUNREACH)"),
        _ => None,
    }
}

/// ICMPステータスコードの解説文字列を取得
pub fn get_icmp_status_detail(status: u32) -> String {
    if let Some(desc) = get_ip_status_desc(status) {
        if status == 0 {
            desc.to_string()
        } else {
            format!("ステータス {status}: {desc}")
        }
    } else {
        format!("ステータスコード {status}")
    }
}

/// Win32 エラーコードの詳細解説を取得
pub fn get_win32_error_detail(code: u32) -> String {
    if let Some(desc) = get_known_error_desc(code) {
        return format!("エラーコード {code}: {desc}");
    }

    // Windows標準メッセージ取得 (FormatMessageW)
    let mut buffer: [u16; 256] = [0; 256];
    let len = unsafe {
        FormatMessageW(
            FORMAT_MESSAGE_FROM_SYSTEM | FORMAT_MESSAGE_IGNORE_INSERTS,
            ptr::null(),
            code,
            0,
            buffer.as_mut_ptr(),
            buffer.len() as u32,
            ptr::null_mut(),
        )
    };
    if len > 0 {
        let msg = String::from_utf16_lossy(&buffer[..len as usize]);
        let trimmed = msg.trim_end_matches(['\r', '\n', ' ', '.']);
        format!("エラーコード {code}: {trimmed}")
    } else {
        format!("エラーコード {code}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_icmp_status_detail() {
        assert_eq!(get_icmp_status_detail(0), "成功 (IP_SUCCESS)");
        assert_eq!(
            get_icmp_status_detail(11010),
            "ステータス 11010: 要求タイムアウト (IP_REQ_TIMED_OUT)"
        );
        assert_eq!(get_icmp_status_detail(99999), "ステータスコード 99999");
    }

    #[test]
    fn test_get_win32_error_detail_known() {
        assert_eq!(
            get_win32_error_detail(11010),
            "エラーコード 11010: 要求タイムアウト (IP_REQ_TIMED_OUT)"
        );
        assert_eq!(
            get_win32_error_detail(12007),
            "エラーコード 12007: サーバー名解決失敗 (ERROR_INTERNET_NAME_NOT_RESOLVED)"
        );
        assert_eq!(
            get_win32_error_detail(10060),
            "エラーコード 10060: 接続タイムアウト (WSAETIMEDOUT)"
        );
    }
}
