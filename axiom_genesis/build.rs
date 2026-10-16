fn main() {
    println!("cargo:rerun-if-changed=axiom_genesis/linker.ld");
    println!("cargo:rustc-link-arg=-Taxiom_genesis/linker.ld");
}
