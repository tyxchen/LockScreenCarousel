fn main() {
    if let Ok(bin_name) = std::env::var("CARGO_BIN_NAME") {
        if bin_name != "app" {
            return;
        }
    } else {
        return;
    }

    windows_reactor_setup::as_self_contained();
}
