use std::error::Error;
//use std::time::Duration;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::str::FromStr;
use std::net::SocketAddr;
use tracing_subscriber::EnvFilter;
use libp2p::tcp;
use libp2p::yamux;
use libp2p::swarm::SwarmEvent;
use libp2p::swarm::NetworkBehaviour;
use libp2p::gossipsub;
//use libp2p::mdns;
use libp2p::kad;
use libp2p::kad::store::MemoryStore;
use libp2p::noise;
use libp2p::identify;
use libp2p::identity;
use libp2p::pnet::PreSharedKey;
use libp2p::pnet::PnetConfig;
use libp2p::Multiaddr;
use libp2p::PeerId;
use libp2p::multiaddr::Protocol;
use libp2p::Transport;
use libp2p::core::transport::upgrade::Version;
use futures::StreamExt;
use tokio::{io, io::AsyncBufReadExt, select};
use tokio::task;
use axum::Router;
use axum::routing::get;
use axum::extract::{Path, State};
use axum::http::{header::CONTENT_TYPE, Method, StatusCode};
use axum::response::{Html, IntoResponse};


#[derive(NetworkBehaviour)]
struct MyBehaviour {
    gossipsub: gossipsub::Behaviour,
    identify: identify::Behaviour,
    //mdns: mdns::tokio::Behaviour,
    kademlia: kad::Behaviour<MemoryStore>,
}

fn strip_peer_id(addr: &mut Multiaddr) {
    let last = addr.pop();
    match last {
        Some(Protocol::P2p(peer_id)) => {
            let mut addr = Multiaddr::empty();
            addr.push(Protocol::P2p(peer_id));
            println!("removing peer id {addr} so this address can be dialed by libp2p");
        }
        Some(other) => addr.push(other),
        _ => {}
    }
}

fn parse_legacy_multiaddr(text: &str) -> Result<Multiaddr, Box<dyn Error>> {
    let sanitized = text
        .split('/')
        .map(|part| if part == "ipfs" { "p2p" } else { part } )
        .collect::<Vec<_>>()
        .join("/");

    println!("pares Multiaddr: {sanitized}");
    let mut res = Multiaddr::from_str(&sanitized)?;
    strip_peer_id(&mut res);
    Ok(res)
}

fn parse_peer_addr(text: &str) -> Result<(Multiaddr, PeerId), Box<dyn Error>> {
    let mut sanitized: Vec<_> = text.rsplitn(2, '/').collect();
    sanitized.rotate_left(1);
    println!("got {sanitized:?}");
    let addr = Multiaddr::from_str(&sanitized.join("/"))?;
    let peer_id: PeerId = sanitized[1].parse()?;
    println!("parse Multiadddr: {addr} - {peer_id}");
    Ok((addr, peer_id))
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let _ = tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .try_init();

    let args: Vec<String> = std::env::args().collect();
    let ip = args[1].clone();
    let my_key = args[2].parse()?;
    let id_keys = {
        let args: Vec<String> = std::env::args().collect();
        let mut bytes = [0u8; 32];
        bytes[0] = my_key;
        identity::Keypair::ed25519_from_bytes(bytes).unwrap()
    };
    //let local_key = identity::Keypair::generate_ed25519();

    let psk_key = {
        let mut bytes = [0u8; 32];
        let data = "172.118.59.180".as_bytes();

        bytes[..data.len().min(32)].copy_from_slice(&data[..data.len().min(32)]);
        bytes
    };
    let psk = PreSharedKey::new(psk_key);
    println!("using swarm key with fingerprint: {}", psk.fingerprint());

    //let mut swarm = libp2p::SwarmBuilder::with_new_identity()
    let mut swarm = libp2p::SwarmBuilder::with_existing_identity(id_keys)
        .with_tokio()
        .with_other_transport(|key| {
            let noise_config = noise::Config::new(key).unwrap();
            let yamux_config = yamux::Config::default();

            let base_transport = tcp::tokio::Transport::new(tcp::Config::default().nodelay(true));
            let maybe_encrypted = base_transport.and_then(move |socket, _| PnetConfig::new(psk).handshake(socket));
            maybe_encrypted
                .upgrade(Version::V1Lazy)
                .authenticate(noise_config)
                .multiplex(yamux_config)
        })?
        //.with_tcp(
        //    tcp::Config::default(),
        //    noise::Config::new,
        //    yamux::Config::default,
        //)?
        .with_dns()?
        //.with_quic()
        .with_behaviour(|key| {
            let message_id_fn = |message: &gossipsub::Message| {
                let mut s = DefaultHasher::new();
                message.data.hash(&mut s);
                gossipsub::MessageId::from(s.finish().to_string())
            };

            let gossipsub_config = gossipsub::ConfigBuilder::default()
                //.heartbeat_interval(Duration::from_secs(10))
                .max_transmit_size(262144)
                //.validation_mode(gossipsub::ValidationMode::Strict)
                //.message_id_fn(message_id_fn)
                .build()
                .map_err(io::Error::other)?;

            let gossipsub = gossipsub::Behaviour::new(
                gossipsub::MessageAuthenticity::Signed(key.clone()),
                gossipsub_config,
            )?;

            //let mdns =
            //    mdns::tokio::Behaviour::new(mdns::Config::default(), key.public().to_peer_id())?;

            let kademlia = kad::Behaviour::new(
                key.public().to_peer_id(),
                MemoryStore::new(key.public().to_peer_id()),
            );
            let identify = identify::Behaviour::new(
                identify::Config::new(
                    "/network/0.1.0".into(),
                    key.public(),
                )
                .with_agent_version("/network/0.1.0".to_string())
            );
            //Ok(MyBehaviour { gossipsub, identify, mdns })
            Ok(MyBehaviour { gossipsub, identify, kademlia })
        })?
        //.with_swarm_config(|c| c.with_idle_connection_timeout(Duration::from_secs(60)))
        .build();

    //swarm
    //    .behaviour_mut()
    //    .kademlia
    //    .set_mode(Some(kad::Mode::Server));

    println!("Local Peer ID: {}", swarm.local_peer_id());
    let topic = gossipsub::IdentTopic::new("test-net");
    swarm.behaviour_mut().gossipsub.subscribe(&topic)?;

    //for to_dial in std::env::args().skip(3) {
    //    let addr: Multiaddr = parse_legacy_multiaddr(&to_dial)?;
    //    swarm.dial(addr)?;
    //    println!("Dialed {to_dial:?}");
    //}
    for peer in std::env::args().skip(3) {
        let (addr, peer) = parse_peer_addr(&peer)?;
        swarm
            .behaviour_mut()
            .kademlia
            .add_address(&peer, addr);
    }

    let mut stdin = io::BufReader::new(io::stdin()).lines();

    //swarm.listen_on(format!("/ip4/{ip}/udp/4444/quic-v1").parse()?)?;
    swarm.listen_on(format!("/ip4/{ip}/tcp/4444").parse()?)?;

    let address = loop {
        if let SwarmEvent::NewListenAddr { address, .. } = swarm.select_next_some().await {
            break address;
        }
    };
    let Some(Protocol::Ip4(listen_addr)) = address.iter().next() else {
        panic!("Expected 1st protocol to be IP4")
    };

    println!("address is {listen_addr}");
    let router = Router::new()
        .route("/", get(|| async { "Hello, Rust!" }))
        .route("/{path}", get(|Path(path): Path<String>| async move { format!("Got path: {}", path.clone()) } ));
    //let listener = tokio::net::TcpListener::bind(&format!("{ip}:4444")).await?;
    let addr = SocketAddr::new(listen_addr.into(), 4444);
    let listener = tokio::net::TcpListener::bind(addr).await?;

    task::spawn(axum::serve(listener, router).into_future());

    println!("Enter messages via STDIN and they will be sent to connected peers using Gossipsub");

    loop {
        select! {
            Ok(Some(line)) = stdin.next_line() => {
                if let Err(e) = swarm
                    .behaviour_mut().gossipsub
                    .publish(topic.clone(), line.as_bytes()) {
                    println!("Publish error: {e:?}");
                }
            }
            event = swarm.select_next_some() => match event {
                SwarmEvent::Behaviour(MyBehaviourEvent::Kademlia(kad::Event::OutboundQueryProgressed { result, .. })) => {
                    match result {
                        kad::QueryResult::GetClosestPeers(Ok(ok)) => {
                            if ok.peers.is_empty() {
                                println!("Query finished with no closet peers.");
                            } else {
                                println!("Query finished with closet peers: {:#?}", ok.peers);
                            }
                        }
                        kad::QueryResult::GetRecord(Ok(
                            kad::GetRecordOk::FoundRecord(kad::PeerRecord {
                                record: kad::Record { key, value, .. },
                                ..
                            })
                        )) => {
                            println!(
                                "Got record {:?} {:?}",
                                std::str::from_utf8(key.as_ref()).unwrap(),
                                std::str::from_utf8(&value).unwrap(),
                            );
                        }
                        kad::QueryResult::GetRecord(Ok(_)) => {}
                        kad::QueryResult::GetRecord(Err(err)) => {
                            eprintln!("Failed to get record: {err:?}");
                        }
                        kad::QueryResult::PutRecord(Ok(kad::PutRecordOk { key })) => {
                            println!(
                                "Successfully put record {:?}",
                                std::str::from_utf8(key.as_ref()).unwrap()
                            );
                        }
                        kad::QueryResult::PutRecord(Err(err)) => {
                            eprintln!("Failed to put record: {err:?}");
                        }
                        _ => {}
                    }
                }
                //SwarmEvent::Behaviour(MyBehaviourEvent::Mdns(mdns::Event::Discovered(list))) => {
                //    for (peer_id, multiaddr) in list {
                //        println!("mDNS discovered a new peer: {peer_id}");
                //        swarm.behaviour_mut().gossipsub.add_explicit_peer(&peer_id);
                //        //swarm.dial(multiaddr)?;
                //    }
                //},
                //SwarmEvent::Behaviour(MyBehaviourEvent::Mdns(mdns::Event::Expired(list))) => {
                //    for (peer_id, _multiaddr) in list {
                //        println!("mDNS discover peer has expired: {peer_id}");
                //        //swarm.behaviour_mut().gossipsub.remove_explicit_peer(&peer_id);
                //    }
                //},
                SwarmEvent::Behaviour(MyBehaviourEvent::Gossipsub(gossipsub::Event::Message {
                    propagation_source: peer_id,
                    message_id: id,
                    message,
                })) => println!(
                        "Got message: '{}' with id: {id} from peer: {peer_id}",
                        String::from_utf8_lossy(&message.data),
                ),
                SwarmEvent::Behaviour(MyBehaviourEvent::Identify(event)) => {
                    println!("identify: {event:?}");
                },
                SwarmEvent::NewListenAddr { address, .. } => {
                    println!("Local node is listening on {address}");
                },
                evt => println!("Not handled event: {evt:?}"),
            }
        }
    }
}
