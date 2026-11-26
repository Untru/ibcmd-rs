"""Prints the measured table of the runs (runs/*.summary.json, runs/*.probe.json), and with --csv writes it.

  python table.py [--csv <file>] [--markdown]
The model of the log: 2 x data + indexes of the rebuilt tables (the data is written by the load into a heap and
again by the clustered index built over it; the other indexes once). ratio is the measured structure-phase log
over the model.
"""
import argparse
import csv
import glob
import json
import os

runs = os.path.join(os.environ.get('S1J_LAB', r'F:\ibcmd\lab\04\ui-codec\s1j'), 'runs')


def load():
    out = []
    for path in sorted(glob.glob(os.path.join(runs, '*.summary.json'))):
        d = json.load(open(path, encoding='utf-8'))
        if d.get('exit') not in (0, '0') or not d.get('structure_log_used'):
            continue
        b = d['before']
        data = sum(v['data_bytes'] for v in b.values())
        idx = sum(v['index_bytes'] for v in b.values())
        ph = d.get('phases', {})
        structure_s = sum(ph.get(k, 0) for k in ('create', 'load', 'indexes', 'drop-old', 'rename')) / 1000
        out.append(dict(
            run=d['tag'], kind=d['mode'], recovery=d['recovery'], rows=sum(v['rows'] for v in b.values()), data_mb=data / 1e6, index_mb=idx / 1e6,
            model_mb=(2 * data + idx) / 1e6, log_mb=d['structure_log_used'] / 1e6, reserved_mb=d.get('structure_log_reserved', 0) / 1e6,
            structure_s=structure_s, load_s=ph.get('load', 0) / 1000, indexes_s=ph.get('indexes', 0) / 1000,
            rollback_s=max(d.get('t_end', 0) - d.get('t_rollback_start', 0), 0) if d['mode'] == 'trial' else '',
            tempdb_mb=d.get('tempdb_peak_mb', 0) - d.get('tempdb_start_mb', 0)))
    for path in sorted(glob.glob(os.path.join(runs, '*.probe.json'))):
        d = json.load(open(path, encoding='utf-8'))
        b = d['before']
        out.append(dict(
            run=d['tag'], kind='probe (real table, rolled back)', recovery=d['recovery'], rows=b['rows'], data_mb=b['data_bytes'] / 1e6,
            index_mb=b['index_bytes'] / 1e6, model_mb=d['model_2d_plus_i'] / 1e6, log_mb=d['log_after_indexes'] / 1e6,
            reserved_mb=d['reserved'] / 1e6, structure_s=d['phases']['load'] + d['phases']['indexes'], load_s=d['phases']['load'],
            indexes_s=d['phases']['indexes'], rollback_s=d['phases']['rollback'], tempdb_mb=''))
    for r in out:
        r['ratio'] = r['log_mb'] / r['model_mb'] if r['model_mb'] else 0
    return out


def main():
    p = argparse.ArgumentParser()
    p.add_argument('--csv')
    p.add_argument('--markdown', action='store_true')
    a = p.parse_args()
    rows = load()
    fields = ['run', 'kind', 'recovery', 'rows', 'data_mb', 'index_mb', 'model_mb', 'log_mb', 'ratio', 'reserved_mb', 'structure_s', 'load_s',
              'indexes_s', 'rollback_s', 'tempdb_mb']
    if a.csv:
        with open(a.csv, 'w', newline='', encoding='utf-8') as fh:
            w = csv.writer(fh, lineterminator=chr(10))
            w.writerow(fields)
            for r in rows:
                w.writerow([('%.3f' % r[f]) if isinstance(r[f], float) else r[f] for f in fields])
    if a.markdown:
        print('| run | recovery | rows | data MB | index MB | 2d+i MB | log MB | log/(2d+i) | structure s | rollback s |')
        print('|---|---|---|---|---|---|---|---|---|---|')
        for r in rows:
            if r['recovery'] != 'FULL':
                continue
            print('| %s | %s | %d | %.0f | %.0f | %.0f | %.0f | %.2f | %.0f | %s |' % (
                r['run'], r['recovery'], r['rows'], r['data_mb'], r['index_mb'], r['model_mb'], r['log_mb'], r['ratio'], r['structure_s'],
                ('%.0f' % r['rollback_s']) if r['rollback_s'] != '' else ''))
        return
    print('%-24s %-6s %10s %8s %8s %8s %8s %6s %7s %6s %6s %6s' % ('run', 'model', 'rows', 'dataMB', 'idxMB', '2d+iMB', 'logMB', 'ratio', 'str_s', 'load_s', 'idx_s', 'rb_s'))
    for r in rows:
        print('%-24s %-6s %10d %8.0f %8.0f %8.0f %8.0f %6.2f %7.1f %6.1f %6.1f %6s' % (
            r['run'], r['recovery'], r['rows'], r['data_mb'], r['index_mb'], r['model_mb'], r['log_mb'], r['ratio'], r['structure_s'], r['load_s'],
            r['indexes_s'], ('%.1f' % r['rollback_s']) if r['rollback_s'] != '' else ''))
    full = [r for r in rows if r['recovery'] == 'FULL' and not r['kind'].startswith('probe')]
    if full:
        print('FULL runs: log/(2d+i) from %.2f to %.2f over %d runs' % (min(r['ratio'] for r in full), max(r['ratio'] for r in full), len(full)))
    probes = [r for r in rows if r['kind'].startswith('probe')]
    if probes:
        print('real tables: %.2f to %.2f over %d probes' % (min(r['ratio'] for r in probes), max(r['ratio'] for r in probes), len(probes)))


if __name__ == '__main__':
    main()
