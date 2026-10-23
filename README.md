# Bootcraft

Post-install setup for Linux workstations: drivers, codecs, Flatpak, everyday
tools and optional debloating, for **Fedora**, **Arch Linux**, **Ubuntu**,
**Linux Mint** and **Debian**. Bootcraft is the successor to the
[post-linux](https://github.com/ArcticLatent/post-linux) script and is being
rebuilt as a Rust + Qt Quick desktop app.

> **Status:** early development. System detection and dry-run planning work
> on Arch Linux; nothing is installed yet. Other distributions and the GUI
> follow.

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

### NVIDIA driver branches

| Cards | Branch |
|:--|:--|
| Turing and newer (GTX 16, RTX 20 and later) | Current driver, open kernel modules |
| Maxwell, Pascal, Volta (GTX 900/10, Titan V) | 580 legacy driver |
| Kepler and older | None packaged; keep nouveau |

## Layout

```
crates/core     detection, task catalog, planner, probe (no root, no Qt)
crates/helper   bootcraft-helper: the privileged side, also a headless CLI
```

The GUI (`crates/app`, CXX-Qt + Qt Quick Controls) will run unprivileged and
start `bootcraft-helper` once through pkexec. The helper accepts task IDs only
and re-detects the system itself.

## Try it

```bash
cargo run -p bootcraft-helper -- detect           # what Bootcraft sees
cargo run -p bootcraft-helper -- tasks            # available tasks, ticked = recommended
cargo run -p bootcraft-helper -- plan             # dry run of the recommended set
cargo run -p bootcraft-helper -- plan --select gpu.nvidia,debloat.kde
cargo run -p bootcraft-helper -- plan --nvidia legacy --desktop gnome --json
```

## Development

```bash
cargo test                 # unit tests + fixture-based snapshot tests
cargo insta review         # after an intended change to a plan snapshot
cargo clippy --all-targets
```

## License

MIT © 2026 Burce Boran
