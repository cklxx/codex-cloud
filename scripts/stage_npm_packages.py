#!/usr/bin/env python3
"""Stage one or more Codex npm packages for release."""

from __future__ import annotations

import argparse
import importlib.util
import json
import os
import shutil
import subprocess
import tempfile
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parent.parent
BUILD_SCRIPT = REPO_ROOT / "codex-cli" / "scripts" / "build_npm_package.py"
INSTALL_NATIVE_DEPS = REPO_ROOT / "codex-cli" / "scripts" / "install_native_deps.py"
WORKFLOW_NAME = ".github/workflows/rust-release.yml"
GITHUB_REPO = "openai/codex"

_SPEC = importlib.util.spec_from_file_location("codex_build_npm_package", BUILD_SCRIPT)
if _SPEC is None or _SPEC.loader is None:
    raise RuntimeError(f"Unable to load module from {BUILD_SCRIPT}")
_BUILD_MODULE = importlib.util.module_from_spec(_SPEC)
_SPEC.loader.exec_module(_BUILD_MODULE)
PACKAGE_NATIVE_COMPONENTS = getattr(_BUILD_MODULE, "PACKAGE_NATIVE_COMPONENTS", {})

_INSTALL_SPEC = importlib.util.spec_from_file_location(
    "codex_install_native_deps", INSTALL_NATIVE_DEPS
)
if _INSTALL_SPEC is None or _INSTALL_SPEC.loader is None:
    raise RuntimeError(f"Unable to load module from {INSTALL_NATIVE_DEPS}")
_INSTALL_MODULE = importlib.util.module_from_spec(_INSTALL_SPEC)
_INSTALL_SPEC.loader.exec_module(_INSTALL_MODULE)
DEFAULT_NATIVE_WORKFLOW_URL = getattr(
    _INSTALL_MODULE, "DEFAULT_WORKFLOW_URL", ""
).strip()


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--release-version",
        required=True,
        help="Version to stage (e.g. 0.1.0 or 0.1.0-alpha.1).",
    )
    parser.add_argument(
        "--package",
        dest="packages",
        action="append",
        required=True,
        help="Package name to stage. May be provided multiple times.",
    )
    parser.add_argument(
        "--workflow-url",
        help="Optional workflow URL to reuse for native artifacts.",
    )
    parser.add_argument(
        "--output-dir",
        type=Path,
        default=None,
        help="Directory where npm tarballs should be written (default: dist/npm).",
    )
    parser.add_argument(
        "--keep-staging-dirs",
        action="store_true",
        help="Retain temporary staging directories instead of deleting them.",
    )
    return parser.parse_args()


def collect_native_components(packages: list[str]) -> set[str]:
    components: set[str] = set()
    for package in packages:
        components.update(PACKAGE_NATIVE_COMPONENTS.get(package, []))
    return components


def _candidate_release_branches(version: str) -> list[str]:
    """Generate branch names we should probe for release workflows.

    Historically, releases used the ``rust-v<version>`` naming scheme, but
    some older automation referenced other prefixes (e.g. ``first`` or
    ``rust-nse``). We attempt a handful of common patterns while keeping the
    original version string intact so that prerelease identifiers continue to
    work.
    """

    candidates: list[str] = []
    bare_version = version

    # Allow callers to pass channel@version (e.g. "rust-nse@0.1.0") or
    # channel:version. In those cases we treat the portion after the separator
    # as the semantic version that should be appended to known branch prefixes.
    for separator in ("@", ":"):
        if separator in version:
            channel, bare_version = version.split(separator, 1)
            candidates.append(f"{channel}-v{bare_version}")
            candidates.append(f"{channel}/v{bare_version}")
            candidates.append(f"{channel}-{bare_version}")
            break

    # Default branch naming conventions.
    candidates.append(f"rust-v{bare_version}")
    candidates.append(f"first-v{bare_version}")
    candidates.append(f"rust-nse-v{bare_version}")

    # Finally, include the raw version in case the branch already includes the
    # "v" prefix or uses an alternate naming scheme.
    candidates.append(bare_version)

    # Remove duplicates while preserving order.
    deduped: list[str] = []
    seen: set[str] = set()
    for branch in candidates:
        if branch and branch not in seen:
            deduped.append(branch)
            seen.add(branch)
    return deduped


def resolve_release_workflow(version: str) -> dict | None:
    errors: list[str] = []
    for branch in _candidate_release_branches(version):
        try:
            stdout = subprocess.check_output(
                [
                    "gh",
                    "run",
                    "list",
                    "--branch",
                    branch,
                    "--json",
                    "workflowName,url,headSha",
                    "--workflow",
                    WORKFLOW_NAME,
                    "--jq",
                    "first(.[])",
                ],
                cwd=REPO_ROOT,
                text=True,
            )
        except subprocess.CalledProcessError as exc:
            errors.append(f"{branch}: {exc.stderr or exc}")
            continue

        workflow = json.loads(stdout or "null")
        if workflow:
            return workflow
        errors.append(f"{branch}: no workflow runs found")

    if errors:
        joined = "; ".join(errors)
        print(
            "Warning: unable to locate release workflow run for version "
            f"{version}. Attempted branches: {joined}"
        )
    return None


def resolve_workflow_url(version: str, override: str | None) -> tuple[str, str | None]:
    if override:
        return override, None

    workflow = resolve_release_workflow(version)
    if workflow:
        return workflow["url"], workflow.get("headSha")

    fallback = os.environ.get("CODEX_DEFAULT_WORKFLOW_URL", DEFAULT_NATIVE_WORKFLOW_URL)
    if not fallback:
        raise RuntimeError(
            "Unable to find a release workflow run and no fallback workflow "
            "URL is configured."
        )

    print(
        "Falling back to default workflow artifacts at "
        f"{fallback}."
    )
    return fallback, None


def install_native_components(
    workflow_url: str,
    components: set[str],
    vendor_root: Path,
) -> None:
    if not components:
        return

    cmd = [str(INSTALL_NATIVE_DEPS), "--workflow-url", workflow_url]
    for component in sorted(components):
        cmd.extend(["--component", component])
    cmd.append(str(vendor_root))
    run_command(cmd)


def run_command(cmd: list[str]) -> None:
    print("+", " ".join(cmd))
    subprocess.run(cmd, cwd=REPO_ROOT, check=True)


def main() -> int:
    args = parse_args()

    output_dir = args.output_dir or (REPO_ROOT / "dist" / "npm")
    output_dir.mkdir(parents=True, exist_ok=True)

    runner_temp = Path(os.environ.get("RUNNER_TEMP", tempfile.gettempdir()))

    packages = list(args.packages)
    native_components = collect_native_components(packages)

    vendor_temp_root: Path | None = None
    vendor_src: Path | None = None
    resolved_head_sha: str | None = None

    final_messsages = []

    try:
        if native_components:
            workflow_url, resolved_head_sha = resolve_workflow_url(
                args.release_version, args.workflow_url
            )
            vendor_temp_root = Path(tempfile.mkdtemp(prefix="npm-native-", dir=runner_temp))
            install_native_components(workflow_url, native_components, vendor_temp_root)
            vendor_src = vendor_temp_root / "vendor"

        if resolved_head_sha:
            print(f"should `git checkout {resolved_head_sha}`")

        for package in packages:
            staging_dir = Path(tempfile.mkdtemp(prefix=f"npm-stage-{package}-", dir=runner_temp))
            pack_output = output_dir / f"{package}-npm-{args.release_version}.tgz"

            cmd = [
                str(BUILD_SCRIPT),
                "--package",
                package,
                "--release-version",
                args.release_version,
                "--staging-dir",
                str(staging_dir),
                "--pack-output",
                str(pack_output),
            ]

            if vendor_src is not None:
                cmd.extend(["--vendor-src", str(vendor_src)])

            try:
                run_command(cmd)
            finally:
                if not args.keep_staging_dirs:
                    shutil.rmtree(staging_dir, ignore_errors=True)

            final_messsages.append(f"Staged {package} at {pack_output}")
    finally:
        if vendor_temp_root is not None and not args.keep_staging_dirs:
            shutil.rmtree(vendor_temp_root, ignore_errors=True)

    for msg in final_messsages:
        print(msg)

    return 0


if __name__ == "__main__":
    raise SystemExit(main())
