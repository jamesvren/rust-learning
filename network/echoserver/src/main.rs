use clap::Parser;
use std::net::IpAddr;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::io::{self, AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{TcpListener, TcpStream, UdpSocket};
use tokio::sync::mpsc;

#[derive(Parser)]
struct Opt {
    /// Client mode
    #[arg(short, long)]
    client: bool,

    /// Listen port (Server) or Connection port (Client)
    #[arg(short, long)]
    port: Option<u32>,

    /// Server address
    #[arg(short, long)]
    ip: Option<IpAddr>,

    /// Udp socket
    #[arg(short, long)]
    udp: bool,

    /// Worker number for server
    #[arg(short, long)]
    worker: Option<usize>,
}

#[tokio::main]
async fn main() -> io::Result<()> {
    let opt = Opt::parse();
    if opt.client && opt.ip.is_none() {
        println!("Error: No ip to connected, please set ip address.");
        return Ok(());
    }

    let port = match opt.port {
        Some(p) => p,
        None => 7788,
    };

    let host = match opt.ip {
        Some(ip) => format!("{ip}:{port}"),
        None => format!("0.0.0.0:{port}"),
    };

    if opt.client {
        let stdin = std::io::stdin();
        if opt.udp {
            let socket = UdpSocket::bind("0.0.0.0:0").await?;
            socket.connect(host.as_str()).await?;

            loop {
                let mut input = String::new();
                stdin.read_line(&mut input)?;
                socket.send(&input.as_bytes()).await?;

                let mut buf = [0; 512];
                match socket.recv(&mut buf).await {
                    Ok(_) => println!("@echo back: {}", String::from_utf8(buf.to_vec()).unwrap()),
                    Err(e) => eprintln!("Failed to receive packet: {e:?}"),
                };
            }
        } else {
            let mut stream = TcpStream::connect(host.as_str()).await?;
            println!("Connected to remote {host}");
            loop {
                let mut input = String::new();
                stdin.read_line(&mut input)?;
                stream.write_all(&input.as_bytes()).await?;

                // Read echo data
                let mut reader = BufReader::new(&mut stream);
                let mut line = String::new();
                reader.read_line(&mut line).await?;
                println!("@echo back: {line}");
            }
        }
    } else {
        let (tx, mut rx) = mpsc::channel::<(u64, SocketAddr)>(1_000);

        tokio::spawn(async move {
            while let Some((len, remote)) = rx.recv().await {
                println!("INFO: echo back to {remote}: {len} bytes");
            }
        });

        if opt.udp {
            let cpus = match opt.worker {
                Some(w) => w,
                None => num_cpus::get(),
            };

            udp_server(&host, cpus, tx).await?;
        } else {
            tcp_server(&host, tx).await?;
        }
    }
    Ok(())
}

async fn udp_server(host: &str, worker: usize, tx: mpsc::Sender<(u64, SocketAddr)>) -> io::Result<()> {
    let socket = UdpSocket::bind(host).await?;
    println!("Start UDP server at {}", socket.local_addr()?);
    let socket = Arc::new(socket);

    let mut handles = Vec::new();
    for _ in 0..worker {
        let socket = socket.clone();
        let stx = tx.clone();

        handles.push(tokio::spawn(async move {
            let mut buf = [0; 1500];
            loop {
                let (len, remote) = socket.recv_from(&mut buf).await.unwrap();
                let len = socket.send_to(&buf[..len], remote).await.unwrap();

                stx.send((len.try_into().unwrap(), remote)).await.unwrap();
            }
        }));
    }

    for handle in handles {
        handle.await?;
    }
    Ok(())
}

async fn tcp_server(host: &str, tx: mpsc::Sender<(u64, SocketAddr)>) -> io::Result<()> {
    let listener = TcpListener::bind(host).await?;
    println!("Start TCP server at {}", listener.local_addr()?);

    loop {
        let stx = tx.clone();

        let (mut socket, remote) = listener.accept().await?;
        println!("Client connected: {remote}");

        tokio::spawn(async move {
            let (mut rd, mut wr) = socket.split();

            match io::copy(&mut rd, &mut wr).await {
                Ok(len) => stx.send((len, remote)).await.unwrap(),
                Err(e) => eprintln!("failed to copy: {e}"),
            };
        });
    }
}
