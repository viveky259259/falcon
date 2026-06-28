#!/usr/bin/env python3
"""Build the static Falcon pub.dev leaderboard from official pub.dev APIs."""

from __future__ import annotations

import argparse
import concurrent.futures
import gzip
import hashlib
import json
import os
import shutil
import subprocess
import sys
import tarfile
import urllib.parse
import urllib.request
from datetime import datetime, timezone
from pathlib import Path
from typing import Any


USER_AGENT = "falcon-leaderboard/0.1 (+https://github.com/falcon-lint/falcon)"
PUB_ACCEPT = "application/vnd.pub.v2+json"


def main() -> int:
    args = parse_args()
    workdir = args.workdir.resolve()
    workdir.mkdir(parents=True, exist_ok=True)
    args.manifest.parent.mkdir(parents=True, exist_ok=True)

    names = fetch_candidate_names(
        args.pub_api_base, args.candidate_limit, args.opt_out_file
    )
    print(f"Fetched {len(names)} pub.dev candidate package names")

    ranked = fetch_ranked_scores(args.pub_api_base, names, args.jobs)
    selected = ranked[: args.limit]
    print(f"Selected {len(selected)} packages by pub points")

    packages = []
    for index, package in enumerate(selected, start=1):
        print(f"[{index}/{len(selected)}] scoring {package['name']}")
        result = score_package(args, workdir, package)
        if result is None:
            continue
        packages.append(result)

    manifest = {
        "generated_at": datetime.now(timezone.utc)
        .replace(microsecond=0)
        .isoformat()
        .replace("+00:00", "Z"),
        "packages": packages,
    }
    args.manifest.write_text(json.dumps(manifest, indent=2) + "\n", encoding="utf-8")
    print(f"Wrote manifest to {args.manifest}")

    subprocess.run(
        [
            str(args.falcon_bin),
            "x",
            "leaderboard",
            "--input",
            str(args.manifest),
            "--output",
            str(args.site_output),
        ],
        check=True,
    )
    return 0


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--limit", type=int, default=500)
    parser.add_argument("--candidate-limit", type=int, default=5000)
    parser.add_argument("--jobs", type=int, default=16)
    parser.add_argument("--pub-api-base", default="https://pub.dev")
    parser.add_argument("--falcon-bin", type=Path, default=Path("target/release/falcon"))
    parser.add_argument("--workdir", type=Path, default=Path("target/leaderboard"))
    parser.add_argument(
        "--opt-out-file",
        type=Path,
        default=Path("docs/leaderboard-opt-outs.txt"),
    )
    parser.add_argument(
        "--manifest",
        type=Path,
        default=Path("target/leaderboard/leaderboard-input.json"),
    )
    parser.add_argument("--site-output", type=Path, default=Path("site/leaderboard"))
    return parser.parse_args()


def fetch_candidate_names(api_base: str, limit: int, opt_out_file: Path) -> list[str]:
    url = f"{api_base.rstrip('/')}/api/package-name-completion-data"
    data = fetch_json(url)
    names = data.get("packages")
    if not isinstance(names, list):
        raise RuntimeError("pub.dev completion response did not include packages")
    opt_outs = load_opt_outs(opt_out_file)
    return [str(name) for name in names if str(name) not in opt_outs][:limit]


def load_opt_outs(path: Path) -> set[str]:
    if not path.exists():
        return set()
    names = set()
    for line in path.read_text(encoding="utf-8").splitlines():
        stripped = line.strip()
        if stripped and not stripped.startswith("#"):
            names.add(stripped)
    return names


def fetch_ranked_scores(
    api_base: str, names: list[str], jobs: int
) -> list[dict[str, Any]]:
    results: list[dict[str, Any]] = []
    with concurrent.futures.ThreadPoolExecutor(max_workers=max(1, jobs)) as executor:
        futures = {
            executor.submit(fetch_score_metadata, api_base, name): name for name in names
        }
        for index, future in enumerate(concurrent.futures.as_completed(futures), start=1):
            name = futures[future]
            try:
                score = future.result()
            except Exception as error:  # noqa: BLE001 - keep CI progressing on one bad package.
                print(f"warning: failed to fetch score for {name}: {error}", file=sys.stderr)
                continue
            results.append(score)
            if index % 100 == 0:
                print(f"Fetched score metadata for {index}/{len(names)} candidates")

    results.sort(
        key=lambda item: (
            -int(item.get("pub_points") or 0),
            -int(item.get("likes") or 0),
            str(item["name"]),
        )
    )
    return results


def fetch_score_metadata(api_base: str, name: str) -> dict[str, Any]:
    encoded = urllib.parse.quote(name, safe="")
    url = f"{api_base.rstrip('/')}/api/packages/{encoded}/score"
    data = fetch_json(url)
    return {
        "name": name,
        "pub_points": int(data.get("grantedPoints") or 0),
        "likes": data.get("likeCount"),
        "download_count_30_days": data.get("downloadCount30Days"),
    }


def score_package(
    args: argparse.Namespace, workdir: Path, package: dict[str, Any]
) -> dict[str, Any] | None:
    name = str(package["name"])
    try:
        metadata = fetch_package_metadata(args.pub_api_base, name)
        archive = download_archive(workdir, name, metadata)
        package_dir = extract_archive(workdir, name, archive)
        run_pub_get(package_dir, workdir)
        score = run_falcon_score(args.falcon_bin, package_dir)
    except Exception as error:  # noqa: BLE001 - one package should not kill the snapshot.
        print(f"warning: skipping {name}: {error}", file=sys.stderr)
        return None

    latest = metadata.get("latest") or {}
    pubspec = latest.get("pubspec") or {}
    return {
        "name": name,
        "version": latest.get("version"),
        "description": pubspec.get("description"),
        "pub_points": package["pub_points"],
        "likes": package.get("likes"),
        "popularity": None,
        "pub_url": f"https://pub.dev/packages/{name}",
        "score": score,
    }


def fetch_package_metadata(api_base: str, name: str) -> dict[str, Any]:
    encoded = urllib.parse.quote(name, safe="")
    url = f"{api_base.rstrip('/')}/api/packages/{encoded}"
    return fetch_json(url)


def download_archive(workdir: Path, name: str, metadata: dict[str, Any]) -> Path:
    latest = metadata.get("latest") or {}
    archive_url = latest.get("archive_url")
    if not archive_url:
        raise RuntimeError("latest package metadata has no archive_url")

    archive_dir = workdir / "archives"
    archive_dir.mkdir(parents=True, exist_ok=True)
    archive = archive_dir / f"{name}.tar.gz"
    request = urllib.request.Request(archive_url, headers={"User-Agent": USER_AGENT})
    with urllib.request.urlopen(request, timeout=120) as response:
        archive.write_bytes(response.read())

    expected_sha = latest.get("archive_sha256")
    if expected_sha:
        actual_sha = hashlib.sha256(archive.read_bytes()).hexdigest()
        if actual_sha != expected_sha:
            raise RuntimeError("archive sha256 did not match pub.dev metadata")
    return archive


def extract_archive(workdir: Path, name: str, archive: Path) -> Path:
    extract_root = workdir / "packages" / name
    if extract_root.exists():
        shutil.rmtree(extract_root)
    extract_root.mkdir(parents=True)

    with tarfile.open(archive, "r:gz") as tar:
        safe_extract(tar, extract_root)

    if (extract_root / "pubspec.yaml").exists():
        return extract_root

    pubspecs = sorted(extract_root.rglob("pubspec.yaml"), key=lambda path: len(path.parts))
    if not pubspecs:
        raise RuntimeError("archive did not contain pubspec.yaml")
    return pubspecs[0].parent


def safe_extract(tar: tarfile.TarFile, destination: Path) -> None:
    resolved_destination = destination.resolve()
    for member in tar.getmembers():
        target = (destination / member.name).resolve()
        if not is_relative_to(target, resolved_destination):
            raise RuntimeError(f"unsafe archive member path: {member.name}")
        if member.issym() or member.islnk():
            link_target = (target.parent / member.linkname).resolve()
            if not is_relative_to(link_target, resolved_destination):
                raise RuntimeError(f"unsafe archive link target: {member.name}")
    tar.extractall(destination)


def is_relative_to(path: Path, parent: Path) -> bool:
    try:
        path.relative_to(parent)
    except ValueError:
        return False
    return True


def run_pub_get(package_dir: Path, workdir: Path) -> None:
    pubspec = package_dir / "pubspec.yaml"
    pubspec_text = pubspec.read_text(encoding="utf-8", errors="replace")
    flutter = shutil.which("flutter")
    dart = shutil.which("dart")
    if "sdk: flutter" in pubspec_text and flutter:
        command = [flutter, "pub", "get"]
    elif dart:
        command = [dart, "pub", "get"]
    else:
        print("warning: neither dart nor flutter was found on PATH", file=sys.stderr)
        return

    env = os.environ.copy()
    env.setdefault("PUB_CACHE", str(workdir / "pub-cache"))
    try:
        subprocess.run(command, cwd=package_dir, env=env, check=True, timeout=300)
    except subprocess.SubprocessError as error:
        print(f"warning: pub get failed for {package_dir}: {error}", file=sys.stderr)


def run_falcon_score(falcon_bin: Path, package_dir: Path) -> dict[str, Any]:
    completed = subprocess.run(
        [str(falcon_bin), "score", str(package_dir), "--json"],
        check=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )
    return json.loads(completed.stdout)


def fetch_json(url: str) -> dict[str, Any]:
    request = urllib.request.Request(
        url,
        headers={
            "Accept": PUB_ACCEPT,
            "User-Agent": USER_AGENT,
        },
    )
    with urllib.request.urlopen(request, timeout=60) as response:
        body = response.read()
        if response.headers.get("Content-Encoding") == "gzip":
            body = gzip.decompress(body)
    return json.loads(body.decode("utf-8"))


if __name__ == "__main__":
    raise SystemExit(main())
