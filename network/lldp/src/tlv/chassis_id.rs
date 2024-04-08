use std::net::IpAddr;

use crate::tlv::TlvType;

#[derive(Debug)]
pub struct ChassisId {
    pub tlv_type: TlvType,
    pub len: u16,
    pub subtype: SubType,
    pub value: Value,
}

impl ChassisId {
    pub fn parser(len: u16, value: &[u8]) -> Self {
        let subtype = SubType::from(value[0]);
        let value = match subtype {
            SubType::Mac => Value::Mac(value[..6].try_into().unwrap()),
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

        ChassisId {
            tlv_type: TlvType::ChassisId,
            len: len,
            subtype: subtype,
            value: value,
        }
    }
}

#[repr(u8)]
#[derive(Debug)]
pub enum SubType {
    Chassis = 1,
    InterfaceAlias = 2,
    Port = 3,
    Mac = 4,
    NetworkAddress = 5,
    InterfaceName = 6,
    Local = 7,
    Unknown(u8),
}

impl From<u8> for SubType {
    fn from(v: u8) -> Self {
        match v {
            1 => Self::Chassis,
            2 => Self::InterfaceAlias,
            3 => Self::Port,
            4 => Self::Mac,
            5 => Self::NetworkAddress,
            6 => Self::InterfaceName,
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
