# patches/

Client patches for this pack. **Experimental**: the format of the files is
settled (it's the one the [STAR FORGE Patcher](../../../patcher/docs/FORMAT.md)
uses for server binaries), but how they are applied may still grow new methods.

- `<module>.patchset.toml`: byte patches for one module, usually
  `game.exe.patchset.toml`. One file per module.
- `*.dll`: native plugins, listed in `pack.json` under `patches.plugins`.

Delete this folder if your pack doesn't patch the game.

## Patchsets

```toml
format = 1
exe = "game.exe"
sha1 = "…sha1sum of the pristine win64/game.exe…"
build = "1.99.0.167293"      # informational
image_base = 0x140000000     # addresses below assume this base

[[patch]]
id = "my-patch"              # players can turn patches on/off by id
title = "One line"
description = """Why and what, with RE notes."""
default = true               # on unless the player turns it off
optional = false             # true: a mismatch skips this patch instead of failing

[[patch.write]]
va = 0x140123456             # virtual address, image base included
orig = "0f8512000000"        # bytes that must be there now
bytes = "909090909090"       # replacement, same length
comment = "nop out the jne"
```

Every write lists the bytes it expects, so a patch never lands on the wrong
code. If `game.exe`'s sha1 differs from `sha1` (a game update), nothing is
applied. The launcher checks this before starting the game and when the pack
is installed. With `"on_error": "exit"` (the default) it refuses to start the game;
with `"continue"` it starts the game without the patches that don't match.

`midhook` writes (C++ hooks) are not supported for the 64-bit client yet; use
a plugin for hooks. Code caves can go in zero slack at the end of a section,
as with the server patcher.

## pack.json

```json
"patches": {
  "method": "inject",
  "on_error": "exit",
  "plugins": ["patches/my-hooks.dll"]
}
```

All of it is optional; the defaults are `inject` and `exit`, with no plugins.

- `method`: how patches are applied. `inject` is the only one so far: the
  launcher's agent DLL writes them into the game's memory while it is still
  suspended, and patches a DLL the moment it loads. The files on disk are
  never changed. Other methods (for example a patched copy of the
  executable) can be added later without changing patchsets.
- `plugins`: x64 DLLs the agent loads into the game after patching. A plugin
  can export `int starforge_plugin_init(const PluginApi *api)` (return 0 for
  success), with:

  ```c
  typedef struct {
      uint32_t version;                        /* 1 */
      const wchar_t *profile_dir;              /* valid during the call only */
      void (*log)(const char *utf8, size_t n); /* writes to agent.log */
  } PluginApi;
  ```

  The plugin runs under the loader lock before the game's own startup code,
  so keep `starforge_plugin_init` short: install hooks, start a thread if needed.
  Players are warned that the pack runs native code.

## Players

Players can turn single patches off in their profile's `local.json`, which
pack updates never touch:

```json
"patches": { "game.exe": { "my-patch": false } }
```

The agent logs what it applied (or why it didn't) to `agent.log` in the
profile folder.
