# Development

## Building

Requirements:

- [Rust](https://rustup.rs) (stable)
- [cargo-xwin](https://github.com/rust-cross/cargo-xwin) on Linux, to
  cross-build the Windows injector and agent
  (`cargo install cargo-xwin`, `rustup target add x86_64-pc-windows-msvc`)
- [Tauri CLI](https://v2.tauri.app/reference/cli/) for packaged builds
  (`cargo install tauri-cli --version "^2"`, or `npx @tauri-apps/cli@2`)
- Linux system libraries for Tauri, e.g. on Debian/Ubuntu:
  `libwebkit2gtk-4.1-dev libayatana-appindicator3-dev librsvg2-dev libxdo-dev libssl-dev`

```sh
scripts/build.sh               # injector, agent and sc-launch into dist/linux and dist/windows
cargo build -p star-forge      # desktop launcher (development build)
./target/debug/star-forge      # run it; it finds the tools in dist/ automatically
cargo test --workspace         # unit tests
```

Run `scripts/build.sh` again after changing anything the injector or agent
read (`crates/sc-profile`, `crates/sc-patch`): an older `sc-inject.exe` refuses
profiles in the newer format.

On Linux Wayland desktops the taskbar only shows the launcher's icon if a
desktop entry is installed. Packages do this for you; for a local build run
`scripts/install-desktop-entry.sh`.

## Local configuration (`.env`)

Paths that differ per machine can be set through `STAR_FORGE_*` variables.
Copy `.env.example` to `.env` (it is gitignored) and uncomment what you need;
development builds and the scripts read it automatically. The variables are
listed in [Command line](command-line.md#environment); `STAR_FORGE_BIN` also
sets the binary that `install-desktop-entry.sh` registers.

Order of precedence: launcher settings and command-line flags, then these
variables, then auto-detection.

## Repository layout

| Path | What |
|---|---|
| `crates/sc-profile` | pack and profile format, path rules shared by everything |
| `crates/sc-patch` | patchset format, checking and applying (shared by the agent and the launcher; same format as `patcher/`) |
| `crates/sc-agent` | `sc_agent.dll`, the injected agent (patches, file redirection, game data folder) |
| `crates/sc-inject` | `sc-inject.exe`, starts the game and injects the agent |
| `crates/sc-launch` | Steam, game and Proton detection; packs, signing, trust, file lists; the `sc-launch` CLI |
| `gui/` | the Tauri desktop launcher (`gui/src` web UI, `gui/src-tauri` Rust side) |
| `patcher/` | server patcher (C++); see [patcher/README.md](../patcher/README.md) |
| `samples/` | ready-made community server packs, offered in **Add profile** (listed in `SAMPLES` in `gui/src-tauri/src/profiles.rs`) |
| `schemas/` | JSON schema for `pack.json` |
| `templates/pack/` | the pack template for server teams |
| `docs/` | this documentation |
| `scripts/` | build and desktop-integration scripts |

## Releasing

1. Rename the **Unreleased** section of [CHANGELOG.md](../CHANGELOG.md) to the
   new version (`## 0.2.2`); it becomes the release notes.
2. Set the same version in `Cargo.toml` and `gui/src-tauri/tauri.conf.json`.
3. Push a tag like `v0.2.2`. GitHub Actions builds the Windows and Linux
   packages and attaches them to a draft release.
4. Review the draft and publish it. Installed launchers only see published
   releases.

The packages are signed for the launcher's self-update, so the repository
needs two secrets: `TAURI_SIGNING_PRIVATE_KEY` (the key file's contents) and
`TAURI_SIGNING_PRIVATE_KEY_PASSWORD`. Make a key pair with
`npx @tauri-apps/cli signer generate -w ~/.tauri/star-forge-updater.key`; its
public half is `plugins.updater.pubkey` in `tauri.conf.json`. Keep the private
key and password safe: installed launchers only accept updates signed with it.
The release also carries a `latest.json`, which installed launchers find at
`releases/latest/download/latest.json`. A fork that publishes its own builds
needs its own key pair and its own `plugins.updater.endpoints`.
