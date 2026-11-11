# Valet Linux+ Desktop

Optional desktop UI for [valet-linux-plus](https://github.com/FernandoEbert/valet-linux-plus).

It shows the state of your Valet environment in a window: services, PHP versions and every served site. The CLI stays the source of truth. The app reads what Valet reports and never reimplements its rules, so nothing changes for anyone who only uses Composer.

> [!NOTE]
> Early development (0.0.x). The current build is **read-only**. Actions such as restarting services, park/link, secure and isolate are planned (see [Roadmap](#roadmap)).

## Features

- **Dashboard:** status of nginx, dnsmasq, PHP-FPM (global and isolated versions), MySQL/MariaDB, Redis and Mailpit, plus the Valet version, domain and ports, the global PHP and the PHP your CLI resolves.
- **Sites:** parked, linked and proxied sites in one table, with URL, type, effective PHP version (isolated → `.valetphprc` → global), SSL, folder or proxy target and custom nginx. You can filter the table by text and open any site in the browser.
- Refreshes every 5 seconds while the window is visible, or on demand.

## Compatibility

No minimum Valet version is required:

| Valet installed | How the app reads the state |
|---|---|
| `>= 2.1.3` | `valet state --json`, after checking its `schema_version` |
| older, or when the command fails | read-only fallback that reads `~/.config/valet` and `systemctl` directly; the app shows a "limited data" notice |

Everything runs as your user, and the app never needs sudo to read the state. Secrets stored in `~/.config/valet/config.json` (such as database credentials) are never read into the app.

The app looks for the `valet` binary in `PATH`, then `~/.config/composer/vendor/bin`, then `~/.composer/vendor/bin`, then `composer global config bin-dir`.

## Installation

Prebuilt packages (AppImage, `.deb`, `.rpm`) will be published on [GitHub Releases](https://github.com/FernandoEbert/valet-linux-plus-desktop/releases), and later on Flathub. Until then, build from source.

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
pnpm tauri build        # build release bundles
```

Backend checks, from `src-tauri/`:

```bash
cargo test                          # unit tests with fixtures
cargo test -- --include-ignored     # also compares CLI vs. files fallback on this machine (needs valet >= 2.1.3)
cargo clippy --all-targets -- -D warnings
cargo fmt --check
```

To exercise the fallback in the UI, start a debug build with `VALET_UI_FORCE_FILES=1 pnpm tauri dev`.

### Project layout

```
src/                        Vue frontend
  stores/valet.ts           Pinia store: state, refresh, polling
  views/                    Dashboard, Sites
  components/               ServiceCard, SiteTable, SourceNotice, ...
  types/valet.ts            TypeScript mirror of the state contract
src-tauri/src/valet/
  state.rs                  get_state command: CLI first, files fallback otherwise
  cli.rs                    `valet state --json` + schema check
  files.rs                  read-only fallback over ~/.config/valet
  services.rs               systemd unit status (systemctl show)
  locate.rs                 finds the valet binary
  runner.rs                 HostRunner trait: every process goes through it
  model.rs                  serde structs for the state (schema v1)
```

### Security notes

- Every external process goes through the `HostRunner` trait as a program plus an argument list (`Command::args()`), never through a shell.
- Tauri capabilities expose only the app's own command and opening `http(s)` URLs. The webview runs with a strict CSP.

## Roadmap

- [x] `valet state --json` in the CLI (valet-linux-plus 2.1.3)
- [x] Read-only Dashboard and Sites, with a fallback for older Valet versions
- [ ] Actions through the CLI (an allowlist of commands, `pkexec` + a polkit policy for privileged ones), plus Paths, PHP and Logs screens
- [ ] System tray, autostart, start hidden, notifications and live updates via inotify
- [ ] Release CI: AppImage, `.deb`, `.rpm`, auto-update
- [ ] Flatpak / Flathub

## License

MIT
