"""Derive, merge and verify upgrade rules for older stored records.

    python upgrade_sim.py derive <old.cf> <new.cf> [<old.cf> <new.cf> ...] --out rules.json
    python upgrade_sim.py verify rules.json <old.cf> <new.cf> [...]

A rule rewrites one brace list: matched by its parent's tag and its own tag
and length, it takes a new tag, has members set (index, old, new) and members
inserted (index in the new list, value). "Append" rules (inserted members
all at the end) generalize over lengths. `verify` applies the rules bottom-up
to every old row and counts rows that come out equal to the re-serialized
row, printing the first mismatches.
"""
import collections, json, os, sys
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from upgrade_rules import parse, render, tag, lcs, rows


def edits(old, new):
    """(sets, inserts) turning list old into list new, or None when the
    difference is not insert-only at this level."""
    if len(new) < len(old):
        return None
    pairs = lcs(old, new)
    anchors = [(-1, -1)] + pairs + [(len(old), len(new))]
    inserts, pairs_changed = [], []
    for (ia, ja), (ib, jb) in zip(anchors, anchors[1:]):
        olds, news = list(range(ia + 1, ib)), list(range(ja + 1, jb))
        if len(news) < len(olds):
            return None
        # changed members pair up in order; surplus new members are inserted
        # after them
        for k, i in enumerate(olds):
            pairs_changed.append((i, news[k]))
        for j in news[len(olds):]:
            inserts.append(j)
    return pairs_changed, inserts


def collect(old, new, parent_tag, out):
    """Record list-level rules for every differing list, recursively."""
    if render(old) == render(new) or not (isinstance(old, list) and isinstance(new, list)):
        return
    e = edits(old, new)
    if e is None:
        out.append(('unaligned', parent_tag, tag(old), len(old), tag(new), len(new)))
        return
    changed, inserts = e
    sets = []
    for i, j in changed:
        a, b = old[i], new[j]
        if isinstance(a, list) and isinstance(b, list):
            collect(a, b, tag(old), out)
        elif not isinstance(a, list) and not isinstance(b, list):
            sets.append((i, a, b))
        else:
            out.append(('shape', parent_tag, tag(old), len(old), i))
    ins = [(j, render(new[j])) for j in inserts]
    if ins or sets:
        out.append(('rule', parent_tag, tag(old), len(old), tag(new), len(new), tuple(sets), tuple(ins)))


def derive(pairs):
    found = collections.Counter()
    problems = collections.Counter()
    for old_path, new_path in pairs:
        old, new = rows(old_path), rows(new_path)
        for name in set(old) & set(new):
            if old[name] == new[name]:
                continue
            a, _ = parse(old[name], old[name].index('{'))
            b, _ = parse(new[name], new[name].index('{'))
            out = []
            collect(a, b, None, out)
            for item in out:
                (found if item[0] == 'rule' else problems)[item] += 1
    return found, problems


def key_of(parent_tag, t, n):
    return '%s|%s|%s' % (parent_tag, t, n)


def generalize(found):
    """{key: rule}; append-only rules over several lengths of one tag merge
    into a length-free rule."""
    rules, conflicts = {}, []
    for (_, parent, t, n, nt, nn, sets, ins), count in found.items():
        k = key_of(parent, t, n)
        rule = {'parent': parent, 'tag': t, 'len': n, 'new_tag': nt, 'new_len': nn,
                'sets': [list(s) for s in sets], 'inserts': [list(i) for i in ins], 'count': count}
        if k in rules and (rules[k]['sets'], rules[k]['inserts']) != (rule['sets'], rule['inserts']):
            conflicts.append((k, rules[k], rule))
            if rule['count'] <= rules[k]['count']:
                continue
        rules[k] = rule
    # append-only: every insert at an index >= len, values equal across lengths
    by_tag = collections.defaultdict(list)
    for k, r in rules.items():
        appended = all(j >= r['len'] for j, _ in r['inserts']) and not [s for s in r['sets'] if s[0] != 0]
        if appended:
            by_tag[(r['parent'], r['tag'])].append(r)
    for (parent, t), group in by_tag.items():
        values = {tuple(v for _, v in r['inserts']) for r in group}
        tags = {r['new_tag'] for r in group}
        if len(group) > 1 and len(values) == 1 and len(tags) == 1:
            for r in group:
                rules.pop(key_of(parent, t, r['len']))
            rules[key_of(parent, t, '*')] = {
                'parent': parent, 'tag': t, 'len': '*', 'new_tag': group[0]['new_tag'],
                'sets': [s for s in group[0]['sets'] if s[0] == 0],
                'append': list(values.pop()), 'count': sum(r['count'] for r in group)}
    return rules, conflicts


def apply(node, rules, parent_tag=None):
    if not isinstance(node, list):
        return node
    own = tag(node)
    node = [apply(child, rules, own) for child in node]
    rule = rules.get(key_of(parent_tag, own, len(node))) or rules.get(key_of(parent_tag, own, '*'))
    if rule is None:
        return node
    out = list(node)
    for i, a, b in rule['sets']:
        if i < len(out) and out[i] == a:
            out[i] = b
    if 'append' in rule:
        out.extend(rule['append'])
    else:
        for j, v in sorted(rule['inserts']):
            out.insert(j, v)
    return out


def verify(rules, pairs, show=8):
    total = equal = 0
    shown = 0
    for old_path, new_path in pairs:
        old, new = rows(old_path), rows(new_path)
        for name in sorted(set(old) & set(new)):
            if old[name] == new[name]:
                continue
            total += 1
            a, _ = parse(old[name], old[name].index('{'))
            b, _ = parse(new[name], new[name].index('{'))
            got = render(apply(a, rules))
            if got == render(b):
                equal += 1
            elif shown < show:
                shown += 1
                x, y = got, render(b)
                i = next((k for k in range(min(len(x), len(y))) if x[k] != y[k]), min(len(x), len(y)))
                print('MISMATCH %s %s\n   ours: ...%s\n   new : ...%s' % (os.path.basename(old_path), name,
                                                                        x[max(0, i - 80):i + 80], y[max(0, i - 80):i + 80]))
    print('upgraded rows equal to re-serialized: %d/%d' % (equal, total))


def main():
    mode = sys.argv[1]
    if mode == 'derive':
        args = sys.argv[2:sys.argv.index('--out')]
        out = sys.argv[sys.argv.index('--out') + 1]
        pairs = list(zip(args[0::2], args[1::2]))
        found, problems = derive(pairs)
        rules, conflicts = generalize(found)
        json.dump(rules, open(out, 'w', encoding='utf-8'), ensure_ascii=False, indent=1)
        print('rules %d, conflicts %d, problems %d' % (len(rules), len(conflicts), sum(problems.values())))
        for c in conflicts[:10]:
            print('CONFLICT', c[0], c[1]['inserts'][:3], c[2]['inserts'][:3])
        for p, n in problems.most_common(15):
            print('PROBLEM %5d %s' % (n, p))
    else:
        rules = json.load(open(sys.argv[2], encoding='utf-8'))
        args = sys.argv[3:]
        verify(rules, list(zip(args[0::2], args[1::2])))


if __name__ == '__main__':
    main()
