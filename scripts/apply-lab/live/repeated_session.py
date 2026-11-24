"""Summarize a poll-mode observer across two independently timed publications."""

import argparse
from collections import Counter
import hashlib
import json
from pathlib import Path


def summarize(data, first_start, first_end, second_start, second_end):
    if not 0 < first_start <= first_end < second_start <= second_end:
        raise ValueError("publication intervals must be positive, ordered and disjoint")
    phases = {key: Counter() for key in ("before", "between", "after")}
    previous = None
    for line_no, line in enumerate(data.decode("utf-8-sig").splitlines(), 1):
        tick, label, event, sid, client, server, detail = line.split("|", 6)
        tick = int(tick)
        if previous is None:
            if (event, client, server, detail) != ("open", "", "", "режим=poll"):
                raise ValueError("expected a poll-mode open record")
            if not label or not sid.isascii() or not sid.isdecimal() or int(sid) <= 0:
                raise ValueError("invalid observer identity")
            identity = (label, sid)
        else:
            if (label, sid) != identity or event != "poll" or tick < previous:
                raise ValueError(f"observer identity, event or clock changed at line {line_no}")
            if not detail.startswith("sid-source=marker-call ") or any(
                value in ("", "ОШИБКА") for value in (client, server)
            ):
                raise ValueError(f"missing paired server observation at line {line_no}")
            # A poll finishes before the next one begins. Its previous journal
            # tick bounds the earliest possible start of either marker read.
            if tick < first_start:
                phases["before"][client, server] += 1
            elif first_end < previous and tick < second_start:
                phases["between"][client, server] += 1
            elif second_end < previous:
                phases["after"][client, server] += 1
        previous = tick
    if not all(phases.values()):
        raise ValueError("each stable interval needs at least one complete poll")
    return {
        "label": identity[0],
        "sid": identity[1],
        "journal_sha256": hashlib.sha256(data).hexdigest(),
        "publication_intervals_ms": [[first_start, first_end], [second_start, second_end]],
        "observations": {
            phase: [
                {"client": client, "server": server, "samples": count}
                for (client, server), count in counts.items()
            ]
            for phase, counts in phases.items()
        },
    }


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("journal", type=Path)
    parser.add_argument("bounds", type=int, nargs=4,
                        help="start/end of each publication, UTC milliseconds since 0001-01-01")
    args = parser.parse_args()
    try:
        report = summarize(args.journal.read_bytes(), *args.bounds)
    except (OSError, ValueError) as error:
        parser.exit(1, f"{error}\n")
    print(json.dumps(report, ensure_ascii=False, indent=2))
