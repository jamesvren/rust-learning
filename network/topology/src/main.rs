//use std::collections::BTreeMap;
use tokio::sync::mpsc;
use tokio::time::{sleep, Duration};
use pnet::datalink;
//use pnet::datalink::MacAddr;
//use pnet::datalink::Channel::Ethernet;
//use pnet::datalink::NetworkInterface;
//use pnet::datalink::DataLinkSender;
//use pnet::datalink::DataLinkReceiver;
//use pnet::packet::ethernet::EtherTypes;
//use rustix::system::uname;
//use rustix::net::{socket, AddressFamily, SocketType, eth};
//use rustix::fd::IntoRawFd;
use clap::Parser;
use log::{warn, info, debug};

//use topology::MacAddress;
use topology::packet;
use topology::packet::Peer;
use topology::hostname;
use topology::get_physical_nics;
use topology::open_socket;
use topology::recv;
use topology::Interface;
use topology::send;
use topology::OperState;
use topology::Topo;
use topology::Node;

use std::collections::HashMap;
//use std::sync::{LazyLock, Mutex};

// key = (host, my_nic, my_mac, peer, peer_nic, peer_mac)
// value = vlans
//type Topo = BTreeMap<(String, String, MacAddress, String, String, MacAddress), Vec<u16>>;
//static TOPO: LazyLock<Mutex<Topo>> = LazyLock::new(Default::default);

#[derive(Parser, Debug)]
struct Opt {
    /// Vlan range used to detect, split by `-`. Untagged packet will always be sent.
    #[arg(short, long, value_parser=vlan_range)]
    vlan: Vec<(u16, u16)>,

    /// Nics used to detect, default are all UP nic
    #[arg(short, long)]
    interface: Option<Vec<String>>,
}

fn vlan_range(s: &str) -> Result<(u16, u16), String> {
    match s.split_once('-') {
        None => Err(format!("Vlan range format error. Should be like `2-4`")),
        Some((start, end)) => {
            let s: u16 = start.parse().map_err(|_| format!("`{start}` is not a number"))?;
            let e: u16 = end.parse().map_err(|_| format!("`{end}` is not a number"))?;
            if e < s {
                Err(format!("{e} is smaller than {s}"))
            } else {
                Ok((s, e))
            }
        }
    }
}


//async fn detect_connection(host: String, interface: NetworkInterface, vlan_range: Vec<(u16, u16)>) {
//    //let sock = socket(AddressFamily::PACKET, SocketType::RAW, Some(eth::RARP)).unwrap();
//    let config = datalink::Config {
//        channel_type: datalink::ChannelType::Layer2(0x80F3),
//        //promiscuous: false,
//        //socket_fd: Some(sock.into_raw_fd()),
//        ..Default::default()
//    };
//    let (mut eth_tx, mut eth_rx) = match datalink::channel(&interface, config) {
//        Ok(Ethernet(tx, rx)) => (tx, rx),
//        _ => panic!("Failed to create network channel"),
//    };
//
//    let mac = datalink::MacAddr::broadcast();
//    for (start, end) in vlan_range {
//        for vlan in start..=end {
//            let mut buf = [0u8; 1500];
//            let len = packet::builder(&mut buf, &interface, mac, vlan, &host);
//            let result = eth_tx.send_to(&buf[0..len], Some(interface.clone()));
//            debug!("Send out from {} with VLAN {vlan} MAC({mac}), result {result:?}", interface.name);
//
//            //match eth_rx.next() {
//            //    Ok(packet) => {
//            //        debug!("Receive {packet:0x?}");
//            //        if let Some((peer, request)) = packet::parse(packet) {
//            //            debug!("Receive {} at {} from {peer:?}", if request {"REQUEST"} else {"REPLY"}, interface.name);
//            //            if request {
//            //                // always reply with untag
//            //                //let vlans = vec![(0,0), (peer.vlan, peer.vlan)];
//            //                let mut buf = [0u8; 1500];
//            //                let vlan = peer.vlan;
//            //                let mac = peer.mac;
//            //                let len = packet::builder(&mut buf, &interface, mac, vlan, &host);
//            //                let result = eth_tx.send_to(&buf[0..len], Some(interface.clone()));
//            //                debug!("Send out from {} with VLAN {vlan} MAC({mac}), result {result:?}", interface.name);
//            //            }
//            //            TOPO.lock()
//            //                .unwrap()
//            //                .entry((host.clone(), interface.name.clone(), interface.mac.unwrap(), peer.host.clone(), peer.nic.clone(), peer.mac))
//            //                .and_modify(|p| { p.push(peer.vlan); p.sort(); p.dedup(); })
//            //                .or_insert(vec![peer.vlan]);
//            //        }
//            //    },
//            //    Err(_) => println!("No packet at all"),
//            //}
//        }
//    }
//
//    loop {
//        match eth_rx.next() {
//            Ok(packet) => {
//                debug!("Receive {packet:0x?}");
//                if let Some((peer, request)) = packet::parse(packet) {
//                    debug!("{}: Receive {} from {peer:?}", interface.name, if request {"REQUEST"} else {"REPLY"});
//                    if request {
//                        // always reply with untag
//                        //let vlans = vec![(0,0), (peer.vlan, peer.vlan)];
//                        let mut buf = [0u8; 1500];
//                        let vlan = peer.vlan;
//                        let mac = peer.mac;
//                        let len = packet::builder(&mut buf, &interface, mac, vlan, &host);
//                        let result = eth_tx.send_to(&buf[0..len], Some(interface.clone()));
//                        debug!("{}: Reply with VLAN {vlan} MAC({mac}), result {result:?}", interface.name);
//                    }
//                    TOPO.lock()
//                        .unwrap()
//                        .entry((host.clone(), interface.name.clone(), interface.mac.unwrap(), peer.host.clone(), peer.nic.clone(), peer.mac))
//                        .and_modify(|p| { p.push(peer.vlan); p.sort(); p.dedup(); })
//                        .or_insert(vec![peer.vlan]);
//                }
//            },
//            Err(_) => println!("No packet at all"),
//        }
//    }
//}

//async fn show_topo() {
//    loop {
//        sleep(Duration::from_millis(1000)).await;
//
//        fn show_vlan(vlans: &Vec<u16>) -> String {
//            if vlans.is_empty() {
//                return String::new();
//            }
//            let mut start = vlans.get(0);
//            let mut vlan_str = String::new();
//            let mut range = false;
//            let len = vlans.len();
//
//            for i in 0..len {
//                let end = vlans.get(i);
//                let next = vlans.get(i + 1);
//                match next {
//                    Some(&v) => {
//                        if v != end.unwrap() + 1 {
//                            range = true
//                        }
//                    },
//                    None => range = true,
//                }
//                if range {
//                    if !vlan_str.is_empty() {
//                        vlan_str += ",";
//                    }
//                    if start == end {
//                        vlan_str += &format!("{}", start.unwrap());
//                    } else {
//                        vlan_str += &format!("{}-{}", start.unwrap(), end.unwrap());
//                    }
//                    range = false;
//                    start = next;
//                }
//            }
//            vlan_str
//        }
//
//        print!("\x1b[2J"); // clear screen with new line
//        print!("\x1b[H");  // move cursor to left-top
//        println!("    {:<24} {:^12} {:>24}", "local", "<-->", "Peer");
//        let topo = TOPO.lock().unwrap();
//        for (connect, vlan) in &*topo {
//            println!("{} {} {} <--> {} {} {} - vlans: {}",
//                     connect.0,  // host
//                     connect.1,  // nic
//                     connect.2,  // mac
//                     connect.3,  // peer host
//                     connect.4,  // peer nic
//                     connect.5,  // peer mac
//                     show_vlan(&vlan),
//                     );
//        }
//    }
//}

async fn show_topo(
    mut rx: mpsc::UnboundedReceiver<(u32, Peer)>,
    host: String,
    nics: Vec<Interface>,
) {
    let mut topo = Topo {
        connection: HashMap::new(),
    };
    while let Some((ifindex, peer)) = rx.recv().await {
        info!("TOPO: updating for {ifindex} connect {peer:?}");
        let interface: Vec<_> = nics.iter().filter(|n| n.index == ifindex).collect();
        if interface.len() == 0 {
            //warn!("Topology not updated, ignore invalid nic {ifindex}");
            continue;
        }
        let me = Node {
            host: host.clone(),
            nic: interface[0].clone(),
        };
        let p = Node {
            host: peer.host,
            nic: Interface {
                name: peer.nic,
                mac: peer.mac.octets().into(),
                ..Default::default()
            }
        };

        topo.connection
            .entry((me, p))
            .and_modify(|v| { v.push(peer.vlan); v.sort(); v.dedup(); })
            .or_insert(vec![peer.vlan]);
        info!("TOPO: updated for {ifindex}");
        //if rx.len() == 0 {
            //sleep(Duration::from_millis(300)).await;
            //if rx.len() != 0 {
            //    // read message from channel
            //    continue;
            //}
            print!("\x1b[2J"); // clear screen with new line
            print!("\x1b[H");  // move cursor to left-top
            println!("{:<24} {:^12} {:>24}", "local", "<-->", "Peer");
            println!("{topo}");
        //}
    }
}

async fn recv_packet(
    packet: mpsc::UnboundedSender<(u32, Peer)>,
    topo: mpsc::UnboundedSender<(u32, Peer)>,
) {
    let fd = open_socket(libc::ETH_P_AARP as u16, true).unwrap();
    loop {
        let mut buf: [u8; 1024] = [0; 1024];
        let ifindex = recv(fd, &mut buf).await;
        if let Some((peer, request)) = packet::parse(&buf) {
            debug!("Receive {} at {} from {peer:?}", if request {"REQUEST"} else {"REPLY"}, ifindex);
            if request {
                packet.send((ifindex, peer.clone())).unwrap();
            }
            topo.send((ifindex, peer)).unwrap();
          }
    }
}

async fn send_packet(mut rx: mpsc::UnboundedReceiver<(u32, Peer)>, host: String, nics: Vec<Interface>) {
    let fd = open_socket(libc::ETH_P_AARP as u16, true).unwrap();
    while let Some((ifindex, peer)) = rx.recv().await {
        let interface: Vec<_> = nics.iter().filter(|n| n.index == ifindex).collect();
        if interface.len() == 0 {
            //warn!("Packet not sending. Ignore invalid interface ({ifindex})");
            continue;
        }
        let mut buf = [0u8; 64];
        let vlan = peer.vlan;
        let mac = peer.mac;
        packet::builder(&mut buf, &interface[0], mac, vlan, &host);
        debug!("{}: Reply with VLAN {vlan} MAC({mac})", interface[0].name);
        send(fd, &buf, ifindex).await;
    }
}

#[tokio::main]
async fn main() {
    env_logger::init();
    let opt = Opt::parse();
    let mut vlans = if opt.vlan.is_empty() {
        vec![(1, 4094)]
    } else {
        opt.vlan
    };
    // always sent untag packet
    vlans.push((0, 0));

    //let name = uname();
    let name = hostname();
    info!("{name:?}");
    //let hostname = name.nodename().to_str().unwrap().to_string();

    let nics = get_physical_nics();

    let nics = if opt.interface.is_some() {
        nics.into_iter().filter(|n| opt.interface.as_ref().unwrap().contains(&n.name)).collect()
    } else {
        nics
    };
    let nics: Vec<_> = nics
        .into_iter()
        .filter(|n| if n.state == OperState::Up { true } else { println!("{} is not UP!", n.name); false})
        .collect();

    info!("{nics:?}");
    let mut handlers: Vec<_> = Vec::new();
    if !nics.is_empty() {
        let (ptx, prx) = mpsc::unbounded_channel::<(u32, Peer)>();
        let (ttx, trx) = mpsc::unbounded_channel::<(u32, Peer)>();

        let handler = tokio::spawn(show_topo(trx, name.clone(), nics.clone()));
        handlers.push(handler);
        let handler = tokio::spawn(recv_packet(ptx.clone(), ttx));
        handlers.push(handler);
        let handler = tokio::spawn(send_packet(prx, name.clone(), nics.clone()));
        handlers.push(handler);

        for nic in nics {
            for (start, end) in &vlans {
                for vlan in *start..=*end {
                    let peer = Peer {
                        mac: datalink::MacAddr::broadcast(),
                        vlan: vlan,
                        ..Default::default()
                    };
                    ptx.send((nic.index, peer)).unwrap();
                }
            }
        }
    }

    //for ifname in nics {
    //    let interface = match datalink::interfaces().into_iter().find(|iface| iface.name == *ifname) {
    //        None => {
    //            println!("Interface {} is not present.", ifname);
    //            continue;
    //        },
    //        Some(interface) => interface,
    //    };
    //    if !interface.is_up() || !interface.is_lower_up() {
    //        println!("{ifname} is not up, skip!");
    //        continue;
    //    }
    //    info!("nic: {interface:?}");

    //    let handler = tokio::spawn(detect_connection(hostname.clone(), interface, vlans.clone()));
    //    handlers.push(handler);
    //}

    for handler in handlers {
        let _ = handler.await;
    }
}
