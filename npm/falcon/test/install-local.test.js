const assert = require("node:assert/strict");
const { execFileSync } = require("node:child_process");
const { createHash } = require("node:crypto");
const fs = require("node:fs");
const os = require("node:os");
const path = require("node:path");

const { assetName, platformInfo, verifyChecksum } = require("../lib/install");

function sha256(file) {
  const hash = createHash("sha256");
  hash.update(fs.readFileSync(file));
  return hash.digest("hex");
}

assert.deepEqual(platformInfo("darwin", "arm64"), {
  os: "macos",
  arch: "arm64",
  label: "macos-arm64",
});
assert.deepEqual(platformInfo("darwin", "x64"), {
  os: "macos",
  arch: "x64",
  label: "macos-x64",
});
assert.deepEqual(platformInfo("linux", "x64"), {
  os: "linux",
  arch: "x64",
  label: "linux-x64",
});
assert.throws(() => platformInfo("win32", "x64"), /unsupported platform win32-x64/);

const info = platformInfo();
const packageRoot = path.resolve(__dirname, "..");
const tmp = fs.mkdtempSync(path.join(os.tmpdir(), "falcon-npm-test-"));
const archiveName = assetName(undefined, info);
const archivePath = path.join(tmp, archiveName);
const staging = path.join(tmp, archiveName.replace(/\.tar\.gz$/, ""));
const installDir = path.join(tmp, "vendor");
const checksumsPath = path.join(tmp, "SHA256SUMS");

fs.mkdirSync(staging, { recursive: true });
for (const binary of ["falcon", "falcon-lsp", "falcon-mcp"]) {
  const binaryPath = path.join(staging, binary);
  fs.writeFileSync(
    binaryPath,
    "#!/usr/bin/env node\nconsole.log('fixture ' + process.argv.slice(2).join(' '));\n",
    { mode: 0o755 },
  );
}

execFileSync("tar", ["-czf", archivePath, "-C", tmp, path.basename(staging)]);
fs.writeFileSync(checksumsPath, `${sha256(archivePath)}  ${archiveName}\n`);
assert.throws(
  () => verifyChecksum(archivePath, `${"0".repeat(64)}  ${archiveName}\n`, archiveName),
  /checksum mismatch/,
);

const env = {
  ...process.env,
  FALCON_INSTALL_DIR: installDir,
  FALCON_NPM_LOCAL_ARCHIVE: archivePath,
  FALCON_NPM_LOCAL_SHA256SUMS: checksumsPath,
  FALCON_INSTALL_STRICT: "1",
};

execFileSync("node", ["scripts/install.js"], {
  cwd: packageRoot,
  env,
  stdio: "pipe",
});

const installed = path.join(installDir, info.label, "falcon");
assert.equal(fs.existsSync(installed), true);

const output = execFileSync("node", ["bin/falcon.js", "review", "--help"], {
  cwd: packageRoot,
  env,
  encoding: "utf8",
});
assert.equal(output.trim(), "fixture review --help");
