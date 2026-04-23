# Contributing to dofek

Thanks for considering a contribution. dofek is a small project and PRs, bug reports, and plugin ideas are all welcome.

## Quick links

- **Bug reports & feature requests:** [Issues](https://github.com/AsafSaar/dofek/issues)
- **Discussion, ideas, plugin showcase:** [Discussions](https://github.com/AsafSaar/dofek/discussions)
- **Security issues:** see [SECURITY.md](./SECURITY.md) — please do **not** open a public issue

## Dev setup

Prerequisites:

- [Rust toolchain](https://rustup.rs/) (stable, edition 2024)
- [Visual Studio Build Tools](https://visualstudio.microsoft.com/visual-cpp-build-tools/) with the C++ workload
- [Tauri CLI](https://v2.tauri.app/start/prerequisites/) for GUI builds: `cargo install tauri-cli --version "^2"`

Build & run:

```bash
cargo tui                  # Run the TUI (debug)
cargo gui                  # Run the GUI (debug, hot-reload)
cargo build-tui            # Release TUI → target/release/dofek-tui.exe
cargo build-gui            # Release GUI → target/release/dofek-gui.exe + MSI
.\build-all.ps1            # Single MSI bundling both binaries
```

Cargo aliases live in `.cargo/config.toml`.

## Code style

- Format with `cargo fmt` before committing.
- `cargo clippy --all-targets -- -D warnings` must pass — CI gates on this.
- Tests: `cargo test`. Add tests for new logic where it's reasonable.
- No `unwrap()` / `expect()` on paths reachable from user input or external services. Propagate errors with `anyhow`.
- Comments only when the *why* isn't obvious from the code. Don't restate what the code says.

## PR process

1. Fork and branch off `main`.
2. Keep PRs focused — one logical change per PR.
3. Update `CHANGELOG.md` under an `## [Unreleased]` section if your change is user-facing.
4. Make sure clippy + tests are clean locally.
5. Open the PR; describe what changed and why. Screenshots help for UI changes.

## Plugins

Plugins are external executables that speak JSON over stdio. The protocol is documented in [`plugins/README.md`](./plugins/README.md). Plugins do not need to live in this repo — feel free to publish your own and link from Discussions.

> ⚠️ The plugin API is experimental and may change before hitting a stable contract. Pin against a specific dofek version if stability matters.

## License

By contributing, you agree that your contributions will be licensed under the MIT License (see [LICENSE](./LICENSE)).
