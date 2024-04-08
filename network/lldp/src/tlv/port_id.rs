use std::net::IpAddr;

use crate::tlv::TlvType;

#[derive(Debug)]
pub struct PortId {
    pub tlv_type: TlvType,
    pub len: u16,
    pub subtype: SubType,
    pub value: Value,
}

impl PortId {
    pub fn parser(len: u16, value: &[u8]) -> Self {
        let subtype = SubType::from(value[0]);
        let value = match subtype {
            SubType::Mac => Value::Mac(value[1..7].try_into().unwrap()),
            SubType::NetworkAddress => match value[1] {
                1_u8 => {
                    let addr: [u8; 4] = value[2..6].try_into().unwrap();
                    Value::Ip(IpAddr::from(addr))
                },
                2_u8 => {
                    let addr: [u8; 16] = value[2..18].try_into().unwrap();
                    Value::Ip(IpAddr::from(addr))
                },
                _ => Value::Str(String::from_utf8_lossy(&value[2..len as usize]).to_string()),
            },
            _ => Value::Str(String::from_utf8_lossy(&value[1..len as usize]).to_string()),
        };

        PortId {
            tlv_type: TlvType::PortId,
            len: len,
            subtype: subtype,
            value: value,
        }
    }
}

#[repr(u8)]
#[derive(Debug)]
pub enum SubType {
    InterfaceAlias = 1,
    Port = 2,
    Mac = 3,
    NetworkAddress = 4,
    InterfaceName = 5,
    CircuitId = 6,
    Local = 7,
    Unknown(u8),
}

impl From<u8> for SubType {
    fn from(v: u8) -> Self {
        match v {
            1 => Self::InterfaceAlias,
            2 => Self::Port,
            3 => Self::Mac,
            4 => Self::NetworkAddress,
            5 => Self::InterfaceName,
            6 => Self::CircuitId,
            7 => Self::Local,
            n => Self::Unknown(n),
        }
    }
}

#[derive(Debug)]
pub enum Value {
    Mac([u8; 6]),
    Ip(IpAddr),
    Str(String),
}
