use pnet::datalink;
use pnet::datalink::Channel::Ethernet;

use lldp::Tlv;
use lldp::Lldpdu;

use std::fs;

fn get_physical_nics() -> Vec<String> {
    const NIC_PATH: &str = "/sys/class/net/";
    let mut nics: Vec<_> = vec![];

    if let Ok(dir) = fs::read_dir(NIC_PATH) {
        for entry in dir {
            let entry = entry.unwrap();
            if let Ok(path) = fs::read_link(entry.path()) {
                if !path.starts_with("../../devices/virtual/") {
                    nics.push(entry.file_name().into_string().unwrap());
                }
            }
        }
    }
    nics
}

#[tokio::main]
async fn main() -> std::io::Result<()> {
    //let ifname = std::env::args().nth(1).unwrap_or_else(|| "lo".into());
    let mut handles = Vec::new();
    let nics = get_physical_nics();
    for ifname in nics {
        let interface = datalink::interfaces()
            .into_iter()
            .find(|iface| iface.name == *ifname)
            .unwrap_or_else(|| panic!("Interface {} is not present.", ifname));
        handles.push(tokio::spawn(async move {
            let config = datalink::Config {
                promiscuous: false,
                ..Default::default()
            };
            let (_tx, mut rx) = match datalink::channel(&interface, config) {
                Ok(Ethernet(tx, rx)) => (tx, rx),
                _ => panic!("Failed to create network channel"),
            };

            println!("Channel establised for {}", interface.name);
            loop {
                match rx.next() {
                    Ok(frame) => {
                        if let Ok(lldp) = Lldpdu::parser(frame) {
                            println!("Neighbor of interface {ifname}");
                            for tlv in lldp.tlvs {
                                //println!("    {:?}", tlv);
                                match tlv {
                                    Tlv::SystemName(tlv) => println!("  type: {:?}, value: {}", tlv.tlv_type, tlv.value),
                                    Tlv::PortId(tlv) => println!("  type: {:?}, value: {:?}", tlv.tlv_type, tlv.value),
                                    Tlv::PortDescription(tlv) => println!("  type: {:?}, value: {:?}", tlv.tlv_type, tlv.value),
                                    _ => (),
                                }
                            }
                        }
                    },
                    Err(_) => println!("Failed to receive packet"),
                };
            }
        }));
    }

    for task in handles {
        task.await?;
    }

    Ok(())

    //let interface = datalink::interfaces()
    //    .into_iter()
    //    .find(|iface| iface.name == *ifname)
    //    .unwrap_or_else(|| panic!("Interface {} is not present.", ifname));

    //let (_tx, mut rx) = match datalink::channel(&interface, Default::default()) {
    //    Ok(Ethernet(tx, rx)) => (tx, rx),
    //    _ => panic!("Failed to create network channel"),
    //};

    //loop {
    //    match rx.next() {
    //        Ok(frame) => {
    //            if let Ok(lldp) = Lldpdu::parser(frame) {
    //                println!("Neighbor of interface {ifname}");
    //                for tlv in lldp.tlvs {
    //                    //println!("    {:?}", tlv);
    //                    match tlv {
    //                        Tlv::SystemName(tlv) => println!("  type: {:?}, value: {}", tlv.tlv_type, tlv.value),
    //                        _ => (),
    //                    }
    //                }
    //                //println!("{:?}", lldp.unwrap().tlvs.find(|tlv|tlv.tlv_type == TlvType::SystemName));
    //            }
    //        },
    //        Err(_) => println!("Failed to receive packet"),
    //    };
    //}
    //println!("Hello, world!");
}
