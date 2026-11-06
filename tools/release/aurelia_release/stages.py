"""Concrete operations for the git and full profiles.

The same engine runs both profiles; profiles only change which stages are
enabled. No operation is considered successful until its `verify` callback has
independently re-read the world it mutated.
"""

from __future__ import annotations

import hashlib
import os
import re
from pathlib import Path
from typing import Any

from .config import ReleaseConfig
from .engine import Context, Operation
from .errors import CompensationError, PreflightError, VerificationError
from .gitops import Git
from .github import client_for_context
from .journal import OpState
from .scan import ScanError, VirusTotalClient, evaluate
from .signing import (
    load_keypair,
    load_public_key,
    sign_bytes,
    verify_bytes,
    verify_checksums,
    write_checksums,
)
from .util import expand_globs, find_iscc, run_command, sha256_file
from .versioning import (
    _cargo_lock_versions,
    bump_files,
    check_files,
    declared_versions,
)

VERSION_TAG_RE = re.compile(r"^v\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?$")


def _safe_format(template: str, values: dict[str, str]) -> str:
    class _Values(dict[str, str]):
        def __missing__(self, key: str) -> str:
            return "{" + key + "}"

    return template.format_map(_Values(values))


def _file_hashes(repo: Path, paths: list[str]) -> dict[str, str]:
    return {rel: sha256_file(repo / rel) for rel in paths}


def _version_files(ctx: Context) -> list[str]:
    """Configured declarations plus the workspace lockfile.

    Cargo rewrites `Cargo.lock` when the manifests move to the release version, but that
    happens during quality/build, *after* the version commit. Including the lockfile in the
    version stage means the release commit owns the final lock and the tree is clean when the
    release finishes, without a manual follow-up commit.
    """
    files = list(ctx.cfg.version_files)
    lock = "Cargo.lock"
    if lock not in files and (ctx.repo / lock).is_file():
        files.append(lock)
    return files


def _workspace_package_names(ctx: Context) -> set[str]:
    """Cargo package names that belong to this workspace.

    Cargo.lock also contains every dependency's version; the release bump must only touch
    the project's own package blocks.
    """
    names: set[str] = set()
    for rel in ctx.cfg.version_files:
        if not rel.endswith("Cargo.toml"):
            continue
        path = ctx.repo / rel
        if not path.is_file():
            continue
        with open(path, "r", encoding="utf-8") as handle:
            match = re.search(r'(?m)^name\s*=\s*"([^"]+)"', handle.read())
        if match:
            names.add(match.group(1))
    return names


def _current_versions_problem(ctx: Context) -> list[str]:
    states = ctx.journal.state()
    bump = states.get("version.bump")
    if bump and bump.status == "ok":
        return check_files(
            ctx.repo,
            _version_files(ctx),
            ctx.version,
            _workspace_package_names(ctx),
        )

    problems: list[str] = []
    per_file: dict[str, set[str]] = {}
    packages = _workspace_package_names(ctx)
    for rel in _version_files(ctx):
        path = ctx.repo / rel
        if not path.is_file():
            problems.append(f"{rel}: missing")
            continue
        with open(path, "r", encoding="utf-8", newline="") as handle:
            text = handle.read()
        found = (
            _cargo_lock_versions(text, packages)
            if rel == "Cargo.lock"
            else declared_versions(text)
        )
        if not found:
            # The workspace root manifest has no package version; a file with no
            # declaration is simply not part of the consistency check.
            continue
        per_file[rel] = found

    union: set[str] = set()
    for found in per_file.values():
        union.update(found)
    if len(union) > 1:
        detail = "; ".join(f"{rel}={','.join(sorted(v))}" for rel, v in sorted(per_file.items()))
        problems.append(f"version declarations disagree: {detail}")
    return problems


def _preflight(ctx: Context) -> dict[str, Any]:
    git = ctx.git
    if not git.is_worktree():
        raise PreflightError("not inside a git work tree")

    dirty = git.status_porcelain()
    if dirty:
        raise PreflightError(f"working tree is not clean:\n{dirty}")

    branch = git.current_branch()
    if branch != ctx.cfg.git.branch:
        raise PreflightError(
            f"on branch {branch!r}, expected {ctx.cfg.git.branch!r}"
        )

    remote = ctx.cfg.git.remote
    if not git.remote_url(remote):
        raise PreflightError(f"remote {remote!r} is not configured")

    head = git.head()
    refs = git.ls_remote(remote)
    remote_branch = refs.get(f"refs/heads/{branch}")
    if remote_branch is None:
        raise PreflightError(f"{remote}/{branch} does not exist")
    if remote_branch != head and not git.is_ancestor(remote_branch, head):
        raise PreflightError(
            f"local {branch} is not a descendant of {remote}/{branch}; "
            "pull or rebase before releasing"
        )

    states = ctx.journal.state()
    if git.tag_exists(ctx.tag):
        created = states.get("tag.create")
        if not (created and created.status == "ok"):
            raise PreflightError(f"tag {ctx.tag} already exists and was not created by this run")
        if git.tag_commit(ctx.tag) != head:
            raise PreflightError(f"tag {ctx.tag} exists but does not point at HEAD")

    remote_tag = refs.get(f"refs/tags/{ctx.tag}")
    if remote_tag:
        pushed = states.get("publish.push")
        if not (pushed and pushed.status == "ok"):
            raise PreflightError(f"tag {ctx.tag} already exists on {remote}")

    if ctx.cfg.git.sign_tags and not git.config_get("user.signingkey"):
        raise PreflightError(
            "git user.signingkey is not configured; set it or run with sign_tags = false"
        )

    if any("{iscc}" in command for command in ctx.cfg.commands.values()) and not find_iscc():
        raise PreflightError(
            "a configured command uses {iscc}, but Inno Setup's ISCC.exe was not found "
            "in ISCC/INNO_ISCC, PATH, Program Files, or %LOCALAPPDATA%\\Programs\\Inno"
        )

    problems = _current_versions_problem(ctx)
    if problems:
        raise PreflightError("version declarations are inconsistent:\n  " + "\n  ".join(problems))

    return {
        "head": head,
        "branch": branch,
        "remote": remote,
        "remote_branch": remote_branch,
    }


def _version_bump(ctx: Context) -> dict[str, Any]:
    changed = bump_files(
        ctx.repo,
        _version_files(ctx),
        ctx.version,
        _workspace_package_names(ctx),
    )
    return {"paths": changed}


def _version_bump_verify(ctx: Context, outputs: dict[str, Any]) -> dict[str, Any]:
    files = _version_files(ctx)
    problems = check_files(
        ctx.repo, files, ctx.version, _workspace_package_names(ctx)
    )
    if problems:
        raise VerificationError("; ".join(problems))
    return {"files": len(files)}


def _version_bump_snapshot(ctx: Context) -> dict[str, Any]:
    return {
        "files": {
            rel: sha256_file(ctx.repo / rel)
            for rel in _version_files(ctx)
            if (ctx.repo / rel).is_file()
        }
    }


def _version_bump_compensate(ctx: Context, state: OpState) -> None:
    # Local-only: restore the version files from HEAD. If the bump was already
    # committed, version.commit compensates first (reverse order) and this is a no-op.
    ctx.git.checkout(_version_files(ctx))


def _version_commit(ctx: Context) -> dict[str, Any]:
    message = f"chore(release): {ctx.tag}"
    if ctx.git.status_porcelain():
        ctx.git.add(_version_files(ctx))
        ctx.git.commit(message)
        return {"message": message}

    if ctx.git.commit_subject("HEAD") == message:
        return {"message": message, "noop": True}

    raise PreflightError(
        "version files are already at the target but HEAD is not the release commit; "
        "commit them manually or start from a clean checkout"
    )


def _version_commit_verify(ctx: Context, outputs: dict[str, Any]) -> dict[str, Any]:
    subject = ctx.git.commit_subject("HEAD")
    if subject != outputs.get("meta", {}).get("message", f"chore(release): {ctx.tag}"):
        raise VerificationError(f"HEAD is {subject!r}, not the release commit")
    problems = check_files(
        ctx.repo,
        _version_files(ctx),
        ctx.version,
        _workspace_package_names(ctx),
    )
    if problems:
        raise VerificationError("; ".join(problems))
    return {"head": ctx.git.head()}


def _version_commit_snapshot(ctx: Context) -> dict[str, Any]:
    return {"head": ctx.git.head()}


def _version_commit_compensate(ctx: Context, state: OpState) -> None:
    before = state.before.get("head")
    if not before:
        return
    head = ctx.git.head()
    if head == before:
        return
    if ctx.git.rev_parse(f"{head}^") != before:
        raise VerificationError(
            "refusing to reset: HEAD is no longer the single release commit on top of the base"
        )
    ctx.git.reset_hard(before)


def _tag(ctx: Context) -> dict[str, Any]:
    if ctx.git.tag_exists(ctx.tag):
        if ctx.git.tag_commit(ctx.tag) == ctx.git.head():
            return {"tag": ctx.tag, "noop": True}
        raise PreflightError(f"tag {ctx.tag} exists and does not point at HEAD")

    ctx.git.tag(ctx.tag, ctx.tag, sign=ctx.cfg.git.sign_tags)
    return {"tag": ctx.tag, "signed": ctx.cfg.git.sign_tags}


def _tag_verify(ctx: Context, outputs: dict[str, Any]) -> dict[str, Any]:
    if not ctx.git.tag_exists(ctx.tag):
        raise VerificationError(f"tag {ctx.tag} is missing")
    if ctx.git.tag_commit(ctx.tag) != ctx.git.head():
        raise VerificationError(f"tag {ctx.tag} does not point at HEAD")
    if ctx.cfg.git.sign_tags:
        ctx.git.verify_tag(ctx.tag)
    elif ctx.git.tag_type(ctx.tag) != "tag":
        raise VerificationError(f"tag {ctx.tag} is not an annotated tag")
    return {"tag": ctx.tag, "commit": ctx.git.tag_commit(ctx.tag)}


def _tag_snapshot(ctx: Context) -> dict[str, Any]:
    return {
        "tag": ctx.tag,
        "existed": ctx.git.tag_exists(ctx.tag),
    }


def _tag_compensate(ctx: Context, state: OpState) -> None:
    if state.before.get("existed"):
        return
    if ctx.git.tag_exists(ctx.tag):
        ctx.git.delete_tag(ctx.tag)


def _desired_refs(ctx: Context) -> dict[str, str]:
    return {
        f"refs/heads/{ctx.cfg.git.branch}": ctx.git.head(),
        f"refs/tags/{ctx.tag}": ctx.git.rev_parse(f"refs/tags/{ctx.tag}") or "",
    }


def _publish(ctx: Context) -> dict[str, Any]:
    remote = ctx.cfg.git.remote
    refs = ctx.git.ls_remote(remote)
    desired = _desired_refs(ctx)
    if all(refs.get(name) == sha for name, sha in desired.items()):
        return {"remote": remote, "noop": True, "refs": desired}

    ctx.git.push(
        remote,
        [f"HEAD:refs/heads/{ctx.cfg.git.branch}", f"refs/tags/{ctx.tag}"],
        atomic=True,
    )
    return {"remote": remote, "refs": desired}


def _publish_verify(ctx: Context, outputs: dict[str, Any]) -> dict[str, Any]:
    remote = ctx.cfg.git.remote
    refs = ctx.git.ls_remote(remote)
    desired = _desired_refs(ctx)
    for name, sha in desired.items():
        if refs.get(name) != sha:
            raise VerificationError(
                f"{remote}/{name} is {refs.get(name)!r}, expected {sha!r}"
            )
    if ctx.git.tag_commit(ctx.tag) != ctx.git.head():
        raise VerificationError(f"remote tag {ctx.tag} does not resolve to HEAD")
    if ctx.cfg.git.sign_tags:
        ctx.git.verify_tag(ctx.tag)
    return {"remote": remote, "refs": desired}


def _publish_snapshot(ctx: Context) -> dict[str, Any]:
    remote = ctx.cfg.git.remote
    refs = ctx.git.ls_remote(remote)
    return {
        "remote": remote,
        "branch_ref": f"refs/heads/{ctx.cfg.git.branch}",
        "tag_ref": f"refs/tags/{ctx.tag}",
        "remote_branch": refs.get(f"refs/heads/{ctx.cfg.git.branch}"),
        "remote_tag": refs.get(f"refs/tags/{ctx.tag}"),
    }


def _publish_compensate(ctx: Context, state: OpState) -> None:
    before = state.before
    remote = before.get("remote") or ctx.cfg.git.remote
    tag_ref = before.get("tag_ref") or f"refs/tags/{ctx.tag}"
    remote_tag = before.get("remote_tag")
    if remote_tag:
        return  # the tag predates this run; never delete someone else's ref
    refs = ctx.git.ls_remote(remote)
    local_tag = ctx.git.rev_parse(f"refs/tags/{ctx.tag}")
    if refs.get(tag_ref) and refs.get(tag_ref) == local_tag:
        ctx.git.delete_remote_tag(remote, ctx.tag)


def _verify_remote(ctx: Context) -> dict[str, Any]:
    return {}


def _verify_remote_check(ctx: Context, outputs: dict[str, Any]) -> dict[str, Any]:
    return _publish_verify(ctx, outputs)


def _command_operation(stage: str, cfg: ReleaseConfig) -> Operation:
    def run(ctx: Context) -> dict[str, Any]:
        template = cfg.command(stage)
        command = _safe_format(template, ctx.replacements)
        ctx.log(f"    $ {command}")
        run_command(command, cwd=ctx.repo)
        patterns = cfg.outputs.get(stage, ())
        paths = expand_globs(ctx.repo, patterns, ctx.replacements)
        return {
            "paths": [path.relative_to(ctx.repo).as_posix() for path in paths],
            "command": command,
        }

    def verify(ctx: Context, outputs: dict[str, Any]) -> dict[str, Any]:
        paths = outputs.get("files", {})
        if not paths and cfg.outputs.get(stage):
            raise VerificationError(f"{stage}: no output files matched {cfg.outputs[stage]}")
        return {"files": len(paths)}

    return Operation(
        name=stage,
        stage=stage,
        description=f"run configured {stage} command",
        run=run,
        verify=verify,
        params={"command": cfg.commands.get(stage, "")},
    )


def _github(ctx: Context):
    return client_for_context(ctx)


def _release_id(ctx: Context) -> int:
    state = ctx.journal.state().get("release.draft")
    if not state or state.status != "ok":
        raise PreflightError("release.draft has not completed; cannot use the release id")
    release_id = state.outputs.get("meta", {}).get("release_id")
    if not release_id:
        raise PreflightError("journal has no GitHub release id for release.draft")
    return int(release_id)


def _release_assets(ctx: Context) -> list[Path]:
    return expand_globs(ctx.repo, ctx.cfg.publish.assets, ctx.replacements)


def _signable_assets(ctx: Context) -> list[Path]:
    return [
        path
        for path in _release_assets(ctx)
        if not path.name.endswith(".sig") and path.name != "SHA256SUMS.txt"
    ]


def _signing_operation(cfg: ReleaseConfig) -> Operation:
    def run(ctx: Context) -> dict[str, Any]:
        keypair = load_keypair(ctx.repo, cfg.signing)
        assets = _signable_assets(ctx)
        if not assets:
            raise PreflightError(
                f"sign: no files matched {cfg.publish.assets!r}"
            )
        directory = assets[0].parent
        checksums = write_checksums(directory, assets)
        generated: list[Path] = [checksums]
        for path in assets:
            signature = path.with_name(path.name + ".sig")
            signature.write_bytes(sign_bytes(keypair.private, path.read_bytes()))
            generated.append(signature)
        checksums_sig = checksums.with_name(checksums.name + ".sig")
        checksums_sig.write_bytes(sign_bytes(keypair.private, checksums.read_bytes()))
        generated.append(checksums_sig)
        return {
            "paths": [
                path.relative_to(ctx.repo).as_posix() for path in assets + generated
            ],
            "signed": [path.name for path in assets],
        }

    def verify(ctx: Context, outputs: dict[str, Any]) -> dict[str, Any]:
        keypair = load_keypair(ctx.repo, cfg.signing)
        assets = _signable_assets(ctx)
        for path in assets:
            signature = path.with_name(path.name + ".sig")
            if not signature.is_file():
                raise VerificationError(f"missing signature for {path.name}")
            verify_bytes(keypair.public, path.read_bytes(), signature.read_bytes())
        checksums = assets[0].parent / "SHA256SUMS.txt"
        checksums_sig = checksums.with_name(checksums.name + ".sig")
        if not checksums.is_file() or not checksums_sig.is_file():
            raise VerificationError("SHA256SUMS.txt is not signed")
        verify_bytes(
            keypair.public, checksums.read_bytes(), checksums_sig.read_bytes()
        )
        verify_checksums(checksums, assets)
        return {"signed": len(assets)}

    def snapshot(ctx: Context) -> dict[str, Any]:
        assets = _signable_assets(ctx)
        files = list(assets)
        if assets:
            directory = assets[0].parent
            files.append(directory / "SHA256SUMS.txt")
            files.extend(path.with_name(path.name + ".sig") for path in assets)
            files.append(directory / "SHA256SUMS.txt.sig")
        return {
            "files": sorted(
                path.relative_to(ctx.repo).as_posix()
                for path in files
                if path.is_file()
            )
        }

    def compensate(ctx: Context, state: OpState) -> None:
        before = set(state.before.get("files", []))
        assets = _signable_assets(ctx)
        generated = []
        if assets:
            directory = assets[0].parent
            generated.append(directory / "SHA256SUMS.txt")
            generated.append(directory / "SHA256SUMS.txt.sig")
            generated.extend(path.with_name(path.name + ".sig") for path in assets)
        for path in generated:
            rel = path.relative_to(ctx.repo).as_posix()
            if rel not in before and path.is_file():
                path.unlink()

    return Operation(
        name="sign",
        stage="sign",
        description="Ed25519-sign release assets and SHA256SUMS.txt",
        run=run,
        verify=verify,
        compensate=compensate,
        snapshot=snapshot,
        params={
            "enabled": cfg.signing.enabled,
            "public_key": cfg.signing.public_key,
        },
    )


def _release_draft(ctx: Context) -> dict[str, Any]:
    client = _github(ctx)
    existing = client.get_release(ctx.tag)
    if existing:
        return {
            "release_id": existing["id"],
            "draft": bool(existing.get("draft")),
            "created": False,
        }
    release = client.create_release(
        ctx.tag, name=ctx.tag, body="", draft=True
    )
    return {
        "release_id": release["id"],
        "draft": bool(release.get("draft")),
        "created": True,
    }


def _release_draft_verify(ctx: Context, outputs: dict[str, Any]) -> dict[str, Any]:
    release = _github(ctx).get_release(ctx.tag)
    if not release:
        raise VerificationError(f"GitHub release for {ctx.tag} is missing")
    if release.get("tag_name") != ctx.tag:
        raise VerificationError(
            f"release points at {release.get('tag_name')!r}, expected {ctx.tag!r}"
        )
    meta = outputs.get("meta", {})
    published = ctx.journal.state().get("release.publish")
    verified_publish = bool(published and published.status == "ok" and published.verified)
    if (
        meta.get("created")
        and ctx.cfg.publish.draft
        and not release.get("draft")
        and not verified_publish
    ):
        raise VerificationError("a new release must start as a draft")
    return {"release_id": release["id"], "draft": bool(release.get("draft"))}


def _release_draft_snapshot(ctx: Context) -> dict[str, Any]:
    release = _github(ctx).get_release(ctx.tag)
    if not release:
        return {"release_id": None, "draft": None}
    return {"release_id": release["id"], "draft": bool(release.get("draft"))}


def _release_draft_compensate(ctx: Context, state: OpState) -> None:
    if state.before.get("release_id"):
        return
    client = _github(ctx)
    release = client.get_release(ctx.tag)
    if not release:
        return
    if not release.get("draft"):
        raise CompensationError("release is published; use yank instead of deleting it")
    client.delete_release(release["id"])


def _release_upload(ctx: Context) -> dict[str, Any]:
    client = _github(ctx)
    release_id = _release_id(ctx)
    release = client.get_release(ctx.tag)
    if release and not release.get("draft"):
        raise PreflightError(
            "release is already published; refusing to replace its assets"
        )
    assets = _release_assets(ctx)
    if not assets:
        raise PreflightError(
            f"release.upload: no files matched {ctx.cfg.publish.assets!r}"
        )

    existing = {asset["name"]: asset for asset in client.list_assets(release_id)}
    uploaded: list[str] = []
    for path in assets:
        current = existing.get(path.name)
        local_digest = sha256_file(path)
        if current and current.get("size") == path.stat().st_size:
            remote_digest = str(current.get("digest") or "")
            if remote_digest:
                if remote_digest.lower() == f"sha256:{local_digest}":
                    continue
            elif hashlib.sha256(client.download_asset(current["id"])).hexdigest() == local_digest:
                continue
        if current:
            client.delete_asset(current["id"])
        client.upload_asset(release_id, path)
        uploaded.append(path.name)

    return {
        "paths": [path.relative_to(ctx.repo).as_posix() for path in assets],
        "release_id": release_id,
        "uploaded": uploaded,
    }


def _release_upload_verify(ctx: Context, outputs: dict[str, Any]) -> dict[str, Any]:
    client = _github(ctx)
    release_id = _release_id(ctx)
    remote = {asset["name"]: asset for asset in client.list_assets(release_id)}
    for path in _release_assets(ctx):
        asset = remote.get(path.name)
        if asset is None:
            raise VerificationError(f"GitHub release is missing asset {path.name}")
        if asset.get("size") != path.stat().st_size:
            raise VerificationError(f"{path.name}: remote size differs from local")
        local_digest = sha256_file(path)
        remote_digest = str(asset.get("digest") or "")
        if remote_digest:
            if remote_digest.lower() != f"sha256:{local_digest}":
                raise VerificationError(f"{path.name}: remote digest differs from local")
        else:
            downloaded = client.download_asset(asset["id"])
            if hashlib.sha256(downloaded).hexdigest() != local_digest:
                raise VerificationError(f"{path.name}: downloaded bytes differ from local")
    return {"assets": len(remote)}


def _release_upload_snapshot(ctx: Context) -> dict[str, Any]:
    client = _github(ctx)
    release = client.get_release(ctx.tag)
    if not release:
        return {"assets": []}
    return {"assets": sorted(asset["name"] for asset in client.list_assets(release["id"]))}


def _release_upload_compensate(ctx: Context, state: OpState) -> None:
    client = _github(ctx)
    release = client.get_release(ctx.tag)
    if not release or not release.get("draft"):
        return
    before = set(state.before.get("assets", []))
    for asset in client.list_assets(release["id"]):
        if asset["name"] not in before:
            client.delete_asset(asset["id"])


def _release_publish(ctx: Context) -> dict[str, Any]:
    client = _github(ctx)
    release = client.get_release(ctx.tag)
    if not release:
        raise PreflightError(f"no GitHub release for {ctx.tag}")
    if release.get("draft"):
        release = client.update_release(release["id"], draft=False)
    return {"release_id": release["id"], "draft": bool(release.get("draft"))}


def _release_publish_verify(ctx: Context, outputs: dict[str, Any]) -> dict[str, Any]:
    release = _github(ctx).get_release(ctx.tag)
    if not release:
        raise VerificationError(f"GitHub release for {ctx.tag} is missing")
    if release.get("draft"):
        raise VerificationError("release is still a draft")
    return {"release_id": release["id"], "published": True}


def _release_publish_snapshot(ctx: Context) -> dict[str, Any]:
    release = _github(ctx).get_release(ctx.tag)
    if not release:
        return {"release_id": None, "draft": None}
    return {"release_id": release["id"], "draft": bool(release.get("draft"))}


def _release_publish_compensate(ctx: Context, state: OpState) -> None:
    if state.before.get("draft") is False:
        return
    raise CompensationError(
        "a published release cannot be un-published; use `release yank --reason ...`"
    )


def _release_verify(ctx: Context) -> dict[str, Any]:
    return {}


def _release_verify_check(ctx: Context, outputs: dict[str, Any]) -> dict[str, Any]:
    client = _github(ctx)
    release = client.get_release(ctx.tag)
    if not release:
        raise VerificationError(f"GitHub release for {ctx.tag} is missing")
    if release.get("draft"):
        raise VerificationError("release is still a draft")
    remote = {asset["name"]: asset for asset in client.list_assets(release["id"])}
    downloaded: dict[str, bytes] = {}
    for path in _release_assets(ctx):
        asset = remote.get(path.name)
        if asset is None:
            raise VerificationError(f"published release is missing {path.name}")
        data = client.download_asset(asset["id"])
        if hashlib.sha256(data).hexdigest() != sha256_file(path):
            raise VerificationError(f"{path.name}: published bytes differ from local")
        downloaded[path.name] = data

    if ctx.cfg.signing.enabled:
        public = load_public_key(ctx.repo / ctx.cfg.signing.public_key)
        for path in _signable_assets(ctx):
            signature = remote.get(path.name + ".sig")
            if signature is None:
                raise VerificationError(f"published release is missing {path.name}.sig")
            verify_bytes(public, downloaded[path.name], client.download_asset(signature["id"]))
        checksums = remote.get("SHA256SUMS.txt")
        checksums_sig = remote.get("SHA256SUMS.txt.sig")
        if checksums and checksums_sig:
            verify_bytes(
                public,
                client.download_asset(checksums["id"]),
                client.download_asset(checksums_sig["id"]),
            )
    return {"assets": len(remote)}


def _scan(ctx: Context) -> dict[str, Any]:
    cfg = ctx.cfg
    if cfg.scan.mode == "off":
        return {"scans": [], "skipped": "scanning disabled"}

    assets = _release_assets(ctx)
    if not assets:
        raise PreflightError(f"scan: no files matched {cfg.publish.assets!r}")

    api_key = os.environ.get(cfg.scan.api_key_env, "")
    if not api_key:
        if cfg.scan.mode == "advisory":
            return {"scans": [], "skipped": "no VirusTotal API key; advisory mode"}
        raise ScanError(
            f"scan.mode = {cfg.scan.mode!r} requires {cfg.scan.api_key_env}"
        )

    client = VirusTotalClient(api_key)
    scans: list[dict[str, Any]] = []
    for path in assets:
        digest, stats = client.scan_file(path)
        decision = evaluate(stats, cfg.scan, digest)
        scans.append(
            {
                "file": path.name,
                "sha256": digest,
                "detections": decision.detections,
                "engines": list(decision.engines),
                "allowed": decision.allowed,
                "reason": decision.reason,
            }
        )
        if not decision.allowed:
            raise ScanError(f"{path.name}: {decision.reason}")

    return {"scans": scans, "skipped": ""}


def _scan_verify(ctx: Context, outputs: dict[str, Any]) -> dict[str, Any]:
    meta = outputs.get("meta", {})
    if meta.get("skipped"):
        return {"skipped": meta["skipped"]}
    for scan in meta.get("scans", []):
        stats = VirusTotalClient(os.environ.get(ctx.cfg.scan.api_key_env, "")).get_report(
            scan["sha256"]
        )
        if stats is None:
            raise VerificationError(f"{scan['file']}: VirusTotal report disappeared")
        decision = evaluate(stats, ctx.cfg.scan, scan["sha256"])
        if not decision.allowed:
            raise VerificationError(f"{scan['file']}: {decision.reason}")
    return {"scans": len(meta.get("scans", []))}


def build_operations(ctx: Context) -> list[Operation]:
    cfg = ctx.cfg
    operations: list[Operation] = [
        Operation(
            name="preflight",
            stage="preflight",
            description="clean tree, branch, remote, tag, signing key, versions",
            run=_preflight,
            verify=lambda ctx, outputs: {"head": ctx.git.head()},
            always_run=True,
        ),
        Operation(
            name="version.bump",
            stage="version",
            description=f"rewrite version declarations to {ctx.version}",
            run=_version_bump,
            verify=_version_bump_verify,
            compensate=_version_bump_compensate,
            snapshot=_version_bump_snapshot,
        ),
        Operation(
            name="version.commit",
            stage="version",
            description=f"commit version bump as chore(release): {ctx.tag}",
            run=_version_commit,
            verify=_version_commit_verify,
            compensate=_version_commit_compensate,
            snapshot=_version_commit_snapshot,
            probe=lambda ctx: ctx.git.commit_subject("HEAD") == f"chore(release): {ctx.tag}",
        ),
    ]

    for stage in ("quality", "build", "package"):
        if stage in cfg.stages():
            operations.append(_command_operation(stage, cfg))

    if "sign" in cfg.stages():
        operations.append(
            _signing_operation(cfg)
            if cfg.signing.enabled
            else _command_operation("sign", cfg)
        )

    if "scan" in cfg.stages():
        operations.append(
            Operation(
                name="scan",
                stage="scan",
                description="scan release assets with the configured VirusTotal policy",
                run=_scan,
                verify=_scan_verify,
                params={
                    "mode": cfg.scan.mode,
                    "threshold": cfg.scan.threshold,
                    "allowlist": cfg.scan.allowlist,
                },
            )
        )

    operations.append(
        Operation(
            name="tag.create",
            stage="tag",
            description=f"create signed tag {ctx.tag}",
            run=_tag,
            verify=_tag_verify,
            compensate=_tag_compensate,
            snapshot=_tag_snapshot,
            probe=lambda ctx: ctx.git.tag_exists(ctx.tag)
            and ctx.git.tag_commit(ctx.tag) == ctx.git.head(),
            params={"tag": ctx.tag, "sign": cfg.git.sign_tags},
        )
    )
    operations.append(
        Operation(
            name="publish.push",
            stage="publish",
            description=f"atomically push {cfg.git.branch} and {ctx.tag}",
            run=_publish,
            verify=_publish_verify,
            compensate=_publish_compensate,
            snapshot=_publish_snapshot,
            probe=lambda ctx: all(
                ctx.git.ls_remote(ctx.cfg.git.remote).get(name) == sha
                for name, sha in _desired_refs(ctx).items()
            ),
            params={"remote": cfg.git.remote, "branch": cfg.git.branch, "tag": ctx.tag},
            remote=True,
        )
    )

    if "release" in cfg.stages():
        operations.extend(
            [
                Operation(
                    name="release.draft",
                    stage="release",
                    description=f"create or reuse a draft GitHub release for {ctx.tag}",
                    run=_release_draft,
                    verify=_release_draft_verify,
                    compensate=_release_draft_compensate,
                    snapshot=_release_draft_snapshot,
                    params={"repo": cfg.publish.repo, "tag": ctx.tag},
                    remote=True,
                ),
                Operation(
                    name="release.upload",
                    stage="release",
                    description="upload assets and verify their remote digests",
                    run=_release_upload,
                    verify=_release_upload_verify,
                    compensate=_release_upload_compensate,
                    snapshot=_release_upload_snapshot,
                    params={"assets": cfg.publish.assets},
                    remote=True,
                ),
                Operation(
                    name="release.publish",
                    stage="release",
                    description="publish the draft release",
                    run=_release_publish,
                    verify=_release_publish_verify,
                    compensate=_release_publish_compensate,
                    snapshot=_release_publish_snapshot,
                    params={"repo": cfg.publish.repo, "tag": ctx.tag},
                    remote=True,
                ),
            ]
        )

    operations.append(
        Operation(
            name="verify.remote",
            stage="verify",
            description="re-read remote refs and tag signature",
            run=_verify_remote,
            verify=_verify_remote_check,
            params={"remote": cfg.git.remote},
            always_run=True,
        )
    )
    if "release" in cfg.stages():
        operations.append(
            Operation(
                name="release.verify",
                stage="verify",
                description="download published assets and compare hashes",
                run=_release_verify,
                verify=_release_verify_check,
                params={"repo": cfg.publish.repo, "tag": ctx.tag},
                always_run=True,
                remote=True,
            )
        )
    return operations
