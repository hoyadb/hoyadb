"""SPEC-0043 / TC-0047: run_kotlin.py reports only real passes and stops only its own emulator.

Pure logic with fakes for adb and the device: no compiler, emulator or JVM runs here.
"""
import json
import sys
import tempfile
import types
import unittest
from pathlib import Path
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parent))
import run_kotlin  # noqa: E402


class Process:
    def __init__(self, alive):
        self.alive, self.killed = alive, False

    def poll(self):
        return None if self.alive else 1

    def wait(self, timeout=None):
        self.alive = False
        return 0

    def kill(self):
        self.killed, self.alive = True, False

    terminate = kill


class RunReport(unittest.TestCase):
    def run_guide(self, stdout, returncode=0):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            manifest, report = root / 'manifest.json', root / 'run.json'
            manifest.write_text('{}')
            report.write_text('{"exit": 0, "stale": true}')
            fake_manifest = {'run': {'dex': 'x.dex', 'jni': 'x.so', 'main_class': 'TodoGuideKt'}, 'artifacts': {}}
            device = {'ro.product.cpu.abi': 'x86_64'}
            adb = lambda *args, check=True, timeout=120: device.get(args[-1], '1')
            guide = types.SimpleNamespace(returncode=returncode, stdout=stdout, stderr='')
            order = []
            def server_probe():
                order.append('probe')
                return True
            def recording_adb(*args, **kwargs):
                order.append('adb')
                return adb(*args, **kwargs)
            with patch.multiple(run_kotlin, KOTLIN=root, MANIFEST=manifest, RUN_REPORT=report,
                                adb_server_running=server_probe,
                                validated_manifest=lambda: fake_manifest,
                                running_avd=lambda: (order.append('adb'), run_kotlin.AVD)[1], adb=recording_adb), \
                 patch.object(run_kotlin.subprocess, 'run', return_value=guide):
                try:
                    run_kotlin.run()
                    passed = True
                except SystemExit:
                    passed = False
            self.assertEqual(order[0], 'probe', 'the adb server is probed before any adb command')
            return passed, (json.loads(report.read_text()) if report.exists() else None)

    def test_no_report_without_the_completion_line(self):
        passed, report = self.run_guide('setup only\n')
        self.assertFalse(passed)
        self.assertIsNone(report)

    def test_no_report_after_a_failing_exit(self):
        passed, report = self.run_guide('kotlin tour: ok\n', returncode=1)
        self.assertFalse(passed)
        self.assertIsNone(report)

    def test_report_after_a_real_pass(self):
        passed, report = self.run_guide('kotlin tour: ok\n')
        self.assertTrue(passed)
        self.assertEqual(report['exit'], 0)
        self.assertEqual(report['completion_line'], 'kotlin tour: ok')
        self.assertIs(report['adb_server_already_running'], True)
        self.assertNotIn('stale', report)


class StopEmulator(unittest.TestCase):
    def stop(self, process, current):
        calls = []
        with patch.object(run_kotlin, 'running_avd', lambda: current), \
             patch.object(run_kotlin.subprocess, 'run', lambda args, **kw: calls.append(args)):
            run_kotlin.stop_emulator(process)
        return [call for call in calls if 'kill' in call]

    def test_kills_only_the_owned_emulator_it_started(self):
        self.assertEqual(len(self.stop(Process(alive=True), run_kotlin.AVD)), 1)

    def test_leaves_the_serial_alone_when_its_emulator_already_exited(self):
        self.assertEqual(self.stop(Process(alive=False), run_kotlin.AVD), [])

    def test_leaves_another_emulator_on_the_serial_alone(self):
        process = Process(alive=True)
        self.assertEqual(self.stop(process, 'Monstera_E2E'), [])
        self.assertTrue(process.killed, 'its own emulator process is still stopped')


if __name__ == '__main__':
    unittest.main()
