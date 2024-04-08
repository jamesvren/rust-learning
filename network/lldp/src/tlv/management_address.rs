use std::net::{IpAddr, Ipv4Addr};

use crate::tlv::TlvType;

#[derive(Debug)]
pub struct ManagementAddress {
    pub tlv_type: TlvType,
    pub len: u16,
    pub value: IpAddr,
}

impl ManagementAddress {
    pub fn parser(len: u16, value: &[u8]) -> Self {
        let _addr_len = value[0];
        let addr_type = value[1];
        let value = match addr_type {
            1_u8 => {
                let addr: [u8; 4] = value[2..6].try_into().unwrap();
                IpAddr::from(addr)
            },
            2_u8 => {
                let addr: [u8; 16] = value[2..18].try_into().unwrap();
                IpAddr::from(addr)
            },
            _ => IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0))
        };

        Self {
            tlv_type: TlvType::ManagementAddress,
            len: len,
            value: value,
        }
    }
}
