"""Deterministic OOXML/PDF fixtures. Python standard library only; no Excel needed."""
import argparse, json, zipfile
from pathlib import Path
from xml.sax.saxutils import escape

NS = 'http://schemas.openxmlformats.org/spreadsheetml/2006/main'
def column(n):
    out = ''
    while n:
        n, r = divmod(n - 1, 26); out = chr(65 + r) + out
    return out

def workbook(path, sheets):
    path.parent.mkdir(parents=True, exist_ok=True)
    with zipfile.ZipFile(path, 'w', zipfile.ZIP_DEFLATED) as z:
        def put(name, value):
            info = zipfile.ZipInfo(name, (2026, 1, 1, 0, 0, 0)); info.compress_type = zipfile.ZIP_DEFLATED
            z.writestr(info, value.encode())
        put('[Content_Types].xml', '<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/xl/workbook.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml"/>' + ''.join(f'<Override PartName="/xl/worksheets/sheet{i}.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml"/>' for i in range(1,len(sheets)+1)) + '</Types>')
        put('_rels/.rels', '<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="xl/workbook.xml"/></Relationships>')
        put('xl/workbook.xml', f'<workbook xmlns="{NS}" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"><sheets>' + ''.join(f'<sheet name="{escape(name)}" sheetId="{i}" r:id="rId{i}"/>' for i,(name,_) in enumerate(sheets,1)) + '</sheets></workbook>')
        put('xl/_rels/workbook.xml.rels', '<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">' + ''.join(f'<Relationship Id="rId{i}" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet" Target="worksheets/sheet{i}.xml"/>' for i in range(1,len(sheets)+1)) + '</Relationships>')
        for i,(_,rows) in enumerate(sheets,1):
            info = zipfile.ZipInfo(f'xl/worksheets/sheet{i}.xml', (2026,1,1,0,0,0)); info.compress_type = zipfile.ZIP_DEFLATED
            with z.open(info, 'w') as f:
                f.write(f'<worksheet xmlns="{NS}"><sheetData>'.encode())
                for rn,row in enumerate(rows,1):
                    cells = []
                    for cn,value in enumerate(row,1):
                        ref = f'{column(cn)}{rn}'
                        if isinstance(value,(int,float)): cells.append(f'<c r="{ref}"><v>{value}</v></c>')
                        elif value is not None: cells.append(f'<c r="{ref}" t="inlineStr"><is><t xml:space="preserve">{escape(str(value))}</t></is></c>')
                    f.write((f'<row r="{rn}">'+''.join(cells)+'</row>').encode())
                f.write(b'</sheetData></worksheet>')

def pdf(path, text):
    stream = f'BT /F1 18 Tf 50 750 Td ({text}) Tj ET'.encode()
    objects = [b'<< /Type /Catalog /Pages 2 0 R >>', b'<< /Type /Pages /Kids [3 0 R] /Count 1 >>', b'<< /Type /Page /Parent 2 0 R /MediaBox [0 0 595 842] /Resources << /Font << /F1 4 0 R >> >> /Contents 5 0 R >>', b'<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>', b'<< /Length '+str(len(stream)).encode()+b' >>\nstream\n'+stream+b'\nendstream']
    data = b'%PDF-1.4\n'; offsets = [0]
    for i,obj in enumerate(objects,1): offsets.append(len(data)); data += f'{i} 0 obj\n'.encode()+obj+b'\nendobj\n'
    xref = len(data); data += b'xref\n0 6\n0000000000 65535 f \n'+b''.join(f'{o:010} 00000 n \n'.encode() for o in offsets[1:])+f'trailer\n<< /Size 6 /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n'.encode()
    path.write_bytes(data)

def generate(root, scale=False, offers_count=None):
    data = root/'data'; docs = root/'documents'; docs.mkdir(parents=True,exist_ok=True)
    n = offers_count or (100000 if scale else 120)
    def offers():
        yield ['OfferID','Customer','Description','Amount','Status','CreatedAt']
        for i in range(1,n+1): yield [f'OFF-{i:03}', ['ABC Corp','XYZ Ltd','Northwind','Meridian Works'][(i-1)%4], f'Project {i:03} — equipment and installation', f'{i*125}.50', ['Active','Draft','Completed'][(i-1)%3], '2026-03-01']
    def invoices():
        yield ['InvoiceID','OfferID','InvoiceDate','Amount','PDF']
        yield ['INV-1001','OFF-001','2026-03-10','5000.00','invoice-1001.pdf']
        yield ['INV-1045','OFF-001','2026-03-15','7500.00','invoice-1045.pdf']
        for i in range(3,5*n+1 if scale else 2*n+1): yield [f'INV-{i:06}',f'OFF-{((i-3)%(n-1))+2:03}','2026-03-16','1250.25',f'invoice-{i:06}.pdf' if (i<50000 if scale else i==3) else None]
        yield ['INV-ORPHAN','OFF-UNKNOWN','2026-03-17','12.50',None]
    def protocols():
        yield ['ProtocolID','OfferID','Type','PDF']
        yield ['PR-022','OFF-001','Acceptance','protocol-022.pdf']
        for i in range(2,n+1 if scale else n//2+1): yield [f'PR-{i+100:06}',f'OFF-{i:03}','Handover',None]
    workbook(data/'offers.xlsx',[('Offers',offers())]); workbook(data/'finance.xlsx',[('Invoices',invoices()),('Protocols',protocols())])
    for name in ['invoice-1001.pdf','invoice-1045.pdf','protocol-022.pdf']: pdf(docs/name, name)
    workbook(data/'alternate.xlsx',[('Orders',[['Client','Code','Total'],['Acme','0007','123.4500'],['Beta','0008','0.10']])])
    manifest = {'offers':n,'invoices':5*n+1 if scale else 2*n+1,'protocols':n if scale else n//2,'offer_001':{'invoices':['INV-1001','INV-1045'],'protocols':['PR-022']},'documents_available':3,'documents_missing':min(5*n+1,50000)-3 if scale else 1,'orphans':1}
    (root/'expected-results.json').write_text(json.dumps(manifest,indent=2)+'\n')
    print(json.dumps(manifest))
if __name__ == '__main__':
    p=argparse.ArgumentParser(); p.add_argument('--output',type=Path,default=Path(__file__).parent/'demo'); p.add_argument('--benchmark',action='store_true'); p.add_argument('--offers',type=int); a=p.parse_args(); generate(a.output,a.benchmark,a.offers)
