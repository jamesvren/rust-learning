use std::mem::MaybeUninit;
use std::os::unix::io::AsFd as _;
use std::ops::DerefMut;
use libbpf_rs::skel::OpenSkel;
use libbpf_rs::skel::SkelBuilder;
use libbpf_rs::MapCore as _;
//use libbpf_rs::RingBufferBuilder;
use libbpf_rs::TcHookBuilder;
use libbpf_rs::TC_CUSTOM;
use libbpf_rs::TC_EGRESS;
use libbpf_rs::TC_H_CLSACT;
use libbpf_rs::TC_H_MIN_INGRESS;
use libbpf_rs::TC_INGRESS;

use clap::Parser;

mod portmirror {
    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/src/bpf/portmirror.skel.rs"
    ));
}
use portmirror::PortmirrorSkelBuilder;

#[derive(Parser)]
struct Opts {
    /// ifindex of source interface
    #[arg(short, long)]
    src: Vec<u32>,

    /// ifindex of mirror interface
    #[arg(short, long)]
    mirror: u32,

    /// attach a hook
    #[arg(short, long)]
    attach: bool,

    /// attach a hook
    #[arg(long)]
    ingress: bool,

    /// attach a hook
    #[arg(long)]
    egress: bool,

    /// detach existing hook
    #[arg(short, long)]
    detach: bool,

    saddr: Option<u32>,
    daddr: Option<u32>,
    sport: Option<u16>,
    dport: Option<u16>,
    proto: Option<u8>,
}

//#[repr(C)]
//struct Event {
//    tap_ifindex: u32,
//    mirror_ifindex: u32,
//    packet_len: u32,
//}

#[repr(C, packed)]
struct FlowKey {
    saddr: [u32; 4],
    daddr: [u32; 4],
    sport: u16,
    dport: u16,
    proto: u8,
    ip_version: u8,
}

const HASH_TYPE_SRC: u8   = 0x01;
const HASH_TYPE_DST: u8   = 0x02;
const HASH_TYPE_PROTO: u8 = 0x04;
const HASH_TYPE_SPORT: u8 = 0x08;
const HASH_TYPE_DPORT: u8 = 0x10;

fn bump_memlock_rlimit() -> Result<(), Box<dyn std::error::Error>> {
    //let rlimit = libc::rlimit {
    //    rlim_cur: 128 << 20,
    //    rlim_max: 128 << 20,
    //};
    let rlimit = libc::rlimit {
        rlim_cur: libc::RLIM_INFINITY,
        rlim_max: libc::RLIM_INFINITY,
    };

    if unsafe { libc::setrlimit(libc::RLIMIT_MEMLOCK, &rlimit) } != 0 {
        return Err("Failed to increate rlimit".into());
    }

    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    bump_memlock_rlimit()?;
    let opt = Opts::parse();

    for tap in opt.src {
    let mut skel_builder = PortmirrorSkelBuilder::default();
    skel_builder.obj_builder.debug(true);
    let mut open_object = MaybeUninit::uninit();
    let mut open_skel = skel_builder.open(&mut open_object)?;
    let rodata = open_skel
        .maps
        .rodata_data
        .deref_mut();
        //.expect("`rodata` is not memory mapped");

    let saddr = opt.saddr.unwrap_or(0).to_be();
    let daddr = opt.daddr.unwrap_or(0).to_be();
    let sport = opt.sport.unwrap_or(0).to_be();
    let dport = opt.dport.unwrap_or(0).to_be();
    let proto = opt.proto.unwrap_or(0).to_be();

    if saddr != 0 { rodata.hash_type |= HASH_TYPE_SRC }
    if daddr != 0 { rodata.hash_type |= HASH_TYPE_DST }
    if sport != 0 { rodata.hash_type |= HASH_TYPE_SPORT }
    if dport != 0 { rodata.hash_type |= HASH_TYPE_DPORT }
    if proto != 0 { rodata.hash_type |= HASH_TYPE_PROTO }

    let skel = open_skel.load()?;

    let mirror_ifindex = opt.mirror;

    let key = FlowKey {
        saddr: [saddr, 0, 0, 0],
        daddr: [daddr, 0, 0, 0],
        sport,
        dport,
        proto,
        ip_version: 4,
    };

    let flow_key = unsafe { plain::as_bytes(&key) };
    skel.maps.filter_map.update(
        //unsafe { std::mem::transmute(&key) },
        //unsafe { std::slice::from_raw_parts(&key as *const _ as *const u8, std::mem::size_of::<FlowKey>()) },
        flow_key,
        &[1u8],
        libbpf_rs::MapFlags::ANY,
    )?;

    //for tap in opt.src {
        let mut tc_builder = TcHookBuilder::new(skel.progs.port_mirror.as_fd());
        tc_builder
            .ifindex(tap as i32)
            .replace(true)
            .handle(1)
            .priority(1);

        let mut egress = tc_builder.hook(TC_EGRESS);
        let mut ingress = tc_builder.hook(TC_INGRESS);
        let mut custom = tc_builder.hook(TC_CUSTOM);
        custom.parent(TC_H_CLSACT, TC_H_MIN_INGRESS).handle(2);

        if opt.detach {
            if let Err(e) = ingress.detach() {
                println!("failed to detach ingress hook {e}");
            }
            if let Err(e) = egress.detach() {
                println!("failed to detach egress hook {e}");
            }
            if let Err(e) = custom.detach() {
                println!("failed to detach custom hook {e}");
            }
        }

        if opt.attach {
            skel.maps.mirror_map.update(
                &tap.to_ne_bytes(),
                &mirror_ifindex.to_ne_bytes(),
                libbpf_rs::MapFlags::ANY,
            )?;

            //let rb = RingBufferBuilder::new(skel.maps.events)
            //    .handler(|data: &[u8]| {
            //        let event: &Event = unsafe { &*(data.as_ptr() as *const Event) };
            //        println!("Mirrored packet: {} bytes", event.packet_len);
            //    })
            //    .build()?;

            ingress.create()?;

            if opt.ingress {
                let _ = ingress.attach();
            }

            if opt.egress {
                let _ = egress.attach();
            }

            if !opt.ingress && !opt.egress {
                if let Err(e) = egress.attach() {
                    println!("failed to attach egress hook {e}");
                }

                if let Err(e) = ingress.attach() {
                    println!("failed to attach ingress hook {e}");
                }

                if let Err(e) = custom.attach() {
                    println!("failed to attach custom hook {e}");
                }
            }

            //loop {
            //    rb.poll(std::time::Duration::from_secs(1))?;
            //}
        }
    }

    Ok(())
}
