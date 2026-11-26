#!/usr/bin/env python3
"""Run only the recorded independent SDK consumer on owned emulator5588."""
import argparse
import hashlib
import json
from pathlib import Path
import subprocess
import time

ADB = '/Users/boristacyniak/Library/Android/sdk/platform-tools/adb'
SERIAL = 'emulator-5588'


def require(value, reason):
    if not value:
        raise SystemExit(reason)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--project', required=True, type=Path)
    parser.add_argument('--out', required=True, type=Path)
    parser.add_argument('--uninstall-after', action='store_true')
    args = parser.parse_args()
    require(not args.out.exists() and not args.out.is_symlink(), 'evidence output must be fresh')
    args.out.mkdir(parents=True)
    manifest_path = args.project / 'owned-consumer.json'
    commands, command_results, reports = [], [], {}
    manifest = {'runtime_executed': False, 'installed': False}
    app = None
    project_owned = False
    install_attempted = False
    success = False
    phase = 'validation'
    failure = None

    def adb(*arguments, check=True):
        command = [ADB, '-s', SERIAL, *arguments]
        commands.append(command)
        try:
            result = subprocess.run(command, stdout=subprocess.PIPE, stderr=subprocess.PIPE, timeout=30)
        except subprocess.TimeoutExpired:
            command_results.append({'index': len(commands) - 1, 'exit': None, 'timeout': True})
            raise SystemExit('owned adb command timed out')
        command_results.append({'index': len(commands) - 1, 'exit': result.returncode, 'timeout': False})
        if check:
            require(result.returncode == 0, 'owned adb command failed')
        return result

    try:
        manifest = json.loads(manifest_path.read_text())
        manifest['runtime_executed'] = False
        app = manifest['app_id']
        prefix = 'dev.hoyadb.consumer.p'
        suffix = app[len(prefix):]
        require(app.startswith(prefix) and len(suffix) == 16 and all(c in '0123456789abcdef' for c in suffix), 'invalid owned app identity')
        require(args.project.name.startswith('hoyadb-kotlin-consumer-'), 'invalid owned project')
        project_owned = True
        apk = args.project / 'consumer.apk'
        require(hashlib.sha256(apk.read_bytes()).hexdigest() == manifest['apk_sha256'], 'APK changed')
        expected_path = args.project / 'expected-cases.json'
        require(hashlib.sha256(expected_path.read_bytes()).hexdigest() == manifest['templates_sha256']['expected-cases.json'], 'consumer matrix changed')
        expected = json.loads(expected_path.read_text())
        require(set(expected) == {'full', 'seed', 'restart'} and all(isinstance(ids, list) and ids and len(ids) == len(set(ids)) and all(isinstance(value, str) for value in ids) for ids in expected.values()), 'invalid consumer matrix')
        phase = 'device'
        require(adb('shell', 'getprop', 'sys.boot_completed').stdout.strip() == b'1', 'owned emulator not booted')
        require(adb('shell', 'getconf', 'PAGE_SIZE').stdout.strip() == b'16384', 'actual device is not16KiB')
        existing = adb('shell', 'pm', 'path', app, check=False)
        require(not existing.stdout.strip(), 'task app unexpectedly already installed')
        phase = 'install'
        install_attempted = True
        manifest['installation_state'] = 'unknown'
        adb('install', '--no-streaming', str(apk))
        manifest['installed'] = True
        manifest['installation_state'] = 'installed'
        for mode in ['full', 'seed', 'restart']:
            phase = mode
            adb('shell', 'am', 'start', '-W', '-n', app + '/dev.hoyadb.consumer.MainActivity', '--es', 'mode', mode)
            deadline = time.monotonic() + 900
            report = None
            while time.monotonic() < deadline:
                reply = adb('shell', 'run-as', app, 'cat', 'no_backup/results-' + mode + '.json', check=False)
                if reply.returncode == 0:
                    try:
                        report = json.loads(reply.stdout)
                    except (json.JSONDecodeError, UnicodeError):
                        report = None
                    if report is not None:
                        break
                time.sleep(0.2)
            require(report is not None, 'consumer fixture watchdog expired')
            (args.out / (mode + '.json')).write_text(json.dumps(report, indent=2) + '\n')
            require(isinstance(report, dict) and report.get('mode') == mode and report.get('page_size') == 16384 and isinstance(report.get('cases'), list), 'invalid consumer result')
            require([case.get('id') for case in report['cases'] if isinstance(case, dict)] == expected[mode], 'consumer result matrix mismatch')
            require(all(isinstance(case.get('ok'), bool) for case in report['cases']), 'invalid consumer result')
            require(type(report.get('pid')) is int and report['pid'] > 0, 'invalid consumer process')
            reports[mode] = report
            # Seed intentionally retains an unfinished native scope; this exact
            # owned force-stop is the independently reopened crash boundary.
            adb('shell', 'am', 'force-stop', app)
            require(not adb('shell', 'pidof', app, check=False).stdout.strip(), 'owned app did not stop')
            require(type(report.get('failed')) is int and report['failed'] == 0 and all(case['ok'] for case in report['cases']), 'consumer behavior failure; preserved JSON')
        require(reports['seed']['pid'] != reports['restart']['pid'], 'restart did not use an independent process')
        success = True
        manifest['runtime_executed'] = True
        manifest['runtime_groups'] = sum(len(report['cases']) for report in reports.values())
    except BaseException as error:
        failure = {'phase': phase, 'kind': str(error) if isinstance(error, SystemExit) else type(error).__name__}
        raise
    finally:
        if install_attempted:
            try:
                adb('shell', 'am', 'force-stop', app, check=False)
                if args.uninstall_after and success:
                    adb('uninstall', app)
                    manifest['installed'] = False
                    manifest['installation_state'] = 'uninstalled'
            except BaseException:
                success = False
                manifest['runtime_executed'] = False
                failure = {'phase': 'cleanup', 'kind': 'owned app cleanup failed'}
        if project_owned:
            manifest_path.write_text(json.dumps(manifest, indent=2) + '\n')
        evidence = {'artifact': manifest, 'commands': commands, 'command_results': command_results,
            'serial': SERIAL, 'failure': failure,
            'process_restart_checked': 'seed' in reports and 'restart' in reports and reports['seed']['pid'] != reports['restart']['pid']}
        (args.out / 'runtime-manifest.json').write_text(json.dumps(evidence, indent=2) + '\n')
    require(success, 'owned consumer cleanup failed')
    print('owned independent consumer cases:', manifest['runtime_groups'])


if __name__ == '__main__':
    main()
