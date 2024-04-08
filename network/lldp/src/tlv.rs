pub(crate) mod chassis_id;
pub(crate) mod port_id;
pub(crate) mod ttl;
pub(crate) mod port_description;
pub(crate) mod sys_name;
pub(crate) mod sys_description;
pub(crate) mod capabilities;
pub(crate) mod management_address;
pub(crate) mod org_specific;
pub(crate) mod end_pdu;
pub(crate) mod reserved;

#[derive(Debug)]
pub enum Tlv {
    ChassisId(chassis_id::ChassisId),
    PortId(port_id::PortId),
    Ttl(ttl::Ttl),
    PortDescription(port_description::PortDescription),
    SystemName(sys_name::SystemName),
    SystemDescription(sys_description::SystemDescription),
    Capabilities(capabilities::SystemCapabilities),
    ManagementAddress(management_address::ManagementAddress),
    OrganizationSpecific(org_specific::OrganizationSpecific),
    Reserved(reserved::Reserved),
    EndOfPdu(end_pdu::EndOfPdu),
}

#[repr(u8)]
#[derive(Debug)]
pub enum TlvType {
    EndOfLLDPDU = 0,
    ChassisId = 1,
    PortId = 2,
    Ttl = 3,
    PortDescription = 4,
    SystemName = 5,
    SystemDescription = 6,
    SystemCapabilities = 7,
    ManagementAddress = 8,
    OrganizationSpecific = 127,
    Reserved(u8),
}

impl From<u8> for TlvType {
    fn from(v: u8) -> Self {
        match v {
            0 => Self::EndOfLLDPDU,
            1 => Self::ChassisId,
            2 => Self::PortId,
            3 => Self::Ttl,
            4 => Self::PortDescription,
            5 => Self::SystemName,
            6 => Self::SystemDescription,
            7 => Self::SystemCapabilities,
            8 => Self::ManagementAddress,
            127 => Self::OrganizationSpecific,
            n => Self::Reserved(n),
        }
    }
}
