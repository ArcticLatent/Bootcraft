# Changelog

Release notes for Bootcraft, newest first. `scripts/release.sh` releases the
version at the top: each section starts with `## <version>`.

## 0.1.0

The first release of Bootcraft, the successor to the post-linux script, rebuilt
as a native Qt Quick desktop app.

- Post-install setup for Fedora 44+, Arch Linux, Ubuntu 24.04/26.04, Linux Mint
  22 and Debian 13: system update, the right NVIDIA driver for your card (open
  modules for Turing and newer, the 580 legacy driver for Maxwell, Pascal and
  Volta), media codecs, Flatpak with Flathub, archive tools, NTFS/exFAT, fonts
  and firmware updates.
- Workflows that stack on top of the essentials: Creative, Development (with AI
  tools and ComfyPilot) and Gaming, from about 75 curated apps.
- Apps from Flathub or your distribution: automatic per app, or your choice.
- Shows the full plan before changing anything, skips what is already in place,
  and can export the plan as a bash script.
- Optional debloating, Snap to Flatpak on Ubuntu, and a firewall.
- Dark and light themes, in your distribution's colours.
- Updates itself: checks for a new version at startup and installs it from
  Settings after verifying the release's signature.
- Packages for Arch, Fedora, Debian 13 and Ubuntu 26.04, and an AppImage for
  Ubuntu 24.04, Linux Mint 22 and everything else.
