/** Keep the artwork point under an anchor fixed as its scale changes.
 * The anchor is relative to the viewport center; pan is in screen pixels. */
export function zoomPan(pan: { x: number; y: number }, anchor: { x: number; y: number }, ratio: number) {
  return { x: anchor.x - (anchor.x - pan.x) * ratio, y: anchor.y - (anchor.y - pan.y) * ratio };
}
