import json, math
from pathlib import Path
P=Path(__file__).parent
W,H,N=192,160,24
colors=['00000000','393F43','505451','686958','858371','A49B7B','594934','795737','A57446','BD9259','E2BD7C','EEE0AE','355747','487055','63915E','86AC70','B4C98A','326775','438B96','6DB9B8','B7E1D3','785348','A97555','CA9464','E5B786']
palette=['#'+c+('FF' if len(c)==6 else '') for c in colors]
cels=[]; layers=[]
def canvas(): return [[0]*W for _ in range(H)]
a=canvas()
def rect(x,y,w,h,c):
 for yy in range(max(0,y),min(H,y+h)):
  for xx in range(max(0,x),min(W,x+w)): a[yy][xx]=c
def poly(p,c):
 for y in range(max(0,math.floor(min(v[1] for v in p))),min(H,math.ceil(max(v[1] for v in p))+1)):
  hits=[]
  for i,(x1,y1) in enumerate(p):
   x2,y2=p[(i+1)%len(p)]
   if (y1<=y<y2) or (y2<=y<y1): hits.append(x1+(y-y1)*(x2-x1)/(y2-y1))
  hits.sort()
  for l,r in zip(hits[::2],hits[1::2]):
   for x in range(max(0,math.ceil(l)),min(W,math.floor(r)+1)): a[y][x]=c
def line(x1,y1,x2,y2,c,width=1):
 n=max(abs(x2-x1),abs(y2-y1),1)
 for i in range(n+1): rect(round(x1+(x2-x1)*i/n),round(y1+(y2-y1)*i/n),width,width,c)
def ellipse(x,y,w,h,c):
 for yy in range(y,y+h):
  for xx in range(x,x+w):
   if ((xx-x-(w-1)/2)/(w/2))**2+((yy-y-(h-1)/2)/(h/2))**2<=1: rect(xx,yy,1,1,c)
def add(id):
 cels.append(dict(id=id,encoding='grid',pixels=[r[:] for r in a])); return id
def static(id,name):
 add(id);layers.append(dict(id=id,name=name,timeline=[dict(cel=id,link=id+'-held') for _ in range(N)]))
# Sculpted underside: broad planes rather than speckled texture.
poly([(19,86),(43,75),(149,75),(174,87),(166,105),(145,119),(126,128),(111,145),(88,140),(72,129),(50,124),(32,108)],1)
poly([(22,87),(80,98),(95,137),(74,126),(51,120),(34,105)],3)
poly([(80,97),(130,98),(119,124),(110,141),(94,136)],2)
poly([(130,98),(170,89),(163,103),(145,116),(121,127)],3)
poly([(30,94),(49,100),(60,116),(48,111)],4)
poly([(59,99),(76,104),(84,125),(72,119)],4)
poly([(134,103),(155,98),(148,109),(128,119)],4)
poly([(95,112),(106,116),(105,133),(99,128)],3)
poly([(22,86),(48,76),(146,76),(173,87),(166,97),(139,106),(93,108),(52,101),(29,95)],6)
static('rock','Rock and earth')
a=canvas()
poly([(18,84),(28,76),(49,69),(73,65),(107,65),(139,70),(161,77),(175,85),(168,93),(142,101),(100,104),(63,100),(36,93)],12)
poly([(20,82),(48,70),(78,66),(107,67),(140,72),(163,80),(171,85),(162,90),(137,97),(98,100),(63,96),(36,90)],14)
poly([(25,80),(52,71),(82,68),(108,69),(127,72),(109,79),(72,85),(40,86)],15)
poly([(27,83),(40,87),(64,93),(99,97),(137,94),(162,87),(168,86),(160,93),(138,99),(99,102),(63,98),(36,91)],13)
# Stream winds from rear right to foreground, clear of the cabin footprint.
poly([(131,73),(139,75),(129,80),(121,83),(120,87),(127,91),(124,98),(116,103),(105,103),(116,96),(116,93),(110,88),(111,82),(121,77)],17)
poly([(131,74),(135,75),(122,82),(116,84),(116,87),(123,92),(121,97),(112,102),(108,102),(118,95),(113,89),(113,83)],18)
for x,y in [(32,82),(42,76),(64,88),(80,93),(142,87),(151,83),(58,76)]:
 line(x,y,x-1,y-3,13);line(x,y,x+2,y-2,16)
# A little stepping-stone path to the door.
for x,y in [(95,90),(90,94),(85,97)]: poly([(x-3,y),(x,y-2),(x+4,y),(x+1,y+2)],5)
static('meadow','Meadow and stream bed')
a=canvas()
# Mill tapered timber tower.
poly([(40,48),(53,48),(60,83),(51,87),(34,82)],6)
poly([(41,48),(49,48),(52,83),(37,81)],9)
poly([(49,49),(53,48),(58,82),(53,84)],8)
for y in [58,67,76]: line(39,y,51,y+2,7)
poly([(35,49),(46,36),(57,48),(53,52),(40,51)],6)
poly([(36,48),(46,38),(51,48)],21)
poly([(46,38),(56,48),(51,50)],22)
rect(43,72,6,12,6);rect(44,73,3,10,7)
rect(44,59,5,6,6);rect(45,60,3,4,19)
# Cozy cabin: short wall, 2:1 receding side, broad modest roof.
poly([(74,62),(103,62),(117,55),(117,79),(104,87),(74,80)],6)
poly([(76,63),(103,65),(103,84),(76,78)],9)
poly([(104,65),(115,59),(115,78),(104,84)],8)
for y in [68,73,78]: line(76,y,102,y+3,8)
for y in [66,71,76]: line(105,y,114,y-5,7)
poly([(69,63),(81,47),(108,47),(122,57),(104,68)],6)
poly([(71,61),(82,49),(107,49),(102,64)],21)
poly([(82,49),(107,49),(119,57),(103,65)],22)
for y,x in [(52,81),(56,78),(60,75)]: line(x,y,x+25,y,23)
line(83,49,106,49,24);line(71,62,102,66,10);line(104,66,120,58,8)
rect(91,69,9,15,6);rect(93,70,5,13,7);rect(96,76,1,1,11)
rect(79,66,9,9,7);rect(80,67,7,6,11);rect(83,67,1,6,8);rect(80,70,7,1,8);rect(78,75,11,2,6)
poly([(108,67),(113,64),(113,71),(108,74)],6)
line(110,67,110,71,19,2)
rect(97,40,5,9,6);rect(98,41,3,7,4);rect(96,39,7,2,5)
# Timber doorstep and flower box.
poly([(91,83),(102,85),(99,88),(88,86)],10)
rect(78,76,10,3,8)
for x,y in [(79,74),(83,75),(86,74)]: rect(x,y,2,2,24)
static('timber','Windmill tower and cozy shack')
a=canvas()
# Tree trunk and layered canopy clusters.
poly([(142,58),(149,57),(149,77),(155,83),(149,84),(145,80),(139,83),(136,81),(142,76)],6)
poly([(144,59),(147,59),(147,78),(150,81),(146,79),(143,79)],9)
line(145,68,136,59,7,2);line(147,67,155,56,7,2)
poly([(126,49),(130,40),(139,37),(143,31),(155,32),(160,38),(169,41),(173,52),(169,62),(159,67),(145,65),(137,68),(127,62),(123,56)],12)
poly([(126,48),(132,41),(141,40),(145,34),(155,35),(159,41),(167,43),(169,53),(162,60),(150,59),(140,63),(129,59)],14)
poly([(128,47),(135,42),(144,43),(147,36),(154,37),(158,44),(154,49),(143,49),(137,54),(128,53)],15)
poly([(133,45),(141,43),(145,45),(141,48),(132,49)],16)
poly([(151,52),(161,46),(166,48),(166,54),(159,57)],15)
poly([(129,58),(139,56),(145,59),(140,62),(132,61)],13)
static('tree','Leafy tree')
water=[];rotor=[]
for f in range(N):
 a=canvas()
 # Fixed taper, moving coherent light streaks, periodic at the loop seam.
 poly([(106,101),(117,101),(117,111),(115,124),(113,137),(110,149),(108,144),(108,131),(106,116)],17)
 poly([(108,102),(115,102),(115,115),(113,130),(111,143),(109,137),(109,118)],18)
 poly([(108,102),(111,102),(111,119),(110,130),(109,123)],19)
 for k in range(4):
  y=103+((k*12+f*2)%48)
  x=112 if y<125 else 110
  line(x,y,x,y+3 if y<140 else y+1,20)
 for x,y in [(128,77),(118,83),(118,91),(116,97)]:
  dx=(f//4)%3
  line(x-dx,y,x+2-dx,y,19)
 water.append(add('water-'+str(f)))
 a=canvas();cx,cy=46,50
 # Identical four sails give an exact quarter-turn loop, full rotation 14.4s.
 angle=2*math.pi*f/(N*4)
 for j in range(4):
  t=angle+j*math.pi/2
  def point(u,v): return (round(cx+u*math.cos(t)-v*math.sin(t)),round(cy+u*math.sin(t)+v*math.cos(t)))
  poly([point(2,-1),point(25,-1),point(25,1),point(2,1)],6)
  poly([point(10,-5),point(25,-5),point(25,0),point(10,0)],7)
  poly([point(11,-4),point(24,-4),point(24,-1),point(11,-1)],11)
  p=point(17,-4);q=point(17,0);line(*p,*q,9)
 ellipse(43,47,7,7,6);ellipse(44,48,4,4,10);rect(45,49,2,2,8)
 rotor.append(add('sails-'+str(f)))
layers.append(dict(id='water',name='Flowing water',timeline=[dict(cel=c) for c in water]))
layers.append(dict(id='sails',name='Rotating linen sails',timeline=[dict(cel=c) for c in rotor]))
doc=dict(version=2,canvas=dict(width=W,height=H),palette=palette,cels=cels,frames=[dict(duration_ms=150) for _ in range(N)],fps=20/3,layers=layers,tags=[dict(name='gentle-loop',**{'from':1,'to':N})],annotations=dict(metadata=dict(authoring='Original scripted pixel clusters; no generated or external imagery',loop='24 frames x 150 ms; four identical sails; quarter-turn per loop')))
(P/'floating-garden.pixloom.json').write_text(json.dumps(doc,separators=(',',':')))
