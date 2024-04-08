use crate::tlv::TlvType;

#[derive(Debug)]
pub struct SystemName {
    pub tlv_type: TlvType,
    pub len: u16,
    pub value: String,
}

impl SystemName {
    pub fn parser(len: u16, value: &[u8]) -> Self {
        Self {
            tlv_type: TlvType::SystemName,
            len: len,
            value: String::from_utf8_lossy(&value[..len as usize]).to_string(),
        }
    }
}
