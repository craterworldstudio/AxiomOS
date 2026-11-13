fn main() {
    println!("cargo:rerun-if-changed=kernel/microkernel/linker.ld");
    println!("cargo:rustc-link-arg=-Tkernel/microkernel/linker.ld");
}
