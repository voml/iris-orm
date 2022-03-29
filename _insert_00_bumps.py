# -*- coding: utf-8 -*-
"""Insert virtual 0.0.0 and 0.0.1 workspace bump commits after monorepo relocate."""
import json
import re
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent
BRANCH = "dev"
REBUILD = "dev-rebuild"
INSERT_AFTER = "1ace0c93"
BUMP_TEMPLATE = "1ace0c93"

WORKSPACE_BUMP_RE = re.compile(r"^🔖 Bump iris workspace packages to (\d+\.\d+\.\d+)$")
NPM_BUMP_RE = re.compile(
    r"^🔧 Bump npm packages(?: to `([0-9.]+)`)? for CI trusted publish$"
)
OLD_PKG_BUMP_RE = re.compile(
    r"^📦 Bump `@yydb/iris` Trusted Publisher package versions"
)


def run(cmd, check=True):
    print("+", " ".join(cmd))
    result = subprocess.run(
        cmd,
        cwd=ROOT,
        check=False,
        text=True,
        encoding="utf-8",
        errors="replace",
        capture_output=True,
    )
    if check and result.returncode != 0:
        out = (result.stdout or "") + (result.stderr or "")
        raise RuntimeError(f"command failed ({result.returncode}): {out}")
    return result


def git_out(*args):
    return run(["git", *args]).stdout.strip()


def subject(commit):
    return git_out("log", "-1", "--format=%s", commit)


def meta(commit):
    out = git_out("log", "-1", "--format=%an%n%ae%n%at", commit).splitlines()
    return {"name": out[0], "email": out[1], "date": out[2]}


def git_env(author):
    return [
        "-c",
        f"user.name={author['name']}",
        "-c",
        f"user.email={author['email']}",
        "-c",
        f"user.date={author['date']}",
    ]


def list_publish_pkg_paths():
    paths = []
    base = ROOT / "projects/packages"
    if not base.exists():
        return paths
    for pkg_json in sorted(base.glob("*/package.json")):
        if pkg_json.parent.name == "homepage":
            continue
        try:
            data = json.loads(pkg_json.read_text(encoding="utf-8"))
        except json.JSONDecodeError:
            continue
        name = data.get("name", "")
        if name.startswith("@yydb/iris") and not data.get("private", False):
            paths.append(pkg_json)
    return paths


def read_json_version(path: Path) -> str:
    text = path.read_text(encoding="utf-8")
    match = re.search(r'"version":\s*"([^"]+)"', text)
    if not match:
        raise RuntimeError(f"cannot read version from {path}")
    return match.group(1)


def write_json_version(path: Path, to_ver: str) -> None:
    text = path.read_text(encoding="utf-8")
    new, count = re.subn(
        r'"version":\s*"[^"]+"', f'"version": "{to_ver}"', text, count=1
    )
    if count != 1:
        raise RuntimeError(f"failed to set version in {path}")
    path.write_text(new, encoding="utf-8", newline="\n")


def normalize_publish_versions(target_ver: str) -> list[str]:
    changed = []
    for path in list_publish_pkg_paths():
        rel = path.relative_to(ROOT).as_posix()
        if read_json_version(path) != target_ver:
            write_json_version(path, target_ver)
            changed.append(rel)
    if changed:
        run(["git", "add", *changed])
        run(["git", "commit", "--amend", "--no-edit"])
    return changed


def create_bump(target_ver: str, template_commit: str, allow_empty: bool = False):
    author = meta(template_commit)
    changed = []
    for path in list_publish_pkg_paths():
        rel = path.relative_to(ROOT).as_posix()
        if read_json_version(path) != target_ver:
            write_json_version(path, target_ver)
            changed.append(rel)
    msg = f"🔖 Bump iris workspace packages to {target_ver}"
    if changed:
        run(["git", *git_env(author), "add", *changed])
        run(["git", *git_env(author), "commit", "-m", msg])
    elif allow_empty:
        run(["git", *git_env(author), "commit", "--allow-empty", "-m", msg])
    else:
        run(["git", *git_env(author), "commit", "--allow-empty", "-m", msg])
    print(f"  bump {target_ver} ({len(changed)} files)")


def cherry_pick(commit):
    result = run(["git", "cherry-pick", commit], check=False)
    if result.returncode == 0:
        return
    combined = (result.stdout or "") + (result.stderr or "")
    if "previous cherry-pick is now empty" in combined or "nothing to commit" in combined:
        run(["git", "cherry-pick", "--skip"])
        return
    if "CONFLICT" in combined:
        run(["git", "checkout", "--theirs", "."])
        run(["git", "add", "-A"])
        author = meta(commit)
        cont = run(
            ["git", *git_env(author), "cherry-pick", "--continue"],
            check=False,
        )
        if cont.returncode != 0:
            out = (cont.stdout or "") + (cont.stderr or "")
            if "nothing to commit" in out or "no changes" in out:
                run(["git", "cherry-pick", "--skip"])
                return
            raise RuntimeError(f"cherry-pick continue failed for {commit}: {out}")
        return
    raise RuntimeError(f"cherry-pick failed for {commit}: {combined}")


def cherry_pick_feature(commit, current_version: str):
    cherry_pick(commit)
    normalize_publish_versions(current_version)


def parse_bump(commit):
    subj = subject(commit)
    ws = WORKSPACE_BUMP_RE.match(subj)
    if ws:
        return ws.group(1), "workspace"
    npm = NPM_BUMP_RE.match(subj)
    if npm:
        version = npm.group(1) or "0.0.6"
        return version, "npm"
    if OLD_PKG_BUMP_RE.match(subj):
        return None
    return None


def commit_list(original_tip: str):
    insert = git_out("rev-parse", INSERT_AFTER)
    return git_out("rev-list", "--reverse", f"{insert}..{original_tip}").splitlines()


def main():
    original_tip = git_out("rev-parse", BRANCH)
    insert = git_out("rev-parse", INSERT_AFTER)
    template = git_out("rev-parse", BUMP_TEMPLATE)
    commits = commit_list(original_tip)
    if not commits:
        raise RuntimeError("no commits to replay after relocate")

    run(["git", "checkout", BRANCH])
    run(["git", "branch", "-D", REBUILD], check=False)
    run(["git", "checkout", "-B", REBUILD, insert])

    create_bump("0.0.0", template)
    current_version = "0.0.0"
    create_bump("0.0.1", template)
    current_version = "0.0.1"

    for commit in commits:
        bump = parse_bump(commit)
        if bump:
            version, _style = bump
            create_bump(version, commit)
            current_version = version
            continue
        if OLD_PKG_BUMP_RE.match(subject(commit)):
            print(f"  skip old package bump {commit[:8]}")
            continue
        cherry_pick_feature(commit, current_version)

    run(["git", "branch", "-f", BRANCH, REBUILD])
    run(["git", "checkout", BRANCH])
    run(["git", "branch", "-D", REBUILD], check=False)
    new_tip = git_out("rev-parse", "HEAD")
    print(f"done: {original_tip[:8]} -> {new_tip[:8]}")


if __name__ == "__main__":
    main()
