"""Check dbschema.py's DBSchema -> SQL model against the real database dump (snapshot schema.txt).
usage: python dbschema_check.py <snapshot-label> [--verbose]"""
import collections
import os
import re
import sys

import bracefmt as bf
import dbschema as D
import lab


def load_sql(label, db=lab.DB):
    sch, cur = {}, None
    with open(os.path.join(lab.ROOT, "snap", db, label, "schema.txt"), encoding="utf-8") as f:
        for line in f:
            line = line.rstrip("\n")
            if line.startswith("T "):
                cur = line[2:]
                sch[cur] = {"cols": [], "idx": []}
            elif line.startswith("  C "):
                m = re.match(r"  C (\S+) (\S+) (NULL|NOT NULL)( IDENTITY)?( COMPUTED)?( DEFAULT .*?)?( COLLATE \S+)?$", line)
                sch[cur]["cols"].append((m.group(1), m.group(2), m.group(3) == "NULL"))
            elif line.startswith("  I "):
                m = re.match(r"  I (\S+) (CLUSTERED|NONCLUSTERED) \[([^\]]*)\] \(([^)]*)\)(?: INCLUDE \(([^)]*)\))?", line)
                flags = m.group(3).split()
                sch[cur]["idx"].append((m.group(1), "UNIQUE" in flags or "PK" in flags, m.group(2) == "CLUSTERED",
                                        m.group(4).split(",")))
    return sch


def main():
    label = sys.argv[1]
    verbose = "--verbose" in sys.argv
    db = next((a for a in sys.argv[2:] if a.startswith("ibcmd_rs_")), lab.DB)
    sql = load_sql(label, db)
    tables = D.parse_schema(lab.dbschema(label, db))
    stats = collections.Counter()
    bad = collections.defaultdict(list)
    for t in tables:
        name = D.s(t[0])
        jobs = [("_" + name, t, None)]
        for st in t[5][1:]:
            jobs.append(("_" + name + "_" + D.s(st[0]), st, t))
        for sqlname, ent, owner in jobs:
            stats["tables"] += 1
            real = sql.get(sqlname)
            if real is None:
                stats["missing_in_sql"] += 1
                bad["missing"].append(sqlname)
                continue
            try:
                exp_cols = [(c.name, c.typ.replace(", ", ","), c.nullable) for c in D.table_columns(ent, owner)]
            except Exception as e:
                stats["gen_error"] += 1
                bad["gen_error"].append((sqlname, repr(e)))
                continue
            if exp_cols == real["cols"]:
                stats["cols_ok"] += 1
            else:
                stats["cols_bad"] += 1
                bad["cols"].append((sqlname, exp_cols, real["cols"]))
            try:
                exp_idx = D.index_list(ent, owner)
            except Exception as e:
                stats["idx_gen_error"] += 1
                bad["idx_gen_error"].append((sqlname, repr(e)))
                continue
            # SQL side: the primary/clustered key of the R tables (…_HPK) and inline PKs are not in the DBSchema index list
            real_idx = [(i[0], i[1], i[2], i[3]) for i in real["idx"]]
            names_exp = [e[0] for e in exp_idx]
            extra_real = [i for i in real_idx if i[0] not in names_exp]
            if owner is None:
                imp = D.implicit_indexes(ent)
                unexplained = []
                for i in extra_real:
                    hit = any((m[0] == i[0] if m[0] else re.match(r"PK__", i[0])) and m[1] == i[1] and m[2] == i[2] and m[3] == i[3]
                              for m in imp)
                    if not hit:
                        unexplained.append(i)
                missing_imp = [m for m in imp if not any((m[0] == i[0] if m[0] else re.match(r"PK__", i[0])) and m[3] == i[3] for i in real_idx)]
                if unexplained or missing_imp:
                    stats["implicit_idx_bad"] += 1
                    bad["implicit"].append((sqlname, unexplained, missing_imp))
                else:
                    stats["implicit_idx_ok"] += 1
            ok = all(e in real_idx for e in exp_idx)
            if ok:
                stats["idx_ok"] += 1
                stats["extra_real_idx"] += len(extra_real)
                for i in extra_real:
                    bad["extra_real_idx_kinds"].append((re.sub(r"\d+", "#", i[0]), i[1], i[2]))
            else:
                stats["idx_bad"] += 1
                bad["idx"].append((sqlname, exp_idx, real_idx))
    print(dict(stats))
    for k in ("missing", "gen_error", "idx_gen_error"):
        if bad[k]:
            print(k, len(bad[k]), bad[k][:5])
    print("extra real index kinds:", collections.Counter(bad["extra_real_idx_kinds"]).most_common(12))
    print("== implicit index mismatches:", len(bad["implicit"]))
    for item in bad["implicit"][:8]:
        print("  ", item)
    for k in ("cols", "idx"):
        print("== %s mismatches: %d" % (k, len(bad[k])))
        for item in bad[k][: (30 if verbose else 4)]:
            print(item[0])
            if k == "cols":
                e, r = item[1], item[2]
                print("  exp-only:", [c for c in e if c not in r][:6])
                print("  real-only:", [c for c in r if c not in e][:6])
                if [c[0] for c in e] != [c[0] for c in r] and set(e) == set(r):
                    print("  (same set, different order)")
            else:
                print("  exp:", [e for e in item[1] if e not in item[2]][:4])
                print("  real:", [r for r in item[2] if r[0] not in [e[0] for e in item[1]]][:4])


if __name__ == "__main__":
    main()
