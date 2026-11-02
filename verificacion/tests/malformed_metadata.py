"""Bounded, process-isolated mutations of generated MKV headers; never user media."""
import argparse,json,os,struct,subprocess,tempfile,time,tomllib
from pathlib import Path
app=Path(__file__).resolve().parents[2]
p=argparse.ArgumentParser();p.add_argument('--player',type=Path,required=True);p.add_argument('--phase',required=True);a=p.parse_args()
raw=(app/'verificacion/muestras/metadata-qa.mkv').read_bytes()
pos=raw.find(b'\x44\x89',0,16384)
assert pos>=0,'Duration element missing'
size=raw[pos+2];assert size in [0x84,0x88],hex(size)
width=size&0x7f;fmt='>d' if width==8 else '>f'
cases=[('valid',raw)]
for name,value in [('negative',-1.0),('nan',float('nan')),('infinite',float('inf')),('extreme',1e30 if width==4 else 1e300)]:
 data=bytearray(raw);data[pos+3:pos+3+width]=struct.pack(fmt,value);cases.append((name,bytes(data)))
for n in [1,16,64,256,1024]:cases.append(('truncated-'+str(n),raw[:n]))
version=tomllib.loads((app/'Cargo.toml').read_text(encoding='utf-8'))['package']['version']
out=app/f'verificacion/{version}/malformed-metadata';out.mkdir(parents=True,exist_ok=True);rows=[]
with tempfile.TemporaryDirectory(prefix='cineplayer-mkv-mutations-') as temp:
 for name,data in cases:
  source=Path(temp)/(name+'.mkv');source.write_bytes(data)
  env={k:v for k,v in os.environ.items() if not k.startswith('ENIGMAX_PROBE_')};env['ENIGMAX_PROBE_PLAY_SECONDS']='0.25'
  started=time.monotonic()
  try:
   r=subprocess.run([str(a.player.resolve()),'--probe',str(source),'original',str(out/(a.phase+'-'+name+'.png'))],env=env,stdout=subprocess.PIPE,stderr=subprocess.PIPE,timeout=20,creationflags=subprocess.CREATE_NO_WINDOW)
   row=dict(case=name,exit=r.returncode,seconds=round(time.monotonic()-started,3),terminated=True,stderr=r.stderr.decode(errors='replace')[-500:])
  except subprocess.TimeoutExpired:row=dict(case=name,terminated=False,timeout=True)
  rows.append(row);print(json.dumps(row),flush=True)
(out/(a.phase+'.json')).write_text(json.dumps(rows,indent=2),encoding='utf-8')
raise SystemExit(0 if all(r['terminated'] for r in rows) and rows[0]['exit']==0 else 1)
