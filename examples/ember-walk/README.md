# Ember Walk

A small editable version of the creature shown in the timeline-matrix sketch: 16×16 pixels, ten frames at 10 FPS, and four layers (Background, Body, Face, Highlights). The ground cells are linked; the animated layers are independent so they can be used to try rectangular selections, linking, copying, clearing, and undo.

Open `ember-walk.pixloom` directly in Pixen. The JSON source is also included. Run `node examples/ember-walk/create.mjs` to regenerate it. This is a simple bobbing walk-cycle example rather than finished game art.
