#!/usr/bin/env node

const { ensureInstalled, UnsupportedPlatformError } = require("../lib/install");

ensureInstalled()
  .then((binary) => {
    console.log(`Falcon installed at ${binary}`);
  })
  .catch((error) => {
    const isPostinstall = process.env.npm_lifecycle_event === "postinstall";
    if (isPostinstall && error instanceof UnsupportedPlatformError) {
      console.warn(`Falcon was not installed: ${error.message}`);
      return;
    }
    if (isPostinstall && process.env.FALCON_INSTALL_STRICT !== "1") {
      console.warn(`Falcon download skipped: ${error.message}`);
      console.warn("The binary will be downloaded on first run.");
      return;
    }

    console.error(`Falcon install failed: ${error.message}`);
    process.exit(1);
  });
