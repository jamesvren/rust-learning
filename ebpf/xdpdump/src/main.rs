use clap::Parser;
use anyhow::{bail, Result};
use plain::Plain;
use libbpf_rs::skel::SkelBuilder;
use libbpf_rs::skel::OpenSkel;
use libbpf_rs::MapFlags;
use std::net::Ipv4Addr;
use std::str::FromStr;
use std::time::Duration;
use std::thread;

mod xdpdump {
    include!(concat!(env!("OUT_DIR"), "/xdpdump.skel.rs"));
}
use xdpdump::*;

#[derive(Parser)]
struct Opts {
    /// Interface index to be attached
    #[arg(short, long, required = true)]
    ifindex: i32,
    /// Source IP of arp to be filtered
    #[arg(long, name="SRC")]
    arp: Option<String>,
}

fn bump_memlock_rlimit() -> Result<()> {
    let rlimit = libc::rlimit {
        rlim_cur: 128 << 20,
        rlim_max: 128 << 20,
    };

    if unsafe { libc::setrlimit(libc::RLIMIT_MEMLOCK, &rlimit) } != 0 {
        bail!("Failed to increate rlimit");
    }

    Ok(())
}

fn main() -> Result<()> {
    bump_memlock_rlimit()?;

    let opts = Opts::parse();
    let skel_builder = XdpdumpSkelBuilder::default();
    let open_skel = skel_builder.open()?;
    let mut skel = open_skel.load()?;

    if let Some(arp) = &opts.arp {
        let ip_src = Ipv4Addr::from_str(arp)?.octets();
        //let ip_src = Ipv4Addr::from_str(arp)?.to_bits();
        println!("addr is {ip_src:?}");

        skel
            .maps_mut()
            .filterlist()
            .update(&ip_src, &ip_src, MapFlags::ANY)?;
        println!("update ok");
    }
    let link = skel.progs_mut().xdp_dump().attach_xdp(opts.ifindex)?;
    skel.links = XdpdumpLinks {
        xdp_dump: Some(link),
    };

    loop {
        thread::sleep(Duration::from_secs(1));
    }
}
