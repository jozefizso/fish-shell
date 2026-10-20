fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!(
        "cargo:rustc-env=FISH_XTASK_TARGET={}",
        std::env::var("TARGET").expect("Cargo provides this maintainer executable's target")
    );
}
