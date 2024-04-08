use crate::tlv::TlvType;

#[derive(Debug)]
pub struct PortDescription {
    pub tlv_type: TlvType,
    pub len: u16,
    pub value: String,
}

impl PortDescription {
    pub fn parser(len: u16, value: &[u8]) -> Self {
        Self {
            tlv_type: TlvType::PortDescription,
            len: len,
            value: String::from_utf8_lossy(&value[..len as usize]).to_string(),
        }
    }
}
