use pnet::datalink::MacAddr;
use pnet::datalink::NetworkInterface;
use pnet::packet::ethernet::MutableEthernetPacket;
use pnet::packet::ethernet::EthernetPacket;
use pnet::packet::ethernet::EtherTypes;
use pnet::packet::vlan::MutableVlanPacket;
use pnet::packet::vlan::VlanPacket;
use pnet::packet::arp::MutableArpPacket;
use pnet::packet::arp::ArpPacket;
//use pnet::packet::arp::ArpOperations;
use pnet::packet::arp::ArpOperation;
use pnet::packet::arp::ArpHardwareTypes;
use pnet::packet::PacketSize;
use pnet::packet::MutablePacket;


pub fn builder(
        buf: &mut [u8],
        interface: &NetworkInterface,
        dst_mac: MacAddr,
        vlan: u16,
        hostname: &str
    ) -> usize {
   let mut eth = MutableEthernetPacket::new(buf).unwrap();
   let src_mac = interface.mac.unwrap();

   // encap ethernet header
   eth.set_destination(dst_mac);
   eth.set_source(MacAddr::from(src_mac));
   let mut len = eth.packet_size();

   //let mut ethernet_packet = MutableVlanPacket::new(&mut ethernet_buffer[14..]).unwrap();
   // vlan 0 means no vlan at all
   if vlan != 0 {
       eth.set_ethertype(EtherTypes::Vlan);
       let mut packet = MutableVlanPacket::new(eth.payload_mut()).unwrap();
       packet.set_vlan_identifier(vlan);
       packet.set_ethertype(EtherTypes::Rarp);
       len += packet.packet_size();
   } else {
       eth.set_ethertype(EtherTypes::Rarp);
   }

   // encap self information to RARP packet
   let mut rarp = MutableArpPacket::new(&mut buf[len..]).unwrap();
   rarp.set_hardware_type(ArpHardwareTypes::Ethernet);
   rarp.set_protocol_type(EtherTypes::Ipv4);
   rarp.set_hw_addr_len(6);
   rarp.set_proto_addr_len(4);
   if dst_mac.is_broadcast() {
       //rarp.set_operation(ArpOperations::Request);
       rarp.set_operation(ArpOperation(3));
   } else {
       //rarp.set_operation(ArpOperations::Reply);
       rarp.set_operation(ArpOperation(4));
   }
   len += rarp.packet_size();
   //if dst_mac.is_broadcast() {
   //    buf[len-21] = 3;
   //} else {
   //    buf[len-21] = 4;
   //}

   let start = len;
   // fill interface name
   let interface = &interface.name;
   let mut payload: Vec<u8> = Vec::from([interface.len() as u8]);
   payload.extend_from_slice(interface.as_bytes());
   len += 1 + interface.len();

   // fill hostname
   payload.insert(payload.len(), hostname.len() as u8);
   payload.extend_from_slice(hostname.as_bytes());
   len += 1 + hostname.len();

   // fill vlan since pnet will receive packet without vlan tag - maybe a bug
   for v in vlan.to_be_bytes() {
       payload.insert(payload.len(), v);
       len += 1;
   }

   // add payload to packet
   let end = len;
   buf[start..end].copy_from_slice(&payload);

   len
}

#[derive(Debug, Clone)]
pub struct Peer {
    pub host: String,
    pub nic: String,
    pub mac: MacAddr,
    pub vlan: u16,
}

pub fn parse(buf: &[u8]) -> Option<(Peer, bool)> {
    let packet = EthernetPacket::new(buf).unwrap();
    let mac = packet.get_source();
    let mut offset = packet.packet_size();

    let (_vlan, ether_type) = if packet.get_ethertype() == EtherTypes::Vlan {
        let vlan = VlanPacket::new(&buf[offset..]).unwrap();
        offset += vlan.packet_size();
        (vlan.get_vlan_identifier(), vlan.get_ethertype())
    } else {
        (0, packet.get_ethertype())
    };

    if ether_type == EtherTypes::Rarp {
        let rarp = ArpPacket::new(&buf[offset..]).unwrap();
        //let request = if rarp.get_operation() == ArpOperations::Request {
        let request = if rarp.get_operation() == ArpOperation(3) {
            // should reply with unicast
            true
        } else {
            false
        };
        offset += rarp.packet_size();

        // interface
        let len = buf[offset] as usize;
        let start = offset + 1;
        let end = start + len;
        let nic = String::from_utf8_lossy(&buf[start..end]).to_string();

        // hostname
        offset = end;
        let len = buf[offset] as usize;
        let start = offset + 1;
        let end = start + len;
        let host = String::from_utf8_lossy(&buf[start..end]).to_string();

        // vlan
        offset = end;
        let v: &[u8; 2] = &buf[offset..offset + 2].try_into().unwrap();
        let vlan = u16::from_be_bytes(*v);

        Some((
            Peer {
                host,
                nic,
                mac,
                vlan,
            },
            request
            ))

    } else {
        None
    }
}

