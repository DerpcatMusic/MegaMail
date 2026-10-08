#!/usr/bin/env python3
"""Generate a local license bundle from the locked GPUI app dependency graph."""

from __future__ import annotations

import argparse
import hashlib
import json
import re
import shutil
import stat
import subprocess
import sys
import tempfile
from pathlib import Path
from urllib.parse import quote
from urllib.request import Request, urlopen


ROOT = Path(__file__).resolve().parent.parent
DEFAULT_MANIFEST = ROOT / "apps/megamail/Cargo.toml"
OUTPUT = ROOT / "docs/licenses"
MAX_PACKAGES = 5_000
MAX_FILES_PER_PACKAGE = 32
MAX_FILE_BYTES = 4 * 1024 * 1024
MAX_BUNDLE_BYTES = 128 * 1024 * 1024
MAX_UPSTREAM_LICENSE_BYTES = 4 * 1024 * 1024

PROJECT_NOTICES = (
    ("LICENSE", "project/LICENSE"),
    ("crates/zeron-theme/LICENSE-MIT", "project/zeron-theme/LICENSE-MIT"),
    ("THIRD_PARTY_NOTICES.md", "project/THIRD_PARTY_NOTICES.md"),
    (
        "apps/megamail/assets/fonts/licenses/Geist-OFL.txt",
        "project/assets/Geist-OFL.txt",
    ),
    (
        "apps/megamail/assets/icons/LICENSE-LUCIDE",
        "project/assets/LICENSE-LUCIDE",
    ),
)


class InventoryError(RuntimeError):
    pass


def sha256(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def regular_file(path: Path) -> tuple[bool, int]:
    try:
        mode = path.lstat().st_mode
        size = path.lstat().st_size
    except OSError:
        return False, 0
    return stat.S_ISREG(mode), size


def cargo_metadata(manifest: Path) -> dict:
    command = [
        "cargo",
        "metadata",
        "--format-version",
        "1",
        "--locked",
        "--manifest-path",
        str(manifest),
    ]
    try:
        result = subprocess.run(
            command,
            check=True,
            cwd=ROOT,
            capture_output=True,
            text=True,
        )
    except FileNotFoundError as error:
        raise InventoryError("cargo is required to read the locked dependency graph") from error
    except subprocess.CalledProcessError as error:
        detail = error.stderr.strip() or error.stdout.strip()
        raise InventoryError(f"cargo metadata failed: {detail}") from error
    try:
        return json.loads(result.stdout)
    except json.JSONDecodeError as error:
        raise InventoryError(f"cargo metadata returned invalid JSON: {error}") from error


def resolved_remote_packages(metadata: dict) -> list[dict]:
    resolution = metadata.get("resolve")
    if not isinstance(resolution, dict) or not isinstance(resolution.get("nodes"), list):
        raise InventoryError("Cargo metadata has no resolved package graph; omit --no-deps")
    resolved_ids = {node["id"] for node in resolution["nodes"]}
    packages = [
        package
        for package in metadata.get("packages", [])
        if package.get("id") in resolved_ids and package.get("source")
    ]
    if len(packages) > MAX_PACKAGES:
        raise InventoryError(
            f"resolved graph has {len(packages)} remote packages; limit is {MAX_PACKAGES}"
        )
    return sorted(
        packages,
        key=lambda package: (
            package.get("name", ""),
            package.get("version", ""),
            package.get("source", ""),
        ),
    )


def package_graph_hash(packages: list[dict]) -> str:
    ids = sorted(package["id"] for package in packages)
    return sha256(("\n".join(ids) + "\n").encode("utf-8"))


def safe_directory_name(package: dict, duplicate: bool) -> str:
    raw = f"{package['name']}-{package['version']}"
    safe = re.sub(r"[^A-Za-z0-9._+-]", "_", raw).strip("._-")[:120]
    if not safe:
        safe = "dependency"
    if duplicate:
        safe += "-" + sha256(package["id"].encode("utf-8"))[:10]
    return safe


def package_directories(packages: list[dict]) -> dict[str, str]:
    by_base: dict[str, list[dict]] = {}
    for package in packages:
        base = safe_directory_name(package, False)
        by_base.setdefault(base, []).append(package)
    result: dict[str, str] = {}
    for base, entries in by_base.items():
        for package in entries:
            result[package["id"]] = safe_directory_name(package, len(entries) > 1)
    return result


def normalized_package_root(package: dict) -> Path:
    manifest_path = Path(package["manifest_path"])
    root = manifest_path.parent.resolve()
    if not root.is_dir():
        raise InventoryError(
            f"package source directory is unavailable for {package['name']} {package['version']}"
        )
    return root


def is_license_name(name: str) -> bool:
    upper = name.upper()
    return upper.startswith(("LICENSE", "COPYING", "NOTICE"))


def add_candidate(
    candidates: dict[str, tuple[Path, set[str]]],
    root: Path,
    path: Path,
    origin: str,
    issues: list[str],
) -> None:
    try:
        if path.is_symlink():
            issues.append(f"ignored symlink: {path.name}")
            return
        resolved = path.resolve(strict=True)
        relative = resolved.relative_to(root).as_posix()
        mode = path.lstat().st_mode
        if not stat.S_ISREG(mode):
            issues.append(f"not a regular file: {relative}")
            return
    except (OSError, ValueError):
        issues.append(f"missing or outside package root: {path}")
        return
    if not is_license_name(Path(relative).name):
        issues.append(f"declared license file has a nonstandard name: {relative}")
    previous = candidates.get(relative)
    if previous is None:
        candidates[relative] = (resolved, {origin})
    else:
        previous[1].add(origin)


def find_package_license_files(package: dict) -> tuple[Path, list[dict], list[str]]:
    root = normalized_package_root(package)
    candidates: dict[str, tuple[Path, set[str]]] = {}
    issues: list[str] = []

    declared_file = package.get("license_file")
    if declared_file:
        declared_path = Path(declared_file)
        if not declared_path.is_absolute():
            declared_path = root / declared_path
        add_candidate(candidates, root, declared_path, "cargo-license_file", issues)

    try:
        entries = sorted(root.iterdir(), key=lambda path: path.name.casefold())
    except OSError as error:
        issues.append(f"cannot read package root: {error}")
        entries = []

    for path in entries:
        if not is_license_name(path.name):
            continue
        try:
            mode = path.lstat().st_mode
        except OSError as error:
            issues.append(f"cannot inspect {path.name}: {error}")
            continue
        if stat.S_ISREG(mode):
            add_candidate(candidates, root, path, "package-root-name", issues)
        elif stat.S_ISLNK(mode):
            issues.append(f"ignored symlink: {path.name}")
        else:
            issues.append(f"ignored non-file license candidate: {path.name}")

    if len(candidates) > MAX_FILES_PER_PACKAGE:
        issues.append(
            f"found {len(candidates)} license candidates; limit is {MAX_FILES_PER_PACKAGE}"
        )
        candidates = dict(list(sorted(candidates.items()))[:MAX_FILES_PER_PACKAGE])

    if not candidates:
        declaration = package.get("license") or "no Cargo SPDX declaration"
        issues.append(f"no package-root license text found (Cargo declares {declaration})")

    files = [
        {"source": source, "relative": relative, "origins": sorted(origins)}
        for relative, (source, origins) in sorted(candidates.items())
    ]
    return root, files, issues


def fetch_gpui_kit_license(package: dict, cache: Path) -> dict | None:
    """Recover GPUI Kit's tagged Apache text, omitted from its crates.io archive."""
    if package.get("name") != "gpui-kit":
        return None
    version = package.get("version", "")
    url = f"https://raw.githubusercontent.com/longbridge/gpui-kit/v{version}/LICENSE-APACHE"
    request = Request(url, headers={"User-Agent": "MegaMail-license-inventory/1"})
    try:
        with urlopen(request, timeout=20) as response:
            if response.geturl().split("/", 3)[2] != "raw.githubusercontent.com":
                raise InventoryError("GPUI Kit license URL redirected away from raw.githubusercontent.com")
            data = response.read(MAX_UPSTREAM_LICENSE_BYTES + 1)
    except InventoryError:
        raise
    except Exception as error:
        raise InventoryError(
            f"could not fetch GPUI Kit {version} LICENSE-APACHE from {url}: {error}"
        ) from error
    if len(data) > MAX_UPSTREAM_LICENSE_BYTES:
        raise InventoryError(f"GPUI Kit license exceeds size limit: {url}")
    if b"Apache License" not in data or b"Version 2.0" not in data:
        raise InventoryError(f"GPUI Kit license response was not Apache-2.0 text: {url}")
    source = cache / f"gpui-kit-{version}" / "LICENSE-APACHE"
    source.parent.mkdir(parents=True, exist_ok=True)
    source.write_bytes(data)
    source.chmod(0o444)
    return {
        "source": source,
        "relative": "LICENSE-APACHE",
        "origins": [f"upstream-tag-file:{url}"],
    }


def copy_bounded_file(
    source: Path,
    destination: Path,
    *,
    bundle_root: Path,
    total_bytes: int,
    issues: list[str],
    description: str,
) -> tuple[dict | None, int]:
    is_regular, size = regular_file(source)
    if not is_regular:
        issues.append(f"not a regular file: {description}")
        return None, total_bytes
    if size > MAX_FILE_BYTES:
        issues.append(f"file exceeds {MAX_FILE_BYTES} byte limit: {description} ({size} bytes)")
        return None, total_bytes
    if total_bytes + size > MAX_BUNDLE_BYTES:
        issues.append(
            f"bundle would exceed {MAX_BUNDLE_BYTES} byte limit; omitted: {description}"
        )
        return None, total_bytes
    try:
        data = source.read_bytes()
    except OSError as error:
        issues.append(f"cannot read {description}: {error}")
        return None, total_bytes
    if len(data) != size:
        issues.append(f"file changed while reading; omitted: {description}")
        return None, total_bytes
    destination.parent.mkdir(parents=True, exist_ok=True)
    destination.write_bytes(data)
    destination.chmod(0o444)
    record = {
        "path": destination.relative_to(bundle_root).as_posix(),
        "source_name": source.name,
        "size_bytes": size,
        "sha256": sha256(data),
    }
    return record, total_bytes + size


def md(value: object) -> str:
    text = "" if value is None else str(value)
    return text.replace("|", "\\|").replace("`", "\\`").replace("\n", " ")


def output_markdown(inventory: dict) -> str:
    lines = [
        "# Locked dependency and license inventory",
        "",
        "Generated from the locked `apps/megamail` Cargo graph by `tools/license-inventory.py`.",
        "This inventory records declared SPDX identifiers and copies actual package-root license,",
        "copyright, and notice files when present. A declaration is not a substitute for a",
        "missing license text; missing or skipped files are listed explicitly below. GPUI Kit's",
        "crates.io archive omits its Apache file, so its exact tagged upstream copy is included.",
        "",
        f"Lockfile SHA-256: `{inventory['lockfile_sha256']}`  ",
        f"Resolved remote package graph SHA-256: `{inventory['dependency_package_ids_sha256']}`  ",
        f"Remote packages: {len(inventory['packages'])}",
        "",
        "## Project and bundled asset notices",
        "",
        "| Bundle file | Source | SHA-256 |",
        "|---|---|---|",
    ]
    for notice in inventory["project_notices"]:
        link = quote(notice["path"], safe="/._+-")
        lines.append(
            f"| [`{md(notice['path'])}`]({link}) | `{md(notice['source'])}` | `{notice['sha256']}` |"
        )
    lines.extend(
        [
            "",
            "`project/THIRD_PARTY_NOTICES.md` is included verbatim. It contains the full Zeron MIT notice;",
            "the Geist OFL and Lucide ISC/Feather MIT texts are included as separate files above.",
            "GPUI Kit's tagged Apache-2.0 text appears with its dependency entry below.",
            "",
            "## Locked remote dependency packages",
            "",
        "| Package | Version | Cargo SPDX declaration | Bundled license/notice files |",
            "|---|---:|---|---|",
        ]
    )
    for package in inventory["packages"]:
        links = []
        for file in package["license_files"]:
            file_path = quote(file["path"], safe="/._+-")
            file_link = f"[`{md(file['source_name'])}`]({file_path})"
            upstream = next(
                (origin.removeprefix("upstream-tag-file:") for origin in file.get("origins", [])
                 if origin.startswith("upstream-tag-file:")),
                None,
            )
            if upstream:
                file_link += f" (from [the matching upstream tag]({upstream}))"
            links.append(file_link)
        file_summary = ", ".join(links) if links else "**No regular license file found**"
        lines.append(
            f"| `{md(package['name'])}` | `{md(package['version'])}` | "
            f"`{md(package['license']) or '—'}` | {file_summary} |"
        )
    missing = inventory["packages_with_issues"]
    lines.extend(["", "## Missing or skipped package license files", ""])
    if missing:
        lines.append(
            "The following package roots had license candidates that were missing, unsafe, "
            "outside the package root, or over a size/count limit. Cargo's declarations are "
            "reported above; review these entries before distribution."
        )
        lines.append("")
        for package in missing:
            lines.append(
                f"- `{md(package['name'])}@{md(package['version'])}` "
                f"(`{md(package['license']) or 'no SPDX declaration'}`): "
                + "; ".join(md(issue) for issue in package["issues"])
            )
    else:
        lines.append("None found.")
    lines.extend(
        [
            "",
            "Dependency source roots are read-only Cargo registry or Git checkouts. The generator copies",
            "bounded regular files named `LICENSE*`, `COPYING*`, or `NOTICE*`, plus any Cargo-declared",
            "`license_file` that resolves inside the package root. GPUI Kit's missing Apache file comes",
            "from the matching official upstream tag; its URL and hash are recorded in `inventory.json`.",
            "",
        ]
    )
    return "\n".join(lines)


def write_readme(inventory: dict) -> str:
    missing_count = len(inventory["packages_with_issues"])
    lines = [
        "# MegaMail license bundle",
        "",
        "These read-only notices accompany the Linux MegaMail binary. The inventory was generated",
        "from the locked dependency graph for `apps/megamail`.",
        "",
        "- [`inventory.md`](inventory.md) lists every resolved registry/Git package, version, Cargo",
        "  SPDX license declaration, and copied package or upstream license or notice file.",
        "- [`inventory.json`](inventory.json) records the same data, hashes, lockfile identity, and",
        "  any missing or skipped package files.",
        "- [`project/`](project/) contains MegaMail's AGPL license, the complete third-party notice",
        "  file (including Zeron's MIT text), Geist's OFL text, and the bundled Lucide/Feather notices.",
        "- [`dependencies/`](dependencies/) contains package license and notice files found in the",
        "  locked Cargo sources; GPUI Kit's omitted Apache text is copied from its matching official tag.",
        "",
        f"Packages with missing or skipped license-file issues: **{missing_count}**.",
        "Read the issue list in `inventory.md` before redistributing. Cargo SPDX declarations are",
        "included even when a crate source did not provide a matching text file.",
        "",
        "Regenerate after dependency or lockfile changes with `python3 tools/license-inventory.py`.",
        "The generator uses Python's standard library and `cargo metadata --locked`; it does not",
        "build the application. The published GPUI Kit crate omits its Apache text, so generation",
        "also reads the matching `v<version>/LICENSE-APACHE` from the official Longbridge GitHub tag.",
        "",
    ]
    return "\n".join(lines)


def ensure_project_notice(relative_source: str) -> Path:
    path = ROOT / relative_source
    try:
        path.resolve().relative_to(ROOT)
    except ValueError as error:
        raise InventoryError(f"project notice is outside the repository: {relative_source}")
    is_regular, size = regular_file(path)
    if not is_regular:
        raise InventoryError(f"required project notice is missing or not a regular file: {path}")
    if size > MAX_FILE_BYTES:
        raise InventoryError(f"required project notice exceeds size limit: {path}")
    return path


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--manifest",
        type=Path,
        default=DEFAULT_MANIFEST,
        help="locked native app manifest (default: apps/megamail/Cargo.toml)",
    )
    args = parser.parse_args()
    manifest = args.manifest if args.manifest.is_absolute() else ROOT / args.manifest
    manifest = manifest.resolve()
    try:
        manifest_relative = manifest.relative_to(ROOT).as_posix()
    except ValueError as error:
        raise InventoryError("manifest must be inside the MegaMail checkout") from error
    lockfile = manifest.parent / "Cargo.lock"
    is_regular, _ = regular_file(lockfile)
    if not is_regular:
        raise InventoryError(f"locked app lockfile is missing: {lockfile}")

    metadata = cargo_metadata(manifest)
    packages = resolved_remote_packages(metadata)
    if not any(package["name"] == "gpui-kit" for package in packages):
        raise InventoryError("locked dependency graph does not contain GPUI Kit")

    output_parent = OUTPUT.parent
    output_parent.mkdir(parents=True, exist_ok=True)
    if OUTPUT.is_symlink():
        raise InventoryError(f"refusing to replace a symlink at {OUTPUT}")
    if OUTPUT.exists() and not OUTPUT.is_dir():
        raise InventoryError(f"refusing to replace a non-directory at {OUTPUT}")
    stage = Path(tempfile.mkdtemp(prefix=".licenses-stage-", dir=output_parent))
    total_bytes = 0
    source_cache_context = None
    try:
        source_cache_context = tempfile.TemporaryDirectory(prefix=".megamail-license-source-")
        source_cache = Path(source_cache_context.name)
        project_records = []
        required_project_issues = []
        for source_relative, bundle_relative in PROJECT_NOTICES:
            source = ensure_project_notice(source_relative)
            record, total_bytes = copy_bounded_file(
                source,
                stage / bundle_relative,
                bundle_root=stage,
                total_bytes=total_bytes,
                issues=required_project_issues,
                description=source_relative,
            )
            if record is None:
                raise InventoryError(
                    f"required project legal notice could not be copied: {source_relative}"
                )
            record["source"] = source_relative
            project_records.append(record)
        if required_project_issues:
            raise InventoryError("; ".join(required_project_issues))

        directory_names = package_directories(packages)
        package_records = []
        for package in packages:
            root, candidates, issues = find_package_license_files(package)
            if package["name"] == "gpui-kit" and not any(
                Path(candidate["relative"]).name.upper().startswith("LICENSE")
                for candidate in candidates
            ):
                upstream_candidate = fetch_gpui_kit_license(package, source_cache)
                if upstream_candidate is None:
                    raise InventoryError("GPUI Kit Apache-2.0 text is required in the license bundle")
                candidates.append(upstream_candidate)
                issues = [
                    issue
                    for issue in issues
                    if not issue.startswith("no package-root license text found")
                ]
            copied_files = []
            for candidate in candidates:
                relative = Path(candidate["relative"])
                target = (
                    stage
                    / "dependencies"
                    / directory_names[package["id"]]
                    / Path(*relative.parts)
                )
                record, total_bytes = copy_bounded_file(
                    candidate["source"],
                    target,
                    bundle_root=stage,
                    total_bytes=total_bytes,
                    issues=issues,
                    description=(
                        f"{package['name']} {package['version']}:{candidate['relative']}"
                    ),
                )
                if record is not None:
                    record["origins"] = candidate["origins"]
                    copied_files.append(record)
            package_records.append(
                {
                    "id": package["id"],
                    "name": package["name"],
                    "version": package["version"],
                    "source": package["source"],
                    "license": package.get("license"),
                    "license_file": package.get("license_file"),
                    "license_files": copied_files,
                    "issues": sorted(set(issues)),
                }
            )

        lock_hash = sha256(lockfile.read_bytes())
        inventory = {
            "schema_version": 1,
            "generated_by": "tools/license-inventory.py",
            "manifest": manifest_relative,
            "lockfile": lockfile.relative_to(ROOT).as_posix(),
            "lockfile_sha256": lock_hash,
            "dependency_package_ids_sha256": package_graph_hash(packages),
            "remote_package_count": len(package_records),
            "project_notices": project_records,
            "packages": package_records,
            "packages_with_issues": [
                {
                    "name": package["name"],
                    "version": package["version"],
                    "license": package["license"],
                    "issues": package["issues"],
                }
                for package in package_records
                if package["issues"]
            ],
            "copied_file_count": len(project_records)
            + sum(len(package["license_files"]) for package in package_records),
            "copied_bytes": total_bytes,
        }
        (stage / "inventory.json").write_text(
            json.dumps(inventory, indent=2, ensure_ascii=False, sort_keys=True) + "\n",
            encoding="utf-8",
        )
        (stage / "inventory.md").write_text(output_markdown(inventory), encoding="utf-8")
        (stage / "README.md").write_text(write_readme(inventory), encoding="utf-8")

        for path in sorted(stage.rglob("*"), key=lambda item: len(item.parts), reverse=True):
            if path.is_dir():
                path.chmod(0o755)
            elif path.is_file():
                path.chmod(0o444)
        stage.chmod(0o755)

        backup = None
        if OUTPUT.exists():
            backup = Path(tempfile.mkdtemp(prefix=".licenses-backup-", dir=output_parent))
            backup.rmdir()
            OUTPUT.replace(backup)
        try:
            stage.replace(OUTPUT)
        except Exception:
            if backup is not None and backup.exists():
                backup.replace(OUTPUT)
            raise
        if backup is not None:
            shutil.rmtree(backup)
    except Exception:
        shutil.rmtree(stage, ignore_errors=True)
        raise
    finally:
        if source_cache_context is not None:
            source_cache_context.cleanup()

    missing_count = len(inventory["packages_with_issues"])
    print(
        f"Wrote {OUTPUT} with {len(package_records)} remote packages, "
        f"{inventory['copied_file_count']} legal files, and {missing_count} packages "
        f"with missing/skipped-file issues ({total_bytes} bytes)."
    )
    if missing_count:
        print("Review docs/licenses/inventory.md before binary redistribution.", file=sys.stderr)
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except InventoryError as error:
        print(f"license inventory: {error}", file=sys.stderr)
        raise SystemExit(1)
