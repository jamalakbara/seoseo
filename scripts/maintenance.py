#!/usr/bin/env python3
"""Portable SeoSeo Postgres backup/restore. Backups include secrets: keep them private."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
from datetime import datetime, timezone


def digest(path):
    with path.open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def docker(args, command, **kwargs):
    result = subprocess.run(["docker", "exec", "-i", args.container, *command], **kwargs)
    if result.returncode:
        raise RuntimeError("Database command failed; no credentials were printed. Check the target container and database permissions.")
    return result


def private_directory(path):
    path.mkdir(mode=0o700, parents=False, exist_ok=False)
    if os.name == "nt":
        who = subprocess.run(["whoami"], check=True, capture_output=True, text=True).stdout.strip()
        subprocess.run(["icacls", str(path), "/inheritance:r", "/grant:r", who + ":(OI)(CI)F"], check=True, capture_output=True)


def backup(args):
    folder = Path(args.directory).resolve()
    root = Path(__file__).resolve().parents[1]
    if folder.is_relative_to(root):
        raise ValueError("Choose a new backup directory outside the source repository")
    env = Path(args.env_file).resolve()
    if not env.is_file():
        raise ValueError("An operator environment file is required to preserve encryption/OAuth secrets")
    private_directory(folder)
    dump = folder / "database.dump"
    with dump.open("xb") as stream:
        docker(args, ["pg_dump", "--format=custom", "--no-password", "--username", args.db_user, "--dbname", args.db_name], stdout=stream, stderr=subprocess.DEVNULL)
    shutil.copyfile(env, folder / "operator.env")
    manifest = {"format": 1, "created_at": datetime.now(timezone.utc).isoformat(), "database": args.db_name,
                "files": {name: digest(folder / name) for name in ("database.dump", "operator.env")}}
    (folder / "manifest.json").write_text(json.dumps(manifest, indent=2) + "\n", encoding="utf-8")
    for name in manifest["files"]:
        (folder / name).chmod(0o600)
    print("Backup completed:", folder)
    print("Includes operator secrets. Store securely; test restoration to an empty database.")


def restore(args):
    if args.confirm != "RESTORE EMPTY DATABASE":
        raise ValueError("Pass --confirm 'RESTORE EMPTY DATABASE' after selecting the intended empty target")
    folder = Path(args.directory).resolve()
    manifest = json.loads((folder / "manifest.json").read_text(encoding="utf-8"))
    if manifest.get("format") != 1 or set(manifest.get("files", {})) != {"database.dump", "operator.env"}:
        raise ValueError("Unsupported backup manifest")
    for name, expected in manifest["files"].items():
        if (folder / name).is_symlink() or digest(folder / name) != expected:
            raise ValueError("Backup checksum mismatch: " + name)
    output = Path(args.env_output).resolve()
    if output.exists():
        raise ValueError("Environment output must be a new file; existing configuration will never be overwritten")
    check = docker(args, ["psql", "--no-password", "-U", args.db_user, "-d", args.db_name, "-At", "-c",
                          "SELECT count(*) FROM information_schema.tables WHERE table_schema NOT IN ('pg_catalog','information_schema')"], capture_output=True, text=True)
    if check.stdout.strip() != "0":
        raise ValueError("Restore refused: target database is not empty. No data was deleted or replaced.")
    fd = os.open(output, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
    try:
        if os.name == "nt":
            who = subprocess.run(["whoami"], check=True, capture_output=True, text=True).stdout.strip()
            subprocess.run(["icacls", str(output), "/inheritance:r", "/grant:r", who + ":F"], check=True, capture_output=True)
        with (folder / "database.dump").open("rb") as stream:
            docker(args, ["pg_restore", "--single-transaction", "--exit-on-error", "--no-owner", "--no-privileges", "--no-password",
                          "--username", args.db_user, "--dbname", args.db_name], stdin=stream, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        with os.fdopen(fd, "wb") as stream:
            fd = None
            stream.write((folder / "operator.env").read_bytes())
    except Exception:
        if fd is not None:
            os.close(fd)
        output.unlink(missing_ok=True)
        raise
    print("Restored backup into the selected empty database.")
    print("Operator environment restored to:", output)
    print("Review container/database addresses before starting matching app images; preserve APP_ENCRYPTION_KEY.")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("action", choices=["backup", "restore"])
    parser.add_argument("--container", required=True)
    parser.add_argument("--db-user", default="seoseo")
    parser.add_argument("--db-name", default="seoseo")
    parser.add_argument("--directory", required=True)
    parser.add_argument("--env-file")
    parser.add_argument("--env-output")
    parser.add_argument("--confirm", default="")
    args = parser.parse_args()
    if args.action == "backup" and not args.env_file:
        parser.error("backup requires --env-file")
    if args.action == "restore" and not args.env_output:
        parser.error("restore requires --env-output")
    try:
        (backup if args.action == "backup" else restore)(args)
    except (OSError, ValueError, RuntimeError, subprocess.CalledProcessError) as error:
        print(str(error), file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
