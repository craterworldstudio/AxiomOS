fn main() {
    println!("cargo:rerun-if-changed=os/genesis/linker.ld");
    println!("cargo:rustc-link-arg=-Tos/genesis/linker.ld");
}
