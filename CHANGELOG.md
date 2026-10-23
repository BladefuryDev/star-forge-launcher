# Changelog

Each release's section becomes the notes of its GitHub release draft, so add
to **Unreleased** as you go and rename it to the version when you tag
(`## 0.2.2`, matching the tag without the `v`).

## Unreleased

- New logo and emblem (SVG) and new app icons
- Settings: colour theme dropdown with swatches and a live preview on hover
- Dialogs keep their title and close button in place while the content scrolls
- Checkboxes are no longer cut off on Linux

## 0.2.1

- The launcher updates itself: it checks for a new version at startup (can be
  turned off in Settings, or checked by hand) and installs it and restarts.
  Update packages are signed. Install this version by hand; later ones arrive
  through the launcher
- Client patches: server packs can ship patchsets that are applied to the game
  in memory, checked against `game.exe` before launch, plus native plugins
- Processes the game leaves running (local server, MongoDB) are stopped when
  the game exits, so the launcher no longer stays on "Running"

## 0.2.0

- Server packs: import from a zip, folder, link or `starforge://` link, with a
  review before installing; checksum-verified pack updates that keep your own
  changes
- Per-pack branding (logo, icon, background, colour) and a background folder
  setting for your own wallpapers
- Mono app and taskbar icon

## 0.1.1

- STAR FORGE v1 art; the splash screen is now still
- Linux: Proton and links open with the desktop's environment, not the
  AppImage's

## 0.1.0

- First release: server profiles, Steam and Proton detection, game file
  overrides through the injected agent
