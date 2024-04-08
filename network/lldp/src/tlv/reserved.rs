use crate::tlv::TlvType;

#[derive(Debug)]
pub struct Reserved {
    pub tlv_type: TlvType,
    pub len: u16,
    pub value: Vec<u8>,
}

impl Reserved {
    pub fn parser(tlv_type: u8, len: u16, value: &[u8]) -> Self {
        Self {
            tlv_type: TlvType::Reserved(tlv_type),
            len: len,
            value: value[..len as usize].to_vec(),
        }
    }
}
