use crate::tlv::TlvType;

#[derive(Debug)]
pub struct OrganizationSpecific {
    pub tlv_type: TlvType,
    pub len: u16,
    //pub subtype: SubType,
    //pub value: Value,
}

impl OrganizationSpecific {
    pub fn parser(len: u16, _value: &[u8]) -> Self {
        Self {
            tlv_type: TlvType::OrganizationSpecific,
            len: len,
        }
    }
}
