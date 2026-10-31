#!/bin/sh
set -eu
cd "$(dirname "$0")/.."
project=$(pwd)
app_id=io.github.battlenet_linux.Launcher
version=$(sed -n 's/^version = "\([^"]*\)"/\1/p' Cargo.toml | head -1)
toolchain=$(rustc --print sysroot)
sh scripts/build-bridge.sh
cargo build --manifest-path login-helper/Cargo.toml --release --locked --offline
flatpak run --command=sh --filesystem="$project" --filesystem="$HOME/.cargo" --filesystem="$HOME/.rustup" org.freedesktop.Sdk//25.08 -c '
 export PATH="$1/bin:/usr/bin:/bin"
 export CARGO_HOME="$2/.cargo"
 export CARGO_TARGET_DIR="$3/target/portable"
 cd "$3"
 cargo build --release --locked --offline
' sh "$toolchain" "$HOME" "$project"
bundle_dir="$project/target/flatpak-app"
mkdir -p "$bundle_dir/files/bin" "$bundle_dir/files/share/applications" "$bundle_dir/files/share/icons/hicolor/256x256/apps" "$bundle_dir/files/share/doc/$app_id/docs" "$project/dist"
install -m755 target/portable/release/battlenet-linux-launcher "$bundle_dir/files/bin/"
install -m755 login-helper/target/release/bnet-login "$bundle_dir/files/bin/"
install -m755 packaging/bnet-host-run "$bundle_dir/files/bin/"
install -m644 assets/battlenet-icon.png "$bundle_dir/files/share/icons/hicolor/256x256/apps/$app_id.png"
install -m644 packaging/$app_id.desktop "$bundle_dir/files/share/applications/"
install -m644 README.md assets/SOURCES.md assets/fonts/LICENSE "$bundle_dir/files/share/doc/$app_id/"
install -m644 docs/chat.md docs/release.md "$bundle_dir/files/share/doc/$app_id/docs/"
# Remove only generated exports so repeated builds can be finalized again.
python3 - "$bundle_dir/export" <<'PYEXPORT'
import shutil,sys
shutil.rmtree(sys.argv[1],ignore_errors=True)
PYEXPORT
cat > "$bundle_dir/metadata" <<EOF
[Application]
name=$app_id
runtime=org.gnome.Platform/x86_64/50
sdk=org.freedesktop.Sdk/x86_64/25.08
command=battlenet-linux-launcher
EOF
flatpak build-finish --share=network --share=ipc --socket=wayland --socket=fallback-x11 --device=dri --filesystem=home --filesystem=/mnt --filesystem=/media --filesystem=/run/media --talk-name=org.freedesktop.Flatpak --talk-name=org.kde.StatusNotifierWatcher "$bundle_dir"
flatpak build-export --disable-sandbox "$project/target/flatpak-repo" "$bundle_dir"
flatpak build-bundle --runtime-repo=https://flathub.org/repo/flathub.flatpakrepo "$project/target/flatpak-repo" "$project/dist/battlenet-linux-$version-x86_64.flatpak" "$app_id"
printf 'Release: %s/dist/battlenet-linux-%s-x86_64.flatpak\n' "$project" "$version"
