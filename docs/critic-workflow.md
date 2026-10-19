# Future critic workflow

Status: design guidance only. No critic agent, skill, model call or automatic
review loop is implemented by Pixloom. Those integrations will be a later task.

## Purpose

A critic would supply an independent account of what the artwork communicates.
Pixloom supplies inspectable images, measurements, constraints and before/after
comparisons. The creating agent would combine that evidence with the user's brief.
This can reveal a recognizable-shape failure before spending time on details.

## First pass: blind recognition

Provide only neutral-labeled review images at native/intended/enlarged scales,
together with scale information and the relevant viewing background. Withhold
the intended subject, generating prompt, semantic filenames/layer names, activity
messages and prior praise or criticism. Do not accidentally disclose intent in
image labels or surrounding conversation.

Ask for a literal description of visible shapes, subject/action if recognizable,
ambiguous regions and the coordinates of evidence. Ask the critic to distinguish
what it sees from what it guesses. A confidence statement is not a calibrated
probability or proof of correctness.

Compare that description with the brief afterward. A mismatch is a reason to
inspect silhouette, pose or context; it can also come from critic error, unfamiliar
symbols or intentionally ambiguous art. It does not prove that the artwork is bad.

## Second pass: informed critique

Reveal the brief and intended use, including display size, art direction, palette,
lighting and motion goals. Ask for a few concrete observations in this form:

- Visible issue and location.
- Consequence for the intended use.
- Small proposed correction and stylistic tradeoff.
- Evidence or follow-up inspection that would confirm it.

Separate objective violations from heuristic suspicions and personal preference.
An isolated pixel may be an eye or sparkle. Center movement may be intentional.
Fewer colors, greater symmetry, matching loop endpoints and smaller frame changes
are not universal indicators of improvement.

## Test a correction

Before editing, capture an immutable checkpoint. Define protected regions and
objective rules where appropriate. Correct one important issue at a time, then
request review views, compare_checkpoint, validate_artwork and relevant motion
measurements. Check both intended improvement and unintended changes elsewhere.

A before/after visual preference test may randomize presentation order and conceal
revision labels from the critic. The orchestrator must retain the mapping. Use
the same scales/backgrounds so presentation does not favor a version. Preserve
any disagreement between subjective preference and objective rule results.

## Ownership and stopping

The future skill/client would choose whether a critic runs, its model, access,
time/cost budget and acceptance criteria. Recommended initial correction budget:
at most three rounds, then report unresolved issues or request human direction.
Pixloom itself never starts a critic because a review endpoint was called.

The user remains the authority on acceptance. Save/export an immutable .pixloom
copy for a durable baseline: MCP checkpoints are session-local and end when the
project finishes or the server exits. Keep the editable project and honest review
evidence, including inspected revisions, limitations and deliberately retained
stylistic choices. No automatic promotion to production artwork follows a score.
