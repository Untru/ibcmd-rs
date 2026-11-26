#!/usr/bin/env python3
"""EDT -> XML byte comparison against a fresh macOS native file database.

Research only. No cleanup, original database access, or XML normalization.
"""
from __future__ import annotations

import argparse
import contextlib
import fcntl
import json
import os
import re
from pathlib import Path
import shutil
import signal
import subprocess
import sys
import time

from oracle import (OracleError, compare_tree_snapshots, digest, no_links,
                    require_project, require_xml, snapshot, write_json, command_diagnostics, has_error_diagnostics)

LAB = Path('/Volumes/Work/Dev/ibcmd-rs-lab')
MODEL_BASELINE = Path(__file__).resolve().parents[2] / 'crates/ibcmd-schema/data/edt-2025.2.3-model-inventory.json'


def lab_path(path: Path) -> Path:
    path = path.absolute()
    no_links(path)
    if not path.is_relative_to(LAB) or '..' in path.parts:
        raise OracleError(f'Only physical external laboratory paths are allowed: {path}')
    return path


def physical_contains(ancestor, descendant):
    if descendant.is_relative_to(ancestor):
        return True
    if ancestor.exists():
        return any(part.exists() and part.samefile(ancestor) for part in [descendant, *descendant.parents])
    return False


def disjoint(paths):
    for i, left in enumerate(paths):
        for right in paths[i + 1:]:
            if physical_contains(left, right) or physical_contains(right, left):
                raise OracleError(f'Overlapping inputs/run refused: {left}, {right}')


def installation_root(edt):
    """A copied Mac application is the immutable installation boundary."""
    eclipse = edt.parent
    contents = eclipse.parent
    bundle = contents.parent
    if edt.name != '1cedtcli' or eclipse.name != 'Eclipse' or contents.name != 'Contents' or bundle.suffix != '.app':
        raise OracleError('Expected copied .app/Contents/Eclipse/1cedtcli layout')
    return lab_path(bundle)


def inventory_equal(left, right):
    return left['files'] == right['files']


def comparison(ours, native):
    # Preserve the strict comparison as well as the existing source-oracle
    # contract: native storage generations cannot be invented by EDT.
    strict = compare_tree_snapshots(ours, native)
    content = compare_tree_snapshots(ours, native, excluded_paths=('ConfigDumpInfo.xml',))
    return {'strict': strict, 'configuration_content': content,
            'counts': {'ours': ours['file_count'], 'native': native['file_count'],
                       'strict_differences': len(strict['differing_rows']),
                       'content_differences': len(content['differing_rows'])}}


def bind_installation(path, edt):
    """Rebind #358's complete plugin inventory and immutable runtime files."""
    lab_path(path)
    data = json.loads(path.read_text(encoding='utf-8'))
    if data.get('schema_version') != 1 or Path(data['edt_cli']) != edt or data['edt_cli_sha256'] != digest(edt):
        raise OracleError('Installation manifest does not bind this EDT launcher')
    plugins = data['plugins']
    root = lab_path(Path(plugins['root']))
    if root != edt.parent / 'plugins' or snapshot(root) != plugins:
        raise OracleError('Installation plugin inventory differs from live EDT')
    rows = data['runtime_files']
    if not rows or len({row['path'] for row in rows}) != len(rows):
        raise OracleError('Missing or duplicate runtime inventory rows')
    java_rows = [Path(row['path']) for row in rows if Path(row['path']).name == 'java']
    if len(java_rows) != 1:
        raise OracleError('Runtime inventory must bind one explicit Java executable')
    required = {edt, edt.parent / '1cedt.ini', edt.parent / 'configuration/config.ini',
                edt.parent / '1cedtcli-startup.txt', edt.parent.parent / 'Info.plist',
                edt.parent.parent / 'MacOS/1cedt',
                edt.parent / 'configuration/org.eclipse.equinox.simpleconfigurator/bundles.info',
                java_rows[0], java_rows[0].parent.parent / 'release'}
    if not required.issubset({Path(row['path']) for row in rows}):
        raise OracleError('Runtime inventory omits launcher configuration')
    for row in rows:
        file = Path(row['path'])
        if not file.is_absolute():
            raise OracleError('Runtime inventory paths must be absolute')
        no_links(file)
        if file.stat().st_size != row['bytes'] or digest(file) != row['sha256']:
            raise OracleError(f'Runtime inventory differs: {file}')
    return data


def select_vm(installation, library=None):
    java = next(Path(row['path']) for row in installation['runtime_files'] if Path(row['path']).name == 'java')
    if library is None:
        return java
    library = library.absolute()
    no_links(library)
    if library != java.parent.parent / 'lib/server/libjvm.dylib':
        raise OracleError('Explicit Server JVM must belong to the bound Java home')
    if str(library) not in {row['path'] for row in installation['runtime_files']}:
        raise OracleError('Selected JVM library is absent from installation inventory')
    return library


def bind_schema(path, installation, build):
    data = json.loads(path.read_text(encoding='utf-8'))
    expected_release = '.'.join(build.split('.')[:3]) + '+' + build.split('.')[-1]
    baseline = json.loads(MODEL_BASELINE.read_text(encoding='utf-8'))
    if data.get('schema_version') != 1 or data.get('declared_installed_release') != expected_release:
        raise OracleError('Schema evidence release differs from tested EDT build')
    if data.get('baseline_sha256') != digest(MODEL_BASELINE):
        raise OracleError('Schema evidence does not bind the preserved legacy model inventory')
    rows = data.get('bundles', [])
    expected = {row['symbolicName'] for row in baseline['bundles']}
    if len(rows) != len(expected) or {row['symbolicName'] for row in rows} != expected:
        raise OracleError('Incomplete or duplicate schema bundle accounting')
    files = installation['plugins']['files']
    for row in rows:
        candidates = [file for file in files if '/' not in file['path'] and
                      file['path'].startswith(row['symbolicName'] + '_') and file['path'].endswith('.jar')]
        if row['status'] == 'unresolved':
            if row.get('candidate_count') != len(candidates) or len(candidates) == 1:
                raise OracleError('Unresolved schema row disagrees with installed bundles')
        elif row['status'] == 'inspected':
            if len(candidates) != 1 or row.get('jar_sha256') != candidates[0]['sha256'] or row.get('jar_bytes') != candidates[0]['bytes']:
                raise OracleError('Schema bundle hash differs from bound installation')
        else:
            raise OracleError('Unknown schema evidence status')
    return data


@contextlib.contextmanager
def native_slot(run):
    # A persistent advisory lock is released by the OS on termination. The
    # planner still coordinates other harnesses that do not use this lock.
    path = LAB / 'tmp/edt-native-macos.lock'
    no_links(path)
    with path.open('a+b') as stream:
        try:
            fcntl.flock(stream, fcntl.LOCK_EX | fcntl.LOCK_NB)
        except BlockingIOError as error:
            raise OracleError('Another macOS EDT/native harness holds the lab slot') from error
        write_json(run / 'slot.json', {'lock': str(path), 'pid': os.getpid(),
                   'acquired_utc': time.strftime('%Y-%m-%dT%H:%M:%SZ', time.gmtime())})
        try:
            yield
        finally:
            fcntl.flock(stream, fcntl.LOCK_UN)


def command(run, label, argv, timeout):
    env = os.environ.copy()
    for key in ('JAVA_TOOL_OPTIONS', '_JAVA_OPTIONS', 'JDK_JAVA_OPTIONS'):
        env.pop(key, None)
    env.update({key: str(run / 'tmp') for key in ('TMPDIR', 'TEMP', 'TMP')})
    for key, directory in [('HOME', 'home'), ('XDG_CACHE_HOME', 'cache'),
                           ('XDG_CONFIG_HOME', 'config'), ('XDG_DATA_HOME', 'data'),
                           ('XDG_STATE_HOME', 'state')]:
        env[key] = str(run / directory)
    record = {'argv': [str(x) for x in argv], 'cwd': str(run),
              'environment_overrides': {k: env[k] for k in ('TMPDIR', 'TEMP', 'TMP',
                  'HOME', 'XDG_CACHE_HOME', 'XDG_CONFIG_HOME', 'XDG_DATA_HOME', 'XDG_STATE_HOME')},
              'started_utc': time.strftime('%Y-%m-%dT%H:%M:%SZ', time.gmtime()),
              'timeout_seconds': timeout}
    start = time.monotonic()
    try:
        with (run / f'{label}.stdout').open('xb') as out, (run / f'{label}.stderr').open('xb') as err:
            proc = subprocess.Popen(record['argv'], cwd=run, env=env,
                                    stdin=subprocess.DEVNULL, stdout=out, stderr=err,
                                    start_new_session=True)
            try:
                record['exit_code'] = proc.wait(timeout=timeout)
            except BaseException as error:
                record['timed_out' if isinstance(error, subprocess.TimeoutExpired) else 'interrupted'] = True
                # Only the new process group created by this invocation.
                try:
                    os.killpg(proc.pid, signal.SIGKILL)
                except ProcessLookupError:
                    pass
                record['exit_code'] = proc.wait(timeout=30)
                if not isinstance(error, subprocess.TimeoutExpired):
                    raise
    except OSError as error:
        record['launch_error'] = str(error)
        raise
    finally:
        record['duration_seconds'] = round(time.monotonic() - start, 3)
        if '-data' in record['argv']:
            workspace = Path(record['argv'][record['argv'].index('-data') + 1])
            for name in ('.log', '1cedtcli.log', '1cedtcli-shutdown-hook.log'):
                log = workspace / '.metadata' / name
                if log.is_file():
                    with log.open('rb') as source, (run / f"{label}.workspace-{name.lstrip('.')}").open('xb') as target:
                        shutil.copyfileobj(source, target)
        write_json(run / f'{label}.command.json', record)
    if record.get('timed_out') or record['exit_code']:
        raise OracleError(f'{label} failed; preserved command/stdout/stderr in {run}')
    return (run / f'{label}.stdout').read_bytes()


def capture_diagnostics(run, labels):
    result = command_diagnostics(run, labels)
    # Mac EDT also emits mixed-case "[Fatal Error]". Preserve every raw
    # location without changing the shared historical Windows scanner.
    for label, row in result.items():
        lines = {(item['file'], item['line']): item for item in row['error_lines']}
        for suffix in ('stdout', 'stderr', 'workspace-log', 'workspace-1cedtcli.log', 'workspace-1cedtcli-shutdown-hook.log'):
            path = run / f'{label}.{suffix}'
            if not path.is_file():
                continue
            for number, line in enumerate(path.read_bytes().decode('utf-8', errors='replace').splitlines(), 1):
                if re.search(r'\b(?:error|fatal)\b|^exception in thread ', line, re.IGNORECASE):
                    lines[(path.name, number)] = {'file': path.name, 'line': number, 'text': line}
        row['error_lines'] = list(lines.values())
        row['error_count'] = len(lines)
    return result


def capture(args):
    if sys.platform != 'darwin':
        raise OracleError('Native capture requires macOS')
    project, run = lab_path(args.project), lab_path(args.run)
    args.edt, args.ours, args.native = (path.absolute() for path in (args.edt, args.ours, args.native))
    lab_path(args.ours)
    lab_path(args.edt)  # #358 supplies a copied external installation, never the internal app.
    disjoint([project, run])
    disjoint([run, installation_root(args.edt)])
    for exe in (args.edt, args.ours, args.native):
        no_links(exe.absolute())
        if not exe.is_file() or not os.access(exe, os.X_OK):
            raise OracleError(f'Missing executable: {exe}')
    require_project(project, args.runtime)
    # Authentic EDT input must not contain our stored reverse-conversion data.
    if any(path.name == '.ibcmd-provenance' for path in project.rglob('*')):
        raise OracleError('Authentic project contains ibcmd-rs provenance')
    if run.exists():
        raise OracleError('Run must be fresh; existing evidence is never replaced')
    run.mkdir(parents=True, exist_ok=False)
    for directory in ('tmp', 'home', 'cache', 'config', 'data', 'state'):
        (run / directory).mkdir()
    source = Path(__file__).read_bytes()
    (run / 'harness-source.py').write_bytes(source)
    (run / 'oracle-source.py').write_bytes(Path(__file__).with_name('oracle.py').read_bytes())
    write_json(run / 'invocation.json', {**{k: str(v) if isinstance(v, Path) else v
               for k, v in vars(args).items()}, 'harness_sha256': digest(run / 'harness-source.py'),
               'oracle_sha256': digest(run / 'oracle-source.py')})
    before = snapshot(project)
    installation = bind_installation(args.installation, args.edt)
    selected_vm = select_vm(installation, getattr(args, 'jvm_library', None))
    write_json(run / 'installation-bound.json', installation)
    installation_hash = digest(args.installation)
    lab_path(args.schema_evidence)
    bind_schema(args.schema_evidence, installation, args.edt_build)
    schema_hash = digest(args.schema_evidence)
    (run / 'schema-evidence.json').write_bytes(args.schema_evidence.read_bytes())
    tools = {name: {'path': str(path), 'sha256': digest(path)}
             for name, path in [('edt', args.edt), ('ours', args.ours), ('native', args.native)]}
    write_json(run / 'project-before.json', before)
    write_json(run / 'tools-before.json', tools)
    configuration = args.edt.parent / 'configuration'
    configuration_before = None
    configuration_copy_before = None
    try:
        with native_slot(run):
            native = ['/usr/bin/arch', '-x86_64', str(args.native)]
            if not configuration.is_dir():
                raise OracleError('Copied EDT installation lacks Eclipse configuration')
            configuration_before = snapshot(configuration)
            write_json(run / 'configuration-source-before.json', configuration_before)
            shutil.copytree(configuration, run / 'edt-configuration')
            configuration_copy_before = snapshot(run / 'edt-configuration')
            write_json(run / 'configuration-copy-before.json', configuration_copy_before)
            if not inventory_equal(configuration_before, configuration_copy_before):
                raise OracleError('Copied Eclipse configuration differs before launch')
            # Mac 1cedtcli is a custom launcher: generic Eclipse switches
            # belong in its -ini-file, not on the CLI (verified by #358).
            original_ini = (args.edt.parent / '1cedt.ini').read_text(encoding='utf-8')
            original_ini = ''.join(line for line in original_ini.splitlines(keepends=True)
                                   if not line.startswith('-Dosgi.debug='))
            if original_ini.count('-vmargs') != 1:
                raise OracleError('Expected one JVM section in pinned EDT INI')
            launcher, vmargs = original_ini.split('-vmargs', 1)
            ini = run / '1cedt.ini'
            launcher = launcher.replace('../Eclipse/', str(args.edt.parent) + '/')
            ini.write_text(launcher + '-vm\n' + str(selected_vm) + '\n-configuration\n' +
                str(run / 'edt-configuration') + '\n-nl\nen_US\n-user\n' + str(run / 'user') +
                '\n-vmargs' + vmargs, encoding='utf-8')
            write_json(run / 'launch-ini.json', {'path': str(ini), 'sha256': digest(ini), 'selected_vm': str(selected_vm), 'selection_sha256': digest(selected_vm)})
            edt = [str(args.edt), '-ini-file', str(ini), '-data', str(run / 'workspace'),
                   '-timeout', str(args.timeout), '-vmargs', f'-Xmx{args.heap_gib}g',
                   f'-Duser.home={run / "home"}', f'-Djava.io.tmpdir={run / "tmp"}', '-command']
            versions = {
                'native': command(run, 'native-version', native + ['--version'], 60).decode('utf-8').strip(),
                'ours': command(run, 'ours-version', [args.ours, '--version'], 60).decode('utf-8').strip(),
                'edt': command(run, 'edt-version', edt + ['version'], 120).decode('utf-8').strip()}
            write_json(run / 'versions.json', versions)
            if versions['native'] != args.native_build or versions['edt'] != args.edt_build:
                raise OracleError('Exact installed tool version mismatch')
            copied = run / 'project-copy'
            shutil.copytree(project, copied)
            if not inventory_equal(before, snapshot(copied)):
                raise OracleError('Disposable project copy differs from authentic input')
            baseline = run / 'edt-xml'
            command(run, 'edt-export', edt + ['export', '--project', str(copied),
                    '--configuration-files', str(baseline)], args.timeout)
            require_xml(baseline, require_dump_info=False)
            if not inventory_equal(before, snapshot(copied)):
                raise OracleError('Installed EDT modified the project during export')
            write_json(run / 'edt-xml-before.json', snapshot(baseline))
            common = [f'--db-path={run / "database"}', f'--data={run / "native-data"}',
                      f'--temp={run / "tmp"}']
            command(run, 'native-create', native + ['infobase', 'create', *common,
                    '--create-database', '--locale=ru_RU'], args.timeout)
            command(run, 'native-import', native + ['infobase', 'config', 'import', *common,
                    str(baseline)], args.timeout)
            reference = run / 'native-xml'
            command(run, 'native-export', native + ['infobase', 'config', 'export', *common,
                    '--threads=2', str(reference)], args.timeout)
            require_xml(reference)
            native_inventory = snapshot(reference)
            write_json(run / 'native-xml.json', native_inventory)
            ours = run / 'ours-xml'
            command(run, 'convert', [args.ours, 'convert', project, ours,
                    '--source-format', 'edt', '--target-format', 'xml',
                    '--source-profile', args.source_profile,
                    '--target-profile', f'xml-{args.xml_version}',
                    '--report', run / 'convert.report.json'], args.timeout)
            require_xml(ours, require_dump_info=False)
            ours_inventory = snapshot(ours)
            write_json(run / 'ours-xml.json', ours_inventory)
            report = comparison(ours_inventory, native_inventory)
            write_json(run / 'comparison.json', report)
            native_after = snapshot(reference)
            write_json(run / 'native-xml-after.json', native_after)
            if native_inventory != native_after:
                raise OracleError('Native reference changed during conversion')
            if json.loads((run / 'edt-xml-before.json').read_text()) != snapshot(baseline):
                raise OracleError('EDT XML changed during native capture')
            diagnostics = capture_diagnostics(run, ['edt-version', 'edt-export'])
            write_json(run / 'edt-diagnostics.json', diagnostics)
            observed_errors = has_error_diagnostics(diagnostics.values())
            result = {'status': ('COMPATIBILITY_PROBE_MATCH' if report['configuration_content']['equal'] else 'COMPATIBILITY_PROBE_DIFFERENT')
                        if getattr(args, 'compatibility_probe', False) else (('CONTENT_MATCH_WITH_SDK_ERRORS' if observed_errors else 'PASS') if report['configuration_content']['equal'] else 'FAIL'),
                'strict_equal': report['strict']['equal'], 'configuration_content_equal': report['configuration_content']['equal'],
                'storage_inventory_exclusion': 'ConfigDumpInfo.xml; full original rows retained in comparison.json',
                'scope': 'Same authentic EDT project, installed EDT export, fresh native file DB create/import/export; no activation claim',
                'corpus_label': args.corpus_label,
                'installed_edt_validation': 'NOT_EVALUATED; content comparison is not clean SDK validation',
                'observed_edt_errors': observed_errors, 'selected_vm': str(selected_vm),
                'configuration_source_sha256': configuration_before['tree_sha256'],
                'configuration_copy_initial_sha256': configuration_copy_before['tree_sha256'],
                'versions': versions, 'source_profile': args.source_profile, 'runtime': args.runtime,
                'xml_version': args.xml_version, 'schema_evidence_sha256': schema_hash,
                'installation_manifest_sha256': installation_hash, 'counts': report['counts']}
    except BaseException as error:
        write_json(run / 'failure.json', {'status': 'FAIL', 'error': str(error), 'type': type(error).__name__})
        raise
    finally:
        custody_errors = {}
        def custody_snapshot(path, label, started):
            if not started and not path.exists():
                return None
            try:
                return snapshot(path)
            except (OracleError, OSError) as error:
                custody_errors[label] = str(error)
                return None
        configuration_after = custody_snapshot(configuration, 'source', configuration_before is not None)
        configuration_copy_after = custody_snapshot(run / 'edt-configuration', 'copy', configuration_copy_before is not None)
        write_json(run / 'configuration-source-after.json', configuration_after)
        write_json(run / 'configuration-copy-after.json', configuration_copy_after)
        configuration_unchanged = configuration_before == configuration_after and 'source' not in custody_errors
        custody_complete = not custody_errors and (configuration_copy_before is None or configuration_copy_after is not None)
        write_json(run / 'configuration-custody.json', {
            'source_unchanged': configuration_unchanged, 'complete': custody_complete, 'errors': custody_errors,
            'copy_changes': compare_tree_snapshots(configuration_copy_before, configuration_copy_after)
                if configuration_copy_before is not None and configuration_copy_after is not None else None,
            'scope': 'Full configuration source and copied startup bytes; private Eclipse cache changes retained without normalization'})
        after = snapshot(project)
        write_json(run / 'project-after.json', after)
        changed = [name for name, path in [('edt', args.edt), ('ours', args.ours), ('native', args.native)]
                   if digest(path) != tools[name]['sha256']]
        installation_after = bind_installation(args.installation, args.edt)
        installation_unchanged = installation_after == installation and digest(args.installation) == installation_hash
        schema_unchanged = digest(args.schema_evidence) == schema_hash
        unchanged = before == after and not changed and installation_unchanged and schema_unchanged and configuration_unchanged and custody_complete
        write_json(run / 'immutability.json', {'project_unchanged': before == after, 'changed_tools': changed,
            'installation_unchanged': installation_unchanged, 'schema_evidence_unchanged': schema_unchanged,
            'configuration_source_unchanged': configuration_unchanged, 'configuration_custody_complete': custody_complete})
        if not unchanged:
            # A comparison verdict alone is never acceptance; this terminal
            # receipt is mandatory even after failed conversion/native commands.
            raise OracleError('Source project or executable changed during capture')
    write_json(run / 'result.json', result)
    return 0 if report['configuration_content']['equal'] else 1


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ('project', 'run', 'edt', 'ours', 'native', 'installation', 'schema-evidence'):
        parser.add_argument(f'--{name}', type=Path, required=True)
    parser.add_argument('--edt-build', required=True, help='Exact CLI build; never inferred from a profile')
    parser.add_argument('--source-profile', required=True, help='Explicit supported EDT release/XML profile')
    parser.add_argument('--jvm-library', type=Path, help='Explicit hash-bound Java-home lib/server/libjvm.dylib; default executable selection preserved')
    parser.add_argument('--corpus-label', required=True, help='Actual supplied corpus; empty controls never establish BSP acceptance')
    parser.add_argument('--compatibility-probe', action='store_true',
                        help='Explicitly compare a different tool/model release; never emit PASS')
    parser.add_argument('--native-build', required=True)
    parser.add_argument('--runtime', required=True)
    parser.add_argument('--xml-version', choices=('2.20', '2.21'), required=True)
    parser.add_argument('--timeout', type=int, default=21600)
    parser.add_argument('--heap-gib', type=int, default=8)
    args = parser.parse_args()
    if not re.fullmatch(r'\d+\.\d+\.\d+\.\d+', args.edt_build) or args.timeout < 1 or args.heap_gib < 1:
        parser.error('Exact numeric CLI build and positive resource settings required')
    release = '.'.join(args.edt_build.split('.')[:3])
    if not re.fullmatch(r'edt-\d+\.\d+\.\d+-xml-' + re.escape(args.xml_version), args.source_profile):
        parser.error('Explicit named source model/XML profile required')
    if args.source_profile != f'edt-{release}-xml-{args.xml_version}' and not args.compatibility_probe:
        parser.error('Source profile must explicitly match the named EDT release and XML version')
    try:
        return capture(args)
    except (OracleError, OSError, ValueError, subprocess.SubprocessError) as error:
        print(f'FAIL: {error}', file=sys.stderr)
        return 1


if __name__ == '__main__':
    sys.exit(main())
