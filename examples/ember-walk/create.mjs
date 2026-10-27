// Editable 16x16 version of the Ember Walk artwork used in the matrix sketch.
import { writeFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
const width = 16, height = 16, count = 10;
const palette = ['#00000000', '#223842FF', '#36584CFF', '#F1AA58FF', '#BF694DFF', '#342E40FF', '#EE706BFF', '#FFE3A0FF'];
const frames = Array.from({ length: count }, (_, i) => ({ duration_ms: 100, annotations: { metadata: { pixloom_frame_id: `frame-${i + 1}`, 'pixen.frame_name': `Step ${i + 1}` } } }));
const cels = [], layers = [];
for (const [index, name] of ['Background', 'Body', 'Face', 'Highlights'].entries()) {
  const timeline = [];
  for (let frame = 0; frame < count; frame++) {
    const lift = [0, 0, 1, 1, 0, 0, 0, 1, 1, 0][frame];
    const pixels = Array.from({ length: height }, () => Array(width).fill(0));
    const rect = (x, y, w, h, color) => { for (let yy=y; yy<y+h; yy++) for (let xx=x; xx<x+w; xx++) pixels[yy][xx] = color; };
    if (index === 0) { rect(1,13,14,2,1); rect(3,12,10,1,2); }
    if (index === 1) {
      rect(4,5-lift,8,7,3); rect(3,7-lift,10,3,3); rect(4,11-lift,8,1,4);
      const stride = frame < 5 ? 0 : 1;
      rect(5-stride,12-lift,2,1,4); rect(9+stride,12-lift,2,1,4);
    }
    if (index === 2) { rect(5,7-lift,1,2,5); rect(10,7-lift,1,2,5); rect(7,10-lift,2,1,5); rect(4,9-lift,2,1,6); rect(10,9-lift,2,1,6); }
    if (index === 3) { rect(5,5-lift,3,1,7); rect(4,6-lift,1,2,7); }
    const id = index === 0 ? 'ground' : `layer-${index}-frame-${frame + 1}`;
    if (index !== 0 || frame === 0) cels.push({ id, pixels });
    timeline.push({ cel: id, link: index === 0 ? 'ground' : null });
  }
  layers.push({ id:`layer-${index}`, name, timeline });
}
const document = { version:1, canvas:{width,height}, fps:10, palette, cels, frames, layers,
  annotations:{metadata:{title:'Ember Walk',description:'Original 16x16 timeline example: linked ground, independently editable body, face, and highlights.'}},
  tags:[{name:'walk',from:1,to:count,color:'#F1AA58FF'}] };
writeFileSync(fileURLToPath(new URL('ember-walk.pixloom.json', import.meta.url)), JSON.stringify(document, null, 2) + '\n');
