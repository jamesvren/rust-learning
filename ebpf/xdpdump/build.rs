use libbpf_cargo::SkeletonBuilder;
use std::env;
use std::path::PathBuf;

const SRC: &str = "src/bpf/xdpdump.bpf.c";

fn main() {
    let mut out =
        PathBuf::from(env::var_os("OUT_DIR").expect("Missing OUT_DIR"));
    out.push("xdpdump.skel.rs");
    SkeletonBuilder::new()
        .source(SRC)
        .build_and_generate(&out)
        .expect("bpf compile failed!");
    println!("cargo:return-if-changed={}", SRC);
}
