"""Puts the edited files of a case over the working tree, and takes them off again.

usage:
  python overlay.py apply   <case dir> <tree>   copy <case dir>/stage/<rel> over <tree>/<rel>
  python overlay.py restore <case dir> <tree>   copy <case dir>/before/<rel> back
  python overlay.py verify  <case dir> <tree>   the tree's files equal the reference (after a restore) or the stage (after an apply)

The working tree is a copy of the reference native export made once (robocopy); a case edits a handful of files of it,
our import reads the tree, and the files go back, so one tree serves every case.
"""
import hashlib
import os
import shutil
import sys


def rel_files(case_dir):
    with open(os.path.join(case_dir, "files.txt"), encoding="utf-8") as f:
        return [line.strip() for line in f if line.strip()]


def sha(path):
    with open(path, "rb") as f:
        return hashlib.sha256(f.read()).hexdigest()


def new_files(case_dir):
    """The files a case adds (new.txt, written by new_objects.py): no original to put back, they are taken out again."""
    path = os.path.join(case_dir, "new.txt")
    if not os.path.exists(path):
        return set()
    with open(path, encoding="utf-8") as f:
        return {line.strip() for line in f if line.strip()}


def main():
    command, case_dir, tree = sys.argv[1], sys.argv[2], sys.argv[3]
    files = rel_files(case_dir)
    added = new_files(case_dir)
    if command == "apply":
        for rel in files:
            target = os.path.join(tree, rel)
            os.makedirs(os.path.dirname(target), exist_ok=True)
            shutil.copyfile(os.path.join(case_dir, "stage", rel), target)
        print("applied", len(files), "files (%d new)" % len(added))
    elif command == "restore":
        for rel in files:
            if rel in added:
                if os.path.exists(os.path.join(tree, rel)):
                    os.remove(os.path.join(tree, rel))
            else:
                shutil.copyfile(os.path.join(case_dir, "before", rel), os.path.join(tree, rel))
        print("restored", len(files), "files (%d removed)" % len(added))
    elif command == "verify":
        bad = 0
        for rel in files:
            stage = os.path.join(case_dir, "stage", rel)
            before = os.path.join(case_dir, "before", rel)
            current = os.path.join(tree, rel)
            if not os.path.exists(current):
                state = "absent" if rel in added else "other"
            else:
                state = "stage" if sha(current) == sha(stage) else "before" if rel not in added and sha(current) == sha(before) else "other"
            print(state, rel)
            bad += state == "other"
        sys.exit(1 if bad else 0)
    else:
        raise SystemExit("unknown command " + command)


if __name__ == "__main__":
    main()
