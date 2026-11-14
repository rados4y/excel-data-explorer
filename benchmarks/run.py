"""Generate portable benchmark inputs and run the release-mode Rust engine.
Usage: python benchmarks/run.py --offers 100000 --output /local/ssd/ede-benchmark
Use a local volume with at least 8 GB free for the 700k-record case.
"""
import argparse,subprocess,json,os
from pathlib import Path
p=argparse.ArgumentParser();p.add_argument('--offers',type=int,default=100000);p.add_argument('--output',type=Path,default=Path('benchmark-work'));a=p.parse_args()
root=Path(__file__).resolve().parent.parent;out=a.output.resolve();out.mkdir(parents=True,exist_ok=True)
subprocess.run([os.sys.executable,str(root/'fixtures/generate_demo_data.py'),'--benchmark','--offers',str(a.offers),'--output',str(out/'fixtures')],check=True)
# Retain the repository's YAML shape without requiring a Python YAML package.
text=(root/'workspaces/demo-workspace.yaml').read_text().replace('id: demo,','id: benchmark,').replace('../fixtures/demo/data',str(out/'fixtures/data').replace('\\','/')).replace('../fixtures/demo/documents',str(out/'fixtures/documents').replace('\\','/')).replace('stable_file_check_ms: 1500','stable_file_check_ms: 10')
config=out/'benchmark.yaml';config.write_text(text)
subprocess.run(['node','scripts/run.mjs','cargo','run','--release','--manifest-path',str(root/'src-tauri/Cargo.toml'),'--bin','benchmark','--',str(config),str(out/'cache.sqlite'),str(out/'results.json')],check=True,cwd=root)
print('Results:',out/'results.json')
