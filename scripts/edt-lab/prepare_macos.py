#!/usr/bin/env python3
"""Retained installed-EDT discovery and native XML import/export on external macOS lab.

PREPARED is evidence capture only. This tool never declares converter acceptance.
"""
from __future__ import annotations
import argparse
import contextlib
import fcntl
import json
import os
from pathlib import Path
import re
import signal
import subprocess
import sys
import time

sys.dont_write_bytecode = True
from oracle import OracleError, digest, no_links, require_project, require_xml, snapshot, write_json

LAB = Path('/Volumes/Work/Dev/ibcmd-rs-lab')


def external(path):
    path = path.absolute()
    if any(part.is_symlink() for part in (path, *path.parents)):
        raise OracleError(f'Symlink refused, including dangling links: {path}')
    no_links(path)
    if '..' in path.parts or path == LAB or not path.is_relative_to(LAB):
        raise OracleError(f'Physical path below external lab required: {path}')
    return path



def require_disjoint_installation(run, edt):
    eclipse = edt.parent
    contents = eclipse.parent
    bundle = contents.parent
    if edt.name != '1cedtcli' or eclipse.name != 'Eclipse' or contents.name != 'Contents' \
            or bundle.suffix != '.app':
        raise OracleError('EDT CLI must have the physical .app/Contents/Eclipse/1cedtcli layout')
    # Eclipse is only one subtree: Resources, MacOS and bundle-root writes also
    # mutate the installation outside its immutable payload inventory.
    def contains(outer, inner):
        if inner.is_relative_to(outer):
            return True
        # Existing ancestors can name the same directory with different case on
        # macOS volumes; lexical ancestry alone does not establish disjointness.
        return outer.exists() and any(ancestor.exists() and ancestor.samefile(outer)
            for ancestor in (inner, *inner.parents))
    if contains(bundle, run) or contains(run, bundle):
        raise OracleError('Whole EDT app bundle and run must not overlap')


def file_record(path):
    no_links(path)
    return {'path': str(path), 'bytes': path.stat().st_size, 'sha256': digest(path)}


def installation(edt, java):
    eclipse = edt.parent
    app = eclipse.parent
    files = [edt, eclipse / '1cedt.ini', eclipse / 'configuration/config.ini',
             eclipse / '1cedtcli-startup.txt', eclipse / '.options', app / 'Info.plist', app / 'MacOS/1cedt',
             eclipse / 'configuration/org.eclipse.equinox.simpleconfigurator/bundles.info',
             java, java.parent.parent / 'release']
    return {'schema_version': 1, 'status': 'INVENTORIED_NOT_RUNTIME_VERIFIED',
            'edt_cli': str(edt), 'edt_cli_sha256': digest(edt),
            'plugins': snapshot(eclipse / 'plugins'),
            'runtime_files': [file_record(path) for path in files]}


def isolated_environment(run):
    env = dict(os.environ)
    for key in ('JAVA_TOOL_OPTIONS', '_JAVA_OPTIONS', 'JDK_JAVA_OPTIONS'):
        env.pop(key, None)
    overrides = {}
    for key, directory in [('HOME', 'home'), ('TMPDIR', 'tmp'), ('TEMP', 'tmp'), ('TMP', 'tmp'),
                           ('XDG_CACHE_HOME', 'cache'), ('XDG_CONFIG_HOME', 'config'),
                           ('XDG_DATA_HOME', 'data'), ('XDG_STATE_HOME', 'state')]:
        (run / directory).mkdir(exist_ok=True)
        overrides[key] = str(run / directory)
    env.update(overrides)
    return env, overrides


def command(run, label, argv, timeout, env, overrides):
    receipt = {'argv': [str(x) for x in argv], 'cwd': str(run),
               'environment_overrides': overrides, 'timeout_seconds': timeout,
               'started_utc': time.strftime('%Y-%m-%dT%H:%M:%SZ', time.gmtime())}
    started = time.monotonic()
    proc = None
    try:
        with (run / f'{label}.stdout').open('xb') as out, (run / f'{label}.stderr').open('xb') as err:
            proc = subprocess.Popen(receipt['argv'], cwd=run, env=env, stdin=subprocess.DEVNULL,
                                    stdout=out, stderr=err, start_new_session=True)
            try:
                receipt['exit_code'] = proc.wait(timeout=timeout)
            except BaseException:
                with contextlib.suppress(ProcessLookupError):
                    os.killpg(proc.pid, signal.SIGKILL)
                receipt['exit_code'] = proc.wait(timeout=30)
                raise
    except BaseException as error:
        receipt['error_type'] = type(error).__name__
        raise
    finally:
        receipt['duration_seconds'] = round(time.monotonic() - started, 3)
        write_json(run / f'{label}.command.json', receipt)
    if receipt['exit_code']:
        raise OracleError(f'{label} exit {receipt["exit_code"]}; see retained raw logs')
    return (run / f'{label}.stdout').read_bytes()


def capture(args):
    if sys.platform != 'darwin':
        raise OracleError('Installed capture requires macOS')
    args.run, args.edt = external(args.run), external(args.edt)
    args.java = args.java.absolute()
    no_links(args.java)
    jvm_library = getattr(args, 'jvm_library', None)
    if jvm_library is not None:
        args.jvm_library = jvm_library.absolute()
        no_links(args.jvm_library)
        if not args.jvm_library.is_file():
            raise OracleError('Selected JVM library must be an existing physical file')
    if args.run.exists() or not args.run.parent.is_dir():
        raise OracleError('Run must be new with existing external parent')
    require_disjoint_installation(args.run, args.edt)
    if not re.fullmatch(r'\d+\.\d+\.\d+\.\d+', args.edt_build):
        raise OracleError('Declare exact four-component installed build')
    if args.timeout < 1 or not 1 <= args.heap_gib <= 64:
        raise OracleError('Invalid timeout/heap')
    if args.mode == 'prepare':
        if args.native is None or args.corpus is None:
            raise OracleError('Preparation requires explicit native input and actual corpus label')
        args.native = external(args.native)
        if args.run.is_relative_to(args.native) or args.native.is_relative_to(args.run):
            raise OracleError('Native input/run overlap')
        require_xml(args.native)
        import xml.etree.ElementTree as ET
        if ET.parse(args.native / 'Configuration.xml').getroot().get('version') != args.xml_version:
            raise OracleError('Native XML root version differs from explicit profile')
    for tool in (args.edt, args.java):
        if not tool.is_file() or not os.access(tool, os.X_OK):
            raise OracleError(f'Missing executable: {tool}')
    args.run.mkdir()
    env, overrides = isolated_environment(args.run)
    for name in ('prepare_macos.py', 'oracle.py'):
        (args.run / name).write_bytes(Path(__file__).with_name(name).read_bytes())
    write_json(args.run / 'invocation.json', {**{k: str(v) if isinstance(v, Path) else v
               for k, v in vars(args).items()}, 'harness_sha256': digest(args.run / 'prepare_macos.py'),
               'oracle_sha256': digest(args.run / 'oracle.py')})
    native_before = None
    installation_before = None
    terminal = None
    slot = None
    try:
        lock = external(LAB / 'tmp/edt-native-macos.lock')
        slot = lock.open('a+b')
        try:
            fcntl.flock(slot, fcntl.LOCK_EX | fcntl.LOCK_NB)
        except BlockingIOError as error:
            raise OracleError('Another EDT/native capture owns the slot') from error
        ini = args.run / '1cedt.ini'
        original_ini = (args.edt.parent / '1cedt.ini').read_text()
        original_ini = original_ini.replace('../Eclipse/', str(args.edt.parent) + '/')
        original_ini = ''.join(line for line in original_ini.splitlines(keepends=True)
            if not line.startswith('-Dosgi.debug='))
        # CLI accepts -ini-file; generic Eclipse options belong inside that INI.
        launcher, vmargs = original_ini.split('-vmargs', 1)
        ini.write_text(launcher + '-vm\n' + str(args.jvm_library if jvm_library else args.java) + '\n-configuration\n' +
            str(args.edt.parent / 'configuration') + '\n-nl\nen_US\n-user\n' + str(args.run / 'user') +
            '\n-vmargs' + vmargs)
        installation_before = installation(args.edt, args.java)
        installation_before['runtime_files'].append(file_record(ini))
        if jvm_library:
            installation_before['runtime_files'].append(file_record(args.jvm_library))
        write_json(args.run / 'installation.json', installation_before)
        prefix = [str(args.edt), '-ini-file', str(ini), '-data', str(args.run / 'workspace'),
                  '-timeout', str(args.timeout), '-vmargs',
                  f'-Xmx{args.heap_gib}g', f'-Duser.home={args.run / "home"}',
                  f'-Djava.io.tmpdir={args.run / "tmp"}', '-command']
        def edt(label, words):
            return command(args.run, label, prefix + words, args.timeout, env, overrides)
        actual = edt('edt-version', ['version']).decode('utf-8').strip()
        write_json(args.run / 'version.json', {'expected': args.edt_build, 'actual': actual})
        if actual != args.edt_build:
            raise OracleError('Installed build mismatch')
        if args.mode == 'discover':
            if not getattr(args, 'version_only', False):
                edt('edt-help', ['help'])
                for name in ('import', 'export', 'validate'):
                    edt(f'edt-help-{name}', ['help', name])
            terminal = ('result.json', {'status': 'DISCOVERED', 'edt_build': actual,
                'note': 'Tool discovery is not converter or corpus acceptance'})
        else:
            native_before = snapshot(args.native)
            write_json(args.run / 'native-before.json', native_before)
            runtime = {'2.20': '8.3.27', '2.21': '8.5.1'}[args.xml_version]
            edt('edt-import-native', ['import', '--version', runtime, '--configuration-files',
                str(args.native), '--project-name', 'OracleConfiguration', '--build', 'true'])
            project = args.run / 'workspace/OracleConfiguration'
            require_project(project, runtime)
            write_json(args.run / 'authentic-project-before.json', snapshot(project))
            output = args.run / 'edt-native-xml'
            edt('edt-export-native', ['export', '--project-name', project.name,
                                    '--configuration-files', str(output)])
            require_xml(output, require_dump_info=False)
            write_json(args.run / 'authentic-project-after.json', snapshot(project))
            write_json(args.run / 'edt-native-xml.json', snapshot(output))
            terminal = ('prepared.json', {'status': 'PREPARED_NOT_ACCEPTED', 'corpus': args.corpus,
                'edt_build': actual, 'native': str(args.native), 'project': str(project),
                'edt_xml': str(output), 'xml_version': args.xml_version, 'runtime': runtime,
                'note': 'Raw diagnostics remain unresolved; no converter acceptance'})
    except BaseException as error:
        write_json(args.run / 'failure.json', {'status': 'FAIL', 'error': str(error), 'type': type(error).__name__})
        raise
    finally:
        try:
            try:
                if native_before is not None:
                    after = snapshot(args.native)
                    write_json(args.run / 'native-after.json', after)
                    if after != native_before:
                        raise OracleError('Native input changed')
                if installation_before is not None:
                    after = installation(args.edt, args.java)
                    after['runtime_files'].append(file_record(args.run / '1cedt.ini'))
                    if jvm_library:
                        after['runtime_files'].append(file_record(args.jvm_library))
                    write_json(args.run / 'installation-after.json', after)
                    if after != installation_before:
                        raise OracleError('Immutable installation payload changed')
            except BaseException as error:
                if not (args.run / 'failure.json').exists():
                    write_json(args.run / 'failure.json', {'status': 'FAIL', 'error': str(error),
                               'type': type(error).__name__})
                raise
        finally:
            if slot is not None:
                slot.close()
    if terminal is not None:
        write_json(args.run / terminal[0], terminal[1])


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('mode', choices=('discover', 'prepare'))
    for name in ('run', 'edt', 'java'):
        parser.add_argument('--' + name, type=Path, required=True)
    parser.add_argument('--edt-build', required=True)
    parser.add_argument('--jvm-library', type=Path, help='Explicit actual JVM library for Eclipse JNI selection; independently hash-bound')
    parser.add_argument('--version-only', action='store_true', help='Bounded installed version capture without repeated command help')
    parser.add_argument('--native', type=Path)
    parser.add_argument('--corpus')
    parser.add_argument('--xml-version', choices=('2.20', '2.21'), default='2.21')
    parser.add_argument('--timeout', type=int, default=7200)
    parser.add_argument('--heap-gib', type=int, default=4)
    args = parser.parse_args()
    def terminate(signum, frame):
        raise KeyboardInterrupt(f'Cancelled by signal {signum}')
    signal.signal(signal.SIGTERM, terminate)
    try:
        capture(args)
        print(f'CAPTURED_NOT_ACCEPTED: {args.run}')
        return 0
    except (OracleError, OSError, ValueError, subprocess.SubprocessError) as error:
        print(f'FAIL: {error}', file=sys.stderr)
        return 1


if __name__ == '__main__':
    raise SystemExit(main())
