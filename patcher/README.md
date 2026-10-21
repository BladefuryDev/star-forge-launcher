# STAR FORGE Patcher

Part of [STAR FORGE](../README.md). MIT licensed like the rest of the repo; vendored toml++ is
MIT as well. Run all commands below from this `patcher/` directory.

Source-controlled patches for the Star Conflict server binaries (`cloud/*.exe`, 32-bit,
build 1.99.0.167293), applied **in memory at startup**. Binaries never go in git, and the
exes on disk stay pristine.

How it works:

- A server's patch project holds patch *sources*: `<Exe>/*.toml`, plus `.s` code caves and
  C++ hooks in `hooks/`. Every write records the original bytes it expects.
- `sfp build` resolves them into `build/dist/<Exe>.patchset.toml`. It assembles caves,
  encodes jmp/call sites, and rejects overlapping patches.
- `build/win32/libcrypto-1_1_tg.dll` is a stand-in for the game's own OpenSSL DLL, which every
  server exe imports (Hub, Shard, MatchMaker, DedicatedServer, ...). All 4050 exports forward
  to the renamed original `libcrypto-1_1_tg_orig.dll`. On process attach it:
  1. identifies the exe and checks its sha1 against the patchset;
  2. applies `cloud/starforge/patcher.toml` overrides;
  3. verifies every enabled patch's original bytes;
  4. applies them and logs the result to `cloud/starforge/logs/<Exe>.log`.

  Anything wrong means the process **refuses to start** (fail-closed), so a server never runs
  unpatched by accident. Exes without a patchset run untouched.
- `sfp bake` applies the same patchset to a file instead, for Ghidra and diffing.

Format reference: [docs/FORMAT.md](docs/FORMAT.md).

## Patch projects

This directory is only the tool. Each server keeps its own patches in its own repo, as a
directory that `make PATCHES=<dir>` points at:

```
<dir>/
  <Exe>/target.toml        pristine sha1 of e.g. DedicatedServer.exe (one folder per exe)
  <Exe>/<patch-id>.toml    one patch per file, plus any .s code caves it references
  hooks/*.cpp              optional C++ midhooks, compiled into the proxy DLL
  expected_sha1.txt        optional: "Exe.exe <sha1>" lines that `make test` checks bakes against
```

```sh
make PATCHES="$HOME/my-server/patches"        # sfp + DLL with that project's hooks + build/dist patchsets
make PATCHES="$HOME/my-server/patches" test   # midhook test + bake test against the project's expected sha1s
```

The DLL embeds the project's hooks, so build it per project. Patchsets and the DLL from one
project's build go together. Config that pairs with a patch (Lua cvars and the like) belongs in
the server's own config, next to the patch sources.

## Toolchain (Linux)

- `g++` (C++17), binutils (`as`, `ld`, `objcopy` with i386 support), `clang-cl` + `lld-link`
  (clang/lld packages), `make`, `wine` (tests only).
- The MSVC CRT + Windows SDK x86 libraries, fetched with
  [cargo-xwin](https://github.com/rust-cross/cargo-xwin). This downloads Microsoft's packages
  and implies accepting their license:
  ```sh
  cargo install cargo-xwin
  XWIN_ARCH=x86 XWIN_CACHE_DIR=~/.cache/star-forge-xwin cargo xwin env --target i686-pc-windows-msvc >/dev/null
  ```
  This is a separate cache from the launcher's x86_64 one. To use another location, override it
  with `make XWIN=/path/to/xwin`.

```sh
make            # build/sfp + build/win32/libcrypto-1_1_tg.dll (no project hooks)
make test       # midhook test under wine (bake test needs PATCHES, see above)
```

The bake test looks for pristine exes in `$SC_CLOUD`, which defaults to
`$STAR_FORGE_GAME_DIR/cloud` or else the Steam install's `cloud/`. It accepts them under their
own name or as any `*.bak_*` copy with the right sha1.

## Using it on a server

The exes in `cloud/` must be the **pristine** Steam files. Hand-patched exes fail the sha1
check, and the server refuses to start.

```sh
make PATCHES=<your server repo>/patches
build/sfp install "<game>/cloud" build/win32/libcrypto-1_1_tg.dll build/dist
build/sfp status  "<game>/cloud"      # proxy installed? exes pristine? which patches are on
```

`install`:
- renames the original DLL to `libcrypto-1_1_tg_orig.dll` (first time only);
- copies in the proxy and the patchsets;
- writes a default `starforge/patcher.toml` if none exists.

Re-run it after every build. `sfp uninstall <cloud>` puts the original DLL back. Steam "verify
files" also restores the original DLL, so re-run `install` after one.

Other commands:

```sh
build/sfp verify build/dist/Shard.exe.patchset.toml Shard.exe   # state of each patch in a file
build/sfp bake   build/dist/Shard.exe.patchset.toml Shard.exe -o Shard.patched.exe [--enable id] [--disable id]
build/sfp gen-def <libcrypto-1_1_tg.dll> libcrypto-1_1_tg_orig src/dll/libcrypto-1_1_tg.def
```

## Adding a patch

In your server's patch project:

1. Find the site in Ghidra. Note the VA and the exact original bytes.
2. Add `<Exe>/<id>.toml`, and a `.s` file or a `hooks/*.cpp` midhook if needed.
   If the exe is new, add its `target.toml` (pristine sha1).
3. `make PATCHES=<dir> && build/sfp verify build/dist/<Exe>.patchset.toml <pristine exe>` should
   show it `pristine`.
4. Test it on a server, then open a PR there. The description in the toml is the documentation.

## Caveats

- Code patches apply in DllMain, before the exe's CRT init. A *data* patch to a global that the
  exe's static initializers write will be overwritten. Use a midhook later in startup for those.
- Windows antivirus may flag a proxy DLL that patches memory. Wine doesn't care.
- `CrashReporter.exe` and the legacy `StatServer.exe` don't import libcrypto. Neither is launched.
- `game.exe` (the client) is 64-bit. Client patches would need an x64 build of the loader;
  the patch format is meant to be shared.
