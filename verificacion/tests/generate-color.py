from pathlib import Path
import array,subprocess,sys
app=Path(__file__).resolve().parents[2];ff=Path(sys.argv[1]).resolve();w,h=512,128
for name,full,hdr in [('limited',False,False),('full',True,False),('hdr',False,True)]:
 vals=[]
 for x in range(w):
  t=x/(w-1)
  if hdr:
   nits=4000*t
   p=(nits/10000)**(2610/16384)
   code=((3424/4096+(2413/128)*p)/(1+(2392/128)*p))**(2523/32)
   v=round(64+876*code)
  else:v=round(t*1023) if full else round(64+t*876)
  vals.append(v)
 data=array.array('H',vals*h+[512]*(w*h//2)).tobytes()
 dst=app/f'verificacion/muestras/color-{name}.mkv'
 args=[str(ff),'-v','error','-y','-f','rawvideo','-pixel_format','yuv420p10le','-video_size',f'{w}x{h}','-framerate','1','-color_range','pc' if full else 'tv','-i','pipe:0','-frames:v','1','-c:v','libx265','-preset','ultrafast','-x265-params',f'lossless=1:pools=2:frame-threads=1:log-level=error:colorprim={9 if hdr else 1}:transfer={16 if hdr else 1}:colormatrix={9 if hdr else 1}:range={"full" if full else "limited"}','-color_range','pc' if full else 'tv','-colorspace','bt2020nc' if hdr else 'bt709','-color_trc','smpte2084' if hdr else 'bt709','-color_primaries','bt2020' if hdr else 'bt709',str(dst)]
 subprocess.run(args,input=data,check=True,timeout=30)
 print(dst)
