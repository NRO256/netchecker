use std::net::Ipv4Addr;

/// 各診断項目の実行結果
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StepResult {
    pub name: &'static str,
    pub success: bool,
    pub detail: String,
    pub elapsed_ms: u64,
}

/// 検出されたネットワークアダプタ情報
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AdapterInfo {
    pub friendly_name: String,
    pub ip_addr: Ipv4Addr,
    pub gateway_addr: Option<Ipv4Addr>,
    pub dns_servers: Vec<Ipv4Addr>,
    pub link_speed_mbps: Option<u64>,
}
