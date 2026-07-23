# Homebrew Install

Falcon publishes preview macOS formula updates to the Homebrew tap when a GitHub
Release tag is published:

```bash
brew tap falcon-lint/tap
brew install falcon
```

The formula installs all three release binaries:

- `falcon`
- `falcon-lsp`
- `falcon-mcp`

Release automation downloads `SHA256SUMS` from the Falcon GitHub Release and
writes `Formula/falcon.rb` in the tap with separate macOS ARM64 and x64 archive
URLs.

Maintainers must configure:

- `HOMEBREW_TAP_TOKEN`: a token with write access to the tap repository.
- `HOMEBREW_TAP_REPOSITORY`: optional repository override; defaults to
  `falcon-lint/homebrew-tap`.

This is the live preview tap path, not GA notarized distribution.
