"""Package MCP-exported PNGs losslessly into an ICO and a delivery archive."""
from pathlib import Path
from PIL import Image, ImageDraw, ImageFont
import struct, zipfile, hashlib, json

root=Path(__file__).parent
sizes=[16,20,24,32,48,64]
payloads=[]
records=[]
for n in sizes:
    p=root/f'pixloom-{n}.png'
    with Image.open(p) as im:
        assert im.size==(n,n)
        assert im.mode=='RGBA'
        assert set(im.getchannel('A').get_flattened_data())=={0,255}
    payloads.append(p.read_bytes())
    records.append({'size':n,'png':p.name,'project':f'pixloom-{n}.pixloom','sha256':hashlib.sha256(p.read_bytes()).hexdigest()})
offset=6+16*len(sizes)
entries=[]
for n,data in zip(sizes,payloads):
    entries.append(struct.pack('<BBBBHHII',n,n,0,0,1,32,len(data),offset))
    offset+=len(data)
ico=struct.pack('<HHH',0,1,len(sizes))+b''.join(entries)+b''.join(payloads)
(root/'pixloom.ico').write_bytes(ico)
with Image.open(root/'pixloom.ico') as im:
    assert im.ico.sizes()=={(n,n) for n in sizes}
    for n in sizes:
        with Image.open(root/f'pixloom-{n}.png') as original:
            assert im.ico.getimage((n,n)).tobytes()==original.tobytes()
(root/'manifest.json').write_text(json.dumps({'lighting':'Upper-left highlight only; no lower-right inner shadow. Colored rim preserved.','icons':records},indent=2)+'\n',encoding='utf-8')
sheet=Image.new('RGB',(1150,620),'#e7e9ed')
d=ImageDraw.Draw(sheet)
font=ImageFont.truetype('C:/Windows/Fonts/segoeui.ttf',18)
small=ImageFont.truetype('C:/Windows/Fonts/segoeui.ttf',14)
d.text((20,12),'PIXLOOM — approved export set',font=font,fill='#182334')
d.text((20,43),'Native size above; enlarged below. Full treatment retains only the upper-left highlight.',font=small,fill='#374151')
for i,n in enumerate(sizes):
    x=20+i*188
    im=Image.open(root/f'pixloom-{n}.png').convert('RGBA')
    d.text((x,80),f'{n} x {n} px',font=font,fill='#182334')
    for y,bg in [(110,'#fafafa'),(365,'#202124')]:
        d.rectangle((x,y,x+176,y+238),fill=bg)
        sheet.paste(im,(x+(176-n)//2,y+10+(64-n)//2),im)
        k=max(1,144//n)
        zoom=im.resize((n*k,n*k),Image.Resampling.NEAREST)
        sheet.paste(zoom,(x+(176-zoom.width)//2,y+80+(144-zoom.height)//2),zoom)
sheet.save(root/'preview.png')
files=[root/f'pixloom-{n}.{ext}' for n in sizes for ext in ['png','pixloom']]
files += [root/'pixloom.ico',root/'manifest.json',root/'README.md',root/'preview.png']
with zipfile.ZipFile(root/'pixloom-icons.zip','w',zipfile.ZIP_DEFLATED) as z:
    for p in files:z.write(p,p.name)
print('Verified all six RGBA exports and byte-identical ICO images; delivery ZIP complete.')
