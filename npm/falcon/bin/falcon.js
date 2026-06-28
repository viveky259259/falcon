#!/usr/bin/env node

const { spawnSync } = require("node:child_process");
const { ensureInstalled } = require("../lib/install");

async function main() {
  const binary = await ensureInstalled();
  const result = spawnSync(binary, process.argv.slice(2), { stdio: "inherit" });

  if (result.error) {
    throw result.error;
  }

  if (result.signal) {
    process.kill(process.pid, result.signal);
    return;
  }

  process.exit(result.status ?? 0);
}

main().catch((error) => {
  console.error(`falcon: ${error.message}`);
  process.exit(1);
});
