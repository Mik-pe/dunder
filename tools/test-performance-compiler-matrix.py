#!/usr/bin/env python3
"""Focused runner checks; no real latency samples or compiler builds."""
import importlib.util
import itertools
import json
from pathlib import Path
import sys
import tempfile
from types import SimpleNamespace
import unittest
from unittest.mock import patch

sys.dont_write_bytecode = True
spec = importlib.util.spec_from_file_location('matrix', Path(__file__).with_name('performance-compiler-matrix.py'))
matrix = importlib.util.module_from_spec(spec)
spec.loader.exec_module(matrix)


class MatrixTests(unittest.TestCase):
    def test_dry_commands_never_read_latency_clock(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            runner = matrix.Runner(root, root, root, True)
            with patch.object(matrix.time, 'perf_counter_ns', side_effect=AssertionError('clock read')):
                out, elapsed, rss = runner.run([sys.executable, '-c', 'print(42)'], 'test dry', expected=b'42\n')
            self.assertEqual(out, b'42\n')
            self.assertIsNone(elapsed)
            self.assertIsNone(rss)
            record = json.loads((root / 'commands.jsonl').read_text())
            self.assertFalse(record['measured'])
            self.assertIsNone(record['elapsed_ms'])
            self.assertIsNone(record['max_rss_bytes'])
            self.assertEqual(record['exit_code'], 0)

    def test_failed_output_is_retained(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp); runner = matrix.Runner(root, root, root, True)
            with self.assertRaisesRegex(RuntimeError, 'command/output failure'):
                runner.run([sys.executable, '-c', 'print(7)'], 'wrong output', expected=b'8\n')
            self.assertEqual(json.loads((root / 'commands.jsonl').read_text())['stdout'], '7\n')

    def test_failed_launch_is_retained(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp); runner = matrix.Runner(root, root, root, True)
            with self.assertRaises(FileNotFoundError):
                runner.run([str(root / 'missing')], 'failed launch')
            record = json.loads((root / 'commands.jsonl').read_text())
            self.assertIsNone(record['exit_code'])
            self.assertIn('FileNotFoundError', record['error'])

    def test_thirty_orders_are_balanced(self):
        names = ['jobs1', 'jobs2', 'jobs4']
        orders = [tuple(matrix.perf.order(names, n)) for n in range(30)]
        for order in itertools.permutations(names):
            self.assertEqual(orders.count(order), 5)

    def test_paired_ratios_are_not_ratio_of_medians(self):
        rows = [dict(operation='build', case='hello', variant=v, round=n,
                     elapsed_ms=t, max_rss_bytes=100 + n)
                for v, times in [('baseline', [1, 10]), ('candidate', [2, 5])]
                for n, t in enumerate(times)]
        result = matrix.summarize(rows, 'baseline')[1]
        self.assertEqual(result['paired_ratio_median'], 1.25)
        self.assertEqual(result['paired_ratio_p95'], 2)
        self.assertEqual(result['median_over_reference'], 3.5 / 5.5)
        self.assertEqual(result['p95_ms'], 5)
        self.assertEqual(result['max_rss_bytes'], 101)

    def test_wait4_clock_and_rss_schema_without_measuring_a_process(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp); runner = matrix.Runner(root, root, root, False)
            process = SimpleNamespace(pid=12345, returncode=None)
            with patch.object(matrix.subprocess, 'Popen', return_value=process), \
                    patch.object(matrix.os, 'wait4', return_value=(12345, 0, SimpleNamespace(ru_maxrss=7))), \
                    patch.object(matrix.time, 'perf_counter_ns', side_effect=[10_000_000, 13_000_000]):
                _, elapsed, rss = runner.run(['fixture-process'], 'synthetic clock/RSS test', True)
            self.assertEqual(elapsed, 3)
            self.assertEqual(rss, 7 * (1 if sys.platform == 'darwin' else 1024))

    def test_timeout_kills_and_reaps_without_real_process_or_clock(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp); runner = matrix.Runner(root, root, root, True)
            process = SimpleNamespace(pid=12345, returncode=None)
            with patch.object(matrix.subprocess, 'Popen', return_value=process), \
                    patch.object(matrix.os, 'wait4', side_effect=[TimeoutError('forced'), (12345, 9, None)]), \
                    patch.object(matrix.os, 'killpg') as kill:
                with self.assertRaises(TimeoutError):
                    runner.run(['fixture-process'], 'synthetic timeout test')
            kill.assert_called_once_with(12345, matrix.signal.SIGKILL)
            record = json.loads((root / 'commands.jsonl').read_text())
            self.assertEqual(record['exit_code'], -9)
            self.assertIn('TimeoutError', record['error'])


if __name__ == '__main__':
    unittest.main()
