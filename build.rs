use embed_manifest::{embed_manifest, new_manifest, manifest::*};

fn main() {
    if let Ok(bin_name) = std::env::var("CARGO_BIN_NAME") {
        if bin_name == "app" {
            windows_reactor_setup::as_self_contained();
        }
    }

    let manifest = new_manifest("LockScreenCarousel")
        .long_path_aware(Setting::Enabled)
        .supported_os(SupportedOS::Windows10..);
    embed_manifest(manifest).expect("Unable to embed manifest.");
    println!("cargo:rerun-if-changed=build.rs");
}
