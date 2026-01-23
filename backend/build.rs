use std::env;
use std::path::Path;

fn main() {
    let dir = env::var("CARGO_MANIFEST_DIR").unwrap();

    #[cfg(any(target_os = "macos", target_os = "linux"))]
    println!(
        "cargo:rustc-link-search=native={}",
        Path::new(&dir).join("target/lib").display()
    );

    // From here: https://crates.io/crates/bind-builder BUT cannot get it working so using rustc-link-search instead. Followed by using install_name_tool as noted in README.md
    #[cfg(target_os = "macos")]
    println!("cargo:rustc-link-arg=-Wl,-rpath,@loader_path/../lib");

    #[cfg(target_os = "linux")]
    println!("cargo:rustc-link-arg=-Wl,-rpath,$ORIGIN/../lib");
}
// rustflags = ["-C", "link-args=-Wl,-rpath,$ORIGIN/../lib/"]
