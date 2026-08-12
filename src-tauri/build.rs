fn main() {
    println!("cargo:rerun-if-env-changed=COLDEM_GITHUB_REPOSITORY");
    println!("cargo:rerun-if-env-changed=COLDEM_MANIFEST_URL");
    println!("cargo:rerun-if-env-changed=COLDEM_CATALOG_PUBKEY");
    tauri_build::build()
}
