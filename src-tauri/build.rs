fn main() {
    println!("cargo:rerun-if-env-changed=COLDEM_GITHUB_REPOSITORY");
    println!("cargo:rerun-if-env-changed=COLDEM_MANIFEST_URL");
    println!("cargo:rerun-if-env-changed=COLDEM_CATALOG_PUBKEY");
    println!("cargo:rerun-if-env-changed=COLDEM_UPDATE_ENDPOINT");
    println!("cargo:rerun-if-env-changed=COLDEM_UPDATE_PUBKEY");
    tauri_build::build()
}
