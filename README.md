<p align="center"><img width="96" src="art/icon.svg" alt=""></p>

<h1 align="center">Valet Linux+ Desktop</h1>

<p align="center">
<a href="https://github.com/FernandoEbert/valet-linux-plus-desktop/releases/latest"><img src="https://img.shields.io/github/v/release/FernandoEbert/valet-linux-plus-desktop" alt="Latest release"></a>
<a href="https://github.com/FernandoEbert/valet-linux-plus-desktop/actions/workflows/ci.yml"><img src="https://github.com/FernandoEbert/valet-linux-plus-desktop/actions/workflows/ci.yml/badge.svg" alt="CI"></a>
<a href="LICENSE"><img src="https://img.shields.io/github/license/FernandoEbert/valet-linux-plus-desktop" alt="License"></a>
</p>

Optional desktop app for [valet-linux-plus](https://github.com/FernandoEbert/valet-linux-plus).

It shows and manages your Valet environment from a window and the system tray: services, sites, parked paths, PHP versions and logs. The CLI stays the source of truth: the app reads what Valet reports and every change runs as a `valet` command. Nothing changes for anyone who only uses Composer.

## Features

- **Dashboard:** nginx, dnsmasq, PHP-FPM (global and isolated versions), MySQL/MariaDB, Redis and Mailpit with start, stop and restart; Valet version, domain and ports, the global PHP and the PHP your CLI resolves.
- **Sites:** parked, linked and proxied sites in one table, with URL, type, effective PHP (isolated → `.valetphprc` → global), SSL, folder and custom nginx. Open in the browser or the file manager, secure/unsecure, isolate to any installed PHP, unlink, and link a new folder.
- **Paths:** park and forget folders.
- **PHP:** switch the version Valet serves (`valet use`, optionally `--update-cli`), or only the `php` of your terminal.
- **Logs:** tail of the nginx logs in `~/.config/valet/Log`, with follow.
- **Tray:** service status, restart all, quick access to sites. Closing the window keeps the app in the tray.
- **Notifications** when a running service stops, and **live updates** when Valet changes from a terminal (`valet link`, `park`, `secure`…).
- **Start at login** (hidden in the tray) and **automatic updates**.

## Requirements

- [valet-linux-plus](https://github.com/FernandoEbert/valet-linux-plus) installed through Composer. Any version works; 2.1.3+ gives the full state (see [Compatibility](#compatibility)).
- An x86_64 Linux desktop with WebKitGTK 4.1 (Ubuntu 22.04+, Debian 12+, Fedora 38+ and derivatives).
- For the tray icon on **GNOME**: the [AppIndicator extension](https://extensions.gnome.org/extension/615/appindicator-support/) (Ubuntu, Zorin and Pop!_OS ship it). Without a tray, the app works as a normal window and closing it quits.

## Installation

Download from the [latest release](https://github.com/FernandoEbert/valet-linux-plus-desktop/releases/latest):

| Format | Install | Updates |
|---|---|---|
| **`.deb`** (Ubuntu, Debian, Zorin, Mint, Pop!_OS) | `sudo apt install ./Valet.Linux.Plus.Desktop_*_amd64.deb` | In the app (installs the new package with your password) |
| **`.rpm`** (Fedora, RHEL, openSUSE) | `sudo dnf install ./Valet.Linux.Plus.Desktop-*.x86_64.rpm` | In the app (installs the new package with your password) |
| **AppImage** (any distro) | `chmod +x Valet.Linux.Plus.Desktop_*_amd64.AppImage` and run it | In the app (replaces the AppImage) |

The installed package is called `valet-linux-plus-desktop` (`sudo apt remove valet-linux-plus-desktop` / `sudo dnf remove valet-linux-plus-desktop`). The `.deb` and `.rpm` also install a small privileged helper (`/usr/libexec/valet-linux-plus-desktop/valet-helper`) and its polkit policy. The app then asks for your password **once per session**, with its own prompt. The AppImage embeds the same helper and uses the generic polkit prompt.

The app checks for a new version when it starts; you can also use **Settings → Check for updates**. Updates are signed and verified before being installed.

## How privileges work

Reading the state never needs root. Commands that the `valet` script itself runs with `sudo` (`start`, `stop`, `restart`, `secure`, `unsecure`, `use`, `isolate`, `unisolate`) go through a helper started with `pkexec` on the first privileged action of the session:

- it accepts only those commands, validated again on every request, and runs the same `valet` binary you would run with sudo;
- it talks to the app only through its own stdin/stdout, so no other process can reach it;
- it passes your `HOME` and user to Valet, so files in `~/.config/valet` stay yours;
- it exits when the app closes, or when you click **End** on the "Admin session" badge.

## Compatibility

No minimum Valet version is required:

| Valet installed | How the app reads the state |
|---|---|
| `>= 2.1.3` | `valet state --json`, after checking its `schema_version` |
| older, or when the command fails | read-only fallback that reads `~/.config/valet` and `systemctl` directly; the app shows a "limited data" notice |

Secrets stored in `~/.config/valet/config.json` (such as database credentials) are never read into the app.

The app looks for the `valet` binary in `PATH`, then `~/.config/composer/vendor/bin`, then `~/.composer/vendor/bin`, then `composer global config bin-dir`.

## Development

Built with [Tauri v2](https://tauri.app) (thin Rust backend), Vue 3, TypeScript, Pinia, vue-router, Tailwind CSS v4 and Vite.

### Requirements

- Rust (stable, via [rustup](https://rustup.rs))
- Node.js 22 and pnpm 10 (`corepack enable pnpm`)
- System libraries (Debian/Ubuntu):

  ```bash
  sudo apt install libwebkit2gtk-4.1-dev libayatana-appindicator3-dev librsvg2-dev \
    libxdo-dev libssl-dev build-essential curl wget file
  ```

  For other distros, see the [Tauri prerequisites](https://tauri.app/start/prerequisites/#linux).

### Commands

```bash
pnpm install
pnpm tauri dev          # run the app with hot reload
pnpm build              # type-check (vue-tsc) and build the frontend
pnpm tauri build --config '{"bundle":{"createUpdaterArtifacts":false}}'   # local packages, unsigned
```

Backend checks, from `src-tauri/`:

```bash
cargo test                          # unit tests with fixtures
cargo test -- --include-ignored     # also runs checks against the real Valet on this machine
cargo clippy --all-targets -- -D warnings
cargo fmt --check
```

To exercise the read-only fallback in the UI, start a debug build with `VALET_UI_FORCE_FILES=1 pnpm tauri dev`.

### Releasing

1. `node scripts/version.mjs 0.2.0` updates `package.json`, `Cargo.toml`, `Cargo.lock` and `tauri.conf.json` together (CI fails if they differ).
2. Merge to `main`, then push a tag: `git tag v0.2.0 && git push origin v0.2.0` (`v0.2.0-rc.1` makes a pre-release).
3. The **Release** workflow builds the AppImage, `.deb` and `.rpm` on Ubuntu 22.04, signs the update artifacts and creates a **draft** release with `latest.json`. Review it and publish.

Update signing uses the `TAURI_SIGNING_PRIVATE_KEY` and `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` repository secrets; the public key is in `src-tauri/tauri.conf.json`.

### Project layout

```
src/                          Vue frontend
  stores/                     valet (state events), actions, updater
  views/                      Dashboard, Sites, Paths, PHP, Logs, Settings
  components/                 ServiceCard, SiteTable, SiteActions, ActionLog, UpdateBanner, ...
  types/valet.ts              TypeScript mirror of the state contract
src-tauri/src/valet/          thin layer over the CLI
  state.rs, cli.rs, files.rs  `valet state --json`, schema check, read-only fallback
  actions.rs                  closed set of actions, validated, run through the CLI
  privileged.rs               per-session pkexec broker client
  logs.rs                     log listing and tail
  runner.rs, locate.rs        HostRunner (no shell, ever) and valet binary lookup
src-tauri/src/app/            desktop shell
  monitor.rs, watcher.rs      background polling, inotify, service-down notifications
  tray.rs, window.rs          tray menu, close to tray, single instance, --hidden
  settings.rs                 app preferences
packaging/linux/              valet-helper (pkexec broker) and its polkit policy
scripts/version.mjs           keeps the version in sync across manifests
```

### Security notes

- Every external process goes through the `HostRunner` trait as a program plus an argument list (`Command::args()`), never through a shell.
- The frontend can only request a closed set of actions; every argument is validated in Rust before it reaches the CLI.
- Tauri capabilities grant only what the UI uses, and the webview runs with a strict CSP.

## Roadmap

- [x] `valet state --json` in the CLI (valet-linux-plus 2.1.3)
- [x] Dashboard and Sites, with a fallback for older Valet versions
- [x] Actions through the CLI (allowlist, per-session `pkexec` broker, polkit policy), Paths, PHP and Logs
- [x] System tray, autostart, start hidden, notifications and live updates
- [x] Release CI: AppImage, `.deb`, `.rpm`, signed auto-update
- [ ] Flatpak / Flathub

## License

[MIT](LICENSE)
