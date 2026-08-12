# Coldem launcher

A modular Tauri 2 + React + TypeScript launcher for DanCold games. Players do
not need an itch.io account, a GitHub account, or an API key. Public GitHub
Releases host the catalog and game artifacts; the bundled `butler` executable
installs and patches them with the Wharf format.

## Run it

```powershell
pnpm install
pnpm sidecar:fetch
pnpm tauri dev
```

`pnpm dev` is a browser-only UI preview backed by a small Robot Rock demo.
The packaged app always uses the Rust delivery backend.

## Delivery architecture

```text
React features
  library / install dialog / download states
        | typed LauncherClient
        v
Tauri commands
        |
        v
DeliveryService
  catalog cache ---- local installation receipts
  resumable HTTPS downloads + SHA-256 verification
        |
        v
bundled butler CLI
  Wharf full package / incremental patch / signature verification
        |
        v
managed game folders + launch executable
```

- `src/lib/launcherClient.ts` is the frontend transport boundary.
- `src/hooks/useLauncher.ts` owns product state and orchestration.
- `src-tauri/src/commands.rs` is the narrow Tauri command API.
- `src-tauri/src/delivery.rs` owns catalogs, downloads, integrity checks,
  installation records, updates, and launching.
- `scripts/publish-game.mjs` creates Wharf packages and GitHub Releases.
- `src-tauri/src/app_updates.rs` independently updates the launcher itself.

The manifest stores ordinary HTTPS URLs. GitHub is the first storage provider,
not a dependency baked into the UI, so moving artifacts to R2 or Bunny later
does not require redesigning the launcher.

## Recommended GitHub setup

Create a dedicated **public** repository such as `OWNER/coldem-delivery`. It
does not need source code; its Releases are the content service. Keep old
Releases because new manifests can reference full packages from earlier tags.

Install and authenticate the GitHub CLI on the creator computer:

```powershell
gh auth login
gh repo create OWNER/coldem-delivery --public --add-readme
```

The initial README gives Releases a default-branch commit to tag. This local
Coldem folder does not currently have to be a Git repository because the
publisher always passes `--repo` explicitly.

## Publish Robot Rock

First validate the build without uploading anything:

```powershell
pnpm game:publish -- `
  --repo OWNER/coldem-delivery `
  --game-id 1 `
  --slug robot-rock `
  --title "Robot Rock" `
  --version 0.1.0 `
  --build "D:\Builds\Robot Rock" `
  --executable "Robot Rock.exe" `
  --summary "Your short description" `
  --cover-file "D:\Art\robot-rock-cover.png" `
  --dry-run
```

Remove `--dry-run` to create the Release. On the first publish the script
creates a full Wharf package. It saves a private local snapshot under
`.coldem-publisher/`; the next version creates both a new full package and a
small direct patch from the previous version.

The command also writes and uploads `coldem-manifest.json`. Each artifact URL
points to an immutable tag, while the launcher reads the moving catalog URL:

```text
https://github.com/OWNER/coldem-delivery/releases/latest/download/coldem-manifest.json
```

GitHub accepts each `.pwr`, `.pwr.sig`, cover, and manifest as a separate
asset. The publisher rejects an individual asset at or above 2 GiB. If a full
Wharf package reaches that threshold, move game artifacts to an object-storage
provider; the catalog format already supports that migration.

## Connect the launcher to the catalog

The repository setting is compiled into distributable builds:

```powershell
$env:COLDEM_GITHUB_REPOSITORY = "OWNER/coldem-delivery"
pnpm tauri build
```

For development or another storage provider, use the complete URL instead:

```powershell
$env:COLDEM_MANIFEST_URL = "https://downloads.example.com/coldem-manifest.json"
pnpm tauri dev
```

Without either setting the launcher opens normally with an empty catalog and a
configuration notice. Once configured, friends simply open Coldem and press
Install; they never authenticate with GitHub.

## Integrity and signing

Every `.pwr` and Wharf `.sig` has an SHA-256 digest in the catalog. Butler also
checks the resulting installed files against the Wharf signature. For a public
production release, sign the catalog itself with Minisign so an attacker cannot
replace both a package and its digest.

```powershell
minisign -G -p "D:\Keys\coldem.pub" -s "D:\Keys\coldem.key"
pnpm game:publish -- <normal arguments> --minisign-key "D:\Keys\coldem.key"

$env:COLDEM_CATALOG_PUBKEY = (Get-Content -Raw "D:\Keys\coldem.pub")
$env:COLDEM_GITHUB_REPOSITORY = "OWNER/coldem-delivery"
pnpm tauri build
```

Back up the private key and never commit or share it. A launcher built with the
public key refuses unsigned or incorrectly signed catalogs. Development builds
without it still verify artifact SHA-256 and show a security notice.

## Update the launcher itself

Game updates and launcher updates are separate. Use another public repository,
for example `OWNER/coldem-launcher`, for signed Tauri updater artifacts. Do not
mix it with `coldem-delivery`, because both workflows need their own `latest`
Release.

Generate and back up the Tauri signing key once, then build the release:

```powershell
pnpm tauri signer generate -w "D:\Keys\coldem-tauri.key"
$env:TAURI_SIGNING_PRIVATE_KEY = "D:\Keys\coldem-tauri.key"
$env:COLDEM_UPDATE_PUBKEY = Get-Content -Raw "D:\Keys\coldem-tauri.key.pub"
$env:COLDEM_UPDATE_ENDPOINT = "https://github.com/OWNER/coldem-launcher/releases/latest/download/latest.json"
$env:COLDEM_GITHUB_REPOSITORY = "OWNER/coldem-delivery"
pnpm tauri build --config src-tauri/tauri.release.conf.json
```

Upload the generated updater bundle, its signature, and a Tauri-compatible
`latest.json` to the launcher Release. The installer can also live in that
Release for friends downloading Coldem for the first time.

## Verification

```powershell
pnpm test
pnpm build
cargo check --manifest-path src-tauri/Cargo.toml
```
