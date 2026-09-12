# Homebrew Install

**Status: not live yet.** The `falcon-lint/homebrew-tap` repository doesn't
exist yet and `HOMEBREW_TAP_TOKEN` isn't configured, so `brew tap
falcon-lint/tap` currently fails. Use the [curl install script or `cargo
install`](../getting-started.md#installation) instead until this is set up.

## How it's meant to work

Falcon's release workflow is wired to publish preview macOS formula updates to
a Homebrew tap when a GitHub Release tag is published:

```bash
brew tap falcon-lint/tap
brew install falcon
```

The formula would install all three release binaries:

- `falcon`
- `falcon-lsp`
- `falcon-mcp`

Release automation (`.github/workflows/release.yml`, `homebrew` job) downloads
`SHA256SUMS` from the Falcon GitHub Release and writes `Formula/falcon.rb` in
the tap with separate macOS ARM64 and x64 archive URLs, via
`scripts/update_homebrew_formula.sh`.

To turn this on, a maintainer needs to:

1. Create the `falcon-lint/homebrew-tap` GitHub repository.
2. Configure `HOMEBREW_TAP_TOKEN`: a token with write access to that repo.
3. Optionally set `HOMEBREW_TAP_REPOSITORY` to override the default
   `falcon-lint/homebrew-tap` target.

Until then, the `homebrew` release job no-ops (see its `::notice::Skipping
Homebrew tap update` log line) rather than failing the release.

This is the live preview tap path, not GA notarized distribution.
