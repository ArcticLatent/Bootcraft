Name:           bootcraft
Version:        0.1.0
Release:        1%{?dist}
Summary:        Set up drivers, codecs and apps after installing Linux
%global debug_package %{nil}
%global _debugsource_packages 0
# The C++ side (cxx-qt) is linked by rustc; with LTO objects ld drops the
# Qt libraries it only needs at link time.
%global _lto_cflags %{nil}

License:        MIT
URL:            https://github.com/ArcticLatent/Bootcraft
Source0:        %{name}-%{version}.tar.gz

BuildRequires:  cargo
BuildRequires:  rust
BuildRequires:  gcc-c++
BuildRequires:  qt6-qtbase-devel
BuildRequires:  qt6-qtdeclarative-devel

# QML modules (Quick, Controls, Dialogs), SVG icons, Wayland, the desktop
# portal (native file dialogs), pkexec (the helper), curl; the libraries
# themselves are found automatically.
Requires:       qt6-qtdeclarative
Requires:       qt6-qtsvg
Requires:       qt6-qtwayland
Requires:       xdg-desktop-portal
Requires:       polkit
Requires:       curl

%description
Post-install setup for Fedora (and Arch, Ubuntu, Linux Mint and Debian): the
right NVIDIA driver from RPM Fusion, media codecs, Flatpak with Flathub, fonts
and everyday tools, plus curated creative, development and gaming apps. Shows
the full plan before changing anything, and can export it as a script.

%prep
%autosetup -n %{name}-%{version}

%build
# A shared target folder (packaging/build-linux.sh) speeds up repeated builds.
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-%{_topdir}/cargo-target}"
export RUSTFLAGS="${RUSTFLAGS:+$RUSTFLAGS }-C link-arg=-fuse-ld=bfd"
export QMAKE=qmake6
cargo build --release --locked --jobs "%{?_smp_build_ncpus}" -p bootcraft -p bootcraft-helper

%global app_id io.github.ArcticLatent.Bootcraft

%install
target="${CARGO_TARGET_DIR:-%{_topdir}/cargo-target}/release"
install -Dpm0755 "$target/bootcraft" %{buildroot}%{_bindir}/bootcraft
install -Dpm0755 "$target/bootcraft-helper" %{buildroot}%{_prefix}/lib/bootcraft/bootcraft-helper
ln -s ../lib/bootcraft/bootcraft-helper %{buildroot}%{_bindir}/bootcraft-helper
install -Dpm0644 packaging/linux/%{app_id}.policy %{buildroot}%{_datadir}/polkit-1/actions/%{app_id}.policy
install -Dpm0644 packaging/linux/%{app_id}.desktop %{buildroot}%{_datadir}/applications/%{app_id}.desktop
install -Dpm0644 packaging/linux/%{app_id}.metainfo.xml %{buildroot}%{_metainfodir}/%{app_id}.metainfo.xml
install -Dpm0644 crates/app/icons/icon.svg %{buildroot}%{_datadir}/icons/hicolor/scalable/apps/%{app_id}.svg

%files
%license LICENSE
%doc README.md
%{_bindir}/bootcraft
%{_bindir}/bootcraft-helper
%{_prefix}/lib/bootcraft/bootcraft-helper
%{_datadir}/polkit-1/actions/%{app_id}.policy
%{_datadir}/applications/%{app_id}.desktop
%{_metainfodir}/%{app_id}.metainfo.xml
%{_datadir}/icons/hicolor/scalable/apps/%{app_id}.svg

%changelog
* Sat Oct 24 2026 Arctic Latent <contact@arcticlatent.com> - 0.1.0-1
- First release.
