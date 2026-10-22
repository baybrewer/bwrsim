# Archived: BWRX-300 refinement plan set (2026-10-20 to 2026-10-22)

Status: **abandoned before execution**. No code from these plans was implemented; the engine stayed at commit `9cac46a`.

Contents:
- `2026-10-20-refinement-master.md` — master interface contract, revision r2 (Codex cycle 1: 22 comments resolved; cycle-2 amendments applied through batch 3).
- `2026-10-20-pkg01..pkg10-*.md` — full-code package plans. P1-P6 fixed per Codex cycle-2 batches 1-3; P7-P10 reviewed (batches 4-5) but the comments were not applied.
- `reviews/` — Codex cycle-2 comment lists for batches 2-5 (batch 1 comments were applied inline).

Why abandoned: writing all ten packages at full-code depth before executing any meant every upstream revision invalidated downstream plans, and plan-level Codex review mostly surfaced what the compiler and tests catch for free. Cost: two days, five Codex review batches, zero landed code.

Reusable parts: the master contract's locked CSV headers, log-line keys, `Sim::step` order, `DomainError` set, and the verified-defect list. The spec that started this remains at `../../2026-10-20-existing-systems-refinement.md`.

Rule going forward: master contract plus a full-depth plan for the next unit only; execute and test it; review real diffs with `/codex-review`; then plan the next unit.
