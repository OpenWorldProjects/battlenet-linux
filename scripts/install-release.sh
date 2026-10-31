#!/bin/sh
set -eu
cd "$(dirname "$0")/.."
version=$(sed -n 's/^version = "\([^"]*\)"/\1/p' Cargo.toml | head -1)
bundle="dist/battlenet-linux-$version-x86_64.flatpak"
if [ ! -f "$bundle" ]; then sh scripts/package-flatpak.sh; fi
if [ "${1:-}" != "--migrate-only" ]; then
    # Existing runtimes need no unrelated upgrade during a local app install.
    if flatpak info org.gnome.Platform//50 >/dev/null 2>&1; then
        flatpak --user install --noninteractive --no-deps --reinstall --bundle "$bundle"
    else
        flatpak --user install --noninteractive --reinstall --bundle "$bundle"
    fi
fi
python3 - <<'PY'
from pathlib import Path
import shutil,os
home=Path.home()
source=Path(os.environ.get('XDG_DATA_HOME',str(home/'.local/share')))
dest=home/'.var/app/io.github.battlenet_linux.Launcher/data'
dest.mkdir(parents=True,exist_ok=True)
# Keep existing Flatpak settings and sessions. Copy only this app's own data.
app=source/'battlenet-linux-launcher'
if app.is_dir():
 target=dest/app.name
 if not target.exists():
  shutil.copytree(app,target)
  for root,dirs,files in os.walk(target):
   os.chmod(root,0o700)
   for name in files: os.chmod(Path(root)/name,0o600)
settings=dest/'io.github.battlenet_linux.launcher'
settings.mkdir(parents=True,exist_ok=True)
if not (settings/'app.ron').exists():
 candidates=[source/name/'app.ron' for name in ['io.github.battlenet_linux.launcher','battle.netlinux','battle.netlinuxlauncher']]
 candidates=[p for p in candidates if p.is_file()]
 if candidates:
  shutil.copy2(max(candidates,key=lambda p:p.stat().st_mtime),settings/'app.ron')
  os.chmod(settings/'app.ron',0o600)
print('Installed Battle.net Linux; existing settings and saved sign-in preserved.')
PY
