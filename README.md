# Bootcraft

Post-install setup for Linux workstations: drivers, codecs, Flatpak, everyday
tools and optional debloating, for **Fedora**, **Arch Linux**, **Ubuntu**,
**Linux Mint** and **Debian**. Bootcraft is the successor to the
[post-linux](https://github.com/ArcticLatent/post-linux) script and is being
rebuilt as a Rust + Qt Quick desktop app.

> **Status:** early development. On Arch Linux, Bootcraft detects, plans and
> applies, from the desktop app or the terminal; other distributions follow.

## How it works

- **Detects instead of asking.** Distribution and release (from
  `os-release`), desktop and session, every GPU (read from sysfs), the NVIDIA
  driver branch each card needs, Secure Boot state and installed kernels.
- **Shows the plan first.** Each task is made of plain-data steps (package
  installs, structured config edits, commands without a shell), so the full
  plan can be reviewed before anything runs.
- **Skips what is already done.** Package databases and config files are
  checked directly, including packages that only *provide* what a task needs
  (for example a custom `nvidia-open-dkms-tkg` build).
- **Opinionated steps are opt-in.** Debloat tasks live in their own section
  and are never ticked by default.
- **Fails safely.** A failed task skips only the tasks that need it; config
  files are written atomically with a `.bootcraft.bak` of the original; every
  run is logged to `/var/log/bootcraft/`.
- **AUR without sudo prompts.** AUR dependencies are resolved by Bootcraft,
  packages are built as your user in `~/.cache/bootcraft/aur` and installed
  with `pacman -U`.

### Workflows

Essentials is the base: updates, drivers, codecs, Flatpak, archive tools,
drive support, fonts and firmware updates; no extra apps. Workflows stack on
top of it, each with apps ticked by default and more to add:

| Workflow | Ticked by default | Also available |
|:--|:--|:--|
| **Creative** | Blender, Krita, GIMP, Inkscape, Kdenlive, OBS Studio, FFmpeg | darktable, digiKam, Shotcut, HandBrake, Audacity, Ardour, DaVinci Resolve (Arch), Synfig, Glaxnimate, OpenToonz, FreeCAD, Scribus, Upscayl and small helpers |
| **Development** | VSCodium, Git, GitHub CLI, modern CLI tools, Podman & Distrobox | VS Code, Zed, Neovim, JetBrains Toolbox, toolchains (C/C++, Rust, Python, Node.js, Go), Docker, Bruno, DBeaver, terminals; AI: [ComfyPilot](https://github.com/ArcticLatent/ComfyPilot), Ollama, Alpaca, CUDA |
| **Gaming** | Steam, Heroic, ProtonPlus, MangoHud, GameMode | Lutris, Bottles, Faugus, Discord, Prism Launcher, RetroArch, Moonlight |

### Flatpak or system packages

`--prefer auto|flatpak|native` (and a per-app choice) decides where apps come
from. **Auto** uses system packages where the distribution keeps apps current
(Arch, Fedora) and Flathub where it freezes them for years (Ubuntu, Mint,
Debian: Ubuntu 24.04 still ships GIMP 2.10). Some apps override this: editors,
terminals and Steam need the host system, Bottles is supported upstream only as
a Flatpak, and a verified Flathub build is preferred over an AUR build.
ComfyPilot is installed from its GitHub release after checking the release's
Ed25519 signature and the package's SHA-256.

### NVIDIA driver branches

| Cards | Branch |
|:--|:--|
| Turing and newer (GTX 16, RTX 20 and later) | Current driver, open kernel modules |
| Maxwell, Pascal, Volta (GTX 900/10, Titan V) | 580 legacy driver |
| Kepler and older | None packaged; keep nouveau |

## Layout

```
crates/core     detection, task catalog, planner, probe, executor (no Qt)
crates/helper   bootcraft-helper: the privileged side, also a headless CLI
crates/app      bootcraft: the Qt Quick desktop app (CXX-Qt)
data/polkit     polkit action for `pkexec /usr/lib/bootcraft/bootcraft-helper serve`
```

The app runs unprivileged and starts `bootcraft-helper serve` once per setup
through pkexec, then shows its progress. The helper accepts task and app IDs
only and re-detects the system itself.

## Try it

```bash
cargo run -p bootcraft                            # the desktop app (needs Qt 6 with QtQuick and QtSvg)

cargo run -p bootcraft-helper -- detect           # what Bootcraft sees
cargo run -p bootcraft-helper -- tasks            # available tasks, ticked = recommended
cargo run -p bootcraft-helper -- plan             # dry run of the recommended set
cargo run -p bootcraft-helper -- plan --select gpu.nvidia,debloat.kde
cargo run -p bootcraft-helper -- plan --nvidia legacy --desktop gnome --json
cargo run -p bootcraft-helper -- apps --packs development      # where each app would come from
cargo run -p bootcraft-helper -- plan --packs creative,gaming --apps comfypilot,gimp:flatpak

cargo build && sudo target/debug/bootcraft-helper apply --select media.codecs
```

`apply` prints the plan and asks before changing anything. On Arch, every
install task pulls in "Update the system" (`pacman -Syu`), because installing
from out-of-date databases is unsupported there.

## Development

```bash
cargo test                 # unit tests + fixture-based snapshot tests
cargo insta review         # after an intended change to a plan snapshot
cargo clippy --all-targets

# The app without a window: save a screenshot (optionally of page N, with
# workflows ticked, or mid-way through a sample run) and quit.
QT_QPA_PLATFORM=offscreen QT_QUICK_BACKEND=software \
  target/debug/bootcraft --screenshot=shot.png [--page=1] [--packs=creative] [--demo-run]
```

Icons are [Lucide](https://lucide.dev) (ISC), logos come from
[Simple Icons](https://simpleicons.org) (CC0; trademarks of their owners), and
the fonts are Geist and JetBrains Mono (OFL). App icons are downloaded from
Flathub on first view and cached in `~/.cache/bootcraft/icons`.

## License

MIT © 2026 Burce Boran
