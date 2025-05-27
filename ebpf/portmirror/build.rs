use libbpf_cargo::SkeletonBuilder;
use std::env;
use std::path::PathBuf;

const SRC: &str = "src/bpf/portmirror.bpf.c";

fn main() {
    let out =
        PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").expect("Missing CARGO_MANIFEST_DIR"))
        .join("src")
        .join("bpf")
        .join("portmirror.skel.rs");
    SkeletonBuilder::new()
        .source(SRC)
        .build_and_generate(&out)
        .expect("bpf compile failed!");
    println!("cargo:rustc-link-lib=static:+whole-archive=zstd");
    println!("cargo:return-if-changed={}", SRC);
}
