"""Small original PGS/text fixtures; no user media is read or modified."""
from pathlib import Path
import struct
import argparse, shutil, subprocess
parser=argparse.ArgumentParser()
parser.add_argument('--ffmpeg',default=shutil.which('ffmpeg'))
args=parser.parse_args()
root=Path(__file__).resolve().parents[1]/'muestras'
def seg(kind,data,pts=0):
    return b'PG'+struct.pack('>IIBH',pts,0,kind,len(data))+data
def pcs(count,number):
    return struct.pack('>HHBHBBBB',640,360,0x10,number,0x80 if number==0 else 0,0,0,count)
palette=bytes([0,0,0,16,128,128,0,1,235,128,128,255])
rle=bytes([0,0x80+60,1,0,0])*20
ods=struct.pack('>HBB',1,0,0xC0)+(len(rle)+4).to_bytes(3,'big')+struct.pack('>HH',60,20)+rle
objects=struct.pack('>HBBHH',1,0,0,290,300)
window=bytes([1,0])+struct.pack('>HHHH',0,0,640,360)
data=seg(0x16,pcs(1,0)+objects)+seg(0x17,window)+seg(0x14,palette)+seg(0x15,ods)+seg(0x80,b'')
data+=seg(0x16,pcs(0,1),900000)+seg(0x80,b'',900000)
(root/'depth-qa.sup').write_bytes(data)
(root/'depth-qa.srt').write_text('1\n00:00:00,000 --> 00:00:10,000\nSUBTÍTULO DE PRUEBA\n',encoding='utf-8')
(root/'depth-qa.ass').write_text('''[Script Info]
ScriptType: v4.00+
PlayResX: 640
PlayResY: 360
[V4+ Styles]
Format: Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, OutlineColour, BackColour, Bold, Italic, Underline, StrikeOut, ScaleX, ScaleY, Spacing, Angle, BorderStyle, Outline, Shadow, Alignment, MarginL, MarginR, MarginV, Encoding
Style: Default,Arial,32,&H00FFFFFF,&H00FFFFFF,&H00000000,&H00000000,0,0,0,0,100,100,0,0,1,0,0,2,10,10,20,1
[Events]
Format: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text
Dialogue: 0,0:00:00.00,0:00:10.00,Default,,0,0,0,,SUBTÍTULO DE PRUEBA
''',encoding='utf-8')
print('PGS, SRT and ASS stereo depth fixtures generated')
if not args.ffmpeg:
    raise SystemExit('Specify --ffmpeg to generate the embedded PGS/title test files')
base=root/'ass3d-black.mkv'
subprocess.run([args.ffmpeg,'-v','error','-i',str(base),'-i',str(root/'depth-qa.sup'),'-map','0:v','-map','1:s','-c','copy','-y',str(root/'depth-qa-pgs.mkv')],check=True)
subprocess.run([args.ffmpeg,'-v','error','-i',str(base),'-map','0:v','-c','copy','-metadata','title=Enigmax · Prueba del título MKV','-y',str(root/'title-qa.mkv')],check=True)
