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
├── files/           game file overlays, mirroring the game folder
└── patches/         client patches (optional, experimental)
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

## Signing your pack

Signing lets players (and the launcher) tell that a pack and every update
really come from you. Make a key once and keep it secret, like a password;
put it in your CI's secrets if you build packs there:

```sh
sc-launch pack keygen my-server.key     # prints the public key and fingerprint
```

Sign the pack folder before zipping it, after every change:

```sh
sc-launch pack sign my-server --key my-server.key   # or STAR_FORGE_PACK_KEY=<key file text>
```

That writes `manifest.json` (every file with its size and SHA-256) and
`manifest.sig`. When a player installs the pack, the launcher checks every
file and remembers your key for this pack `id`. From then on it only accepts
versions signed with the same key, and before each launch it checks the
pack's code (`pack.json`, `patches/`) again. **If you lose the key, players
have to remove the profile and install the pack again**, so back it up.

A signed `pack.json` can't be changed by the launcher, so set
`update.url` yourself when signing a pack you publish with updates.

Publish your key's fingerprint on your website or Discord so players can
compare it with the one the launcher shows. To have the launcher recognise
your packs by name, open a pull request adding your key to `KNOWN` in
`crates/sc-launch/src/trust.rs`.

Check a pack the way the launcher will, without installing it:

```sh
sc-launch pack check my-server          # a folder, zip or published link
```

## Requiring an update

When an update is needed to keep playing (new server data, changed patches),
set `min_version` in the **published** `pack.json`:

```json
"update": { "url": "https://example.org/star-forge/pack.json", "min_version": "1.3.0" }
```

Before each launch the launcher asks that address; a player on an older
version is told to update first. If your server can't be reached the game
still starts, with a warning.

## Things to know

- **`.pak` files win.** The game prefers a file inside a `.pak` archive over a
  loose file at the same path, so you can replace a whole `.pak` but not a
  single file inside one.
- **Patches are experimental.** See [patches/README.md](patches/README.md).
  Patchsets are tied to one `game.exe` build by its sha1 and every write
  checks the original bytes, so after a game update they are refused rather
  than misapplied. List the build in `requires.game` too, and check
  `agent.log` after an update.
- **What a pack can contain.** Only `pack.json`, `resources/`, `files/`, `patches/` and
  top-level README/LICENSE/CHANGELOG files are installed; anything else in
  the zip (including dotfiles) is ignored.
- **Each profile keeps its own game data.** The game's per-user folder (store
  cache, player notes, logs, the local server's database) is separate for
  every profile, so your server never sees another server's cached data.
  Graphics settings and key bindings follow the player across profiles.
- **Players are trusting you.** Overlays and patches change what the game
  runs. Sign your pack, and publish it from a source players can verify,
  such as your own website or repository.
