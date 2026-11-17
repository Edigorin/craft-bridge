# Source-engine probe evidence (owner's design agent)

Probe results produced against the live SolidCraft engine by the owner's
design agent. Filed here because they settle replay-contract questions
the source research left open. Newest first.

## 2026-11-17 - Sweep `path` list order is set-like (Phase C input)

**Probe:** `SweepOrderObliqueProbe` (two sweeps sharing a profile and a
two-segment path, one with `path: ["l1","l2"]`, one with
`path: ["l2","l1"]`, on an oblique 63.435-degree joint) plus the same
reversal applied to `SweepMicro` itself (90-degree joint). Comparison by
`inspect.compare` at 0.001 mm tolerance with dense sampling.

**Result:** forward and reversed lists produce IDENTICAL solids - largest
deviation 8.5e-14 mm (oblique) and 2.3e-14 mm (SweepMicro), volume,
bounding box and centre of mass all equal. The engine accepts the
permuted list without error and evaluates the same sweep.

**Established:** the serialized `path` array is not an ordered sequence
for evaluation - permutation does not change geometry on connected
two-segment paths. A faithful sweep replay must derive the path chain
from curve CONNECTIVITY (endpoint chaining), never trust list order.

**Not established (still open):** behavior on other path topologies
(branching, gaps, three-plus segments) and the semantics of reversing an
individual curve's direction (the `reversed` wrapper flag).

**Verdict change:** `SweepMicro` is corrected to **PASS** (its earlier
FAIL was the reversal-as-fault detector not applying). Its edit-follow
(PathLegA 26.2 -> 31.0 mm: max X +4.8 mm, volume 337.847 -> 378.695 mm3,
restored) and STEP parity (3.4e-16 relative) were always good.

**Source files (untracked, in the design delivery):**
SweepOrderObliqueProbe.json (sha256 26550e1c97f55f761560b557e027e5ea2ea1453cb3fcf481789c449b652e67f0),
SweepOrderObliqueProbe.calls.json (sha256 ed0cdccd84eee78515bbceba1ca2a856425e4c2c9677793392847893b2cc0ff0),
SweepMicro.validation.json (updated with `path_order_probe`).

**MCP command-shape notes from the call log** (for future builder
prompts; the command API differs from the serialized schema):
`sketch.line` takes `points` as a list of 2-positions plus `closed` and
`infer` flags; `sketch.constraint.fix` takes `entities` as point names
like `l1.start`; `solid.sweep` takes `path_sketch`, `path` (curve id
list), `operation: "new"` (the serialized enum writes `new_body`) and
`body_name`. `timeline.inspect` returns the serialized `definition`
under the feature's own field names.

## 2026-11-17 - `fixed: true` + `exprs` is creatable through the MCP

Source-confirmed earlier (nested-schema research [S38]); the design
agent's D02/D10 fixtures were created exactly this way and re-open
correctly, closing open issue 3 of the nested-schema research for these
cases (saved `pos` matches `exprs` evaluation in the delivered files -
though stale-pos remains possible in general per [S34]-[S36], and the
loader's expr-wins rule covers it).
