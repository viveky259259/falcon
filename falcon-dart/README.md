# falcon_cli

Dart wrapper for [Falcon](https://github.com/viveky259259/falcon) — Rust-powered static analysis for Flutter/Dart.

## Install

```bash
dart pub global activate falcon_cli
```

> **Note**: Requires the Falcon Rust binary. Install with:
> ```bash
> cargo install --git https://github.com/viveky259259/falcon
> ```

## Usage

```bash
falcon ai-score .           # AI Code Quality Score (0-100)
falcon analyze .             # Full analysis
falcon check-perf .          # Performance analysis
falcon provenance .          # AI vs human code detection
```

See [full documentation](https://github.com/viveky259259/falcon) for all 74 commands.

## License

MIT
