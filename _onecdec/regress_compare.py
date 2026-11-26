"""Two builds' exports of the local corpora against the platform's dumps:
which files one build matches and the other does not.

    python regress_compare.py <exe A> <exe B> label...

The configuration readers `cf export` runs are the ones `infobase config
export` runs on the rows it reads from SQL, so a file A matches and B does
not is a regression of B in both paths. Inputs and native dumps as in
big_compare.py (corpora.local.json, <IBCMD_WORK>\\native-8.3.27.2214\\<label>).
Prints per label: identical for A and B, then the files only A matches and
the files only B matches (first 30 of each).
"""
import os, subprocess, sys

sys.stdout.reconfigure(encoding='utf-8')
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import big_compare as bc


def export(exe, label, out):
    r = subprocess.run([exe, 'cf', 'export', bc.CORPORA[label], out, '--overwrite'], capture_output=True)
    return r.returncode


def same_files(native, out):
    nat, ours = bc.files(native), bc.files(out)
    return {p for p in nat if p in ours and open(nat[p], 'rb').read() == open(ours[p], 'rb').read()}, len(nat)


def main():
    a, b, labels = sys.argv[1], sys.argv[2], sys.argv[3:]
    for label in labels:
        native = bc.native_of(label)
        results = []
        for tag, exe in (('a', a), ('b', b)):
            out = os.path.join(bc.ROOT, 'regress-' + tag, label)
            code = export(exe, label, out)
            same, total = same_files(native, out)
            results.append((code, same))
        (code_a, same_a), (code_b, same_b) = results
        print('%-5s total %d  A exit=%d identical %d  B exit=%d identical %d' % (
            label, total, code_a, len(same_a), code_b, len(same_b)))
        only_a, only_b = sorted(same_a - same_b), sorted(same_b - same_a)
        print('      only A matches: %d' % len(only_a))
        for p in only_a[:30]:
            print('        ' + p)
        print('      only B matches: %d' % len(only_b))
        for p in only_b[:30]:
            print('        ' + p)
        sys.stdout.flush()


if __name__ == '__main__':
    main()
