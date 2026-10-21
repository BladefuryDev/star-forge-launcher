# Patch source format

Sources live in a server's patch project, `<dir>/<Exe>/` (see the README). `sfp build <dir> build/dist`
(or `make PATCHES=<dir>`) resolves them into one
`build/dist/<Exe>.patchset.toml` per exe, which is what the DLL loads and `sfp bake` applies.
Never edit generated patchsets.

## `<Exe>/target.toml`

```toml
exe = "DedicatedServer.exe"
sha1 = "86089ece275c6aa57b5be0c00de0c832ffd7b79c" # pristine Steam image
build = "1.99.0.167293"                           # informational
image_base = 0x400000                              # the exes are not ASLR; must match at runtime
```

A patchset applies only to the exact pristine image. If the exe on disk has a different
sha1, the process refuses to start (`on_error = "exit"`).

## `<Exe>/<patch-id>.toml`

One logical patch per file (fewer merge conflicts). All writes in a patch are verified first
and then applied together. A patch is never half-applied.

```toml
id = "my-patch"            # defaults to the file name; used in patcher.toml
title = "One line"
description = """Why, what it changes, RE notes (addresses, functions)."""
author = "you"
default = true             # on unless patcher.toml says otherwise
optional = false           # true: a verify failure skips this patch instead of failing the process

[[write]]
va = 0x4a1b2c              # virtual address (image base included)
orig  = "0f85ec000000"     # bytes that must be there now (hex, spaces/_ allowed)
bytes = "31c090909090"     # replacement, same length
comment = "xor eax,eax; nop x4"
```

Every write has exactly one of these kinds:

| key | meaning |
|---|---|
| `bytes = "hex"` | Literal replacement, same length as `orig`. |
| `asm = "file.s"` | GNU as (`.intel_syntax noprefix` OK), linked at `va` (`ld -Ttext=va`), `.text` bytes used. `orig` defaults to zeros: code caves go in the zero slack at the end of `.text`. Absolute targets like `jmp 0x4a1c00` work. |
| `jmp = "label" \| 0xADDR` | Writes `jmp rel32`, padded with `nop` to `len(orig)` (>= 5). |
| `call = "label" \| 0xADDR` | Same with `call rel32`. |
| `midhook = "name"` | C++ midhook (see below). `orig` = the stolen bytes, >= 5, and they must be relocatable: no relative jmp/call/jcc. |

`label = "cave"` on any write names its `va` for `jmp`/`call` writes in the same file.

### Code caves vs C++

Caves use slack bytes inside the exe's `.text`, and each exe has only a few hundred. `sfp build`
fails if two default-enabled patches write the same bytes, and warns for mutually exclusive
pairs. For anything bigger than a few dozen instructions, write a C++ midhook instead.

## C++ midhooks

Put them in the project's `hooks/*.cpp`. `make PATCHES=<dir>` compiles them into the DLL:

```cpp
#include "sfp_hook.hpp"

SFP_MIDHOOK(skip_for_role_3) {                     // r = sfp::Regs*
    auto unit = r->ptr<const uint32_t>(r->esi);
    if (unit[1] == 3) return 0x4a1c00;  // jump there instead (stolen bytes not run)
    return 0;                           // continue: run stolen bytes, resume after them
}
```

The thunk saves all registers and flags, calls you with `Regs*`, and restores them, including
any you changed (`r->eax = ...`). `r->stack()[i]` is `[esp+4*i]` as seen at the hook site.
Hooks run in the game's own thread. Keep them fast and don't throw.

## Runtime config: `cloud/starforge/patcher.toml`

```toml
on_error = "exit"          # or "continue": log and run with whatever verified

[patches."DedicatedServer.exe"]
"my-patch" = false
"my-patch-cpp" = true
```

Read by every process at startup, so changes need a server restart but no rebuild. Logs go
to `cloud/starforge/logs/<Exe>.log`.
