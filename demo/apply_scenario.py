"""Change only the synthetic sync sample. Explorer itself never writes workbooks.
Usage: python demo/apply_scenario.py baseline|updated|invalid|offline|online
"""
import argparse
import os
from pathlib import Path
import shutil
import tempfile


def apply(folder, action):
    active = folder / 'data/offers.xlsx'
    offline = active.with_suffix('.xlsx.offline')
    if action == 'offline':
        if offline.exists():
            raise ValueError('The sample is already offline. Run online first.')
        active.rename(offline)
    elif action == 'online':
        if active.exists() or not offline.exists():
            raise ValueError('No offline workbook to restore, or active workbook already exists.')
        offline.rename(active)
    elif action in ('baseline', 'updated', 'invalid'):
        if offline.exists():
            raise ValueError('The sample is offline. Run online before applying another scenario.')
        source = folder / 'scenarios' / f'{action}.xlsx'
        # Atomic replacement models Excel's save/rename pattern without leaving a partial file.
        with tempfile.NamedTemporaryFile(dir=active.parent, suffix='.xlsx', delete=False) as handle:
            temporary = Path(handle.name)
        try:
            shutil.copyfile(source, temporary)
            os.replace(temporary, active)
        finally:
            temporary.unlink(missing_ok=True)
    else:
        raise ValueError('Unknown demo scenario')
    print(f'Synthetic sync sample: {action}. Use Sync now in Excel Data Explorer.')


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('action', choices=['baseline', 'updated', 'invalid', 'offline', 'online'])
    args = parser.parse_args()
    try:
        apply(Path(__file__).resolve().parent / '05-sync-recovery', args.action)
    except (OSError, ValueError) as error:
        parser.exit(1, f'{error}\n')
