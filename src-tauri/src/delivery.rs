use std::{
    collections::{HashMap, HashSet},
    fs,
    io::Read,
    path::{Component, Path, PathBuf},
    process::{Child, Command},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use futures_util::StreamExt;
use minisign_verify::{PublicKey, Signature};
use reqwest::{header, Client, StatusCode, Url};
use semver::Version;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_shell::ShellExt;
use time::{format_description::well_known::Rfc3339, OffsetDateTime};
use tokio::{fs::OpenOptions, io::AsyncWriteExt, sync::RwLock};

use crate::social::GameBridgeLaunch;

const EMBEDDED_CATALOG: &str = include_str!("../resources/coldem-manifest.json");
const PLATFORM: &str = "windows-x86_64";
const PROFILE_ID: u64 = 1;
const DEFAULT_GITHUB_REPOSITORY: &str = "DnCold/Coldem-delivery";
const DEFAULT_CATALOG_PUBKEY: &str = "RWQvH5Mf5IVDcKzMNgcT3TKJMI0U39FxX0lyOZs4ONyCkWXZVih1IQoj";
const CATALOG_FETCH_ATTEMPTS: usize = 4;
const CATALOG_RETRY_DELAYS_MS: [u64; CATALOG_FETCH_ATTEMPTS - 1] = [250, 750, 1_500];

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogManifest {
    pub schema_version: u32,
    pub published_at: String,
    pub games: Vec<GameRelease>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GameRelease {
    pub id: u64,
    pub slug: String,
    pub title: String,
    #[serde(default)]
    pub summary: String,
    #[serde(default)]
    pub cover_url: String,
    #[serde(default)]
    pub page_url: String,
    pub platforms: HashMap<String, PlatformRelease>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlatformRelease {
    pub version: String,
    pub executable: String,
    #[serde(default)]
    pub installed_size: u64,
    pub full: Artifact,
    #[serde(default)]
    pub patches: Vec<PatchRelease>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PatchRelease {
    pub from_version: String,
    pub artifact: Artifact,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Artifact {
    pub url: String,
    pub size: u64,
    pub sha256: String,
    pub signature_url: String,
    pub signature_sha256: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct InstallReceipt {
    game_id: u64,
    slug: String,
    version: String,
    executable: String,
    install_dir: String,
    installed_at: String,
    last_touched_at: String,
    #[serde(default)]
    seconds_run: u64,
    #[serde(default)]
    installed_size: u64,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct OperationEvent {
    kind: String,
    game_id: u64,
    state: String,
    progress: Option<f64>,
    eta: Option<f64>,
    bps: Option<f64>,
    message: Option<String>,
}

pub struct DeliveryService {
    client: Client,
    catalog: RwLock<Option<CatalogManifest>>,
    warning: RwLock<Option<String>>,
    channel: RwLock<String>,
}

impl Default for DeliveryService {
    fn default() -> Self {
        Self {
            client: Client::builder()
                .user_agent(concat!("Coldem/", env!("CARGO_PKG_VERSION")))
                .connect_timeout(Duration::from_secs(15))
                .timeout(Duration::from_secs(120))
                .build()
                .expect("valid HTTP client"),
            catalog: RwLock::new(None),
            warning: RwLock::new(None),
            channel: RwLock::new("stable".into()),
        }
    }
}

impl DeliveryService {
    pub async fn set_channel(&self, channel: &str) -> Result<(), String> {
        validate_release_channel(channel)?;
        let channel = channel.trim().to_ascii_lowercase();
        let mut current = self.channel.write().await;
        if *current != channel {
            *current = channel;
            *self.catalog.write().await = None;
            *self.warning.write().await = None;
        }
        Ok(())
    }

    pub async fn channel(&self) -> String {
        self.channel.read().await.clone()
    }

    pub async fn butler_version(&self, app: &AppHandle) -> Result<String, String> {
        let output = app
            .shell()
            .sidecar("butler")
            .map_err(|error| format!("Could not find bundled butler: {error}"))?
            .args(["version"])
            .output()
            .await
            .map_err(|error| format!("Could not run bundled butler: {error}"))?;
        if !output.status.success() {
            return Err(command_error("butler version", &output.stderr));
        }
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    }

    pub async fn load_catalog(
        &self,
        app: &AppHandle,
        fresh: bool,
    ) -> Result<CatalogManifest, String> {
        if !fresh {
            if let Some(catalog) = self.catalog.read().await.clone() {
                return Ok(catalog);
            }
        }

        let channel = self.channel().await;
        let Some(url) = self.manifest_url(&channel).await? else {
            if channel == "test" {
                let catalog = empty_catalog();
                *self.catalog.write().await = Some(catalog.clone());
                *self.warning.write().await = Some(
                    "No Test release has been published yet. Stable remains available in Settings."
                        .into(),
                );
                return Ok(catalog);
            }
            let catalog = parse_and_validate(EMBEDDED_CATALOG.as_bytes())?;
            *self.catalog.write().await = Some(catalog.clone());
            *self.warning.write().await = Some(
                "No release catalog is configured yet. Build with COLDEM_GITHUB_REPOSITORY=owner/repository or COLDEM_MANIFEST_URL=https://.../coldem-manifest.json."
                    .into(),
            );
            return Ok(catalog);
        };

        match self.fetch_verified_catalog(&url).await {
            Ok((bytes, signature, security_warning)) => {
                let catalog = parse_and_validate(&bytes)?;
                save_catalog_cache(app, &channel, &bytes, signature.as_deref())?;
                *self.catalog.write().await = Some(catalog.clone());
                *self.warning.write().await = security_warning;
                Ok(catalog)
            }
            Err(network_error) => {
                if let Ok(bytes) = read_catalog_cache(app, &channel) {
                    if let Ok(catalog) = parse_and_validate(&bytes) {
                        *self.catalog.write().await = Some(catalog.clone());
                        *self.warning.write().await = Some(format!(
                            "GitHub could not be reached, so Coldem is using the last verified catalog: {network_error}"
                        ));
                        return Ok(catalog);
                    }
                }
                Err(format!(
                    "Could not load the Coldem release catalog: {network_error}"
                ))
            }
        }
    }

    pub async fn warning(&self) -> Option<String> {
        self.warning.read().await.clone()
    }

    async fn manifest_url(&self, channel: &str) -> Result<Option<String>, String> {
        if channel == "stable" {
            return Ok(stable_manifest_url());
        }

        if let Some(url) = option_env!("COLDEM_TEST_MANIFEST_URL")
            .map(str::to_owned)
            .or_else(|| std::env::var("COLDEM_TEST_MANIFEST_URL").ok())
            .filter(|value| !value.trim().is_empty())
        {
            return Ok(Some(url));
        }

        let Some(repository) = github_repository() else {
            return Ok(None);
        };
        let url = format!("https://api.github.com/repos/{repository}/releases?per_page=100");
        let releases = self
            .client
            .get(url)
            .send()
            .await
            .map_err(|error| format!("Could not list Test releases: {error}"))?
            .error_for_status()
            .map_err(|error| format!("GitHub could not list Test releases: {error}"))?
            .json::<Vec<GithubRelease>>()
            .await
            .map_err(|error| format!("GitHub returned an invalid Test release list: {error}"))?;
        Ok(releases
            .into_iter()
            .filter(|release| {
                release.prerelease
                    && !release.draft
                    && release.tag_name.starts_with("delivery-test-")
            })
            .max_by_key(|release| release.published_at.clone().unwrap_or_default())
            .and_then(|release| {
                release
                    .assets
                    .into_iter()
                    .find(|asset| asset.name == "coldem-manifest.json")
                    .map(|asset| asset.browser_download_url)
            }))
    }

    pub async fn library(&self, app: &AppHandle, fresh: bool) -> Result<Value, String> {
        let catalog = self.load_catalog(app, fresh).await?;
        let receipts = load_receipts(app)?;
        let receipt_by_game = receipts
            .iter()
            .map(|receipt| (receipt.game_id, receipt))
            .collect::<HashMap<_, _>>();

        let records = catalog
            .games
            .iter()
            .filter(|game| game.platforms.contains_key(PLATFORM))
            .map(|game| {
                let receipt = receipt_by_game.get(&game.id);
                json!({
                    "id": game.id,
                    "slug": game.slug,
                    "title": game.title,
                    "url": optional_string(&game.page_url),
                    "cover": optional_string(&game.cover_url),
                    "owned": true,
                    "installedAt": receipt.map(|item| item.installed_at.clone())
                })
            })
            .collect::<Vec<_>>();

        let caves = receipts
            .iter()
            .filter_map(|receipt| {
                catalog
                    .games
                    .iter()
                    .find(|game| game.id == receipt.game_id)
                    .and_then(|game| game.platforms.get(PLATFORM).map(|release| (game, release)))
                    .map(|(game, release)| cave_json(game, release, receipt))
            })
            .collect::<Vec<_>>();

        let warning = self.warning().await;
        Ok(json!({
            "records": records,
            "caves": caves,
            "stale": false,
            "warning": warning
        }))
    }

    pub async fn game_id_for_slug(&self, app: &AppHandle, slug: &str) -> Result<u64, String> {
        let catalog = self.load_catalog(app, false).await?;
        catalog
            .games
            .iter()
            .find(|game| game.slug == slug && game.platforms.contains_key(PLATFORM))
            .map(|game| game.id)
            .ok_or_else(|| format!("The invited game '{slug}' is not available in this Coldem catalog."))
    }

    pub async fn game_social_info(
        &self,
        app: &AppHandle,
        game_id: u64,
    ) -> Result<(String, String), String> {
        let catalog = self.load_catalog(app, false).await?;
        catalog
            .games
            .iter()
            .find(|game| game.id == game_id)
            .map(|game| (game.slug.clone(), game.title.clone()))
            .ok_or_else(|| format!("The game {game_id} is not available in this Coldem catalog."))
    }

    pub async fn updates(&self, app: &AppHandle) -> Result<Value, String> {
        let catalog = self.load_catalog(app, true).await?;
        let receipts = load_receipts(app)?;
        let updates = receipts
            .iter()
            .filter_map(|receipt| {
                let game = catalog
                    .games
                    .iter()
                    .find(|game| game.id == receipt.game_id)?;
                let release = game.platforms.get(PLATFORM)?;
                if !is_newer(&release.version, &receipt.version) {
                    return None;
                }
                let direct = release
                    .patches
                    .iter()
                    .any(|patch| patch.from_version == receipt.version);
                Some(json!({
                    "caveId": cave_id(game.id),
                    "game": game_json(game),
                    "direct": direct,
                    "choices": [{
                        "upload": upload_json(game, release),
                        "build": { "version": release.version },
                        "confidence": if direct { 1.0 } else { 0.75 }
                    }]
                }))
            })
            .collect::<Vec<_>>();
        Ok(Value::Array(updates))
    }

    pub async fn install_options(&self, app: &AppHandle, game_id: u64) -> Result<Value, String> {
        let catalog = self.load_catalog(app, false).await?;
        let (game, release) = find_release(&catalog, game_id)?;
        Ok(json!({
            "game": game_json(game),
            "uploads": [upload_json(game, release)],
            "incompatibleUploads": [],
            "installLocationId": games_dir(app)?.to_string_lossy()
        }))
    }

    pub async fn install_plan(&self, app: &AppHandle, upload_id: u64) -> Result<Value, String> {
        let catalog = self.load_catalog(app, false).await?;
        let (game, release) = find_release(&catalog, upload_id)?;
        let required = release
            .installed_size
            .saturating_add(release.full.size)
            .saturating_add(256 * 1024 * 1024);
        Ok(json!({
            "info": {
                "upload": upload_json(game, release),
                "build": { "version": release.version },
                "type": "wharf",
                "diskUsage": {
                    "finalDiskUsage": release.installed_size,
                    "neededFreeSpace": required,
                    "accuracy": "estimate"
                }
            }
        }))
    }

    pub async fn install(&self, app: &AppHandle, game_id: u64) -> Result<(), String> {
        self.emit(
            app,
            "install",
            game_id,
            "queued",
            Some(0.0),
            None,
            None,
            None,
        );
        let result = self.install_inner(app, game_id).await;
        self.finish_operation(app, "install", game_id, &result);
        result
    }

    async fn install_inner(&self, app: &AppHandle, game_id: u64) -> Result<(), String> {
        let catalog = self.load_catalog(app, true).await?;
        let (game, release) = find_release(&catalog, game_id)?;
        let root = games_dir(app)?;
        fs::create_dir_all(&root)
            .map_err(|error| format!("Could not create games directory: {error}"))?;
        let install_dir = root.join(&game.slug);
        let next_dir = root.join(format!("{}.installing", game.slug));
        let empty_dir = root.join(".empty");
        clear_dir(&next_dir)?;
        fs::create_dir_all(&empty_dir)
            .map_err(|error| format!("Could not create Wharf source directory: {error}"))?;

        let (patch, signature) = self
            .download_artifact(app, game, &release.full, "install")
            .await?;
        let staging = downloads_dir(app)?.join(format!("{}-staging", game.slug));
        clear_dir(&staging)?;
        fs::create_dir_all(&staging)
            .map_err(|error| format!("Could not create install staging directory: {error}"))?;

        self.emit(
            app,
            "install",
            game_id,
            "working",
            Some(0.96),
            None,
            None,
            Some("Applying game files".into()),
        );
        run_butler_apply(app, &patch, &empty_dir, &next_dir, &staging, &signature).await?;
        replace_install_dir(&install_dir, &next_dir)?;
        upsert_receipt(
            app,
            InstallReceipt {
                game_id,
                slug: game.slug.clone(),
                version: release.version.clone(),
                executable: release.executable.clone(),
                install_dir: install_dir.to_string_lossy().into_owned(),
                installed_at: now_string(),
                last_touched_at: now_string(),
                seconds_run: 0,
                installed_size: release.installed_size,
            },
        )?;
        Ok(())
    }

    pub async fn update(&self, app: &AppHandle, game_id: u64) -> Result<(), String> {
        self.emit(
            app,
            "update",
            game_id,
            "queued",
            Some(0.0),
            None,
            None,
            None,
        );
        let result = self.update_inner(app, game_id).await;
        self.finish_operation(app, "update", game_id, &result);
        result
    }

    async fn update_inner(&self, app: &AppHandle, game_id: u64) -> Result<(), String> {
        let catalog = self.load_catalog(app, true).await?;
        let (game, release) = find_release(&catalog, game_id)?;
        let mut receipts = load_receipts(app)?;
        let receipt = receipts
            .iter_mut()
            .find(|receipt| receipt.game_id == game_id)
            .ok_or_else(|| "Install the game before updating it".to_string())?;
        if !is_newer(&release.version, &receipt.version) {
            return Err("This game is already up to date".into());
        }

        if let Some(direct) = release
            .patches
            .iter()
            .find(|patch| patch.from_version == receipt.version)
        {
            let (patch, signature) = self
                .download_artifact(app, game, &direct.artifact, "update")
                .await?;
            let install_dir = PathBuf::from(&receipt.install_dir);
            ensure_managed_install(app, &install_dir)?;
            let staging = downloads_dir(app)?.join(format!("{}-staging", game.slug));
            clear_dir(&staging)?;
            fs::create_dir_all(&staging)
                .map_err(|error| format!("Could not create update staging directory: {error}"))?;
            self.emit(
                app,
                "update",
                game_id,
                "working",
                Some(0.96),
                None,
                None,
                Some("Applying patch".into()),
            );
            run_butler_apply_in_place(app, &patch, &install_dir, &staging, &signature).await?;
        } else {
            self.reinstall_current(app, game, release, "update").await?;
        }

        receipt.version = release.version.clone();
        receipt.executable = release.executable.clone();
        receipt.last_touched_at = now_string();
        receipt.installed_size = release.installed_size;
        save_receipts(app, &receipts)?;
        Ok(())
    }

    async fn reinstall_current(
        &self,
        app: &AppHandle,
        game: &GameRelease,
        release: &PlatformRelease,
        operation: &str,
    ) -> Result<(), String> {
        let root = games_dir(app)?;
        let install_dir = root.join(&game.slug);
        let next_dir = root.join(format!("{}.installing", game.slug));
        let empty_dir = root.join(".empty");
        clear_dir(&next_dir)?;
        fs::create_dir_all(&empty_dir)
            .map_err(|error| format!("Could not create Wharf source directory: {error}"))?;
        let (patch, signature) = self
            .download_artifact(app, game, &release.full, operation)
            .await?;
        let staging = downloads_dir(app)?.join(format!("{}-staging", game.slug));
        clear_dir(&staging)?;
        fs::create_dir_all(&staging)
            .map_err(|error| format!("Could not create install staging directory: {error}"))?;
        run_butler_apply(app, &patch, &empty_dir, &next_dir, &staging, &signature).await?;
        replace_install_dir(&install_dir, &next_dir)
    }

    pub fn play(
        &self,
        app: &AppHandle,
        game_id: u64,
        bridge: &GameBridgeLaunch,
        join_payload: Option<&str>,
    ) -> Result<Child, String> {
        self.emit(app, "play", game_id, "queued", Some(0.0), None, None, None);
        let result = (|| {
            let mut receipts = load_receipts(app)?;
            let receipt = receipts
                .iter_mut()
                .find(|receipt| receipt.game_id == game_id)
                .ok_or_else(|| "Install the game before playing it".to_string())?;
            let install_dir = PathBuf::from(&receipt.install_dir);
            ensure_managed_install(app, &install_dir)?;
            let relative = safe_relative_path(&receipt.executable, "game executable")?;
            let executable = install_dir.join(relative);
            if !executable.is_file() {
                return Err(format!(
                    "The game executable was not found: {}",
                    executable.display()
                ));
            }
            let canonical_install = fs::canonicalize(&install_dir)
                .map_err(|error| format!("Could not validate game installation: {error}"))?;
            let canonical_executable = fs::canonicalize(&executable)
                .map_err(|error| format!("Could not validate game executable: {error}"))?;
            if !canonical_executable.starts_with(&canonical_install) {
                return Err("The game executable resolves outside its managed installation".into());
            }
            let mut command = Command::new(&canonical_executable);
            command
                .current_dir(&install_dir)
                .env("COLDEM_SOCIAL_ENDPOINT", &bridge.endpoint)
                .env("COLDEM_SOCIAL_TOKEN", &bridge.token);
            if let Some(join_payload) = join_payload {
                command.arg(format!("--coldem-join={join_payload}"));
            }
            let child = command
                .spawn()
                .map_err(|error| format!("Could not start the game: {error}"))?;
            receipt.last_touched_at = now_string();
            save_receipts(app, &receipts)?;
            Ok(child)
        })();
        match &result {
            Ok(_) => self.emit(
                app,
                "play",
                game_id,
                "running",
                Some(1.0),
                None,
                None,
                Some("Game session is running".into()),
            ),
            Err(error) => self.finish_operation(app, "play", game_id, &Err(error.clone())),
        }
        result
    }

    pub fn record_play_finished(
        &self,
        app: &AppHandle,
        game_id: u64,
        duration: Duration,
    ) -> Result<(), String> {
        let seconds = duration.as_secs();
        let mut receipts = load_receipts(app)?;
        let receipt = receipts
            .iter_mut()
            .find(|receipt| receipt.game_id == game_id)
            .ok_or_else(|| "Could not find the installed game session".to_string())?;
        receipt.seconds_run = receipt.seconds_run.saturating_add(seconds);
        receipt.last_touched_at = now_string();
        save_receipts(app, &receipts)?;
        self.emit(
            app,
            "play",
            game_id,
            "finished",
            Some(1.0),
            None,
            None,
            Some(format!("Session complete · {seconds}s recorded")),
        );
        Ok(())
    }

    async fn download_artifact(
        &self,
        app: &AppHandle,
        game: &GameRelease,
        artifact: &Artifact,
        operation: &str,
    ) -> Result<(PathBuf, PathBuf), String> {
        let dir = downloads_dir(app)?.join(&game.slug);
        fs::create_dir_all(&dir)
            .map_err(|error| format!("Could not create download directory: {error}"))?;
        let patch = dir.join(filename_from_url(&artifact.url, "game.pwr")?);
        let signature = dir.join(filename_from_url(&artifact.signature_url, "game.pwr.sig")?);
        self.download_verified(
            app,
            game.id,
            operation,
            &artifact.url,
            &patch,
            artifact.size,
            &artifact.sha256,
            0.0,
            0.9,
        )
        .await?;
        self.download_verified(
            app,
            game.id,
            operation,
            &artifact.signature_url,
            &signature,
            0,
            &artifact.signature_sha256,
            0.9,
            0.95,
        )
        .await?;
        Ok((patch, signature))
    }

    #[allow(clippy::too_many_arguments)]
    async fn download_verified(
        &self,
        app: &AppHandle,
        game_id: u64,
        operation: &str,
        url: &str,
        destination: &Path,
        expected_size: u64,
        expected_sha256: &str,
        progress_start: f64,
        progress_end: f64,
    ) -> Result<(), String> {
        validate_download_url(url)?;
        validate_sha256(expected_sha256)?;
        if destination.is_file()
            && sha256_file(destination)? == expected_sha256.to_ascii_lowercase()
        {
            return Ok(());
        }

        let partial = destination.with_extension(format!(
            "{}part",
            destination
                .extension()
                .and_then(|value| value.to_str())
                .map(|value| format!("{value}."))
                .unwrap_or_default()
        ));
        let mut downloaded = fs::metadata(&partial).map(|meta| meta.len()).unwrap_or(0);
        if expected_size > 0 && downloaded > expected_size {
            fs::remove_file(&partial)
                .map_err(|error| format!("Could not reset partial download: {error}"))?;
            downloaded = 0;
        }

        let mut request = self.client.get(url);
        if downloaded > 0 {
            request = request.header(header::RANGE, format!("bytes={downloaded}-"));
        }
        let response = request
            .send()
            .await
            .map_err(|error| format!("Download failed: {error}"))?;
        let append = downloaded > 0 && response.status() == StatusCode::PARTIAL_CONTENT;
        if !response.status().is_success() {
            return Err(format!(
                "Download returned HTTP {} for {url}",
                response.status()
            ));
        }
        if !append {
            downloaded = 0;
        }
        let mut file = OpenOptions::new()
            .create(true)
            .write(true)
            .append(append)
            .truncate(!append)
            .open(&partial)
            .await
            .map_err(|error| format!("Could not open partial download: {error}"))?;
        let response_length = response.content_length().unwrap_or(0);
        let total = if expected_size > 0 {
            expected_size
        } else {
            downloaded + response_length
        };
        let started = Instant::now();
        let initial = downloaded;
        let mut stream = response.bytes_stream();
        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(|error| format!("Download stream failed: {error}"))?;
            file.write_all(&chunk)
                .await
                .map_err(|error| format!("Could not save download: {error}"))?;
            downloaded += chunk.len() as u64;
            let fraction = if total > 0 {
                downloaded as f64 / total as f64
            } else {
                0.0
            };
            let elapsed = started.elapsed().as_secs_f64().max(0.001);
            let bps = (downloaded.saturating_sub(initial)) as f64 / elapsed;
            let eta = if bps > 0.0 && total > downloaded {
                Some((total - downloaded) as f64 / bps)
            } else {
                Some(0.0)
            };
            self.emit(
                app,
                operation,
                game_id,
                "working",
                Some(progress_start + fraction.clamp(0.0, 1.0) * (progress_end - progress_start)),
                eta,
                Some(bps),
                Some("Downloading from GitHub Releases".into()),
            );
        }
        file.flush()
            .await
            .map_err(|error| format!("Could not flush download: {error}"))?;
        drop(file);

        if expected_size > 0 && downloaded != expected_size {
            return Err(format!(
                "Downloaded {downloaded} bytes, expected {expected_size}"
            ));
        }
        let actual_sha = sha256_file(&partial)?;
        if actual_sha != expected_sha256.to_ascii_lowercase() {
            let _ = fs::remove_file(&partial);
            return Err(format!("Downloaded file failed SHA-256 verification (expected {expected_sha256}, got {actual_sha})"));
        }
        if destination.exists() {
            fs::remove_file(destination)
                .map_err(|error| format!("Could not replace cached download: {error}"))?;
        }
        fs::rename(&partial, destination)
            .map_err(|error| format!("Could not finalize download: {error}"))?;
        Ok(())
    }

    async fn fetch_verified_catalog(
        &self,
        url: &str,
    ) -> Result<(Vec<u8>, Option<String>, Option<String>), String> {
        validate_download_url(url)?;
        let public_key_text = catalog_public_key();
        let cache_bust = catalog_cache_bust();
        let mut last_error = String::from("catalog request failed");

        for attempt in 0..CATALOG_FETCH_ATTEMPTS {
            if attempt > 0 {
                tokio::time::sleep(Duration::from_millis(CATALOG_RETRY_DELAYS_MS[attempt - 1])).await;
            }

            let result = async {
                // GitHub's release/latest CDN can briefly serve the manifest and its
                // detached signature from different release generations. Give each
                // pair a fresh cache key so both files converge on the same publish.
                let manifest_url = catalog_request_url(url, None, &cache_bust, attempt)?;
                let response = self
                    .client
                    .get(manifest_url)
                    .send()
                    .await
                    .map_err(|error| error.to_string())?;
                if !response.status().is_success() {
                    return Err(format!("catalog returned HTTP {}", response.status()));
                }
                let bytes = response
                    .bytes()
                    .await
                    .map_err(|error| error.to_string())?
                    .to_vec();

                if let Some(public_key_text) = public_key_text.as_deref() {
                    let signature_url =
                        catalog_request_url(url, Some(".minisig"), &cache_bust, attempt)?;
                    let signature_response = self
                        .client
                        .get(signature_url)
                        .send()
                        .await
                        .map_err(|error| format!("could not download catalog signature: {error}"))?;
                    if !signature_response.status().is_success() {
                        return Err(format!(
                            "catalog signature returned HTTP {}",
                            signature_response.status()
                        ));
                    }
                    let signature_text = signature_response
                        .text()
                        .await
                        .map_err(|error| error.to_string())?;
                    verify_catalog_signature(&bytes, &signature_text, public_key_text)?;
                    Ok((bytes, Some(signature_text), None))
                } else {
                    Ok((
                        bytes,
                        None,
                        Some("This development build verifies every downloaded file with SHA-256, but its catalog signing key has not been configured yet.".into()),
                    ))
                }
            }
            .await;

            match result {
                Ok(catalog) => return Ok(catalog),
                Err(error) => last_error = error,
            }
        }

        Err(last_error)
    }
    fn finish_operation(
        &self,
        app: &AppHandle,
        kind: &str,
        game_id: u64,
        result: &Result<(), String>,
    ) {
        match result {
            Ok(()) => self.emit(
                app,
                kind,
                game_id,
                "finished",
                Some(1.0),
                Some(0.0),
                None,
                None,
            ),
            Err(error) => self.emit(
                app,
                kind,
                game_id,
                "failed",
                None,
                None,
                None,
                Some(error.clone()),
            ),
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn emit(
        &self,
        app: &AppHandle,
        kind: &str,
        game_id: u64,
        state: &str,
        progress: Option<f64>,
        eta: Option<f64>,
        bps: Option<f64>,
        message: Option<String>,
    ) {
        let _ = app.emit(
            "launcher://operation",
            OperationEvent {
                kind: kind.into(),
                game_id,
                state: state.into(),
                progress,
                eta,
                bps,
                message,
            },
        );
    }
}

pub fn player_profile() -> Value {
    json!({
        "id": PROFILE_ID,
        "lastConnected": now_string(),
        "user": {
            "id": PROFILE_ID,
            "username": "player",
            "displayName": "Coldem Player",
            "developer": false,
            "url": "https://github.com",
            "coverUrl": ""
        }
    })
}

#[derive(Deserialize)]
struct GithubRelease {
    tag_name: String,
    prerelease: bool,
    draft: bool,
    #[serde(default)]
    published_at: Option<String>,
    assets: Vec<GithubAsset>,
}

#[derive(Deserialize)]
struct GithubAsset {
    name: String,
    browser_download_url: String,
}

fn github_repository() -> Option<String> {
    option_env!("COLDEM_GITHUB_REPOSITORY")
        .map(str::to_owned)
        .or_else(|| std::env::var("COLDEM_GITHUB_REPOSITORY").ok())
        .filter(|value| !value.trim().is_empty())
        .or_else(|| Some(DEFAULT_GITHUB_REPOSITORY.to_owned()))
}

fn stable_manifest_url() -> Option<String> {
    option_env!("COLDEM_MANIFEST_URL")
        .map(str::to_owned)
        .or_else(|| std::env::var("COLDEM_MANIFEST_URL").ok())
        .or_else(|| {
            let repository = github_repository()?;
            Some(format!(
                "https://github.com/{repository}/releases/latest/download/coldem-manifest.json"
            ))
        })
}

fn catalog_cache_bust() -> String {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .to_string()
}

fn catalog_request_url(
    base_url: &str,
    suffix: Option<&str>,
    cache_bust: &str,
    attempt: usize,
) -> Result<String, String> {
    let mut url = Url::parse(base_url).map_err(|error| format!("Invalid catalog URL: {error}"))?;
    if let Some(suffix) = suffix {
        let mut path = url.path().to_owned();
        path.push_str(suffix);
        url.set_path(&path);
    }
    url.query_pairs_mut().append_pair(
        "coldem_catalog_retry",
        &format!("{cache_bust}-{attempt}"),
    );
    Ok(url.to_string())
}
fn empty_catalog() -> CatalogManifest {
    CatalogManifest {
        schema_version: 1,
        published_at: "1970-01-01T00:00:00Z".into(),
        games: Vec::new(),
    }
}

fn validate_release_channel(value: &str) -> Result<(), String> {
    if matches!(
        value.trim().to_ascii_lowercase().as_str(),
        "stable" | "test"
    ) {
        Ok(())
    } else {
        Err("Release channel must be stable or test".into())
    }
}

fn catalog_public_key() -> Option<String> {
    option_env!("COLDEM_CATALOG_PUBKEY")
        .map(str::to_owned)
        .or_else(|| std::env::var("COLDEM_CATALOG_PUBKEY").ok())
        .filter(|value| !value.trim().is_empty())
        .or_else(|| Some(DEFAULT_CATALOG_PUBKEY.to_owned()))
}

fn parse_and_validate(bytes: &[u8]) -> Result<CatalogManifest, String> {
    let catalog: CatalogManifest = serde_json::from_slice(bytes)
        .map_err(|error| format!("Release catalog is not valid JSON: {error}"))?;
    if catalog.schema_version != 1 {
        return Err(format!(
            "Unsupported catalog schema version {}",
            catalog.schema_version
        ));
    }
    let mut ids = HashSet::new();
    let mut slugs = HashSet::new();
    for game in &catalog.games {
        if !ids.insert(game.id) {
            return Err(format!("Duplicate game ID {} in catalog", game.id));
        }
        validate_slug(&game.slug)?;
        if !slugs.insert(game.slug.clone()) {
            return Err(format!("Duplicate game slug {} in catalog", game.slug));
        }
        for release in game.platforms.values() {
            safe_relative_path(&release.executable, "game executable")?;
            validate_artifact(&release.full)?;
            for patch in &release.patches {
                if patch.from_version.trim().is_empty() {
                    return Err(format!(
                        "{} contains a patch without fromVersion",
                        game.title
                    ));
                }
                validate_artifact(&patch.artifact)?;
            }
        }
    }
    Ok(catalog)
}

fn validate_artifact(artifact: &Artifact) -> Result<(), String> {
    if artifact.size == 0 {
        return Err("A release artifact has a zero size".into());
    }
    validate_download_url(&artifact.url)?;
    validate_download_url(&artifact.signature_url)?;
    validate_sha256(&artifact.sha256)?;
    validate_sha256(&artifact.signature_sha256)
}

fn validate_download_url(value: &str) -> Result<(), String> {
    let url = Url::parse(value).map_err(|error| format!("Invalid release URL: {error}"))?;
    if url.scheme() == "https" {
        return Ok(());
    }
    #[cfg(debug_assertions)]
    if url.scheme() == "http" && matches!(url.host_str(), Some("127.0.0.1" | "localhost")) {
        return Ok(());
    }
    Err("Release downloads must use HTTPS".into())
}

fn validate_sha256(value: &str) -> Result<(), String> {
    if value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        Ok(())
    } else {
        Err("Release catalog contains an invalid SHA-256 digest".into())
    }
}

fn validate_slug(slug: &str) -> Result<(), String> {
    if !slug.is_empty()
        && slug.len() <= 80
        && slug
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
    {
        Ok(())
    } else {
        Err(format!("Unsafe game slug in catalog: {slug}"))
    }
}

fn safe_relative_path(value: &str, label: &str) -> Result<PathBuf, String> {
    let path = Path::new(value);
    if value.trim().is_empty()
        || path.is_absolute()
        || path
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(format!("Unsafe {label} path in catalog: {value}"));
    }
    Ok(path.to_path_buf())
}

fn filename_from_url(value: &str, fallback: &str) -> Result<String, String> {
    let url = Url::parse(value).map_err(|error| format!("Invalid release URL: {error}"))?;
    let name = url
        .path_segments()
        .and_then(Iterator::last)
        .filter(|name| !name.is_empty())
        .unwrap_or(fallback);
    safe_relative_path(name, "download filename")?;
    Ok(name.to_string())
}

fn find_release(
    catalog: &CatalogManifest,
    game_id: u64,
) -> Result<(&GameRelease, &PlatformRelease), String> {
    let game = catalog
        .games
        .iter()
        .find(|game| game.id == game_id)
        .ok_or_else(|| "Game is not present in the current release catalog".to_string())?;
    let release = game
        .platforms
        .get(PLATFORM)
        .ok_or_else(|| "This game has no compatible Windows release".to_string())?;
    Ok((game, release))
}

fn game_json(game: &GameRelease) -> Value {
    json!({
        "id": game.id,
        "url": game.page_url,
        "title": game.title,
        "shortText": game.summary,
        "type": "downloadable",
        "classification": "game",
        "coverUrl": optional_string(&game.cover_url),
        "userId": 1,
        "user": {
            "id": 1,
            "username": "dancold",
            "displayName": "DanCold",
            "developer": true,
            "url": "https://github.com/DnCold"
        }
    })
}

fn upload_json(game: &GameRelease, release: &PlatformRelease) -> Value {
    json!({
        "id": game.id,
        "filename": filename_from_url(&release.full.url, "game.pwr").unwrap_or_else(|_| "game.pwr".into()),
        "displayName": format!("Windows · {}", release.version),
        "size": release.full.size,
        "channelName": PLATFORM,
        "type": "default",
        "demo": false,
        "preorder": false,
        "platforms": { "windows": true },
        "build": { "version": release.version }
    })
}

fn cave_json(game: &GameRelease, release: &PlatformRelease, receipt: &InstallReceipt) -> Value {
    json!({
        "id": cave_id(game.id),
        "game": game_json(game),
        "upload": upload_json(game, release),
        "build": { "version": receipt.version },
        "stats": {
            "installedAt": receipt.installed_at,
            "lastTouchedAt": receipt.last_touched_at,
            "secondsRun": receipt.seconds_run
        },
        "interaction": {
            "userId": PROFILE_ID,
            "gameId": game.id,
            "secondsRun": receipt.seconds_run,
            "lastRunAt": receipt.last_touched_at,
            "syncedAt": receipt.last_touched_at
        },
        "installInfo": {
            "installFolder": receipt.install_dir,
            "installedSize": receipt.installed_size
        }
    })
}

fn cave_id(game_id: u64) -> String {
    format!("coldem-{game_id}")
}

fn optional_string(value: &str) -> Option<&str> {
    (!value.trim().is_empty()).then_some(value)
}

fn is_newer(candidate: &str, current: &str) -> bool {
    match (Version::parse(candidate), Version::parse(current)) {
        (Ok(candidate), Ok(current)) => candidate > current,
        _ => candidate != current,
    }
}

fn app_data_dir(app: &AppHandle) -> Result<PathBuf, String> {
    app.path()
        .app_data_dir()
        .map_err(|error| format!("Could not resolve Coldem data directory: {error}"))
}

fn games_dir(app: &AppHandle) -> Result<PathBuf, String> {
    Ok(app_data_dir(app)?.join("games"))
}

fn downloads_dir(app: &AppHandle) -> Result<PathBuf, String> {
    Ok(app_data_dir(app)?.join("downloads"))
}

fn receipts_path(app: &AppHandle) -> Result<PathBuf, String> {
    Ok(app_data_dir(app)?.join("library.json"))
}

fn catalog_cache_path(app: &AppHandle, channel: &str) -> Result<PathBuf, String> {
    let filename = if channel == "stable" {
        // Preserve the cache location used by existing launcher builds.
        "catalog-cache.json".into()
    } else {
        format!("catalog-cache-{channel}.json")
    };
    Ok(app_data_dir(app)?.join(filename))
}

fn load_receipts(app: &AppHandle) -> Result<Vec<InstallReceipt>, String> {
    let path = receipts_path(app)?;
    if !path.exists() {
        return Ok(Vec::new());
    }
    let bytes =
        fs::read(path).map_err(|error| format!("Could not read installed games: {error}"))?;
    serde_json::from_slice(&bytes)
        .map_err(|error| format!("Installed game records are damaged: {error}"))
}

fn save_receipts(app: &AppHandle, receipts: &[InstallReceipt]) -> Result<(), String> {
    let path = receipts_path(app)?;
    let parent = path
        .parent()
        .ok_or_else(|| "Invalid library path".to_string())?;
    fs::create_dir_all(parent)
        .map_err(|error| format!("Could not create Coldem data directory: {error}"))?;
    let temporary = path.with_extension("json.tmp");
    let bytes = serde_json::to_vec_pretty(receipts).map_err(|error| error.to_string())?;
    fs::write(&temporary, bytes)
        .map_err(|error| format!("Could not save installed games: {error}"))?;
    if path.exists() {
        fs::remove_file(&path)
            .map_err(|error| format!("Could not replace installed game records: {error}"))?;
    }
    fs::rename(temporary, path)
        .map_err(|error| format!("Could not finalize installed game records: {error}"))
}

fn upsert_receipt(app: &AppHandle, receipt: InstallReceipt) -> Result<(), String> {
    let mut receipts = load_receipts(app)?;
    receipts.retain(|current| current.game_id != receipt.game_id);
    receipts.push(receipt);
    save_receipts(app, &receipts)
}

fn save_catalog_cache(
    app: &AppHandle,
    channel: &str,
    bytes: &[u8],
    signature: Option<&str>,
) -> Result<(), String> {
    let path = catalog_cache_path(app, channel)?;
    let parent = path
        .parent()
        .ok_or_else(|| "Invalid catalog cache path".to_string())?;
    fs::create_dir_all(parent)
        .map_err(|error| format!("Could not create catalog cache: {error}"))?;
    fs::write(&path, bytes).map_err(|error| format!("Could not save catalog cache: {error}"))?;
    let signature_path = path.with_extension("json.minisig");
    if let Some(signature) = signature {
        fs::write(signature_path, signature)
            .map_err(|error| format!("Could not save catalog signature cache: {error}"))?;
    } else if signature_path.exists() {
        fs::remove_file(signature_path)
            .map_err(|error| format!("Could not reset catalog signature cache: {error}"))?;
    }
    Ok(())
}

fn read_catalog_cache(app: &AppHandle, channel: &str) -> Result<Vec<u8>, String> {
    let path = catalog_cache_path(app, channel)?;
    let bytes =
        fs::read(&path).map_err(|error| format!("Could not read catalog cache: {error}"))?;
    if let Some(public_key) = catalog_public_key() {
        let signature = fs::read_to_string(path.with_extension("json.minisig"))
            .map_err(|error| format!("Could not read cached catalog signature: {error}"))?;
        verify_catalog_signature(&bytes, &signature, &public_key)?;
    }
    Ok(bytes)
}

fn verify_catalog_signature(
    bytes: &[u8],
    signature_text: &str,
    public_key_text: &str,
) -> Result<(), String> {
    let public_key = if public_key_text.contains('\n') {
        PublicKey::decode(public_key_text)
    } else {
        PublicKey::from_base64(public_key_text.trim())
    }
    .map_err(|error| format!("invalid compiled catalog public key: {error}"))?;
    let signature = Signature::decode(signature_text)
        .map_err(|error| format!("invalid catalog signature: {error}"))?;
    public_key
        .verify(bytes, &signature, false)
        .map_err(|error| format!("catalog signature verification failed: {error}"))
}

fn clear_dir(path: &Path) -> Result<(), String> {
    if path.exists() {
        fs::remove_dir_all(path)
            .map_err(|error| format!("Could not clear {}: {error}", path.display()))?;
    }
    Ok(())
}

fn replace_install_dir(install: &Path, next: &Path) -> Result<(), String> {
    let backup = install.with_extension("previous");
    clear_dir(&backup)?;
    if install.exists() {
        fs::rename(install, &backup)
            .map_err(|error| format!("Could not preserve the previous installation: {error}"))?;
    }
    if let Err(error) = fs::rename(next, install) {
        if backup.exists() {
            let _ = fs::rename(&backup, install);
        }
        return Err(format!("Could not activate the new installation: {error}"));
    }
    clear_dir(&backup)
}

fn ensure_managed_install(app: &AppHandle, install: &Path) -> Result<(), String> {
    let root = games_dir(app)?;
    let canonical_root = fs::canonicalize(&root)
        .map_err(|error| format!("Could not validate games directory: {error}"))?;
    let canonical_install = fs::canonicalize(install)
        .map_err(|error| format!("Could not validate game installation: {error}"))?;
    if canonical_install.parent() != Some(canonical_root.as_path()) {
        return Err("Installed game path is outside Coldem's managed games directory".into());
    }
    Ok(())
}

async fn run_butler_apply(
    app: &AppHandle,
    patch: &Path,
    old: &Path,
    output: &Path,
    staging: &Path,
    signature: &Path,
) -> Result<(), String> {
    let args = vec![
        "apply".to_string(),
        "--assume-yes".to_string(),
        "--staging-dir".to_string(),
        staging.to_string_lossy().into_owned(),
        "--dir".to_string(),
        output.to_string_lossy().into_owned(),
        "--signature".to_string(),
        signature.to_string_lossy().into_owned(),
        patch.to_string_lossy().into_owned(),
        old.to_string_lossy().into_owned(),
    ];
    run_butler(app, args, "apply game files").await
}

async fn run_butler_apply_in_place(
    app: &AppHandle,
    patch: &Path,
    old: &Path,
    staging: &Path,
    signature: &Path,
) -> Result<(), String> {
    let args = vec![
        "apply".to_string(),
        "--assume-yes".to_string(),
        "--staging-dir".to_string(),
        staging.to_string_lossy().into_owned(),
        "--signature".to_string(),
        signature.to_string_lossy().into_owned(),
        patch.to_string_lossy().into_owned(),
        old.to_string_lossy().into_owned(),
    ];
    run_butler(app, args, "apply game update").await
}

async fn run_butler(app: &AppHandle, args: Vec<String>, description: &str) -> Result<(), String> {
    let output = app
        .shell()
        .sidecar("butler")
        .map_err(|error| format!("Could not find bundled butler: {error}"))?
        .args(args)
        .output()
        .await
        .map_err(|error| format!("Could not run butler to {description}: {error}"))?;
    if output.status.success() {
        Ok(())
    } else {
        Err(command_error(description, &output.stderr))
    }
}

fn command_error(description: &str, stderr: &[u8]) -> String {
    let detail = String::from_utf8_lossy(stderr).trim().to_string();
    if detail.is_empty() {
        format!("Could not {description}")
    } else {
        format!("Could not {description}: {detail}")
    }
}

fn sha256_file(path: &Path) -> Result<String, String> {
    let mut file =
        fs::File::open(path).map_err(|error| format!("Could not verify download: {error}"))?;
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 1024 * 1024];
    loop {
        let read = file
            .read(&mut buffer)
            .map_err(|error| format!("Could not verify download: {error}"))?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

fn now_string() -> String {
    OffsetDateTime::now_utc()
        .format(&Rfc3339)
        .unwrap_or_else(|_| "1970-01-01T00:00:00Z".into())
}
