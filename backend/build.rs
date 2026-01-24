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
    // From here: https://crates.io/crates/bind-builder BUT cannot get it working so using rustc-link-search instead. Followed by using install_name_tool as noted in README.md

    // Add absolute path to rpath for doctests or other binaries running outside the target dir
    // This is useful effectively only for local dev but ensures doctests work.
    let lib_path = Path::new(&dir).join("target/lib");

    #[cfg(target_os = "macos")]
    {
        println!("cargo:rustc-link-arg=-Wl,-rpath,@loader_path/../lib");
        println!("cargo:rustc-link-arg=-Wl,-rpath,@loader_path/../../lib");
        println!("cargo:rustc-link-arg=-Wl,-rpath,{}", lib_path.display());
    }

    #[cfg(target_os = "linux")]
    {
        println!("cargo:rustc-link-arg=-Wl,-rpath,$ORIGIN/../lib");
        println!("cargo:rustc-link-arg=-Wl,-rpath,$ORIGIN/../../lib");
        println!("cargo:rustc-link-arg=-Wl,-rpath,{}", lib_path.display());
    }
}
// rustflags = ["-C", "link-args=-Wl,-rpath,$ORIGIN/../lib/"]
