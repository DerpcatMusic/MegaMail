#!/usr/bin/env bash
set -Eeuo pipefail

die() {
    printf 'MegaMail installer: %s\n' "$*" >&2
    exit 1
}

[[ "$(uname -s)" == Linux ]] || die "this installer supports Linux only."
[[ "${EUID:-$(id -u)}" -ne 0 ]] || die "run this as your desktop user, without sudo."
[[ -n "${HOME:-}" && -d "$HOME" ]] || die "HOME must name an existing user directory."

for command_name in cargo python3 install mktemp mv rm cp chmod rmdir; do
    command -v "$command_name" >/dev/null 2>&1 || die "required command not found: $command_name"
done

script_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd -P)"
repo_root="$(cd -- "$script_dir/.." && pwd -P)"
manifest="$repo_root/apps/megamail/Cargo.toml"
desktop_template="$repo_root/data/megamail.desktop"
icon_source="$repo_root/data/megamail.svg"
license_bundle="$repo_root/docs/licenses"
lockfile="$repo_root/apps/megamail/Cargo.lock"
license_inventory="$license_bundle/inventory.json"

[[ -f "$manifest" ]] || die "native app manifest not found: $manifest"
[[ -f "$desktop_template" ]] || die "desktop entry not found: $desktop_template"
[[ -f "$icon_source" ]] || die "application icon not found: $icon_source"
[[ -d "$license_bundle" && ! -L "$license_bundle" ]] || die "generated license bundle not found: $license_bundle"
[[ -f "$license_inventory" && ! -L "$license_inventory" ]] || die "generated license inventory not found: $license_inventory"
[[ -f "$license_bundle/README.md" && -f "$license_bundle/inventory.md" ]] \
    || die "generated license bundle is incomplete; run python3 tools/license-inventory.py"
[[ -f "$lockfile" ]] || die "app lockfile not found: $lockfile"

metadata_file="$(mktemp)"
cargo metadata --format-version 1 --locked --manifest-path "$manifest" >"$metadata_file" \
    || { rm -f -- "$metadata_file"; die "Cargo metadata failed; check the native app manifest and lockfile."; }
if metadata_summary="$(python3 - "$metadata_file" "$lockfile" "$license_inventory" <<'PY'
import hashlib
import json
from pathlib import Path
import sys

try:
    data = json.loads(Path(sys.argv[1]).read_text(encoding="utf-8"))
except (OSError, json.JSONDecodeError) as error:
    raise SystemExit(f"cannot read Cargo metadata: {error}")
lockfile = Path(sys.argv[2])
inventory_path = Path(sys.argv[3])
try:
    inventory = json.loads(inventory_path.read_text(encoding="utf-8"))
except (OSError, json.JSONDecodeError) as error:
    raise SystemExit(f"cannot read generated license inventory: {error}")

packages = [package for package in data["packages"] if package["name"] == "megamail"]
if len(packages) != 1:
    raise SystemExit("expected exactly one megamail package in Cargo metadata")
binary_targets = [target["name"] for target in packages[0]["targets"] if "bin" in target["kind"]]
if len(binary_targets) != 1:
    raise SystemExit("expected exactly one megamail binary target")
target_directory = data["target_directory"]
if not target_directory:
    raise SystemExit("Cargo metadata did not return a target directory")

nodes = data.get("resolve", {}).get("nodes")
if not isinstance(nodes, list):
    raise SystemExit("Cargo metadata returned no resolved dependency graph")
resolved_ids = {node["id"] for node in nodes}
remote_ids = sorted(
    package["id"]
    for package in data["packages"]
    if package["id"] in resolved_ids and package.get("source")
)
graph_hash = hashlib.sha256(("\n".join(remote_ids) + "\n").encode("utf-8")).hexdigest()
lock_hash = hashlib.sha256(lockfile.read_bytes()).hexdigest()
if inventory.get("lockfile_sha256") != lock_hash or inventory.get("dependency_package_ids_sha256") != graph_hash:
    raise SystemExit(
        "generated license bundle does not match the app lockfile/dependency graph; "
        "run python3 tools/license-inventory.py before installing"
    )

print(target_directory)
print(binary_targets[0])
PY
)"; then
    metadata_status=0
else
    metadata_status=$?
fi
rm -f -- "$metadata_file"
[[ "$metadata_status" -eq 0 ]] || die "could not read app metadata or verify the generated license bundle."
target_directory="${metadata_summary%%$'\n'*}"
binary_name="${metadata_summary#*$'\n'}"
[[ -n "$target_directory" && -n "$binary_name" ]] || die "Cargo metadata returned incomplete build paths."

printf 'Building MegaMail (locked release build, two jobs)…\n'
cargo build --release --locked -j 2 --manifest-path "$manifest" --bin "$binary_name"

built_binary="$target_directory/release/$binary_name"
[[ -x "$built_binary" ]] || die "Cargo succeeded but the expected executable is missing: $built_binary"

data_home="${XDG_DATA_HOME:-$HOME/.local/share}"
[[ "$data_home" == /* ]] || data_home="$HOME/.local/share"
bin_directory="$HOME/.local/bin"
applications_directory="$data_home/applications"
icon_directory="$data_home/icons/hicolor/scalable/apps"
app_data_directory="$data_home/megamail"
installed_binary="$bin_directory/megamail"
installed_desktop="$applications_directory/megamail.desktop"
installed_icon="$icon_directory/megamail.svg"
installed_licenses="$app_data_directory/licenses"

mkdir -p -- "$bin_directory" "$applications_directory" "$icon_directory" "$app_data_directory"
[[ ! -L "$installed_licenses" ]] || die "refusing to replace a symlink at $installed_licenses"

temporary_binary=""
temporary_icon=""
temporary_desktop=""
temporary_licenses=""
backup_licenses=""
cleanup() {
    [[ -z "$temporary_binary" ]] || rm -f -- "$temporary_binary"
    [[ -z "$temporary_icon" ]] || rm -f -- "$temporary_icon"
    [[ -z "$temporary_desktop" ]] || rm -f -- "$temporary_desktop"
    [[ -z "$temporary_licenses" ]] || rm -rf -- "$temporary_licenses"
    if [[ -n "$backup_licenses" && -d "$backup_licenses" ]]; then
        if [[ -e "$installed_licenses" ]]; then
            rm -rf -- "$backup_licenses"
        else
            mv -- "$backup_licenses" "$installed_licenses" || true
        fi
    fi
}
trap cleanup EXIT

temporary_binary="$(mktemp "$bin_directory/.megamail.XXXXXX")"
install -m 0755 -- "$built_binary" "$temporary_binary"
mv -f -- "$temporary_binary" "$installed_binary"
temporary_binary=""

temporary_icon="$(mktemp "$icon_directory/.megamail.svg.XXXXXX")"
install -m 0644 -- "$icon_source" "$temporary_icon"
mv -f -- "$temporary_icon" "$installed_icon"
temporary_icon=""

temporary_desktop="$(mktemp "$applications_directory/.megamail.desktop.XXXXXX")"
python3 - "$desktop_template" "$temporary_desktop" "$installed_binary" <<'PY'
from pathlib import Path
import sys

source = Path(sys.argv[1])
destination = Path(sys.argv[2])
executable = sys.argv[3]
if "\n" in executable or "\r" in executable:
    raise SystemExit("the executable path cannot contain a newline")

def quote_exec_argument(value):
    escaped = "".join("\\" + char if char in '\\`"$' else char for char in value)
    return '"' + escaped.replace("%", "%%") + '"'

lines = source.read_text(encoding="utf-8").splitlines()
exec_lines = [index for index, line in enumerate(lines) if line.startswith("Exec=")]
if len(exec_lines) != 1:
    raise SystemExit("the desktop entry must contain exactly one Exec key")
lines[exec_lines[0]] = "Exec=" + quote_exec_argument(executable)
destination.write_text("\n".join(lines) + "\n", encoding="utf-8")
destination.chmod(0o644)
PY
mv -f -- "$temporary_desktop" "$installed_desktop"
temporary_desktop=""

temporary_licenses="$(mktemp -d "$app_data_directory/.licenses.XXXXXX")"
cp -a -- "$license_bundle/." "$temporary_licenses/"
chmod 0755 "$temporary_licenses"
if [[ -e "$installed_licenses" ]]; then
    [[ -d "$installed_licenses" ]] || die "license destination is not a directory: $installed_licenses"
    backup_licenses="$(mktemp -d "$app_data_directory/.licenses-backup.XXXXXX")"
    rmdir -- "$backup_licenses"
    mv -- "$installed_licenses" "$backup_licenses"
fi
mv -- "$temporary_licenses" "$installed_licenses"
temporary_licenses=""
if [[ -n "$backup_licenses" ]]; then
    rm -rf -- "$backup_licenses"
    backup_licenses=""
fi

printf 'Installed MegaMail for this user.\n'
printf 'Binary: %s\n' "$installed_binary"
printf 'Desktop entry: %s\n' "$installed_desktop"
printf 'License bundle: %s\n' "$installed_licenses"
printf 'Open MegaMail from your desktop application menu; this installer does not launch a background service.\n'
