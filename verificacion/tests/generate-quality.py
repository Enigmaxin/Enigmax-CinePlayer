from pathlib import Path
import subprocess,sys
app=Path(__file__).resolve().parents[2]
ff=Path(sys.argv[1]).resolve()
for name in ['fullsbs','hsbs','fullou','hou','pistas-qa','ass3d-fullsbs','ass3d-hou','depth-qa-pgs','uhd_hdr10']:
 src=app/f'verificacion/muestras/{name}.mkv';dst=app/f'verificacion/muestras/quality-{name}.mkv'
 if not dst.exists():
  subprocess.run([str(ff),'-v','error','-nostdin','-stream_loop','-1','-i',str(src),'-map','0','-c','copy','-t','60','-n',str(dst)],check=True,timeout=45)
 print(dst.name)
