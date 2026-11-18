use std::ffi::c_void;
use std::net::Ipv4Addr;
use std::ptr;
use std::time::Instant;

use crate::ffi::error::{get_icmp_status_detail, get_win32_error_detail};
use crate::ffi::types::ICMP_ECHO_REPLY;
use crate::ffi::{AutoCloseIcmp, IP_REQ_TIMED_OUT, IcmpCreateFile, IcmpSendEcho, get_last_error};

use super::dns::resolve_ipv4_host;
use super::{FallbackOutcome, execute_with_fallback};

const ECHO_SEND_DATA: &[u8] = b"NetChecker Echo Test";

#[repr(C)]
struct EchoReplyBuffer {
    reply: ICMP_ECHO_REPLY,
    _payload: [u8; 64],
}

impl Default for EchoReplyBuffer {
    fn default() -> Self {
        Self {
            reply: ICMP_ECHO_REPLY::default(),
            _payload: [0; 64],
        }
    }
}

/// ICMP Echo (Ping) 送信（成功時は (RTT_ms, 実測elapsed_ms)、失敗時は (エラー詳細, 実測elapsed_ms) を返す）
fn send_icmp_ping(dest: Ipv4Addr, timeout_ms: u32) -> Result<(u32, u64), (String, u64)> {
    let start = Instant::now();
    let Some(guard) = AutoCloseIcmp::new(unsafe { IcmpCreateFile() }) else {
        let err_code = get_last_error();
        let elapsed = start.elapsed().as_millis() as u64;
        let err_detail = get_win32_error_detail(err_code);
        return Err((
            format!("ICMPハンドルの作成に失敗しました ({err_detail})"),
            elapsed,
        ));
    };

    let dest_u32 = u32::from_ne_bytes(dest.octets());
    let mut reply_buf = EchoReplyBuffer::default();
    let reply_size = std::mem::size_of::<EchoReplyBuffer>() as u32;

    let ret = unsafe {
        IcmpSendEcho(
            guard.as_ptr(),
            dest_u32,
            ECHO_SEND_DATA.as_ptr() as *const c_void,
            ECHO_SEND_DATA.len() as u16,
            ptr::null(),
            (&raw mut reply_buf) as *mut c_void,
            reply_size,
            timeout_ms,
        )
    };

    let elapsed = start.elapsed().as_millis() as u64;

    if ret > 0 {
        if reply_buf.reply.status == 0 {
            Ok((reply_buf.reply.round_trip_time, elapsed))
        } else {
            let status_msg = get_icmp_status_detail(reply_buf.reply.status);
            Err((format!("応答エラー ({status_msg})"), elapsed))
        }
    } else {
        let err_code = get_last_error();
        if err_code == IP_REQ_TIMED_OUT || err_code == 0 {
            Err((format!("タイムアウト ({timeout_ms}ms超過)"), elapsed))
        } else {
            let err_detail = get_win32_error_detail(err_code);
            Err((
                format!("タイムアウトまたは通信失敗 ({timeout_ms}ms超過, {err_detail})"),
                elapsed,
            ))
        }
    }
}

/// ステップ2: デフォルトゲートウェイ（宅内ルーター）疎通検証
pub fn check_router_ping(gateway_ip: Option<Ipv4Addr>, timeout_ms: u32) -> (bool, String, u64) {
    if let Some(gw_ip) = gateway_ip {
        match send_icmp_ping(gw_ip, timeout_ms) {
            Ok((rtt, ms)) => (true, format!("{gw_ip} 応答あり ({rtt}ms)"), ms),
            Err((err, ms)) => (false, format!("{gw_ip} 応答なし ({err})"), ms),
        }
    } else {
        (
            false,
            "デフォルトゲートウェイが設定されていないか、アダプタ未検出です".to_string(),
            0,
        )
    }
}

/// ステップ3: インターネット出口疎通検証
pub fn check_external_ping(
    primary: Option<Ipv4Addr>,
    secondary: Option<Ipv4Addr>,
    dns_host_fallback: &str,
    timeout_ms: u32,
) -> (bool, String, u64) {
    let (primary_ip, pre_resolve_ms) = match primary {
        Some(ip) => (ip, 0),
        None => match resolve_ipv4_host(dns_host_fallback) {
            Ok((ip, ms)) => (ip, ms),
            Err((_, ms)) => {
                return (
                    false,
                    format!(
                        "ホスト名 '{dns_host_fallback}' のIPv4アドレス未解決のためPingスキップ"
                    ),
                    ms,
                );
            }
        },
    };

    let (ok, detail, elapsed_ms) =
        match execute_with_fallback(primary_ip, secondary, |ip| send_icmp_ping(ip, timeout_ms)) {
            FallbackOutcome::PrimaryOk {
                val: rtt,
                elapsed_ms,
            } => (true, format!("{primary_ip} 応答あり ({rtt}ms)"), elapsed_ms),
            FallbackOutcome::SecondaryOk {
                val: rtt,
                elapsed_ms,
            } => {
                let sec_ip = secondary.unwrap_or(primary_ip);
                (
                    true,
                    format!("{sec_ip} 応答あり ({rtt}ms) [フォールバック: {primary_ip}応答なし]"),
                    elapsed_ms,
                )
            }
            FallbackOutcome::FailedBoth {
                primary_err,
                elapsed_ms,
                ..
            } => {
                let sec_ip = secondary.unwrap_or(primary_ip);
                (
                    false,
                    format!("{primary_ip} および {sec_ip} 応答なし ({primary_err})"),
                    elapsed_ms,
                )
            }
            FallbackOutcome::FailedSingle { err, elapsed_ms } => {
                (false, format!("{primary_ip} 応答なし ({err})"), elapsed_ms)
            }
        };

    (ok, detail, pre_resolve_ms + elapsed_ms)
}
