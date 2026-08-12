# butler sidecar binaries

Tauri expects one target-suffixed binary in this directory, for example:

- `butler-x86_64-pc-windows-msvc.exe`
- `butler-x86_64-unknown-linux-gnu`
- `butler-aarch64-apple-darwin`

Run `pnpm sidecar:fetch` from the project root to download the current butler
build for the host platform and put it here with the correct Tauri filename.

Sidecar executables are intentionally ignored by git. For reproducible release
builds, pin and checksum the butler artifact in your CI pipeline.
