"""Test deterministic generation and the explicit demo scenario helper."""
import contextlib
import io
from pathlib import Path
import shutil
import tempfile
import unittest

from generate_samples import generate
from apply_scenario import apply


class DemoTests(unittest.TestCase):
    def test_generation_is_byte_for_byte_deterministic(self):
        with tempfile.TemporaryDirectory() as directory:
            first, second = Path(directory) / 'first', Path(directory) / 'second'
            with contextlib.redirect_stdout(io.StringIO()):
                generate(first)
                generate(second)
            files = {p.relative_to(first): p.read_bytes() for p in first.rglob('*') if p.is_file()}
            self.assertEqual(files, {p.relative_to(second): p.read_bytes() for p in second.rglob('*') if p.is_file()})
            self.assertEqual(len(list(first.rglob('*.xlsx'))), 12)

    def test_offline_toggle_and_recovery_do_not_overwrite_a_saved_sample(self):
        with tempfile.TemporaryDirectory() as directory:
            folder = Path(directory) / 'sample'
            shutil.copytree(Path(__file__).parent / '05-sync-recovery', folder)
            with contextlib.redirect_stdout(io.StringIO()):
                apply(folder, 'updated')
                updated = (folder / 'data/offers.xlsx').read_bytes()
                apply(folder, 'offline')
                self.assertFalse((folder / 'data/offers.xlsx').exists())
                with self.assertRaises(ValueError):
                    apply(folder, 'baseline')
                apply(folder, 'online')
                self.assertEqual((folder / 'data/offers.xlsx').read_bytes(), updated)
                apply(folder, 'baseline')
                self.assertEqual((folder / 'data/offers.xlsx').read_bytes(), (folder / 'scenarios/baseline.xlsx').read_bytes())


if __name__ == '__main__':
    unittest.main()
