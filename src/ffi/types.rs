#![allow(clippy::upper_case_acronyms)]

use std::ffi::c_void;

#[repr(C)]
#[derive(Default, Clone, Copy, Debug)]
pub struct SYSTEMTIME {
    pub w_year: u16,
    pub w_month: u16,
    pub w_day_of_week: u16,
    pub w_day: u16,
    pub w_hour: u16,
    pub w_minute: u16,
    pub w_second: u16,
    pub w_milliseconds: u16,
}

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct SOCKET_ADDRESS {
    pub lp_sockaddr: *mut SOCKADDR,
    pub i_sockaddr_length: i32,
}

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct SOCKADDR {
    pub sa_family: u16,
    pub sa_data: [u8; 14],
}

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct SOCKADDR_IN {
    pub sin_family: u16,
    pub sin_port: u16,
    pub sin_addr: u32,
    pub sin_zero: [u8; 8],
}

#[repr(C)]
pub struct IP_ADAPTER_UNICAST_ADDRESS {
    pub length: u32,
    pub flags: u32,
    pub next: *mut IP_ADAPTER_UNICAST_ADDRESS,
    pub address: SOCKET_ADDRESS,
}

#[repr(C)]
pub struct IP_ADAPTER_GATEWAY_ADDRESS {
    pub length: u32,
    pub reserved: u32,
    pub next: *mut IP_ADAPTER_GATEWAY_ADDRESS,
    pub address: SOCKET_ADDRESS,
}

#[repr(C)]
pub struct IP_ADAPTER_DNS_SERVER_ADDRESS {
    pub length: u32,
    pub reserved: u32,
    pub next: *mut IP_ADAPTER_DNS_SERVER_ADDRESS,
    pub address: SOCKET_ADDRESS,
}

#[repr(C)]
pub struct IP_ADAPTER_ADDRESSES {
    pub length: u32,
    pub if_index: u32,
    pub next: *mut IP_ADAPTER_ADDRESSES,
    pub adapter_name: *mut i8,
    pub first_unicast_address: *mut IP_ADAPTER_UNICAST_ADDRESS,
    pub first_anycast_address: *mut c_void,
    pub first_multicast_address: *mut c_void,
    pub first_dns_server_address: *mut IP_ADAPTER_DNS_SERVER_ADDRESS,
    pub dns_suffix: *mut u16,
    pub description: *mut u16,
    pub friendly_name: *mut u16,
    pub physical_address: [u8; 8],
    pub physical_address_length: u32,
    pub flags: u32,
    pub mtu: u32,
    pub if_type: u32,
    pub oper_status: u32,
    pub ipv6_if_index: u32,
    pub zone_indices: [u32; 16],
    pub first_prefix: *mut c_void,
    pub transmit_link_speed: u64,
    pub receive_link_speed: u64,
    pub first_wins_server_address: *mut c_void,
    pub first_gateway_address: *mut IP_ADAPTER_GATEWAY_ADDRESS,
}

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct IP_OPTION_INFORMATION {
    pub ttl: u8,
    pub tos: u8,
    pub flags: u8,
    pub options_size: u8,
    pub options_data: *mut u8,
}

impl Default for IP_OPTION_INFORMATION {
    fn default() -> Self {
        Self {
            ttl: 0,
            tos: 0,
            flags: 0,
            options_size: 0,
            options_data: std::ptr::null_mut(),
        }
    }
}

#[repr(C)]
pub struct ICMP_ECHO_REPLY {
    pub address: u32,
    pub status: u32,
    pub round_trip_time: u32,
    pub data_size: u16,
    pub reserved: u16,
    pub data: *mut c_void,
    pub options: IP_OPTION_INFORMATION,
}

impl Default for ICMP_ECHO_REPLY {
    fn default() -> Self {
        Self {
            address: 0,
            status: 0,
            round_trip_time: 0,
            data_size: 0,
            reserved: 0,
            data: std::ptr::null_mut(),
            options: IP_OPTION_INFORMATION::default(),
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct INTERNET_PROXY_INFO {
    pub dw_access_type: u32,
    pub lpsz_proxy: *const i8,
    pub lpsz_proxy_bypass: *const i8,
}
