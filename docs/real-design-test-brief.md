# Test-design generation brief + solidcraft/57 format research

This brief is for a design-side agent producing verification material for the
craft-bridge replay program. Deliverables: six small test designs (Part A) and
a format knowledge report (Part B). Every design must be purpose-built test
material - never real client work.

---

# PART A - TEST DESIGNS

## General rules (apply to every design)

1. **Asymmetric on purpose.** Distinct length/width/height values; avoid
   squares, cubes, regular polygons and circles where a rectangle, scalene
   triangle or ellipse will do. No mirrored or rotationally symmetric
   geometry. Reason: replay verification detects orientation and axis
   mistakes, and symmetric geometry makes wrong-but-plausible replays
   indistinguishable from correct ones.
2. **Distinctive mass.** Choose dimensions so the final volume is an unusual
   number (avoid values like 100x100x100 or 2r cubes where a wrong
   interpretation yields a plausible coincidence).
3. **Units mm, parameters named semantically** (e.g. OffsetDistance,
   PlaqueWidth). Expressions use the exporter's own style - keep them exactly
   as the exporter writes them (unit-suffixed literals inside expressions are
   fine and expected).
4. **Small and isolated.** The fewer features the better. Each design pins ONE
   capability (below). If a capability needs two or three features, keep it to
   that.
5. **Files per design**: `Name.json` (the solidcraft/57 export), `Name.step`
   (STEP export of EXACTLY the same final geometry - mass verification compares
   against it), and optionally `Name.sc` (the native archive).
6. The STEP must be the true final state of the design as exported - do not
   hand-edit it. If the exporter lets you choose STEP options, use the
   simplest solid-body output.
7. Delivery: same as the previous three designs (the uploads folder), with a
   short one-line note per design naming the capability it isolates.

## Design 1 - Offset-plane micro (highest priority)

- One offset construction plane, distance driven by a named parameter
  (e.g. `OffsetDistance = 12 mm`), offset from an origin plane (XY or XZ -
  your choice, but say which).
- One sketch ON that plane: a closed asymmetric profile of 4+ points; at
  least ONE point coordinate expressed as a parameter expression (not a bare
  number).
- One extrude of the profile (simple, single direction, one depth parameter).
- Acceptance intent: replay places the plane at the parameter distance,
  geometry lands in the right 3D position (the STEP proves it), and editing
  `OffsetDistance` (e.g. 12 -> 20) moves the body accordingly.

## Design 2 - Coordinate-expression micro

- One sketch on an ORIGIN plane. 5+ points forming one closed asymmetric
  profile.
- At least three coordinates carry expressions referencing two named
  parameters (e.g. `Width = 80 mm`, `Height = 45 mm`); include at least one
  point marked `fixed: true` that ALSO carries expressions, and at least one
  plain numeric point (no expressions).
- One extrude of the closed profile.
- Acceptance intent: an edit to Width or Height visibly changes the sketch
  geometry (and hence the body) - this is the parameter-following test.

## Design 3 - Body-operations matrix

One small design containing, in order:
- an extrude with `operation: new_body` creating a named body,
- an extrude with `operation: join` targeting that body from a second sketch,
- an extrude with `operation: subtract` cutting it,
- an extrude with `operation: intersect` (its own new body first if needed),
- one of the above variants using `keep_tools: true` if the format supports it
  on that operation (e.g. a combine or an operation with tool bodies).
Keep each shape simple and asymmetric. Acceptance intent: each operation's
merge/scope semantics are verified against the STEP (the body count and mass
prove which bodies exist).

## Design 4 - Move/remove variants

One design with:
- at least two named bodies created by simple extrudes/primitives,
- a `move` translating a named body by parameter-driven distances,
- a `move` ROTATING a named body (angle + rotate_axis as the format defines),
- one `remove` of a named body.
Acceptance intent: final body positions/orientation verified against STEP;
the removed body is gone.

## Design 5 - Feature-type isolates (sweep, torus, combine, construction_plane)

Either four tiny designs or one small design containing:
- a **sweep**: a profile sketch swept along a path sketch (include one rail if
  the exporter writes rails),
- a **torus**: parameter-driven centre/major/minor,
- a **combine**: two named bodies joined (or subtracted) with `keep_tools`
  both ways if possible (two combines),
- a **construction_plane** of type `along_path` (t parameter), used by
  nothing else (or used by the sweep - your choice; say which).
Acceptance intent: each feature type's API shape and result verified in
isolation against its own STEP (if combined, one STEP of the final state).

## Design 6 - Fillet edge variants

- One simple asymmetric body (extrude is fine) with 3+ fillets on DISTINCT
  edges: one long edge, one short edge, one edge in a third orientation -
  so each has a different bounding box.
- Fillet radii parameter-driven and different per fillet.
- Acceptance intent: the edge-reference resolution must match each fillet to
  EXACTLY one edge; ambiguous or unresolvable references must be detectable.

## Design 7 (bonus) - Robustness cases

A design exercising: sparse and large feature ids (ids far apart, e.g. 5 and
9000+), one SUPPRESSED feature between two supported ones, and one parameter
whose expression is a compound expression (parentheses, division, unit
literals). This one may be synthetic-shaped rather than a real object.

---

# PART B - FORMAT KNOWLEDGE RESEARCH

Produce a written report answering, with citations from the exporter source,
its docs, or observed output:

1. **Schema**: the full field list per feature type in `solidcraft/57`
   (sketch, extrude, box, cylinder, fillet, sweep, torus, combine,
   construction_plane, move, remove - and any types we have NOT seen).
   Which fields are required vs optional; what each enum-like field accepts
   (e.g. `operation` values, `extent.convention` values, `extent.direction`
   values, `profiles.type` values, `plane.type` values).
2. **Expression grammar** for parameter expressions and coordinate `exprs`:
   what functions and operators are allowed; how units appear inside
   expressions; evaluation order; what `model: false` on a param means (and
   what `model: true` would mean).
3. **Plane types**: all of them and their fields - `origin` (names?),
   `offset` (base plane, distance, normal flip?), `along_path` (what does `t`
   mean - fraction? length?), and any others (angle/through-points?).
4. **Move semantics**: translate fields (vector? expressions?), rotate fields
   (angle, rotate_axis - axis definition, pivot?), ordering, and whether moves
   are body-level or feature-level.
5. **Fillet edge references**: the meaning of `edge_refs[].dir`, `min`, `max`
   (bounding box? direction?), `edges` vs `edge_names` fields, and how the
   exporter resolves them internally (stable identity? name-based?).
6. **Operations and bodies**: exact semantics of `new_body`, `join`,
   `subtract`, `intersect`; what `body_names`, `targets`, `participants`,
   `tools`, `keep_tools` mean per type; body naming/renaming rules.
7. **Suppression**: what `suppressed: true` guarantees about the exported
   geometry and downstream references.
8. **Sidecars**: does the exporter emit warnings files, snapshots or other
   sidecar artifacts alongside .json? What do the `.sc` archives contain
   (internal structure, what is authoritative)?
9. **Versioning**: what changed from `solidcraft/53` (commands-based) to 57
   (features-based); is 57 current; are newer revisions expected; any
   compatibility guarantees.
10. **Typical edit patterns**: which parameters do designers actually drive
    with edits (so edit-and-follow acceptance mirrors real usage); typical
    scale of designs (feature counts), and common feature combinations.

If any answer cannot be established from available sources, say so explicitly
rather than guessing - unverified items will be probed at the seat.

---

# DELIVERY CHECKLIST

- [ ] 6-7 design folders, each with Name.json + Name.step (+ optional Name.sc)
- [ ] one-line capability note per design
- [ ] the format report (Part B), with sources cited
- [ ] a parameter-edit suggestion per design (which parameter to change and
      what the expected geometric effect is) - this becomes the edit-and-
      follow acceptance check
