use crate::tlv::Tlv;
use crate::tlv::TlvType;
use crate::tlv::chassis_id::ChassisId;
use crate::tlv::port_id::PortId;
use crate::tlv::ttl::Ttl;
use crate::tlv::port_description::PortDescription;
use crate::tlv::sys_name::SystemName;
use crate::tlv::sys_description::SystemDescription;
use crate::tlv::capabilities::SystemCapabilities;
use crate::tlv::management_address::ManagementAddress;
use crate::tlv::org_specific::OrganizationSpecific;
use crate::tlv::end_pdu::EndOfPdu;
use crate::tlv::reserved::Reserved;

#[derive(Debug)]
pub struct Lldpdu {
    pub tlvs: Vec<Tlv>,
}

#[derive(PartialEq, Debug)]
pub enum ParserError {
    NotLLDP,
    WrongLength,
}

const ETH_LEN: usize = 14;
const ETH_TYPE_LEN: usize = 2;

const LLDP: EtherType = EtherType(0x88cc);

struct EtherType(u16);

impl PartialEq<&[u8]> for EtherType {
    fn eq(&self, other: &&[u8]) -> bool {
        match other.len() {
            ETH_TYPE_LEN =>  {
                self.0 == u16::from_be_bytes(other[..2].try_into().unwrap())
            },
            _ => false,
        }
    }
}

impl PartialEq<EtherType> for &[u8] {
    fn eq(&self, other: &EtherType) -> bool {
        match self.len() {
            ETH_TYPE_LEN =>  {
                u16::from_be_bytes(self[..2].try_into().unwrap()) == other.0
            },
            _ => false,
        }
    }
}

impl Lldpdu {
    /// Parse from an ethernet frame into Lldpdu
    ///
    /// Will check the sequence of TLV.
    /// If parse failed, an error message will be returned.
    pub fn parser(frame: &[u8]) -> Result<Self, ParserError> {
        if Self::is_lldp(frame) {
            Self::from_bytes(&frame[14..])
        } else {
            Err(ParserError::NotLLDP)
        }
    }

    /// LLDP packet type checking
    ///
    /// This should be called before `from_bytes`.
    /// It already be called by `parser`.
    pub fn is_lldp(frame: &[u8]) -> bool {
        match frame.len() {
            ETH_LEN.. => &frame[12..14] == LLDP,
            _ => false,
        }
    }

    /// Parse from an ethernet frame payload into Lldpdu
    ///
    /// Will check the sequence of TLV.
    /// If parse failed, an error message will be returned.
    /// It will not check ethernet type since it already be a payload.
    /// Please make sure it is a LLDP PDU before call this function.
    pub fn from_bytes(payload: &[u8]) -> Result<Self, ParserError> {
        let mut lldpdu = Lldpdu { tlvs: vec![] };

        let mut pos = 0;

        while pos + 1 < payload.len() {
            // First 7 bits are type and last 9 bits are length
            let lldp_type = (payload[pos] & 0b11111110) >> 1;
            let length = (((payload[pos] & 1) as u16) << 9) + payload[pos + 1] as u16;

            if payload.len() < pos + 2 + length as usize {
                return Err(ParserError::WrongLength);
            }

            let value = &payload[pos+2..];
            
            let tlv = match TlvType::from(lldp_type) {
                TlvType::ChassisId => Tlv::ChassisId(ChassisId::parser(length, value)),
                TlvType::PortId => Tlv::PortId(PortId::parser(length, value)),
                TlvType::Ttl => Tlv::Ttl(Ttl::parser(length, value)),
                TlvType::PortDescription => Tlv::PortDescription(PortDescription::parser(length, value)),
                TlvType::SystemName => Tlv::SystemName(SystemName::parser(length, value)),
                TlvType::SystemDescription => Tlv::SystemDescription(SystemDescription::parser(length, value)),
                TlvType::SystemCapabilities => Tlv::Capabilities(SystemCapabilities::parser(length, value)),
                TlvType::ManagementAddress => Tlv::ManagementAddress(ManagementAddress::parser(length, value)),
                TlvType::OrganizationSpecific => Tlv::OrganizationSpecific(OrganizationSpecific::parser(length, value)),
                TlvType::EndOfLLDPDU => Tlv::EndOfPdu(EndOfPdu::new()),
                TlvType::Reserved(t) => Tlv::Reserved(Reserved::parser(t, length, value)),
            };

            lldpdu.tlvs.push(tlv);

            pos += 2 + length as usize;
        }
        Ok(lldpdu)
    }
}
