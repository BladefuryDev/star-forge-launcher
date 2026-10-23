# How it works

```
STAR FORGE Launcher ──▶ sc-inject.exe ──▶ game.exe (started suspended)
 (checks the pack,                          └─ sc_agent.dll
  syncs files)                                   ├─ applies the profile's patches
                                                 ├─ redirects file reads to <profile>/files/
                                                 └─ points the game's data folder at <profile>/state/
```

## Starting the game

Before a launch the launcher:

1. checks the pack against its signature and pinned key
   ([Signing and trust](signing-and-trust.md));
2. asks the pack's update address whether this version is still accepted;
3. refreshes files from the pack's file list, if it has one
   ([File lists](file-lists.md));
4. checks the pack's patches against the installed `game.exe`;
5. copies the player's graphics settings and key bindings into the profile.

It then starts `sc-inject.exe`, through the same Proton build and prefix Steam
uses on Linux, or directly on Windows. The injector writes the server address
to the profile's `files/data/address.cfg`, starts `game.exe` suspended, loads
`sc_agent.dll` into it and resumes it. Everything the game starts (such as the
built-in local server) runs in a job object and is stopped when the game
exits. Afterwards the launcher copies changed settings back.

## The agent

The agent runs inside the game before any of the game's own code:

- **Patches.** It applies the pack's patchsets to the game's memory, checking
  every write's original bytes, and patches DLLs as they load. Then it loads
  the pack's plugins. See [Client patches](client-patches.md).
- **File redirection.** It hooks the Windows file functions (`CreateFile`,
  `GetFileAttributes`, `FindFirstFile`…). A path inside the game folder that
  also exists in the profile's `files/` opens the profile's copy, and folder
  listings include files only the profile has, because the engine finds loose
  files by listing `data\`. The game folder itself is never written to.
- **Game data folder.** The game asks Windows for its per-user folder once at
  startup (`SHGetFolderPath(CSIDL_LOCAL_APPDATA)`) and keeps everything there
  under `Targem\StarConflict`: store cache, notes, logs, and the local
  server's database (whose paths it passes on to those processes). The agent
  answers with `<profile>/state/AppData/Local`, so each profile has its own.

It logs what it did to the profile's `agent.log`.

## Limits

- A file inside a `.pak` archive wins over a loose file at the same path, so
  `files/` can't replace a single packed file without a client patch.
- Patches are tied to one `game.exe` build; after a game update the launcher
  refuses them rather than misapplying them.
