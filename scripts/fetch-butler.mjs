import { chmod, cp, mkdtemp, mkdir, readdir, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { basename, join, resolve } from "node:path";
import process from "node:process";
import extract from "extract-zip";

const targets = {
  "win32-x64": {
    package: "windows-amd64",
    triple: "x86_64-pc-windows-msvc",
    executable: "butler.exe"
  },
  "win32-arm64": {
    package: "windows-arm64",
    triple: "aarch64-pc-windows-msvc",
    executable: "butler.exe"
  },
  "linux-x64": {
    package: "linux-amd64",
    triple: "x86_64-unknown-linux-gnu",
    executable: "butler"
  },
  "linux-arm64": {
    package: "linux-arm64",
    triple: "aarch64-unknown-linux-gnu",
    executable: "butler"
  },
  "darwin-x64": {
    package: "darwin-amd64",
    triple: "x86_64-apple-darwin",
    executable: "butler"
  },
  "darwin-arm64": {
    package: "darwin-arm64",
    triple: "aarch64-apple-darwin",
    executable: "butler"
  }
};

const key = `${process.platform}-${process.arch}`;
const target = targets[key];
if (!target) {
  throw new Error(`No butler sidecar mapping exists for ${key}`);
}

const baseUrl = process.env.BUTLER_BROTH_BASE ?? "https://broth.itch.zone/butler";
const url = `${baseUrl}/${target.package}/LATEST/archive/default`;
const temporary = await mkdtemp(join(tmpdir(), "coldem-butler-"));
const archive = join(temporary, "butler.zip");
const extracted = join(temporary, "extracted");
const destinationDirectory = resolve("src-tauri", "binaries");
const extension = process.platform === "win32" ? ".exe" : "";
const destination = join(
  destinationDirectory,
  `butler-${target.triple}${extension}`
);

try {
  process.stdout.write(`Downloading butler for ${target.package}…\n`);
  const response = await fetch(url, { redirect: "follow" });
  if (!response.ok) {
    throw new Error(`Download failed with HTTP ${response.status}`);
  }
  await writeFile(archive, Buffer.from(await response.arrayBuffer()));
  await mkdir(extracted, { recursive: true });
  await extract(archive, { dir: extracted });

  const executable = await findFile(extracted, target.executable);
  if (!executable) {
    throw new Error(`Archive did not contain ${target.executable}`);
  }

  await mkdir(destinationDirectory, { recursive: true });
  await cp(executable, destination);
  if (process.platform !== "win32") await chmod(destination, 0o755);
  process.stdout.write(`Installed ${basename(destination)}\n`);
} finally {
  await rm(temporary, { recursive: true, force: true });
}

async function findFile(directory, filename) {
  for (const entry of await readdir(directory, { withFileTypes: true })) {
    const path = join(directory, entry.name);
    if (entry.isDirectory()) {
      const found = await findFile(path, filename);
      if (found) return found;
    } else if (entry.name.toLowerCase() === filename.toLowerCase()) {
      return path;
    }
  }
  return null;
}
