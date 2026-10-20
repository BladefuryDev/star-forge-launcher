# STAR FORGE server pack template

> Pack format 1. The launcher imports packs from a zip, a folder, a link, or
> a `starforge://` link on your website.

A server pack is everything a player needs to play on your server, in one
folder: where to connect, which game files to replace, and how the launcher
should look while your server is selected. Zip the folder to hand it out.

```
my-server/
├── pack.json        what the pack is and how to launch it
├── resources/       launcher artwork: logo, icon, background
└── files/           game file overlays, mirroring the game folder
```

## Making your pack

1. Copy this folder and rename it after your server.
2. Edit `pack.json`. At minimum set `id`, `name`, `version` and
   `server.address`. Editors such as VS Code read the `$schema` line and
   offer autocomplete and error checking; the full reference is
   [`schemas/pack.schema.json`](../../schemas/pack.schema.json).
3. Put your artwork in `resources/` (sizes in
   [resources/README.md](resources/README.md)), or delete the `branding`
   entries you don't need.
4. Put replacement game files in `files/`, at the same path they have under
   the game folder: `files/data/...` replaces `<game>/data/...`. Delete the
   folder if you don't replace anything.
5. Zip the folder and share it. Players import it with **Add profile →
   Choose zip…**, or `sc-launch install my-server.zip`.

To try it while you work, import the folder itself:
`sc-launch install path/to/my-server`.

## Publishing with updates

Host two files over HTTPS: the zip, and a copy of `pack.json` with a
`download` entry telling the launcher where the zip is and its SHA-256:

```json
"download": {
  "url": "https://example.org/star-forge/my-server-1.0.0.zip",
  "sha256": "…output of sha256sum my-server-1.0.0.zip…"
}
```

Then give players a link to that `pack.json`. They can paste it into the
launcher, or click a link on your site:

```
starforge://install?url=https://example.org/star-forge/pack.json
```

(URL-encode the address if it has its own `?` or `&`.) The launcher checks the
download against the SHA-256, shows the player what the pack does, and
remembers the address: from then on it checks that `pack.json` for new
versions and offers the update.

To release an update: bump `version`, keep `id` the same, upload the new
zip, and update `download` in the hosted `pack.json`. Players keep their own
changes (name, server, extra arguments) across updates, because those live
in their `local.json`, which is never part of a pack.

Optionally list the game builds the pack was made for, so players with a
different game version get a warning:

```json
"requires": { "game": ["…sha256sum of win64/game.exe…"] }
```

## Things to know

- **`.pak` files win.** The game prefers a file inside a `.pak` archive over a
  loose file at the same path, so you can replace a whole `.pak` but not a
  single file inside one.
- **Patches are experimental.** How client patches are applied is likely to
  change before format 1 settles. Each patch checks the original bytes first
  and is skipped if they don't match, so a game update makes them inactive
  rather than harmful. Check `agent.log` after an update.
- **What a pack can contain.** Only `pack.json`, `resources/`, `files/` and
  top-level README/LICENSE/CHANGELOG files are installed; anything else in
  the zip (including dotfiles) is ignored.
- **Players are trusting you.** Overlays and patches change what the game
  runs. Only publish packs from a source players can verify, such as your
  own website or repository.
