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

Game delivery is configured to use the dedicated public repository
`DnCold/Coldem-delivery`. Its Releases are the content service. Keep old
Releases because new manifests can reference full packages from earlier tags.

Install and authenticate the GitHub CLI on the creator computer:

```powershell
gh auth login
gh repo view DnCold/Coldem-delivery
```

The repository README gives Releases a default-branch commit to tag. The game
publisher always targets the repository passed through `--repo` explicitly.

## Publish Robot Rock

First validate the build without uploading anything:

```powershell
pnpm game:publish -- `
  --repo DnCold/Coldem-delivery `
  --game-id 1 `
  --slug robot-rock-reborn `
  --title "Robot Rock Reborn" `
  --version 0.1.0 `
  --build "D:\Builds\Robot Rock Reborn" `
  --executable "RobotRock2026.exe" `
  --summary "Your short description" `
  --cover-file "D:\Art\robot-rock-reborn-cover.png" `
  --dry-run
```

Remove `--dry-run` to create the Release. On the first publish the script
creates a full Wharf package. It saves a private local snapshot under
`.coldem-publisher/`; the next version creates both a new full package and a
small direct patch from the previous version.

The command also writes and uploads `coldem-manifest.json`. Each artifact URL
points to an immutable tag, while the launcher reads the moving catalog URL:

```text
https://github.com/DnCold/Coldem-delivery/releases/latest/download/coldem-manifest.json
```

GitHub accepts each `.pwr`, `.pwr.sig`, cover, and manifest as a separate
asset. The publisher rejects an individual asset at or above 2 GiB. If a full
Wharf package reaches that threshold, move game artifacts to an object-storage
provider; the catalog format already supports that migration.

On Windows, if GitHub CLI is installed but `gh` is not available on `PATH`,
point the publisher directly to the executable before running `game:publish`:

```powershell
$env:GH_PATH = "C:\path\to\gh.exe"
```

## Connect the launcher to the catalog

The distributable launcher defaults to `DnCold/Coldem-delivery`. To override
it for another deployment, set the repository while building:

```powershell
$env:COLDEM_GITHUB_REPOSITORY = "OTHER/DELIVERY-REPOSITORY"
pnpm tauri build
```

For development or another storage provider, use the complete URL instead:

```powershell
$env:COLDEM_MANIFEST_URL = "https://downloads.example.com/coldem-manifest.json"
pnpm tauri dev
```

If the remote catalog is temporarily unreachable, the launcher falls back to
the last verified copy. Friends simply open Coldem and press Install; they
never authenticate with GitHub.

## Integrity and signing

Every `.pwr` and Wharf `.sig` has an SHA-256 digest in the catalog. Butler also
checks the resulting installed files against the Wharf signature. For a public
production release, sign the catalog itself with Minisign so an attacker cannot
replace both a package and its digest.

```powershell
minisign -G -p "D:\Keys\coldem.pub" -s "D:\Keys\coldem.key"
pnpm game:publish -- <normal arguments> --minisign-key "D:\Keys\coldem.key"
```

This deployment uses the catalog private key at
`C:\Users\DanCold\.minisign\coldem-catalog.key`; it stays outside Git and must
be passed to every production `game:publish` command. Its public verification
key is compiled into Coldem. Back up the private key and never commit or share
it: the launcher refuses unsigned or incorrectly signed remote catalogs.

If `minisign` is not available on `PATH`, set `MINISIGN_PATH` or pass
`--minisign-bin` with its full executable path.

## Update the launcher itself

Game updates and launcher updates are separate. Use `DnCold/Coldem-launcher`
for signed Tauri updater artifacts. Do not mix launcher artifacts with
`Coldem-delivery`, because both workflows need their own `latest` Release.

The app checks this endpoint when it starts:

```text
https://github.com/DnCold/Coldem-launcher/releases/latest/download/latest.json
```

When a newer signed version exists, the update icon lights up. The player can
download it, Coldem installs it in passive mode, and then relaunches. The public
verification key is compiled into the app; the private signing key must never
be committed.

The private key for this deployment is stored locally at:

```text
C:\Users\DanCold\.tauri\coldem-launcher.key
```

Back it up somewhere secure. Losing it means existing installations can no
longer accept new updates. Store its contents as the
`TAURI_SIGNING_PRIVATE_KEY` Actions secret in `DnCold/Coldem-launcher`. If the
key is password protected, also store the password as
`TAURI_SIGNING_PRIVATE_KEY_PASSWORD`.

The `Release launcher` workflow builds the Windows NSIS installer, signs it,
publishes a GitHub Release, and generates `latest.json`. To publish, update the
same semantic version in `package.json`, `src-tauri/tauri.conf.json`, and
`src-tauri/Cargo.toml`, commit it, then push the matching tag:

```powershell
git tag v0.2.1
git push origin v0.2.1
```

The tag and app version must match exactly. Version `0.2.0` is the first build
that knows this update endpoint, so older installations need to install it once
manually; every later version can update through the launcher.

## Verification

```powershell
pnpm test
pnpm build
cargo check --manifest-path src-tauri/Cargo.toml
```
