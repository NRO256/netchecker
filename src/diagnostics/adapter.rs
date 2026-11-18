use std::fmt::Write;
use std::net::Ipv4Addr;
use std::ptr;
use std::time::Instant;

use crate::cli::PRIMARY_PING_TARGET;
use crate::ffi::error::get_win32_error_detail;
use crate::ffi::types::{IP_ADAPTER_ADDRESSES, SOCKADDR_IN, SOCKET_ADDRESS};
use crate::ffi::{
    AF_INET, AF_UNSPEC, ERROR_BUFFER_OVERFLOW, GAA_FLAG_INCLUDE_GATEWAYS, GetAdaptersAddresses,
    GetBestInterface, IF_OPER_STATUS_UP, IF_TYPE_SOFTWARE_LOOPBACK, NO_ERROR, pwstr_to_string,
};

use super::types::AdapterInfo;

/// SOCKET_ADDRESS 構造体から IPv4 アドレスを安全に抽出する
unsafe fn extract_ipv4_from_socket_address(sa: &SOCKET_ADDRESS) -> Option<Ipv4Addr> {
    if sa.lp_sockaddr.is_null() || sa.i_sockaddr_length < std::mem::size_of::<SOCKADDR_IN>() as i32
    {
        return None;
    }
    let family = unsafe { (*sa.lp_sockaddr).sa_family };
    if family == AF_INET {
        let sin = unsafe { *(sa.lp_sockaddr as *const SOCKADDR_IN) };
        Some(Ipv4Addr::from(u32::from_be(sin.sin_addr)))
    } else {
        None
    }
}

/// アダプタのビットレート（bps）から Mbps 単位のリンク速度を算出（不明時は None）
fn calc_link_speed_mbps(bps: u64) -> Option<u64> {
    (bps < u64::MAX)
        .then_some(bps / 1_000_000)
        .filter(|&mbps| mbps > 0)
}

/// アダプタ情報の表示文字列を組み立てる
fn format_adapter_detail(ad: &AdapterInfo) -> String {
    let mut out = format!("{} 接続中: {}", ad.friendly_name, ad.ip_addr);
    let has_speed = ad.link_speed_mbps.is_some();
    let has_dns = !ad.dns_servers.is_empty();

    if has_speed || has_dns {
        out.push_str(" [");
        if let Some(mbps) = ad.link_speed_mbps {
            let _ = write!(out, "速度: {mbps}Mbps");
        }
        if has_dns {
            if has_speed {
                out.push_str(", ");
            }
            out.push_str("DNS: ");
            for (i, dns) in ad.dns_servers.iter().enumerate() {
                if i > 0 {
                    out.push_str(", ");
                }
                let _ = write!(out, "{dns}");
            }
        }
        out.push(']');
    }
    out
}

/// 指定宛先への最適インターフェースインデックスを取得
fn get_best_interface_index(dest: Ipv4Addr) -> Option<u32> {
    let dest_u32 = u32::from_ne_bytes(dest.octets());
    let mut best_index: u32 = 0;
    let ret = unsafe { GetBestInterface(dest_u32, &mut best_index) };
    (ret == NO_ERROR).then_some(best_index)
}

/// GetAdaptersAddresses をリサイズリトライ付きで取得する（8バイトアライン済みバッファ）
fn fetch_adapter_addresses_buffer() -> Result<Vec<u64>, u32> {
    let mut buf_size: u32 = 15_000;
    let mut buffer: Vec<u64> = vec![0; (buf_size as usize).div_ceil(8)];
    let mut last_err = NO_ERROR;

    for _ in 0..2 {
        let ret = unsafe {
            GetAdaptersAddresses(
                AF_UNSPEC,
                GAA_FLAG_INCLUDE_GATEWAYS,
                ptr::null_mut(),
                buffer.as_mut_ptr() as *mut IP_ADAPTER_ADDRESSES,
                &mut buf_size,
            )
        };
        if ret == NO_ERROR {
            return Ok(buffer);
        }
        last_err = ret;
        if ret != ERROR_BUFFER_OVERFLOW {
            break;
        }
        buffer.resize((buf_size as usize).div_ceil(8), 0);
    }

    Err(last_err)
}

unsafe fn find_valid_unicast_ipv4(adapter: &IP_ADAPTER_ADDRESSES) -> Option<Ipv4Addr> {
    let mut unicast = adapter.first_unicast_address;
    while !unicast.is_null() {
        let u = unsafe { &*unicast };
        if let Some(ip) = unsafe { extract_ipv4_from_socket_address(&u.address) }
            && !ip.is_link_local()
            && !ip.is_loopback()
            && !ip.is_unspecified()
        {
            return Some(ip);
        }
        unicast = u.next;
    }
    None
}

unsafe fn find_valid_gateway_ipv4(adapter: &IP_ADAPTER_ADDRESSES) -> Option<Ipv4Addr> {
    let mut gateway = adapter.first_gateway_address;
    while !gateway.is_null() {
        let g = unsafe { &*gateway };
        if let Some(gw_ip) = unsafe { extract_ipv4_from_socket_address(&g.address) }
            && !gw_ip.is_unspecified()
        {
            return Some(gw_ip);
        }
        gateway = g.next;
    }
    None
}

unsafe fn collect_dns_servers(adapter: &IP_ADAPTER_ADDRESSES) -> Vec<Ipv4Addr> {
    let mut dns_servers = Vec::new();
    let mut dns_ptr = adapter.first_dns_server_address;
    while !dns_ptr.is_null() {
        let d = unsafe { &*dns_ptr };
        if let Some(dns_ip) = unsafe { extract_ipv4_from_socket_address(&d.address) }
            && !dns_ip.is_unspecified()
            && !dns_ip.is_loopback()
            && !dns_servers.contains(&dns_ip)
        {
            dns_servers.push(dns_ip);
        }
        dns_ptr = d.next;
    }
    dns_servers
}

struct AdapterCandidate {
    ptr: *const IP_ADAPTER_ADDRESSES,
    ip_addr: Ipv4Addr,
    gateway_addr: Option<Ipv4Addr>,
}

/// ステップ1: ネットワークアダプタ検証
pub fn check_network_adapter(
    probe_dest: Option<Ipv4Addr>,
) -> (bool, String, Option<AdapterInfo>, u64) {
    let start = Instant::now();
    let buffer = match fetch_adapter_addresses_buffer() {
        Ok(buf) => buf,
        Err(err_code) => {
            let elapsed = start.elapsed().as_millis() as u64;
            let err_detail = get_win32_error_detail(err_code);
            return (
                false,
                format!("アダプタ情報の取得に失敗しました ({err_detail})"),
                None,
                elapsed,
            );
        }
    };

    let dest_for_best = probe_dest.unwrap_or(PRIMARY_PING_TARGET);
    let best_if_index = get_best_interface_index(dest_for_best);

    let mut current = buffer.as_ptr() as *const IP_ADAPTER_ADDRESSES;
    let mut best_match: Option<AdapterCandidate> = None;
    let mut gateway_match: Option<AdapterCandidate> = None;
    let mut fallback_match: Option<AdapterCandidate> = None;

    while !current.is_null() {
        let adapter = unsafe { &*current };
        current = adapter.next;

        if adapter.oper_status != IF_OPER_STATUS_UP || adapter.if_type == IF_TYPE_SOFTWARE_LOOPBACK
        {
            continue;
        }

        let Some(ip_addr) = (unsafe { find_valid_unicast_ipv4(adapter) }) else {
            continue;
        };
        let gateway_addr = unsafe { find_valid_gateway_ipv4(adapter) };
        let candidate = AdapterCandidate {
            ptr: adapter,
            ip_addr,
            gateway_addr,
        };

        // 最適インターフェースが判明した場合は優先確定し、残りの走査・文字列生成を省略する
        if best_if_index == Some(adapter.if_index) {
            best_match = Some(candidate);
            break;
        }
        if gateway_addr.is_some() {
            if gateway_match.is_none() {
                gateway_match = Some(candidate);
            }
        } else if gateway_match.is_none() && fallback_match.is_none() {
            fallback_match = Some(candidate);
        }
    }

    let chosen_info = best_match
        .or(gateway_match)
        .or(fallback_match)
        .map(|chosen| {
            let adapter = unsafe { &*chosen.ptr };
            let link_speed_mbps = calc_link_speed_mbps(adapter.transmit_link_speed)
                .or_else(|| calc_link_speed_mbps(adapter.receive_link_speed));
            AdapterInfo {
                friendly_name: unsafe { pwstr_to_string(adapter.friendly_name) },
                ip_addr: chosen.ip_addr,
                gateway_addr: chosen.gateway_addr,
                dns_servers: unsafe { collect_dns_servers(adapter) },
                link_speed_mbps,
            }
        });

    let elapsed = start.elapsed().as_millis() as u64;

    if let Some(ad) = chosen_info {
        let detail = format_adapter_detail(&ad);
        (true, detail, Some(ad), elapsed)
    } else {
        (
            false,
            "有効なネットワーク接続が見つかりません (未接続またはAPIPA)".to_string(),
            None,
            elapsed,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_calc_link_speed_mbps() {
        assert_eq!(calc_link_speed_mbps(1_000_000_000), Some(1000));
        assert_eq!(calc_link_speed_mbps(100_000_000), Some(100));
        assert_eq!(calc_link_speed_mbps(0), None);
        assert_eq!(calc_link_speed_mbps(u64::MAX), None);
    }

    #[test]
    fn test_format_adapter_detail() {
        let ad_full = AdapterInfo {
            friendly_name: "イーサネット".to_string(),
            ip_addr: Ipv4Addr::new(192, 168, 1, 10),
            gateway_addr: Some(Ipv4Addr::new(192, 168, 1, 1)),
            dns_servers: vec![Ipv4Addr::new(192, 168, 1, 1), Ipv4Addr::new(8, 8, 8, 8)],
            link_speed_mbps: Some(1000),
        };
        assert_eq!(
            format_adapter_detail(&ad_full),
            "イーサネット 接続中: 192.168.1.10 [速度: 1000Mbps, DNS: 192.168.1.1, 8.8.8.8]"
        );

        let ad_minimal = AdapterInfo {
            friendly_name: "Wi-Fi".to_string(),
            ip_addr: Ipv4Addr::new(10, 0, 0, 5),
            gateway_addr: None,
            dns_servers: vec![],
            link_speed_mbps: None,
        };
        assert_eq!(format_adapter_detail(&ad_minimal), "Wi-Fi 接続中: 10.0.0.5");
    }
}
