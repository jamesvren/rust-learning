use std::time::Duration;
use std::path::Path;
use std::fs;
use std::mem;
use std::io::Error;
use anyhow::{anyhow, bail, Context};
use clap::Parser;

use libbpf_rs::skel::SkelBuilder;
use libbpf_rs::skel::OpenSkel;
use blazesym::symbolize;
use object::Object;
use object::ObjectSymbol;

mod funcprobe {
    include!(concat!(env!("OUT_DIR"), "/funcprobe.skel.rs"));
}
use funcprobe::*;

const MAX_STACK_DEPTH: usize = 128;
const TASK_COMM_LEN: usize = 16;
const ADDR_WITH: usize = 16;

#[repr(C)]
struct stacktrace_event {
    pid: u32,
    cpu_id: u32,
    comm: [u8; TASK_COMM_LEN],
    kstack_size: i32,
    ustack_size: i32,
    kstack: [u64; MAX_STACK_DEPTH],
    ustack: [u64; MAX_STACK_DEPTH],
}

#[derive(Parser)]
struct Opts {
    #[arg(short, long)]
    verbose: bool,

    // Module path to be uprobed
    #[arg(short, long)]
    uprobe: Option<String>,

    // Function to be probed
    #[arg(required = true)]
    name: String,
}

fn print_frame(
    name: &str,
    addr_info: Option<(blazesym::Addr, blazesym::Addr, usize)>,
    code_info: &Option<symbolize::CodeInfo>
) {
    let code_info = code_info.as_ref().map(|code_info| {
        let path = code_info.to_path();
        let path = path.display();

        match (code_info.line, code_info.column) {
            (Some(line), Some(col)) => format!(" {path}:{line}:{col}"),
            (Some(line), None) => format!(" {path}:{line}"),
            (None, _) => format!(" {path}"),
        }
    });

    if let Some((input_addr, addr, offset)) = addr_info {
        println!(
            "{input_addr:#0width$x}: {name} @ {addr:#x}+{offset:#x}{code_info}",
            code_info = code_info.as_deref().unwrap_or(""),
            width = ADDR_WITH
        )
    } else {
        println!(
            "{:width$}  {name}{code_info} [inlined]",
            " ",
            code_info = code_info
                .map(|info| format!(" @{info}"))
                .as_deref()
                .unwrap_or(""),
            width = ADDR_WITH
        )
    }
}

fn show_stack_trace(stack: &[u64], symbolizer: &symbolize::Symbolizer, pid: u32) {
    let converted_stack;
    // The kernel always reports `u64` addresses, whereas blazesym uses `usize`.                                                                               
    // Convert the stack trace as necessary.
    let stack = if mem::size_of::<blazesym::Addr>() != mem::size_of::<u64>() {
        converted_stack = stack
            .iter()
            .copied()
            .map(|addr| addr as blazesym::Addr)
            .collect::<Vec<_>>();
        converted_stack.as_slice()
    } else {
        // SAFETY: `Addr` has the same size as `u64`, so it can be trivially and                                                                               
        //         safely converted.
        unsafe { mem::transmute::<_, &[blazesym::Addr]>(stack) }
    };

    let src = if pid == 0 {
        symbolize::Source::from(symbolize::Kernel::default())
    } else {
        symbolize::Source::from(symbolize::Process::new(pid.into()))
    };

    let syms = match symbolizer.symbolize(&src, symbolize::Input::AbsAddr(stack)) {
        Ok(syms) => syms,
        Err(err) => {
            eprintln!("  failed to symbolize addresses: {err:#}");
            return;
        }
    };

    for (input_addr, sym) in stack.iter().copied().zip(syms) {
        match sym {
            symbolize::Symbolized::Sym(symbolize::Sym{
                name,
                addr,
                offset,
                code_info,
                inlined,
                ..
            }) => {
                print_frame(&name, Some((input_addr, addr, offset)), &code_info);
                for frame in inlined.iter() {
                    print_frame(&frame.name, None, &frame.code_info);
                }
            }
            symbolize::Symbolized::Unknown => {
                println!("{input_addr:#0width$x}: <no-symbol>", width = ADDR_WITH)
            }
        }
    }
}

fn event_handler(symbolizer: &symbolize::Symbolizer, data: &[u8]) -> ::std::os::raw::c_int {
    println!("got event ...");
    if data.len() != mem::size_of::<stacktrace_event>() {
        eprintln!(
            "Invalid size {} != {}",
            data.len(),
            mem::size_of::<stacktrace_event>()
        );
        return 1;
    }

    let event = unsafe { &*(data.as_ptr() as *const stacktrace_event) };

    if event.kstack_size <= 0 && event.ustack_size <= 0 {
        return 1;
    }

    let comm = std::str::from_utf8(&event.comm)
        .or::<Error>(Ok("<unknown>"))
        .unwrap();
    println!("COMM: {} (pid={}) @ CPU {}", comm, event.pid, event.cpu_id);

    if event.kstack_size > 0 {
        println!("Kernel:");
        show_stack_trace(
            &event.kstack[0..(event.kstack_size as usize / mem::size_of::<u64>())],
            symbolizer,
            0,
        );
    } else {
        println!("No Kernel Stack");
    }

    if event.ustack_size > 0 {
        println!("Userspace:");
        show_stack_trace(
            &event.ustack[0..(event.ustack_size as usize / mem::size_of::<u64>())],
            symbolizer,
            event.pid,
        );
    } else {
        println!("No Userspace Stack");
    }

    println!();
    0
}

fn get_symbol_address(so_path: &str, fn_name: &str) -> anyhow::Result<usize> {
    let path = Path::new(so_path);
    let buffer =
        fs::read(path).with_context(|| format!("could not read file `{}`", path.display()))?;
    let file = object::File::parse(buffer.as_slice())?;

    let mut symbols = file.dynamic_symbols();
    let symbol = symbols
        .find(|symbol| {
            if let Ok(name) = symbol.name() {
                return name == fn_name;
            }
            false
        })
        .ok_or(anyhow!("symbol not found"))?;

    Ok(symbol.address() as usize)
}

fn bump_memlock_rlimit() -> anyhow::Result<()> {
    let rlimit = libc::rlimit {
        rlim_cur: 128 << 20,
        rlim_max: 128 << 20,
    };

    if unsafe { libc::setrlimit(libc::RLIMIT_MEMLOCK, &rlimit) } != 0 {
        bail!("Failed to increate rlimit");
    }

    Ok(())
}

fn main() -> anyhow::Result<()> {
    let opts = Opts::parse();
    let mut skel_builder = FuncprobeSkelBuilder::default();

    bump_memlock_rlimit()?;
    if opts.verbose {
        skel_builder.obj_builder.debug(true);
    }

    let open_skel = skel_builder.open()?;
    let mut skel = open_skel.load()?;

    let _uprobe;
    let _uretprobe;
    let _kprobe;
    let _kretprobe;
    if let Some(module) = opts.uprobe {
        let address = get_symbol_address(&module, &opts.name)?;
        _uprobe =
            skel.progs_mut()
                .ufunc_enter()
                .attach_uprobe(false, -1, &module, address)?;
        _uretprobe =
            skel.progs_mut()
                .ufunc_exit()
                .attach_uprobe(true, -1, &module, address)?;
    } else {
        _kprobe =
            skel.progs_mut()
                .kfunc_enter()
                .attach_kprobe(false, &opts.name)?;
        _kretprobe =
            skel.progs_mut()
                .kfunc_enter()
                .attach_kprobe(true, &opts.name)?;
    }

    let symbolizer = symbolize::Symbolizer::new();
    let mut builder = libbpf_rs::RingBufferBuilder::new();
    let binding = skel.maps();
    builder
        .add(binding.events(), move |data| {
            event_handler(&symbolizer, data)
        })
        .unwrap();
    let ringbuf = builder.build().unwrap();
    while ringbuf.poll(Duration::MAX).is_ok() {}

    //let perf = PerfBufferBuilder::new(skel::maps_mut().events)
    //    .sample_cb(handle_event)
    //    .build()?;

    //let running = Arc::new(AtomicBool::new(true));
    //let r = running.clone();
    //ctrlc::set_handler(move || {
    //    r.store(false, Ordering::SeqCst);
    //});?

    //while running.load(Ordering::SeqCst) {
    //    perf.poll(Duration::from::millis(100))?;
    //}

    Ok(())
}

