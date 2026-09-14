fn main() {
    println!("cargo:rerun-if-env-changed=XIX_GATEWAY_URL");
    println!("cargo:rerun-if-env-changed=XIX_GATEWAY_PUBLIC_KEY_B64");
    tauri_build::build()
}
