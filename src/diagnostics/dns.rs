use std::net::{IpAddr, Ipv4Addr, SocketAddr, ToSocketAddrs};
use std::time::Instant;

use super::{FallbackOutcome, execute_with_fallback};

/// SocketAddrイテレータから最初のIPv4アドレスを抽出する純粋関数
fn select_ipv4(addrs: impl IntoIterator<Item = SocketAddr>) -> Option<Ipv4Addr> {
    addrs.into_iter().find_map(|sa| match sa.ip() {
        IpAddr::V4(v4) => Some(v4),
        IpAddr::V6(_) => None,
    })
}

/// SocketAddrイテレータからIPv4優先・IPv6フォールバックでアドレスを選択する純粋関数
fn select_preferred_ip(addrs: impl IntoIterator<Item = SocketAddr>) -> Option<IpAddr> {
    let mut first_ip: Option<IpAddr> = None;
    for sa in addrs {
        let ip = sa.ip();
        if ip.is_ipv4() {
            return Some(ip);
        }
        if first_ip.is_none() {
            first_ip = Some(ip);
        }
    }
    first_ip
}

/// ホスト名からIPv4アドレスを優先抽出して解決する
pub(super) fn resolve_ipv4_host(host: &str) -> Result<(Ipv4Addr, u64), (String, u64)> {
    let start = Instant::now();
    if let Ok(ipv4) = host.parse::<Ipv4Addr>() {
        return Ok((ipv4, 0));
    }
    match (host, 80).to_socket_addrs() {
        Ok(addrs) => {
            let elapsed = start.elapsed().as_millis() as u64;
            if let Some(ipv4) = select_ipv4(addrs) {
                Ok((ipv4, elapsed))
            } else {
                Err(("IPv4アドレスが見つかりませんでした".to_string(), elapsed))
            }
        }
        Err(e) => {
            let elapsed = start.elapsed().as_millis() as u64;
            Err((e.to_string(), elapsed))
        }
    }
}

/// 単一ホストの名前解決を実行（IPv4優先、IPv6フォールバック）
fn check_single_dns_resolution(host: &str) -> Result<(IpAddr, u64), (String, u64)> {
    let start = Instant::now();
    match (host, 80).to_socket_addrs() {
        Ok(addrs) => {
            let elapsed = start.elapsed().as_millis() as u64;
            if let Some(ip) = select_preferred_ip(addrs) {
                Ok((ip, elapsed))
            } else {
                Err(("アドレス解決結果が空でした".to_string(), elapsed))
            }
        }
        Err(e) => {
            let elapsed = start.elapsed().as_millis() as u64;
            Err((e.to_string(), elapsed))
        }
    }
}

/// ステップ4: 名前解決 (DNS) 検証
pub fn check_dns_resolution(
    primary_host: &str,
    secondary_host: Option<&str>,
) -> (bool, String, u64) {
    match execute_with_fallback(primary_host, secondary_host, check_single_dns_resolution) {
        FallbackOutcome::PrimaryOk {
            val: ip,
            elapsed_ms,
        } => (
            true,
            format!("{primary_host} 解決成功 (IP: {ip})"),
            elapsed_ms,
        ),
        FallbackOutcome::SecondaryOk {
            val: ip,
            elapsed_ms,
        } => {
            let sec_host = secondary_host.unwrap_or_default();
            (
                true,
                format!("{sec_host} 解決成功 (IP: {ip}) [フォールバック: {primary_host}解決失敗]"),
                elapsed_ms,
            )
        }
        FallbackOutcome::FailedBoth {
            primary_err,
            elapsed_ms,
            ..
        } => {
            let sec_host = secondary_host.unwrap_or_default();
            (
                false,
                format!("{primary_host} / {sec_host} ともに解決失敗 ({primary_err})"),
                elapsed_ms,
            )
        }
        FallbackOutcome::FailedSingle { err, elapsed_ms } => (
            false,
            format!("{primary_host} 解決失敗 ({err})"),
            elapsed_ms,
        ),
    }
}

#[cfg(test)]
mod tests {
    use std::net::{Ipv6Addr, SocketAddrV4, SocketAddrV6};

    use super::*;

    fn v4_addr(octets: [u8; 4]) -> SocketAddr {
        SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::from(octets), 80))
    }

    fn v6_addr(segments: [u16; 8]) -> SocketAddr {
        SocketAddr::V6(SocketAddrV6::new(Ipv6Addr::from(segments), 80, 0, 0))
    }

    #[test]
    fn test_select_ipv4() {
        let addrs = [
            v6_addr([0x2001, 0xdb8, 0, 0, 0, 0, 0, 1]),
            v4_addr([8, 8, 8, 8]),
        ];
        assert_eq!(select_ipv4(addrs), Some(Ipv4Addr::new(8, 8, 8, 8)));

        let v6_only = [v6_addr([0x2001, 0xdb8, 0, 0, 0, 0, 0, 1])];
        assert_eq!(select_ipv4(v6_only), None);
        assert_eq!(select_ipv4([]), None);
    }

    #[test]
    fn test_select_preferred_ip() {
        let mixed = [
            v6_addr([0x2001, 0xdb8, 0, 0, 0, 0, 0, 1]),
            v4_addr([1, 1, 1, 1]),
        ];
        assert_eq!(
            select_preferred_ip(mixed),
            Some(IpAddr::V4(Ipv4Addr::new(1, 1, 1, 1)))
        );

        let v6_only = [v6_addr([0x2001, 0xdb8, 0, 0, 0, 0, 0, 2])];
        assert_eq!(
            select_preferred_ip(v6_only),
            Some(IpAddr::V6(Ipv6Addr::new(0x2001, 0xdb8, 0, 0, 0, 0, 0, 2)))
        );

        assert_eq!(select_preferred_ip([]), None);
    }

    #[test]
    fn test_resolve_ipv4_host_literal() {
        let res = resolve_ipv4_host("192.168.10.1");
        assert_eq!(res, Ok((Ipv4Addr::new(192, 168, 10, 1), 0)));
    }
}
