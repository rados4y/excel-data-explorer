"""Regenerate the curated sample workbooks and PDFs with Python's standard library.

Configuration files and documentation are authored separately and are never rewritten.
All companies, amounts and documents below are synthetic.
"""
import argparse
import json
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'fixtures'))
from generate_demo_data import workbook, pdf

OFFERS = ['OfferID', 'Customer', 'Description', 'Amount', 'Status', 'CreatedAt']
INVOICES = ['InvoiceID', 'OfferID', 'InvoiceDate', 'Amount', 'PDF']
PROTOCOLS = ['ProtocolID', 'OfferID', 'Type', 'PDF']
SAMPLES = ['01-offers-and-finance', '02-alternate-layout', '03-namespaces',
           '04-documents-and-orphans', '05-sync-recovery']


def manifest(folder, workspace, counts, **extra):
    result = {'workspace_id': workspace, 'record_counts': counts, **extra}
    (folder / 'expected-results.json').write_text(json.dumps(result, indent=2) + '\n', encoding='utf-8')


def document(folder, name):
    directory = folder / 'documents'
    directory.mkdir(parents=True, exist_ok=True)
    pdf(directory / name, 'Synthetic demo - ' + name)


def generate(root):
    for name in SAMPLES:
        (root / name / 'data').mkdir(parents=True, exist_ok=True)
        (root / name / 'documents').mkdir(parents=True, exist_ok=True)

    folder = root / SAMPLES[0]
    offers = [OFFERS,
        ['OFF-001', 'Harbor Works', 'Warehouse lighting upgrade', '12500.00', 'Active', '2026-03-01'],
        ['OFF-002', 'Pine Labs', 'Laboratory ventilation', '8420.50', 'Draft', '2026-03-03'],
        ['OFF-003', 'Beacon Health', 'Clinic access control', '18900.00', 'Active', '2026-03-05'],
        ['OFF-004', 'Cedar Services', 'Reception renovation', '6200.75', 'Completed', '2026-03-07'],
        ['OFF-005', 'Atlas Foods', 'Cold store monitoring', '23450.20', 'Active', '2026-03-09'],
        ['OFF-006', 'Meridian Studio', 'Acoustic treatment', '4700.00', 'Draft', '2026-03-11']]
    invoices = [INVOICES,
        ['INV-1001', 'OFF-001', '2026-03-10', '5000.00', 'invoice-1001.pdf'],
        ['INV-1045', 'OFF-001', '2026-03-15', '7500.00', 'invoice-1045.pdf'],
        ['INV-2001', 'OFF-002', '2026-03-16', '8420.50', None],
        ['INV-3001', 'OFF-003', '2026-03-17', '9000.00', None],
        ['INV-3002', 'OFF-003', '2026-03-20', '9900.00', None],
        ['INV-4001', 'OFF-004', '2026-03-19', '6200.75', None],
        ['INV-5001', 'OFF-005', '2026-03-21', '23450.20', None],
        ['INV-6001', 'OFF-006', '2026-03-22', '4700.00', None]]
    protocols = [PROTOCOLS,
        ['PR-022', 'OFF-001', 'Acceptance', 'protocol-022.pdf'],
        ['PR-023', 'OFF-003', 'Handover', None],
        ['PR-024', 'OFF-004', 'Acceptance', None]]
    workbook(folder / 'data/offers.xlsx', [('Offers', offers)])
    workbook(folder / 'data/finance.xlsx', [('Invoices', invoices), ('Protocols', protocols)])
    for name in ['invoice-1001.pdf', 'invoice-1045.pdf', 'protocol-022.pdf']:
        document(folder, name)
    manifest(folder, 'sample-offers', {'offer': 6, 'invoice': 8, 'protocol': 3},
        document_status_counts={'available': 3}, orphan_count=0,
        relationships=[{'namespace': 'primary', 'parent': 'OFF-001', 'relation': 'offer_invoices', 'children': ['INV-1001', 'INV-1045']},
                       {'namespace': 'primary', 'parent': 'OFF-001', 'relation': 'offer_protocols', 'children': ['PR-022']}])

    folder = root / SAMPLES[1]
    workbook(folder / 'data/orders.xlsx', [('Sales Export', [
        ['TotalNet', 'CustomerName', 'OrderedOn', 'OrderNumber', 'SignedOff'],
        ['123.4500', 'Acme Solar', '2026-04-01', '0007', 'true'],
        ['0.1000', 'Birch Engineering', '2026-04-02', '0008', 'false'],
        ['2500.9900', 'Copper Design', '2026-04-03', '0012', 'true']])])
    manifest(folder, 'sample-layout', {'order': 3}, document_status_counts={}, orphan_count=0,
        keys=['0007', '0008', '0012'], first_record={'code': '0007', 'amount': '123.45', 'approved': True, 'ordered_at': '2026-04-01'})

    folder = root / SAMPLES[2]
    for region, customer, amount in [('north', 'North Campus', '12000'), ('south', 'South Depot', '7400')]:
        prefix = region[0].upper()
        workbook(folder / f'data/{region}.xlsx', [
            ('Offers', [['OfferID', 'Project', 'Customer', 'Amount', 'Status'],
                ['OFF-001', customer, 'Harbor Works' if region == 'north' else 'Atlas Foods', amount, 'Active'],
                ['OFF-002', customer, 'Pine Labs' if region == 'north' else 'Cedar Services', '3600', 'Draft']]),
            ('Invoices', [INVOICES,
                [f'INV-{prefix}-001', 'OFF-001', '2026-04-10', amount, f'invoice-{region}-001.pdf'],
                [f'INV-{prefix}-002', 'OFF-002', '2026-04-11', '3600', f'invoice-{region}-002.pdf']])])
        for number in ['001', '002']:
            document(folder, f'invoice-{region}-{number}.pdf')
    manifest(folder, 'sample-namespaces', {'offer': 4, 'invoice': 4},
        document_status_counts={'available': 4}, orphan_count=0,
        relationships=[{'namespace': region, 'parent': 'OFF-001', 'relation': 'offer_invoices',
                        'children': [f'INV-{region[0].upper()}-001']} for region in ['north', 'south']])

    folder = root / SAMPLES[3]
    workbook(folder / 'data/offers.xlsx', [('Offers', [OFFERS,
        ['OFF-001', 'Harbor Works', 'Document availability examples', '1800', 'Active', '2026-04-01'],
        ['OFF-002', 'Cedar Services', 'Offer without linked documents', '700', 'Completed', '2026-04-02']])])
    workbook(folder / 'data/invoices.xlsx', [('Invoices', [INVOICES,
        ['INV-GOOD', 'OFF-001', '2026-04-10', '1000', 'invoice-present.pdf'],
        ['INV-MISSING', 'OFF-001', '2026-04-11', '500', 'invoice-missing.pdf'],
        ['INV-TRAVERSAL', 'OFF-001', '2026-04-12', '200', '../outside.pdf'],
        ['INV-EXE', 'OFF-001', '2026-04-13', '100', 'blocked.exe'],
        ['INV-ORPHAN', 'OFF-404', '2026-04-14', '25', None]])])
    document(folder, 'invoice-present.pdf')
    manifest(folder, 'sample-documents', {'offer': 2, 'invoice': 5},
        document_status_counts={'available': 1, 'missing': 1, 'blocked': 2}, orphan_count=1,
        relationships=[{'namespace': 'primary', 'parent': 'OFF-001', 'relation': 'offer_invoices',
                        'children': ['INV-EXE', 'INV-GOOD', 'INV-MISSING', 'INV-TRAVERSAL']}])

    folder = root / SAMPLES[4]
    scenarios = folder / 'scenarios'
    scenarios.mkdir(exist_ok=True)
    baseline = [OFFERS,
        ['OFF-001', 'Harbor Works', 'Initial warehouse project', '12500', 'Active', '2026-03-01'],
        ['OFF-002', 'Pine Labs', 'Initial laboratory project', '8420.50', 'Draft', '2026-03-03']]
    updated = [OFFERS,
        ['OFF-001', 'Harbor Works', 'Updated warehouse project', '15000', 'Active', '2026-03-01'],
        baseline[2],
        ['OFF-003', 'Beacon Health', 'Added in the next synchronization', '18900', 'Active', '2026-03-05']]
    invalid = [OFFERS, updated[1], ['OFF-001', 'Duplicate key', 'This workbook must be rejected', '1', 'Draft', '2026-03-06']]
    workbook(folder / 'data/offers.xlsx', [('Offers', baseline)])
    for name, rows in [('baseline', baseline), ('updated', updated), ('invalid', invalid)]:
        workbook(scenarios / f'{name}.xlsx', [('Offers', rows)])
    # Reset the demo's offline toggle if a user explicitly requests regeneration.
    (folder / 'data/offers.xlsx.offline').unlink(missing_ok=True)
    manifest(folder, 'sample-sync', {'offer': 2}, document_status_counts={}, orphan_count=0,
        scenarios={'baseline_count': 2, 'updated_count': 3, 'invalid_after_updated_count': 3,
                   'offline_after_updated_count': 3, 'recovered_count': 2})
    print(json.dumps({'samples': SAMPLES, 'output': str(root)}))


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output', type=Path, default=Path(__file__).resolve().parent)
    generate(parser.parse_args().output)
