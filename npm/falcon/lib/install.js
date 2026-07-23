const { createHash } = require("node:crypto");
const { execFileSync } = require("node:child_process");
const fs = require("node:fs");
const fsp = require("node:fs/promises");
const https = require("node:https");
const os = require("node:os");
const path = require("node:path");

const PACKAGE_ROOT = path.resolve(__dirname, "..");
const PACKAGE_VERSION = require("../package.json").version;
const DEFAULT_REPO = "viveky259259/falcon";
const BINARIES = ["falcon", "falcon-lsp", "falcon-mcp"];

class UnsupportedPlatformError extends Error {}

function platformInfo(platform = process.platform, arch = process.arch) {
  if (platform === "darwin" && arch === "arm64") {
    return { os: "macos", arch: "arm64", label: "macos-arm64" };
  }
  if (platform === "darwin" && arch === "x64") {
    return { os: "macos", arch: "x64", label: "macos-x64" };
  }
  if (platform === "linux" && arch === "x64") {
    return { os: "linux", arch: "x64", label: "linux-x64" };
  }

  throw new UnsupportedPlatformError(
    `unsupported platform ${platform}-${arch}; Falcon npm supports macOS arm64, macOS x64, and Linux x64`,
  );
}

function installRoot() {
  return process.env.FALCON_INSTALL_DIR || path.join(PACKAGE_ROOT, "vendor");
}

function installDir(info = platformInfo()) {
  return path.join(installRoot(), info.label);
}

function binaryPath(info = platformInfo()) {
  return path.join(installDir(info), "falcon");
}

function assetName(version = PACKAGE_VERSION, info = platformInfo()) {
  return `falcon-${version}-${info.label}.tar.gz`;
}

function releaseBaseUrl(version = PACKAGE_VERSION) {
  const repo = process.env.FALCON_RELEASE_REPO || DEFAULT_REPO;
  return `https://github.com/${repo}/releases/download/v${version}`;
}

function parseChecksums(text) {
  const sums = new Map();
  for (const line of text.split(/\r?\n/)) {
    const trimmed = line.trim();
    if (!trimmed) {
      continue;
    }
    const match = trimmed.match(/^([a-fA-F0-9]{64})\s+\*?(.+)$/);
    if (match) {
      sums.set(path.basename(match[2].trim()), match[1].toLowerCase());
    }
  }
  return sums;
}

function sha256File(file) {
  const hash = createHash("sha256");
  hash.update(fs.readFileSync(file));
  return hash.digest("hex");
}

function verifyChecksum(archive, checksumsText, expectedName) {
  const expected = parseChecksums(checksumsText).get(expectedName);
  if (!expected) {
    throw new Error(`SHA256SUMS does not contain ${expectedName}`);
  }

  const actual = sha256File(archive);
  if (actual !== expected) {
    throw new Error(`checksum mismatch for ${expectedName}`);
  }
}

async function downloadFile(url, destination, redirects = 0) {
  if (redirects > 5) {
    throw new Error(`too many redirects while downloading ${url}`);
  }

  await fsp.mkdir(path.dirname(destination), { recursive: true });

  await new Promise((resolve, reject) => {
    const request = https.get(
      url,
      { headers: { "User-Agent": "falcon-npm-installer" } },
      (response) => {
        if (
          response.statusCode >= 300 &&
          response.statusCode < 400 &&
          response.headers.location
        ) {
          response.resume();
          downloadFile(response.headers.location, destination, redirects + 1)
            .then(resolve)
            .catch(reject);
          return;
        }

        if (response.statusCode !== 200) {
          response.resume();
          reject(new Error(`download failed with HTTP ${response.statusCode}: ${url}`));
          return;
        }

        const file = fs.createWriteStream(destination, { mode: 0o644 });
        response.pipe(file);
        file.on("finish", () => file.close(resolve));
        file.on("error", reject);
      },
    );

    request.on("error", reject);
  });
}

async function readChecksums(version, tmpDir) {
  if (process.env.FALCON_NPM_LOCAL_SHA256SUMS) {
    return fsp.readFile(process.env.FALCON_NPM_LOCAL_SHA256SUMS, "utf8");
  }

  const checksums = path.join(tmpDir, "SHA256SUMS");
  await downloadFile(`${releaseBaseUrl(version)}/SHA256SUMS`, checksums);
  return fsp.readFile(checksums, "utf8");
}

async function fetchArchive(version, info, tmpDir) {
  const name = assetName(version, info);
  if (process.env.FALCON_NPM_LOCAL_ARCHIVE) {
    return { archive: process.env.FALCON_NPM_LOCAL_ARCHIVE, name };
  }

  const archive = path.join(tmpDir, name);
  await downloadFile(`${releaseBaseUrl(version)}/${name}`, archive);
  return { archive, name };
}

function findExtractedBinary(root, binary) {
  const entries = fs.readdirSync(root, { withFileTypes: true });
  for (const entry of entries) {
    const fullPath = path.join(root, entry.name);
    if (entry.isDirectory()) {
      const found = findExtractedBinary(fullPath, binary);
      if (found) {
        return found;
      }
    } else if (entry.isFile() && entry.name === binary) {
      return fullPath;
    }
  }
  return null;
}

async function extractBinaries(archive, destination) {
  const tmpExtract = await fsp.mkdtemp(path.join(os.tmpdir(), "falcon-npm-extract-"));
  try {
    execFileSync("tar", ["-xzf", archive, "-C", tmpExtract], { stdio: "ignore" });
    await fsp.rm(destination, { recursive: true, force: true });
    await fsp.mkdir(destination, { recursive: true });

    for (const binary of BINARIES) {
      const source = findExtractedBinary(tmpExtract, binary);
      if (source) {
        const target = path.join(destination, binary);
        await fsp.copyFile(source, target);
        await fsp.chmod(target, 0o755);
      }
    }

    if (!fs.existsSync(path.join(destination, "falcon"))) {
      throw new Error("release archive did not contain the falcon binary");
    }
  } finally {
    await fsp.rm(tmpExtract, { recursive: true, force: true });
  }
}

async function install(options = {}) {
  const version = options.version || PACKAGE_VERSION;
  const info = platformInfo();
  const destination = installDir(info);
  const tmpDir = await fsp.mkdtemp(path.join(os.tmpdir(), "falcon-npm-"));

  try {
    const { archive, name } = await fetchArchive(version, info, tmpDir);
    const checksumsText = await readChecksums(version, tmpDir);
    verifyChecksum(archive, checksumsText, name);
    await extractBinaries(archive, destination);
    return binaryPath(info);
  } finally {
    await fsp.rm(tmpDir, { recursive: true, force: true });
  }
}

async function ensureInstalled(options = {}) {
  const info = platformInfo();
  const binary = binaryPath(info);
  if (fs.existsSync(binary)) {
    return binary;
  }
  return install(options);
}

module.exports = {
  UnsupportedPlatformError,
  assetName,
  binaryPath,
  ensureInstalled,
  install,
  parseChecksums,
  platformInfo,
  verifyChecksum,
};
