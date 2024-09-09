use std::collections::BTreeMap;
//use std::sync::{Arc, Mutex};
use tokio::sync::mpsc;
use tokio::time::{sleep, Duration};
use pnet::datalink;
use pnet::datalink::MacAddr;
use pnet::datalink::Channel::Ethernet;
use pnet::datalink::NetworkInterface;
use pnet::datalink::DataLinkSender;
use pnet::datalink::DataLinkReceiver;
use rustix::system::uname;
use rustix::net::{socket, AddressFamily, SocketType, eth};
use rustix::fd::IntoRawFd;
use clap::Parser;
use log::{info, debug};

use topology::packet;
use topology::packet::Peer;

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

fn get_physical_nics() -> Vec<String> {
    const NIC_PATH: &str = "/sys/class/net/";
    let mut nics: Vec<_> = vec![];

    if let Ok(dir) = std::fs::read_dir(NIC_PATH) {
        for entry in dir {
            let entry = entry.unwrap();
            if let Ok(path) = std::fs::read_link(entry.path()) {
                if !path.starts_with("../../devices/virtual/") {
                    nics.push(entry.file_name().into_string().unwrap());
                }
            }
        }
    }
    nics
}

async fn send_packet(
    interface: NetworkInterface,
    hostname: String,
    mut eth_tx: Box<dyn DataLinkSender>,
    mut rx: mpsc::UnboundedReceiver<(MacAddr, Vec<(u16, u16)>)>
    ) {
    while let Some((mac, vlans)) = rx.recv().await {
        let mut count = 0;
        for (start, end) in vlans {
            for v in start..=end {
                let mut buf = [0u8; 1500];
                let len = packet::builder(
                    &mut buf,
                    &interface,
                    mac,
                    v,
                    &hostname
                    );
                let result = eth_tx.send_to(&buf[0..len], Some(interface.clone()));
                debug!("Send out from {} with VLAN {v} MAC({mac}), result {result:?}", interface.name);
                count += 1;
                if count % 10 == 0 {
                    sleep(Duration::from_millis(1)).await;
                }
            }
        }
    }
}

fn handle_packet(
    //topo: Connection,
    interface: NetworkInterface,
    mut eth_rx: Box<dyn DataLinkReceiver>,
    pkt_tx: mpsc::UnboundedSender<(MacAddr, Vec<(u16, u16)>)>,
    topo_tx: mpsc::UnboundedSender<(NetworkInterface, Peer)>
    ) {
    loop {
        match eth_rx.next() {
            Ok(packet) => {
                if let Some((peer, request)) = packet::parse(packet) {
                    debug!("Receive {} at {} from {peer:?}", if request {"REQUEST"} else {"REPLY"}, interface.name);
                    if request {
                        // always reply with untag
                        //let vlans = vec![(0,0), (peer.vlan, peer.vlan)];
                        let vlans = vec![(peer.vlan, peer.vlan)];
                        pkt_tx.send((peer.mac, vlans)).unwrap();
                    }
                    //topo.lock()
                    //    .unwrap()
                    //    .entry((interface.name.clone(), interface.mac.unwrap(), peer.host.clone(), peer.nic.clone(), peer.mac))
                    //    .and_modify(|p| { p.push(peer.vlan); p.sort(); p.dedup(); })
                    //    .or_insert(vec![peer.vlan]);
                    topo_tx.send((interface.clone(), peer.clone())).unwrap();
                    // need this to let mpsc channel scheduled
                    //tokio::task::yield_now().await;
                }
            },
            Err(_) => println!("No packet at all"),
        }
    }
}

async fn update_topology(
    //topo: Connection,
    host: String,
    mut rx: mpsc::UnboundedReceiver<(NetworkInterface, Peer)>
    ) {
    let mut topo: BTreeMap<(String, MacAddr, String, String, MacAddr), Vec<u16>> = BTreeMap::new();

    fn show_vlan(vlans: &Vec<u16>) -> String {
        if vlans.is_empty() {
            return String::new();
        }
        let mut start = vlans.get(0);
        let mut vlan_str = String::new();
        let mut range = false;
        let len = vlans.len();

        for i in 0..len {
            let end = vlans.get(i);
            let next = vlans.get(i + 1);
            match next {
                Some(&v) => {
                    if v != end.unwrap() + 1 {
                        range = true
                    }
                },
                None => range = true,
            }
            if range {
                if !vlan_str.is_empty() {
                    vlan_str += ",";
                }
                if start == end {
                    vlan_str += &format!("{}", start.unwrap());
                } else {
                    vlan_str += &format!("{}-{}", start.unwrap(), end.unwrap());
                }
                range = false;
                start = next;
            }
        }
        vlan_str
    }
    while let Some((nic, peer)) = rx.recv().await {
        topo
            .entry((nic.name, nic.mac.unwrap(), peer.host, peer.nic, peer.mac))
            .and_modify(|p| { p.push(peer.vlan); p.sort(); p.dedup(); })
            .or_insert(vec![peer.vlan]);

        if rx.len() == 0 {
            sleep(Duration::from_millis(300)).await;
            if rx.len() != 0 {
                // read message from channel
                continue;
            }
            print!("\x1b[2J"); // clear screen with new line
            print!("\x1b[H");  // move cursor to left-top
            println!("    {:<24} {:^12} {:>24}", "local", "<-->", "Peer");
            //let topo = topo.lock().unwrap();
            //for (connect, vlan) in &*topo {
            for (connect, vlan) in &topo {
                println!("{} {} {} <--> {} {} {} - vlans: {}",
                         host,
                         connect.0,
                         connect.1,
                         connect.2,
                         connect.3,
                         connect.4,
                         show_vlan(vlan),
                         );
            }
        }
    }
}

//type Connection = Arc<Mutex< BTreeMap<(String, MacAddr, String, String, MacAddr), Vec<u16>> >>;

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

    let name = uname();
    info!("{name:?}");
    let hostname = name.nodename().to_str().unwrap().to_string();

    // Create a mpsc channel to update topology
    let (ttx, trx) = mpsc::unbounded_channel::<(NetworkInterface, Peer)>();

    //let topo = Arc::new(Mutex::new(BTreeMap::new()));
    //tokio::spawn(update_topology(topo.clone(), hostname.clone(), trx));
    tokio::spawn(update_topology(hostname.clone(), trx));

    let mut handlers: Vec<_> = Vec::new();

    let nics = get_physical_nics();
    let nics = if opt.interface.is_some() {
        opt.interface.unwrap()
    } else {
        nics
    };

    for ifname in nics {
        let interface = match datalink::interfaces().into_iter().find(|iface| iface.name == *ifname) {
            None => {
                println!("Interface {} is not present.", ifname);
                continue;
            },
            Some(interface) => interface,
        };
        if !interface.is_up() {
            println!("{ifname} is not up, skip!");
            continue;
        }
        info!("nic: {interface:?}");

        let sock = socket(AddressFamily::PACKET, SocketType::RAW, Some(eth::AARP)).unwrap();
        let config = datalink::Config {
            //write_buffer_size: 256,
            //read_buffer_size: 256,
            //promiscuous: false,
            socket_fd: Some(sock.into_raw_fd()),
            ..Default::default()
        };
        let (eth_tx, eth_rx) = match datalink::channel(&interface, config) {
            Ok(Ethernet(tx, rx)) => (tx, rx),
            _ => panic!("Failed to create network channel"),
        };

        let (ptx, prx) = mpsc::unbounded_channel::<(MacAddr, Vec<(u16, u16)>)>();

        let topo_tx = ttx.clone();
        let pkt_tx = ptx.clone();
        let nic = interface.clone();
        //let topo2 = topo.clone();
        //let handler = tokio::task::spawn_blocking(move || handle_packet(topo2, nic, eth_rx, pkt_tx, topo_tx));
        let handler = tokio::task::spawn_blocking(move || handle_packet(nic, eth_rx, pkt_tx, topo_tx));
        handlers.push(handler);
        
        tokio::task::yield_now().await;
        sleep(Duration::from_millis(40)).await;

        let handler = tokio::spawn(send_packet(interface.clone(), hostname.clone(), eth_tx, prx));
        handlers.push(handler);
        // Send request
        let vlans = vlans.clone();
        ptx.send((datalink::MacAddr::broadcast(), vlans)).unwrap();
    }

    for handler in handlers {
        let _ = handler.await;
    }
}
