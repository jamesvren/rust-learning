use tokio::net::TcpListener;
use tokio::net::TcpStream;
use tokio::io;
use tokio::time::timeout;
use tokio::time::Duration;

use clap::Parser;

#[derive(Parser)]
struct Opts {
    /// port number to listen
    #[arg(short, long)]
    port: u32,

    /// Destination address, e.g.: 127.0.0.1:22
    #[arg(short, long)]
    dest: String,
}

const CONNECTION_TIME: u64 = 1;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let opts = Opts::parse();
    let address = format!("0.0.0.0:{}", opts.port);
    let listener = TcpListener::bind(&address).await?;
    println!("Proxy started with {address}");

    loop {
        let dest = opts.dest.clone();
        let (mut src, source) = listener.accept().await?;
        println!("connected from {source:?}");
        match timeout(Duration::from_secs(CONNECTION_TIME), TcpStream::connect(&dest)).await {
            Ok(Ok(mut dst)) => {
                tokio::spawn(async move {
                    let _ = io::copy_bidirectional(&mut src, &mut dst).await;
                    println!("disconnected: {source:?}");
                });
            }
            _ => {
                eprintln!("Error: Failed to connect {dest}, please check network.");
                break
            }
        }
    }
    Ok(())
}
