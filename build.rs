use std::env;
use std::path::PathBuf;

fn main() {
    let manifest_dir =
        PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR is required"));
    let sdk_lib_dir = manifest_dir.join("vendor/linux/x86_64/hik_sdk/lib");
    let sdk_com_dir = sdk_lib_dir.join("HCNetSDKCom");

    // Embed RUNPATH entries into the binary so vendored SDK dependencies
    // resolve when loading HCNetSDK/PlayCtrl with cargo run.
    println!("cargo:rustc-link-arg=-Wl,-rpath,{}", sdk_lib_dir.display());
    println!("cargo:rustc-link-arg=-Wl,-rpath,{}", sdk_com_dir.display());
}
