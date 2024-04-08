use crate::tlv::TlvType;

#[derive(Debug)]
pub enum Capability {
    Other = 1,
    Repeater = 2,
    Bridge = 4,
    WlanAP = 8,
    Router = 16,
    Telephone = 32,
    DocsisDevice = 64,
    StationOnly = 128,
    CVlanComponent = 256,
    SVlanComponent = 512,
    TwoPortMacRelay = 1024,
}

#[derive(Debug)]
pub struct SystemCapabilities {
    pub tlv_type: TlvType,
    pub len: u16,
    pub value: Value,
}

impl SystemCapabilities {
    pub fn parser(_len: u16, value: &[u8]) -> Self {
        Self {
            tlv_type: TlvType::SystemCapabilities,
            len: 4,
            value: Value {
                caps: u16::from_be_bytes(value[..2].try_into().unwrap()),
                enabled_caps: u16::from_be_bytes(value[2..4].try_into().unwrap()),
            }
        }
    }
}

#[derive(Debug)]
pub struct Value {
    pub caps: u16,
    pub enabled_caps: u16,
}
