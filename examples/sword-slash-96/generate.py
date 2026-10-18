"""Dependency-free palette-index authoring for the 96x96 Pixloom slash demo.
Run from this folder: python generate.py
It refuses to overwrite the existing generated source.
"""
from pathlib import Path
import json
import math
import argparse

SIZE = 96
PALETTE = [
    '#00000000', '#101827FF', '#253047FF', '#3B4963FF', '#65748BFF',
    '#854D32FF', '#BB7950FF', '#E9AD72FF', '#FFDB9CFF',
    '#493343FF', '#70505AFF', '#152F3DFF', '#1A5961FF', '#26878AFF',
    '#54B9A5FF', '#9BE0BBFF', '#6A493CFF', '#AE7950FF', '#E7B66CFF',
    '#FFF0B0FF', '#7D9FA9FF', '#D5E8D9FF', '#F5FFF0FF', '#ECA65EFF',
    '#29414CFF', '#3C6268FF', '#A8F1D0FF',
]
DURATIONS = [200, 100, 100, 50, 50, 50, 100, 100, 100, 200]
POSES = [
    (0,0,-70,0), (-1,1,-105,1), (-3,2,-140,2),
    (1,0,-40,3), (5,1,5,4), (7,3,45,5),
    (6,3,65,6), (3,2,35,7), (1,1,-35,8), (0,0,-70,0),
]

class Canvas:
    def __init__(self, dx=0, dy=0):
        self.pixels = [[0]*SIZE for _ in range(SIZE)]
        self.dx, self.dy = dx, dy
    def put(self, x, y, color):
        x, y = round(x)+self.dx, round(y)+self.dy
        if 0 <= x < SIZE and 0 <= y < SIZE:
            self.pixels[y][x] = color
    def rect(self, x0,y0,x1,y1,color):
        for y in range(round(y0),round(y1)+1):
            for x in range(round(x0),round(x1)+1): self.put(x,y,color)
    def ellipse(self, cx,cy,rx,ry,color):
        for y in range(math.floor(cy-ry),math.ceil(cy+ry)+1):
            for x in range(math.floor(cx-rx),math.ceil(cx+rx)+1):
                if ((x-cx)/rx)**2+((y-cy)/ry)**2<=1: self.put(x,y,color)
    def line(self,a,b,color,width=1):
        x0,y0=map(round,a); x1,y1=map(round,b)
        dx,dy=abs(x1-x0),-abs(y1-y0); sx=1 if x0<x1 else -1; sy=1 if y0<y1 else -1; error=dx+dy
        while True:
            self.rect(x0-width//2,y0-width//2,x0+width//2,y0+width//2,color)
            if x0==x1 and y0==y1: break
            twice=2*error
            if twice>=dy: error+=dy; x0+=sx
            if twice<=dx: error+=dx; y0+=sy
    def polygon(self, points, color, outline=None):
        for y in range(math.floor(min(p[1] for p in points)),math.ceil(max(p[1] for p in points))+1):
            scan=y+0.5; crossings=[]
            for (x0,y0),(x1,y1) in zip(points,points[1:]+points[:1]):
                if (y0<=scan<y1) or (y1<=scan<y0): crossings.append(x0+(scan-y0)*(x1-x0)/(y1-y0))
            crossings.sort()
            for a,b in zip(crossings[::2],crossings[1::2]):
                for x in range(math.ceil(a),math.ceil(b)): self.put(x,y,color)
        if outline is not None:
            for a,b in zip(points,points[1:]+points[:1]): self.line(a,b,outline)
    def arc(self,cx,cy,r,start,end,color,width=1):
        points=[(cx+math.cos(math.radians(a))*r,cy+math.sin(math.radians(a))*r) for a in range(start,end+1,2)]
        for a,b in zip(points,points[1:]): self.line(a,b,color,width)


def layers_for(dx,dy,angle,phase):
    shadow=Canvas(); shadow.ellipse(36+dx//2,84,19,4,1); shadow.ellipse(37+dx//2,83,14,2,24)
    cloak=Canvas(dx-4,dy)
    tail = -4 if phase in [2,3,4,5] else 0
    cloak.polygon([(33,43),(28,45),(20+tail,45),(22+tail,51),(15+tail,56),(23,60),(29,55),(35,51)],12,1)
    cloak.polygon([(30,46),(24,48),(19+tail,55),(25,55),(32,49)],13)
    cloak.line((23+tail,48),(29,47),14)
    cloak.polygon([(31,49),(26,52),(23,67),(29,64),(35,52)],11,1)
    cloak.line((28,55),(26,62),12)
    legs=Canvas(dx-4,0)
    spread=4 if phase in [3,4,5,6,7] else 0
    legs.polygon([(32,61),(42,62),(39,71),(32-spread,78),(25-spread,77),(29,69)],3,1)
    legs.polygon([(40,61),(48,61),(49,70),(52+spread,77),(44+spread,79),(39,70)],2,1)
    legs.line((32,66),(29-spread,73),4,3)
    legs.polygon([(24-spread,74),(33-spread,75),(33-spread,81),(29-spread,83),(20-spread,83),(20-spread,80)],16,1)
    legs.rect(23-spread,76,30-spread,78,17); legs.rect(21-spread,81,31-spread,81,5)
    legs.polygon([(43+spread,75),(51+spread,73),(53+spread,78),(58+spread,80),(58+spread,83),(46+spread,83)],16,1)
    legs.rect(46+spread,76,50+spread,79,17); legs.rect(47+spread,81,56+spread,81,5)
    body=Canvas(dx-4,dy)
    body.polygon([(31,45),(42,43),(51,48),(52,59),(46,66),(31,63),(28,53)],16,1)
    body.polygon([(34,47),(43,46),(48,50),(46,59),(34,59),(31,54)],17)
    body.polygon([(35,47),(43,46),(46,49),(40,51),(34,51)],18)
    body.line((32,55),(46,55),16); body.line((35,57),(45,58),18)
    body.polygon([(30,59),(48,59),(47,65),(31,64)],5,1)
    body.rect(33,60,44,62,16); body.rect(39,59,43,63,18); body.rect(40,60,42,62,1)
    # Back arm and raised guard.
    body.polygon([(29,47),(34,49),(32,57),(37,57),(39,60),(34,63),(26,58),(25,52)],3,1)
    body.polygon([(26,48),(31,47),(33,51),(30,54),(25,52)],17,1)
    body.rect(28,49,31,50,18)
    body.rect(32,56,36,59,6); body.line((33,56),(36,57),7)
    # Sword arm changes reach with the attack pose.
    hand=(54+(2 if phase in [4,5,6] else 0),50+(2 if phase in [5,6,7] else 0))
    body.polygon([(45,46),(51,47),(54,53),(hand[0],hand[1]+3),(hand[0]-4,hand[1]+5),(47,54)],3,1)
    body.polygon([(46,45),(51,47),(52,51),(47,53),(44,49)],17,1)
    body.line((46,46),(49,47),19)
    body.rect(hand[0]-2,hand[1]-1,hand[0]+2,hand[1]+3,1)
    body.rect(hand[0]-1,hand[1],hand[0]+1,hand[1]+2,7)
    head=Canvas(dx-4,dy)
    head.rect(37,40,44,46,5); head.rect(40,41,44,44,6)
    head.polygon([(32,28),(42,25),(49,29),(49,33),(53,35),(51,38),(48,39),(47,43),(38,44),(31,38)],5,1)
    head.polygon([(38,28),(46,28),(48,33),(51,35),(48,36),(47,41),(40,42),(36,37)],6)
    head.polygon([(43,29),(46,30),(46,34),(49,35),(46,36),(45,39),(41,40),(41,34)],7)
    head.rect(45,32,48,33,1); head.put(47,32,22); head.rect(47,38,49,38,5)
    head.rect(32,33,35,37,6); head.put(33,34,7); head.put(34,38,18)
    head.polygon([(29,31),(29,26),(32,24),(31,21),(37,22),(40,19),(42,22),(47,21),(50,26),(49,30),(44,29),(43,26),(39,29),(37,34),(34,31),(32,34)],2,1)
    head.polygon([(31,26),(36,24),(40,23),(43,24),(41,26),(35,28),(32,30)],3)
    head.line((34,24),(37,23),4); head.put(41,22,4); head.line((30,28),(30,31),3)
    head.polygon([(33,42),(39,43),(47,42),(49,46),(44,49),(36,48),(31,45)],12,1)
    head.polygon([(34,42),(40,44),(47,43),(46,46),(38,46)],14)
    head.line((36,43),(39,44),15); head.line((34,47),(42,48),11)
    weapon=Canvas(dx-4,dy)
    rad=math.radians(angle); u=(math.cos(rad),math.sin(rad)); v=(-u[1],u[0]); hx,hy=hand
    def pt(along,across=0): return (hx+u[0]*along+v[0]*across,hy+u[1]*along+v[1]*across)
    weapon.line(pt(-5),pt(4),1,3); weapon.line(pt(-4),pt(3),16)
    weapon.line(pt(-3,-1),pt(-3,1),18)
    weapon.polygon([pt(3,-4),pt(5,-4),pt(6,4),pt(4,4)],18,1)
    weapon.line(pt(4,-3),pt(4,3),19)
    weapon.polygon([pt(6,-2),pt(28,-2),pt(34,0),pt(28,2),pt(6,2)],20,1)
    weapon.polygon([pt(7,-1),pt(28,-1),pt(33,0),pt(28,0),pt(7,0)],22)
    weapon.line(pt(8,1),pt(27,1),21)
    effects=Canvas(dx-4,dy)
    if phase in [3,4,5,6,7]:
        start,end={3:(-135,-39),4:(-92,10),5:(-20,65),6:(25,83),7:(50,76)}[phase]
        radius=34 if phase<6 else 32
        effects.arc(hx,hy,radius,start,end,12,3)
        effects.arc(hx,hy,radius,start+10,end,14,3)
        effects.arc(hx,hy,radius+1,start+18,end,26)
        effects.arc(hx,hy,radius-2,start+30,end,19)
        # Detached speed streaks leave the silhouette readable.
        if phase in [4,5]:
            effects.line((66,24),(72,21),15); effects.line((78,33),(84,30),26)
            effects.line((71,72),(75,75),18)
        if phase in [5,6]:
            for x,y in [(84,63),(78,77),(87,73)]:
                effects.rect(x,y,x+1,y+1,19); effects.put(x+2,y-1,23)
    return [('shadow',shadow),('scarf',cloak),('legs',legs),('armour',body),('sword',weapon),('head',head),('slash_fx',effects)]

def tokens(row):
    result=[]; i=0
    while i<len(row):
        end=i+1
        while end<len(row) and row[end]==row[i]: end+=1
        count=end-i
        run=[row[i],count]
        if len(json.dumps(run,separators=(',',':')))<len(','.join([str(row[i])]*count)):
            result.append(run)
        else: result.extend([row[i]]*count)
        i=end
    return result

def cel_payload(identifier,canvas):
    rows=[]
    for y,row in enumerate(canvas.pixels):
        spans=[]; x=0
        while x<SIZE:
            if row[x]==0: x+=1; continue
            start=x
            while x<SIZE and row[x]!=0: x+=1
            spans.append({'x':start,'pixels':tokens(row[start:x])})
        if spans: rows.append({'y':y,'spans':spans})
    return {'id':identifier,'encoding':'sparse-rows','default_index':0,'rows':rows}

def main():
    destination=Path(__file__).parent/'duelist.pixloom.json'
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--force',action='store_true',help='Regenerate this demo source explicitly')
    args=parser.parse_args()
    if destination.exists() and not args.force: raise SystemExit('Refusing to overwrite existing duelist.pixloom.json; use --force to regenerate')
    definitions=[]; timelines={}; seen={}
    for i,pose in enumerate(POSES):
        for name,canvas in layers_for(*pose):
            timelines.setdefault(name,[])
            signature=(name,tuple(map(tuple,canvas.pixels)))
            if signature not in seen:
                identifier=f'{name}_{i+1:02}'
                seen[signature]=identifier; definitions.append(cel_payload(identifier,canvas))
            entry={'cel':seen[signature]}
            if name=='shadow' and pose[0]==0: entry['link']='idle_shadow'
            timelines[name].append(entry)
    names={'shadow':'Ground shadow','scarf':'Scarf tails','legs':'Legs and boots','armour':'Armour and arms','sword':'Sword','head':'Head and scarf knot','slash_fx':'Slash trail and sparks'}
    doc={'version':2,'canvas':{'width':SIZE,'height':SIZE},'fps':20,'palette':PALETTE,'cels':definitions,
         'frames':[{'duration_ms':d,'annotations':{'user_data':label}} for d,label in zip(DURATIONS,['Ready','Raise','Anticipation','Release','Slash','Impact','Follow-through','Recover','Settle','Ready'])],
         'layers':[{'id':name,'name':names[name],'timeline':timeline,'annotations':{'user_data':'Editable pixel-art component'}} for name,timeline in timelines.items()],
         'tags':[{'name':'sword_slash','from':1,'to':10,'color':'#54B9A5FF'},{'name':'windup','from':1,'to':3,'color':'#E7B66CFF'},{'name':'strike','from':4,'to':7,'color':'#FFF0B0FF'},{'name':'recovery','from':8,'to':10,'color':'#7D9FA9FF'}],
         'annotations':{'user_data':'Teal-scarfed bronze duelist. Ten-frame clockwise sword slash. Authored directly as palette-index pixels for the Pixloom container demo.','metadata':{'demo':'sword-slash-96','generator':'generate.py','artwork_origin':'procedural authored pixel geometry; no raster tracing','palette_names':['transparent','ink','hair shadow','hair light','steel grey','skin shadow','skin mid','skin light','skin highlight','cloth plum','cloth rose','teal deep','teal shadow','teal mid','teal light','teal highlight','bronze shadow','bronze mid','bronze light','gold light','steel shadow','steel light','blade white','spark amber','ground shadow','ground light','slash mint']}}}
    destination.write_text(json.dumps(doc,separators=(',',':'))+'\n',encoding='utf-8')
    print(f'Authored {SIZE}x{SIZE}, {len(DURATIONS)} frames, {len(timelines)} layers, {len(definitions)} cels; {sum(DURATIONS)} ms')

if __name__=='__main__': main()
