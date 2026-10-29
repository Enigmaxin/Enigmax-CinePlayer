from pathlib import Path
import json, subprocess, sys
root=Path(__file__).resolve().parents[2]
dest=root/'verificacion/muestras/audio'
dest.mkdir(parents=True,exist_ok=True)
ff=Path(sys.argv[1]).resolve() if len(sys.argv)>1 else (root.parents[1]/'work/tools/msys64/ucrt64/bin/ffmpeg.exe').resolve()
profiles=[
 ('aac','aac','stereo',[]),('ac3','ac3','5.1',[]),('eac3','eac3','5.1',[]),
 ('dts','dca','5.1',['-strict','-2']),('truehd','truehd','5.1',['-strict','-2']),
 ('flac24','flac','7.1',['-sample_fmt','s32']),('alac24','alac','5.1',['-sample_fmt','s32p']),
 ('pcm16','pcm_s16le','stereo',[]),('pcm24','pcm_s24le','5.1',[]),('pcmfloat','pcm_f32le','7.1',[]),
 ('mp2','mp2','stereo',[]),('mp3','libmp3lame','stereo',[]),('opus','libopus','stereo',[]),
 ('opus51','libopus','5.1',[]),('vorbis','libvorbis','stereo',[]),
 ('wavpack','wavpack','5.1',[]),('tta','tta','stereo',[]),('wma2','wmav2','stereo',[])]
results=[]
for name,codec,layout,extra in profiles:
    channels={'stereo':2,'5.1':6,'7.1':8}[layout]
    audio='aevalsrc='+ '|'.join(f'0.04*sin(2*PI*{220+110*i}*t)' for i in range(channels))+f':s=48000:d=4:c={layout}'
    target=dest/(name+'.mkv')
    if not target.exists():
        args=[str(ff),'-hide_banner','-loglevel','error','-nostdin','-n','-f','lavfi','-i','color=0x172129:s=160x90:r=24:d=4',
              '-f','lavfi','-i',audio,'-c:v','ffv1','-threads','1','-c:a',codec,*extra,'-t','4',str(target)]
        p=subprocess.run(args,capture_output=True,text=True,timeout=45)
        if p.returncode: raise RuntimeError(name+': '+p.stderr)
    results.append({'name':name,'codec':codec,'layout':layout,'channels':channels})
    print(name,layout,'OK',flush=True)
(dest/'matrix.json').write_text(json.dumps(results,indent=2),encoding='utf8')
