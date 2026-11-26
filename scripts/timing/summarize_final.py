"""Final timing table: native ibcmd, ibcmd-rs before (master aabf2c06) and
after (perf/fast-load), export and load, from timing.tsv.

Before/after are told apart by time: the fast build was measured from
2026-09-25T00:28 on (and its loads carry exe=ibcmd-rs-fast.exe in the note).
usage: summarize_final.py [timing.tsv]"""
import io, sys

sys.stdout.reconfigure(encoding="utf-8")
path = sys.argv[1] if len(sys.argv) > 1 else "F:/ibcmd/lab/timing/timing.tsv"
FAST_FROM = "2026-09-25T00:28"
rows = []
last_stage_note = {}
for line in io.open(path, encoding="utf-8-sig"):
    f = line.rstrip("\n").split("\t")
    if len(f) < 7 or f[0] == "when":
        continue
    seconds = float(f[5].replace("\u00a0", "").replace(" ", "").replace(",", "."))
    note = f[7] if len(f) > 7 else ""
    # A publish row names no build: it belongs to the stage just before it.
    if f[4] == "load: stage" and not f[3].startswith("native"):
        last_stage_note[f[2]] = note
    elif f[4] == "load: publish":
        note = last_stage_note.get(f[2], "")
    rows.append(dict(when=f[0], phase=f[1], corpus=f[2], tool=f[3], step=f[4], s=seconds, rc=int(f[6]),
                     note=note))


def pick(phase, corpus, step, who):
    """Last successful measurement of a step by who = native | before | after."""
    found = None
    for r in rows:
        if r["phase"] != phase or r["corpus"] != corpus or r["step"] != step or r["rc"] != 0:
            continue
        native = r["tool"].startswith("native")
        if who == "native" and not native:
            continue
        if who != "native":
            if native:
                continue
            fast = "ibcmd-rs-fast" in r["note"] or (phase == "export" and r["when"] >= FAST_FROM)
            if (who == "after") != fast:
                continue
        found = r["s"]
    return found


def failed(phase, corpus, step):
    return any(r["phase"] == phase and r["corpus"] == corpus and r["step"] == step
               and r["tool"].startswith("native") and r["rc"] != 0 for r in rows) and pick(phase, corpus, step, "native") is None


def fmt(s):
    if s is None:
        return "—"
    m, sec = divmod(int(round(s)), 60)
    return "%d мин %02d с" % (m, sec) if m else "%.1f с" % s if s < 10 else "%d с" % round(s)


def ratio(a, b):
    return "×%.1f" % (a / b) if a and b else ""


NAMES = {"bsp8327": "БСП 8.3.27", "bsp85": "БСП 8.5", "uha8327": "ERP УХ 8.3.27", "uha85": "ERP УХ 8.5"}
print("Выгрузка (база → XML):\n")
print("| база | штатный ibcmd | ibcmd-rs было | ibcmd-rs стало | ускорение |")
print("|---|---|---|---|---|")
for c in NAMES:
    n, b, a = (pick("export", c, "export", w) for w in ("native", "before", "after"))
    print("| %s | %s | %s | %s | %s |" % (NAMES[c], fmt(n), fmt(b), fmt(a), ratio(b, a)))

print("\nЗагрузка (XML → ConfigSave, затем в Config):\n")
print("| база | штатный import | ibcmd-rs было: stage | ibcmd-rs стало: stage | ускорение | publish было / стало |")
print("|---|---|---|---|---|---|")
for c in NAMES:
    n = pick("load", c, "load: config import", "native")
    nt = fmt(n) if n is not None else ("ошибка" if failed("load", c, "load: config import") else "—")
    b, a = pick("load", c, "load: stage", "before"), pick("load", c, "load: stage", "after")
    pb, pa = pick("load", c, "load: publish", "before"), pick("load", c, "load: publish", "after")
    print("| %s | %s | %s | %s | %s | %s / %s |" % (NAMES[c], nt, fmt(b), fmt(a), ratio(b, a), fmt(pb), fmt(pa)))
