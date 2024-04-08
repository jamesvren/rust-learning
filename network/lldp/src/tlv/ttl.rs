use crate::tlv::TlvType;

#[derive(Debug)]
pub struct Ttl {
    pub tlv_type: TlvType,
    pub len: u16,
    pub value: u16,
}

impl Ttl {
    pub fn parser(_len: u16, value: &[u8]) -> Self {
        Self {
            tlv_type: TlvType::Ttl,
            len: 2,
            value: u16::from_be_bytes(value[..2].try_into().unwrap()),
        }
    }
}
