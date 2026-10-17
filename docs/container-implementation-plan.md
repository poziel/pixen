# Container implementation plan

Implement the locked split-cel ZIP architecture in docs/container-format.md.
No commits, global installs, consuming-project changes or asset renames.

- [x] Add round-trip and lazy-access regression tests and observe missing API failure.
- [x] Reuse Archive safety checks; add strict container limits and entry inventory.
- [x] Implement structural reader, individual payload access, full reconstruction,
  deterministic writer and verified staged pack using existing Document types.
- [x] Route authoring commands through shared input dispatch; retain native unpack.
- [x] Test adversarial archives, every encoding, overwrite safety and determinism.
- [x] Benchmark split ZIP versus single-document ZIP and document actual results.
- [x] Update help, README and local companion skill; run complete checks.
