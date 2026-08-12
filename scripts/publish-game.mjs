import { createHash } from "node:crypto";
import {
  cpSync,
  closeSync,
  existsSync,
  mkdirSync,
  openSync,
  readSync,
  readFileSync,
  readdirSync,
  rmSync,
  statSync,
  writeFileSync
} from "node:fs";
import { basename, dirname, isAbsolute, join, relative, resolve } from "node:path";
import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";

const projectRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const publisherRoot = join(projectRoot, ".coldem-publisher");
const maxGitHubAssetSize = 2 * 1024 * 1024 * 1024;

function fail(message) {
  console.error(`\nError: ${message}`);
  process.exit(1);
}

function usage() {
  console.log(`
Publish a Windows game build to a public GitHub Release.

Required:
  --repo OWNER/REPOSITORY
  --game-id NUMBER
  --slug robot-rock
  --title "Robot Rock"
  --version 0.1.0
  --build "D:\\Builds\\Robot Rock"
  --executable "Robot Rock.exe"

Optional:
  --summary "Short description"
  --page-url https://...
  --cover-url https://...
  --cover-file D:\\art\\cover.png
  --minisign-key D:\\keys\\coldem.key
  --dry-run

Example:
  pnpm game:publish -- --repo DnCold/Coldem-delivery --game-id 1 --slug robot-rock --title "Robot Rock" --version 0.1.0 --build "D:\\Builds\\Robot Rock" --executable "Robot Rock.exe" --dry-run
`);
}

function parseArguments(argv) {
  const result = {};
  for (let index = 0; index < argv.length; index += 1) {
    const token = argv[index];
    if (token === "--") continue;
    if (token === "--help" || token === "-h") result.help = true;
    else if (token === "--dry-run") result.dryRun = true;
    else if (token.startsWith("--")) {
      const key = token.slice(2).replace(/-([a-z])/g, (_, letter) => letter.toUpperCase());
      const value = argv[index + 1];
      if (!value || value.startsWith("--")) fail(`${token} needs a value`);
      result[key] = value;
      index += 1;
    } else fail(`Unknown argument: ${token}`);
  }
  return result;
}

function run(program, args, options = {}) {
  let executable = program;
  if (process.platform === "win32" && program === "gh") {
    executable = process.env.GH_PATH || "gh.exe";
  }
  const result = spawnSync(executable, args, {
    cwd: projectRoot,
    encoding: "utf8",
    stdio: options.capture ? "pipe" : "inherit",
    windowsHide: true
  });
  if (result.error) {
    const hint = program === "gh" && process.platform === "win32"
      ? " Set GH_PATH to the full path of gh.exe if GitHub CLI is not on PATH."
      : "";
    fail(`Could not run ${program}: ${result.error.message}.${hint}`);
  }
  if (result.status !== 0) {
    const detail = options.capture ? `\n${result.stderr || result.stdout}` : "";
    fail(`${program} exited with code ${result.status}.${detail}`);
  }
  return (result.stdout || "").trim();
}

function hashFile(path) {
  const hash = createHash("sha256");
  const descriptor = openSync(path, "r");
  const buffer = Buffer.allocUnsafe(1024 * 1024);
  try {
    let bytesRead;
    do {
      bytesRead = readSync(descriptor, buffer, 0, buffer.length, null);
      if (bytesRead > 0) hash.update(buffer.subarray(0, bytesRead));
    } while (bytesRead > 0);
  } finally {
    closeSync(descriptor);
  }
  return hash.digest("hex");
}

function directorySize(path) {
  return readdirSync(path, { withFileTypes: true }).reduce((total, entry) => {
    const child = join(path, entry.name);
    return total + (entry.isDirectory() ? directorySize(child) : statSync(child).size);
  }, 0);
}

function ensureAssetFits(path) {
  const size = statSync(path).size;
  if (size >= maxGitHubAssetSize) {
    fail(`${basename(path)} is ${(size / 1024 ** 3).toFixed(2)} GiB. A single GitHub Release asset must stay below 2 GiB.`);
  }
}

function artifactFor(repo, tag, patchPath, signaturePath) {
  ensureAssetFits(patchPath);
  ensureAssetFits(signaturePath);
  const base = `https://github.com/${repo}/releases/download/${encodeURIComponent(tag)}`;
  return {
    url: `${base}/${encodeURIComponent(basename(patchPath))}`,
    size: statSync(patchPath).size,
    sha256: hashFile(patchPath),
    signatureUrl: `${base}/${encodeURIComponent(basename(signaturePath))}`,
    signatureSha256: hashFile(signaturePath)
  };
}

function cleanVersion(value) {
  if (!/^[0-9A-Za-z][0-9A-Za-z.+-]{0,63}$/.test(value)) fail(`Unsafe version: ${value}`);
  return value;
}

function loadJson(path, fallback) {
  if (!existsSync(path)) return fallback;
  try {
    return JSON.parse(readFileSync(path, "utf8"));
  } catch (error) {
    fail(`Could not parse ${path}: ${error.message}`);
  }
}

const args = parseArguments(process.argv.slice(2));
if (args.help) {
  usage();
  process.exit(0);
}

for (const required of ["repo", "gameId", "slug", "title", "version", "build", "executable"]) {
  if (!args[required]) {
    usage();
    fail(`Missing --${required.replace(/[A-Z]/g, (letter) => `-${letter.toLowerCase()}`)}`);
  }
}

if (!/^[A-Za-z0-9_.-]+\/[A-Za-z0-9_.-]+$/.test(args.repo)) fail("--repo must look like OWNER/REPOSITORY");
if (!/^[A-Za-z0-9][A-Za-z0-9_-]{0,79}$/.test(args.slug)) fail("--slug may only contain letters, numbers, - and _");
if (!/^\d+$/.test(args.gameId) || Number(args.gameId) <= 0) fail("--game-id must be a positive integer");
cleanVersion(args.version);

const buildDir = resolve(args.build);
if (!existsSync(buildDir) || !statSync(buildDir).isDirectory()) fail(`Build directory does not exist: ${buildDir}`);
const executable = resolve(buildDir, args.executable);
const executableRelative = relative(buildDir, executable);
if (executableRelative.startsWith("..") || isAbsolute(executableRelative)) fail("--executable must stay inside --build");
if (!existsSync(executable) || !statSync(executable).isFile()) fail(`Executable does not exist: ${executable}`);

const butler = join(projectRoot, "src-tauri", "binaries", "butler-x86_64-pc-windows-msvc.exe");
if (!existsSync(butler)) fail(`Bundled butler was not found. Run pnpm sidecar:fetch first.`);

const timestamp = new Date().toISOString().replace(/[-:]/g, "").replace(/\.\d{3}Z$/, "Z");
const tag = `delivery-${args.slug}-${args.version}-${timestamp}`;
const outputDir = join(publisherRoot, "out", tag);
const gameStateDir = join(publisherRoot, "games", args.slug);
const currentBuildDir = join(gameStateDir, "current");
const statePath = join(gameStateDir, "state.json");
const catalogPath = join(publisherRoot, "catalog.json");
const emptyDir = join(publisherRoot, "empty");
mkdirSync(outputDir, { recursive: true });
mkdirSync(emptyDir, { recursive: true });

const fullPatch = join(outputDir, `${args.slug}-${args.version}-full.pwr`);
console.log(`\nCreating full Wharf package for ${args.title} ${args.version}...`);
run(butler, ["diff", "--verify", emptyDir, buildDir, fullPatch]);
const fullSignature = `${fullPatch}.sig`;
if (!existsSync(fullSignature)) fail("butler did not generate the full package signature");

const previousState = loadJson(statePath, null);
let incremental = null;
if (previousState && previousState.version !== args.version && existsSync(currentBuildDir)) {
  cleanVersion(previousState.version);
  const incrementalPatch = join(
    outputDir,
    `${args.slug}-${previousState.version}-to-${args.version}.pwr`
  );
  console.log(`\nCreating incremental patch ${previousState.version} -> ${args.version}...`);
  run(butler, ["diff", "--verify", currentBuildDir, buildDir, incrementalPatch]);
  const incrementalSignature = `${incrementalPatch}.sig`;
  if (!existsSync(incrementalSignature)) fail("butler did not generate the incremental package signature");
  incremental = {
    fromVersion: previousState.version,
    patchPath: incrementalPatch,
    signaturePath: incrementalSignature
  };
}

const releaseAssets = [fullPatch, fullSignature];
const fullArtifact = artifactFor(args.repo, tag, fullPatch, fullSignature);
const patches = [];
if (incremental) {
  releaseAssets.push(incremental.patchPath, incremental.signaturePath);
  patches.push({
    fromVersion: incremental.fromVersion,
    artifact: artifactFor(args.repo, tag, incremental.patchPath, incremental.signaturePath)
  });
}

let coverUrl = args.coverUrl || "";
if (args.coverFile) {
  const coverSource = resolve(args.coverFile);
  if (!existsSync(coverSource) || !statSync(coverSource).isFile()) fail(`Cover file does not exist: ${coverSource}`);
  const coverName = `${args.slug}-cover${basename(coverSource).includes(".") ? `.${basename(coverSource).split(".").pop()}` : ""}`;
  const coverDestination = join(outputDir, coverName);
  cpSync(coverSource, coverDestination);
  ensureAssetFits(coverDestination);
  releaseAssets.push(coverDestination);
  coverUrl = `https://github.com/${args.repo}/releases/download/${encodeURIComponent(tag)}/${encodeURIComponent(coverName)}`;
}

const catalog = loadJson(catalogPath, {
  schemaVersion: 1,
  publishedAt: new Date().toISOString(),
  games: []
});
if (catalog.schemaVersion !== 1 || !Array.isArray(catalog.games)) fail("Publisher catalog has an unsupported format");

const previousGame = catalog.games.find((game) => Number(game.id) === Number(args.gameId));
const game = {
  id: Number(args.gameId),
  slug: args.slug,
  title: args.title,
  summary: args.summary ?? previousGame?.summary ?? "",
  coverUrl: coverUrl || previousGame?.coverUrl || "",
  pageUrl: args.pageUrl ?? previousGame?.pageUrl ?? "",
  platforms: {
    ...(previousGame?.platforms || {}),
    "windows-x86_64": {
      version: args.version,
      executable: executableRelative.replaceAll("\\", "/"),
      installedSize: directorySize(buildDir),
      full: fullArtifact,
      patches
    }
  }
};
catalog.games = catalog.games.filter((item) => Number(item.id) !== Number(args.gameId));
catalog.games.push(game);
catalog.games.sort((left, right) => Number(left.id) - Number(right.id));
catalog.publishedAt = new Date().toISOString();

const manifestPath = join(outputDir, "coldem-manifest.json");
writeFileSync(manifestPath, `${JSON.stringify(catalog, null, 2)}\n`, "utf8");
releaseAssets.push(manifestPath);

if (args.minisignKey) {
  const key = resolve(args.minisignKey);
  if (!existsSync(key)) fail(`Minisign private key does not exist: ${key}`);
  run("minisign", ["-S", "-s", key, "-m", manifestPath]);
  const signature = `${manifestPath}.minisig`;
  if (!existsSync(signature)) fail("minisign did not generate the manifest signature");
  releaseAssets.push(signature);
}

console.log(`\nPrepared ${releaseAssets.length} release assets in ${outputDir}`);
console.log(`Catalog URL: https://github.com/${args.repo}/releases/latest/download/coldem-manifest.json`);

if (args.dryRun) {
  console.log("\nDry run complete. Nothing was uploaded and publisher history was not changed.");
  process.exit(0);
}

console.log(`\nChecking GitHub access for ${args.repo}...`);
run("gh", ["repo", "view", args.repo], { capture: true });
console.log(`Publishing GitHub Release ${tag}...`);
run("gh", [
  "release",
  "create",
  tag,
  "--repo",
  args.repo,
  "--title",
  `${args.title} ${args.version}`,
  "--notes",
  `Coldem delivery release for ${args.title} ${args.version}.`,
  "--latest",
  ...releaseAssets
]);

mkdirSync(gameStateDir, { recursive: true });
rmSync(currentBuildDir, { recursive: true, force: true });
cpSync(buildDir, currentBuildDir, { recursive: true });
writeFileSync(statePath, `${JSON.stringify({ version: args.version, tag }, null, 2)}\n`, "utf8");
writeFileSync(catalogPath, `${JSON.stringify(catalog, null, 2)}\n`, "utf8");
console.log(`\nPublished successfully. The next publish for ${args.slug} will also create an incremental patch from ${args.version}.`);
