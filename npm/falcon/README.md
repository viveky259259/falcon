# Falcon npm package

Run Falcon without a global Rust install:

```bash
npx falcon-flutter@latest review
```

The package downloads the native `falcon` binary for macOS arm64, macOS x64, or
Linux x64 from GitHub Releases and verifies it against `SHA256SUMS` before
running it. Unsupported platforms fail with a clear error.
