fn main() {
    println!("cargo:rerun-if-env-changed=COLDEM_GITHUB_REPOSITORY");
    println!("cargo:rerun-if-env-changed=COLDEM_MANIFEST_URL");
    println!("cargo:rerun-if-env-changed=COLDEM_CATALOG_PUBKEY");
    #[cfg(target_os = "windows")]
    {
        println!("cargo:rustc-link-search=native=vendor/discord");
        println!("cargo:rustc-link-lib=dylib=discord_partner_sdk");
        println!("cargo:rerun-if-changed=vendor/discord/discord_partner_sdk.lib");
    }
    tauri_build::build()
}
