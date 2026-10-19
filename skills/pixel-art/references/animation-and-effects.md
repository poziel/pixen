# Animation and effects

Human starting references: C1 for body motion, I1 for pickups and feedback, A1 for projectiles/explosions, and P1 for specific illustrated techniques. See [sources](sources.md). The review prompts below synthesize practical animation concerns; they are not frame-count recipes from those artists.

## Solve motion with simple forms

Define the action and the moment the player must understand. Establish key poses with flat body masses or simple effect shapes. Check direction, ground contact and weight before adding costume or texture. More frames do not repair weak poses or ambiguous event timing.

Keep a stable canvas and anchor. Track intentional displacement separately from accidental registration changes. During cleanup, preserve nonmoving clusters unless their change is caused by form, lighting or a deliberate effect; random pixel changes often read as crawling texture.

Use timing to communicate action. Anticipation, contact and recovery need not have equal duration. The visual impact should align with the game's event when that is part of the task. Avoid automatically using one duration for every frame.

## Design effects around meaning

Give projectiles a readable direction, impacts a clear onset, and smoke/fire a coherent evolution of masses. Start with shape and lifetime, then add secondary particles. Keep optional embellishment subordinate to targets, hazards and important UI.

Preserve a consistent effect language across a set. Decide which effects indicate damage, healing, protection, collection or purely ambient activity. Similar bursts should not imply contradictory mechanics. For loops, inspect the transition from final to first frame; for one-shot effects, verify termination and any lingering pixels.

## Inspect what players will see

- Play at intended speed and intended display size; then inspect a contact sheet.
- Check all frames for clipping, stray opaque pixels and unintended brightness changes.
- Test the effect over representative backgrounds and actors.
- Compare event timing with motion; inspect the loop seam or the one-shot ending.
- Record durations and preserve intentionally linked cels in the editable source.

Separate intentional low-frame animation, smears or flashing from accidental jitter. Do not claim timing was reviewed from a static screenshot alone.
