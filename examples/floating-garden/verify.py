import json,struct,zlib
from pathlib import Path
p=Path(__file__).parent
d=json.loads((p/'floating-garden.pixloom.json').read_text());w=d['canvas']['width'];h=d['canvas']['height']
cs={c['id']:c['pixels'] for c in d['cels']};pal=[bytes.fromhex(c[1:]) for c in d['palette']]
def frame(f):
 a=[[0]*w for _ in range(h)]
 for l in d['layers']:
  b=cs[l['timeline'][f]['cel']]
  for y in range(h):
   for x in range(w):
    if pal[b[y][x]][3]:a[y][x]=b[y][x]
 return a
frames=[frame(f) for f in range(24)]
# Review source scale, intended 2x and enlarged 4x against both README themes.
qw,qh=1344,640
raw=bytearray()
for y in range(qh):
 raw.append(0)
 for x in range(qw):
  dark=y>=320; yy=y%320;bg=bytes.fromhex('0d1117' if dark else 'ffffff')
  color=bg
  for ox,s in [(0,1),(192,2),(576,4)]:
   sx=(x-ox)//s;sy=yy//s
   if 0<=sx<w and 0<=sy<h and pal[frames[0][sy][sx]][3]:color=pal[frames[0][sy][sx]][:3]
  raw.extend(color)
def chunk(t,b):return struct.pack('>I',len(b))+t+b+struct.pack('>I',zlib.crc32(t+b)&0xffffffff)
(p/'theme-review.png').write_bytes(b'\x89PNG\r\n\x1a\n'+chunk(b'IHDR',struct.pack('>IIBBBBB',qw,qh,8,2,0,0,0))+chunk(b'IDAT',zlib.compress(raw))+chunk(b'IEND',b''))
gif=(p/'floating-garden.gif').read_bytes();apng=(p/'floating-garden.apng').read_bytes()
pos=apng.index(b'acTL');count,plays=struct.unpack('>II',apng[pos+4:pos+12])
assert count==24 and plays==0
assert b'NETSCAPE2.0\x03\x01\x00\x00' in gif
assert all(not f[0][x] and not f[-1][x] for f in frames for x in range(w))
assert all(not row[0] and not row[-1] for f in frames for row in f)
(p/'verification.json').write_text(json.dumps(dict(canvas=[w,h],frames=count,duration_ms=3600,apng_plays=plays,gif_infinite_loop=True,palette_colors=len(pal),alpha_values=[0,255],canvas_border_clear=True,visual_review='Still and contact sheet reviewed; playback not visually reviewed'),indent=2))
(p/'preview.html').write_text('''<!doctype html><meta charset="utf-8"><title>Floating garden</title><style>body{margin:0;display:flex;font:16px system-ui}section{width:50vw;min-height:100vh;display:grid;place-content:center;text-align:center;background:#fff;color:#393f43}section+section{background:#0d1117;color:#eee}img{width:384px;height:320px;image-rendering:pixelated}</style><section><img src="floating-garden.gif"><p>Light theme · 2×</p></section><section><img src="floating-garden.gif"><p>Dark theme · 2×</p></section>''')
