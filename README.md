# STAR FORGE Launcher

A launcher for **Star Conflict** that lets you keep several server setups side by
side and switch between them with one click. Each *server profile* carries its
own server address, replacement game files and code patches. Nothing in your
Steam install is modified; the changes exist only while the game runs with that
profile.

STAR FORGE is built by the [STARFURY](https://starfury.xyz) community (an
unofficial Star Conflict server), but it is meant for anyone running or playing
on a Star Conflict server project.

> **Status:** early development (0.1). It works for day-to-day play on Linux
> (Proton) and Windows, but expect rough edges.

## Features

- **Server packs** – a server hands out one zip or link with its address,
  game files and look; the launcher shows what it changes before installing,
  verifies downloads and offers updates. One-click installs through
  `starforge://` links.
- **Server profiles** – name, master server address (`host:port`), optional
  local server, extra game arguments.
- **File overlays** – put files in a profile's `files/` folder and the game
  reads them instead of the originals, without touching the install.
- **Byte patches** – verified in-memory patches applied before the game starts
  (each one checks the original bytes first and is skipped if they differ).
- **Linux and Windows** – on Linux the game runs through the same Proton build
  and prefix Steam uses, so your settings carry over; the Steam overlay works.
- **Desktop launcher** with themes (STAR FORGE, STARFURY, Empire, Federation, Jericho),
  tray support and a command-line tool (`sc-launch`) for scripting.

## Installing

Download the latest build from the
[Releases page](https://github.com/starfury-xyz/star-forge-launcher/releases):

| Platform | File |
|---|---|
| Windows | `.msi` or `-setup.exe` |
| Linux | `.AppImage` (any distro), `.deb` or `.rpm` |

Star Conflict must be installed through Steam. The launcher finds it in your
Steam libraries automatically; on Linux it needs to have been started from Steam
once so its Proton prefix exists.

## How it works

```
STAR FORGE Launcher ──▶ sc-inject.exe ──▶ game.exe (started suspended)
                                            └─ sc_agent.dll
                                                 ├─ redirects file reads to <profile>/files/
                                                 └─ applies the profile's byte patches
```

`sc-inject.exe` starts the game suspended, loads `sc_agent.dll` into it and
resumes it. The agent hooks the game's file access so that any file present in
the profile's `files/` folder (mirroring the game folder, e.g.
`files/data/address.cfg`) is used instead of the original. On Linux both run
inside Proton.

One limitation comes from the game itself: a file packed inside a `.pak`
archive always wins over a loose file with the same path, so an overlay can
replace a whole `.pak` but not a single file inside one.

## Server profiles and packs

Every profile is a *server pack*: the same format whether a server operator
published it or you made it yourself in the launcher. Profiles live in the
launcher's data folder:

| Platform | Folder |
|---|---|
| Linux | `~/.local/share/star-forge/profiles/` |
| Windows | `%APPDATA%\star-forge\profiles\` |

```
profiles/my-server/
├── pack.json     the pack: name, server, branding, patches (replaced by updates)
├── resources/    the pack's artwork
├── files/        replacement game files, mirroring the game folder
├── local.json    your own changes on top of the pack (never touched by updates)
└── agent.log     what the agent redirected and patched last launch
```

To make a pack for your server, start from
[`templates/pack`](templates/pack/README.md); the format is documented in
[`schemas/pack.schema.json`](schemas/pack.schema.json), which editors use for
autocomplete. Profiles from before 0.2 (`profile.toml`) are converted
automatically.

## Building from source

Requirements:

- [Rust](https://rustup.rs) (stable)
- [cargo-xwin](https://github.com/rust-cross/cargo-xwin) on Linux, to
  cross-build the Windows injector and agent
  (`cargo install cargo-xwin`, `rustup target add x86_64-pc-windows-msvc`)
- [Tauri CLI](https://v2.tauri.app/reference/cli/) for packaged builds
  (`cargo install tauri-cli --version "^2"`)
- Linux system libraries for Tauri, e.g. on Debian/Ubuntu:
  `libwebkit2gtk-4.1-dev libayatana-appindicator3-dev librsvg2-dev libxdo-dev libssl-dev`

```sh
scripts/build.sh               # injector, agent and sc-launch into dist/linux and dist/windows
cargo build -p star-forge      # desktop launcher (development build)
./target/debug/star-forge      # run it; it finds the tools in dist/ automatically
cargo test --workspace         # unit tests
```

On Linux Wayland desktops the taskbar only shows the launcher's icon if a
desktop entry is installed. Packages do this for you; for a local build run
`scripts/install-desktop-entry.sh`.

### Local configuration (`.env`)

Paths that differ per machine can be set through `STAR_FORGE_*` variables.
Copy `.env.example` to `.env` (it is gitignored) and uncomment what you need;
development builds and the scripts read it automatically:

| Variable | Overrides |
|---|---|
| `STAR_FORGE_GAME_DIR` | Star Conflict install folder (also the patcher's default `cloud/` for tests) |
| `STAR_FORGE_TOOLS_DIR` | folder with `sc-inject.exe` and `sc_agent.dll` |
| `STAR_FORGE_PROTON` | Proton build (Linux) |
| `STAR_FORGE_DATA_DIR` | launcher data folder (preferences, profiles, log) |
| `STAR_FORGE_BIN` | binary that `install-desktop-entry.sh` registers |

Order of precedence: launcher settings / command-line flags, then these
variables, then auto-detection.

### Command line

```sh
sc-launch list                       # list profiles
sc-launch info                       # show the detected game, tools and Proton
sc-launch run <profile> [-- args]    # start the game with a profile
sc-launch install <zip|folder|url>   # install or update a server pack
sc-launch updates                    # check installed packs for new versions
sc-launch update <profile>           # install the newest version of a profile's pack
```

### Repository layout

| Path | What |
|---|---|
| `crates/sc-profile` | profile format and path mapping shared by everything |
| `crates/sc-agent` | `sc_agent.dll`, the injected agent (file redirection, patches) |
| `crates/sc-inject` | `sc-inject.exe`, starts the game and injects the agent |
| `crates/sc-launch` | finds Steam, the game and Proton; profile storage; the `sc-launch` CLI |
| `gui/` | the Tauri desktop launcher (`gui/src` web UI, `gui/src-tauri` Rust side) |
| `patcher/` | server patcher (C++): applies a server's source-controlled patches to the `cloud/` exes in memory at startup, via a proxy DLL. Built separately with `make`; see [patcher/README.md](patcher/README.md) |
| `scripts/` | build and desktop-integration scripts |

## Releasing

Push a tag like `v0.2.0` (matching the version in `Cargo.toml` and
`gui/src-tauri/tauri.conf.json`). GitHub Actions builds the Windows and Linux
packages and attaches them to a draft release for review before publishing.

## Goals and to-do

**Profiles for everyone**
- [x] Server packs: import from a zip, folder, link or `starforge://` link,
      with a review of what the pack changes before it's installed
- [x] Pack updates from the server's published `pack.json`, checksum-verified,
      keeping the player's own changes
- [x] Warn when a pack was made for a different game build
- [x] Per-pack branding: the selected pack's logo, icon, background (one, or
      a random pick from a list) and accent colour; can be turned off in Settings
- [x] Background folder setting: a random wallpaper of your own when the pack
      has no background
- [ ] Settle how client patches are applied (currently experimental)
- [ ] Export a profile as a pack zip from the launcher
- [ ] Profile editor for patches and extra arguments in the UI
- [ ] Pack signing, so players can tell a pack really comes from its server

**Launcher**
- [x] STAR FORGE's own logo, icon and default theme (v1)
- [ ] Server status on the play screen (online / player count)
- [ ] Launcher self-update via GitHub releases
- [ ] Settings for the theme label ("Faction colour") now that themes go beyond factions
- [ ] macOS support (Proton/Wine path)

**Project**
- [x] Open-source release (MIT)
- [x] CI tests on Linux and Windows; tagged releases with packaged builds
- [ ] `cargo fmt` the codebase and enforce it (plus clippy) in CI
- [ ] Code signing for Windows builds
- [ ] Contributor guide and a guide for server operators

## Disclaimer

STAR FORGE is an unofficial fan project. It is not affiliated with or endorsed
by the developers or publishers of Star Conflict. Star Conflict and its assets
belong to their respective owners; this repository contains no game files.

## License

[MIT](LICENSE)
