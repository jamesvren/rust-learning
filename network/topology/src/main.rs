use tokio::sync::mpsc;
use tokio::time::{sleep, Duration};
use pnet::datalink;
use clap::Parser;
use log::{info, debug};

use topology::packet;
use topology::packet::Peer;
use topology::hostname;
use topology::get_physical_nics;
use topology::Interface;
use topology::OperState;
use topology::Topo;
use topology::Node;
use topology::Socket;

use std::collections::HashMap;


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
        if rx.len() == 0 {
            sleep(Duration::from_millis(300)).await;
            if rx.len() != 0 {
                // read message from channel
                continue;
            }
            print!("\x1b[2J"); // clear screen with new line
            print!("\x1b[H");  // move cursor to left-top
            println!("{:<24} {:^12} {:>24}", "local", "<-->", "Peer");
            println!("{topo}");
        }
    }
}

async fn recv_packet(
    packet: mpsc::UnboundedSender<(u32, Peer)>,
    topo: mpsc::UnboundedSender<(u32, Peer)>,
    ifindex: u32,
) {
    let mut sock = Socket::new(libc::ETH_P_AARP as u16).unwrap();
    sock.set_promiscuous(true, ifindex).unwrap();
    sock.bind(ifindex).unwrap();
    loop {
        let mut buf: [u8; 1024] = [0; 1024];
        let ifindex = sock.recv(&mut buf).await.unwrap() as u32;
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
    let mut sock = Socket::new(libc::ETH_P_AARP as u16).unwrap();
    while let Some((ifindex, peer)) = rx.recv().await {
        let interface: Vec<_> = nics.iter().filter(|n| n.index == ifindex).collect();
        if interface.len() == 0 {
            continue;
        }
        let mut buf = [0u8; 256];
        let vlan = peer.vlan;
        let mac = peer.mac;
        packet::builder(&mut buf, &interface[0], mac, vlan, &host);
        debug!("{}: Send with VLAN {vlan} MAC({mac})", interface[0].name);
        sock.set_promiscuous(true, ifindex).unwrap();
        //sock.bind(ifindex).unwrap();
        let _len = sock.send(&buf, ifindex).await;
    }
}

#[tokio::main]
//#[tokio::main(flavor = "current_thread")]
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

    let name = hostname();
    info!("{name:?}");

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
        for nic in &nics {
            let handler = tokio::spawn(recv_packet(ptx.clone(), ttx.clone(), nic.index));
            handlers.push(handler);
        }
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

    for handler in handlers {
        let _ = handler.await;
    }
}
