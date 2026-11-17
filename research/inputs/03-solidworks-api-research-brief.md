# SolidWorks API research brief (for craft-bridge)

Research deliverable for the design/research agent. The craft-bridge
replay program generates SolidWorks macros from synthetic and real
SolidCraft designs and verifies them on the owner's licensed seat. Several
API seams are unpinned; this brief asks for **public SolidWorks API
documentation research** that pins them before we write probes.

## Ground rules

- **Sources:** the official SolidWorks API documentation (help.solidworks.com
  API reference and SDK guides), SolidWorks forum/knowledge-base answers, and
  published examples. Cite URLs per fact, like the SolidCraft report did.
- **No documentation excerpts.** Paraphrase prose in your own words. Quoting
  identifiers is fine and needed: function names, argument names, enum
  member names, and full signatures as code (these are factual API shapes,
  not prose). The repo's third-party-materials rule forbids pasting doc
  prose into any file.
- **Nominative use.** SOLIDWORKS is a Dassault Systèmes mark; reference
  only, no affiliation. Same for any add-in product names.
- **Version sensitivity:** note which API version introduced or changed
  each call and flag anything that differs across recent versions (the
  seat's exact version will be confirmed separately; write version-agnostic
  facts first, differences as notes).
- **Status discipline:** mark each fact *documented* (found in official
  docs), *inferred* (from examples/forums), or *unverified* (needs a live
  probe at the seat). Do not smooth over gaps; a named unknown is more
  useful than a guess. Live probes are OUR execution problem (we run them
  as committed synthetic macros on the seat) - you only say what must be
  probed.
- Deliverable: one markdown report, same style as the SolidCraft format
  research (tables per topic, source links, unknowns section).

---

# PART A - API RESEARCH, IN PRIORITY ORDER

## A1. Reference planes (highest priority - it blocks the first real design)

The replay must create a plane offset from an origin plane by a
parameter-driven distance, then open sketches on it.

- `IFeatureManager::InsertRefPlane` (and any newer plane-creation entry
  points): full signature with every argument typed and explained. What
  argument combinations mean: offset from a plane by distance, distance
  with direction flip, midplane, plane through references, plane at angle.
  Which argument selects the reference entity, and what must be selected
  first (selection marks? a pre-selected plane/face)?
- Can the offset distance be DRIVEN BY AN EQUATION (a dimension on the
  plane feature, e.g. `D1@Plane1`, bound via `AddDimEquation`) so the plane
  follows a global variable? What is the dimension's name pattern for a
  reference plane, and how do we read it back (`IRefPlane`?
  `IFeature::GetDimension`?)?
- Selecting a plane for sketch creation: is `SelectByID2("PLANE@<feature>")`
  the right form, what other selection forms exist (by feature name, by
  object), and what mark value does `ISketchManager::CreateSketch` expect?
- What does `IRefPlane` expose to read the plane's transform (origin +
  axes) after creation - needed for our frame-map verification?
- Renaming the plane feature (feature tree name) - API form.

## A2. FeatureExtrusion2 - the merge and feature-scope semantics

We call `FeatureExtrusion2` with 23 positional arguments and only the
new-body case is behaviorally proven. Document:

- The complete argument list, each name and meaning (Sd, Flip, Dir,
  D1/D2 end conditions, depths, draft flags, reverse flags, Merge,
  UseFeatScope, UseAutoSelect, StartCondition, Offset, TranslateSurface,
  etc.) - exactly which argument is Merge and which control feature scope.
- Semantics of Merge = True vs False in a MULTIBODY part: does False always
  create a separate body (matching a `new_body` source operation)? Does
  True merge with adjacent/touching bodies or with the scoped bodies only?
- `swFeatureScope_e` values (AllBodies / SpecifiedBodies etc. and their
  behavior) and how "specified bodies" are supplied to the call or after it.
- The equivalent for `FeatureExtrusion`-family variants (thin extrude,
  two-sided, up-to-surface) only insofar as the 23-arg call covers them;
  note which source operations (join/cut/intersect) map to which argument
  combinations, if the docs state it.
- Any documented gotchas: draft flags interplay, end-condition enums
  (`swEndConditions_e` full value list with numeric values), direction flag
  meaning relative to sketch normal.

## A3. Multibody operations, bodies and their names

- Body enumeration: `IPartDoc::GetBodies2` - full signature including the
  `VisibleOnly` argument, return shapes (array vs single object vs
  Nothing), and how to count bodies safely. Same for `IBody2` face/edge
  enumeration return shapes (`GetEdges`, `GetFaces`).
- Naming and reading body names: which API sets a body's name
  (`IBody2::Name`? feature-scoped?), what names bodies get by default, and
  how to SELECT a body by name (SelectByID2 "BODY@...?" naming pattern).
- Boolean combination: `IFeatureManager::InsertCombineFeature` (or current
  equivalent): full signature, the operation enum (join/cut/intersect),
  the "keep tools" flag semantics, and what must be selected beforehand
  (target body mark, tool bodies mark?).
- Body deletion: the API that removes a body from the part (InsertDeleteBody
  / IPartDoc::DeleteNamedBody? - exact names), and what happens to features
  that referenced it.
- Move/copy body: `MoveCopyBody` feature or body transform APIs
  (`IBody2::Transform2`?): how to translate and how to rotate a body about
  an arbitrary axis + pivot; is the result a feature (rollback-safe) or a
  direct edit; what happens to body names.
- Reading back per-body mass properties and bounding boxes
  (`IBody2::GetMassProperties`, `GetBodyBox`): units and the exact
  returned layout (7 doubles? centroid + inertia?).

## A4. Edge and face identity, selection and verification

The replay must pick exactly one edge for fillets and refuse ambiguity.

- Edge enumeration and descriptors: `IBody2::GetEdges`, `IEdge` reading
  APIs - endpoints (`GetFirstVertex`/`GetLastVertex`?), curve parameters,
  length (`ICurve::GetLength`?), direction at midpoint, whether the edge
  is a line or arc, and the edge's bounding box (computable from curve
  sampling or an API).
- Selection: selecting one edge in a macro - `SelectByID2` naming patterns
  ("EDGE@feature@index"?), `SelectByRay`/`SelectAtPoint` equivalents,
  `ISelectData` with mark numbers. Which is the reliable way to select the
  edge NEAREST a given 3D point, and how to detect the pick missed (we
  already fail closed on `SelectByID2` returning False - but how do we
  detect a WRONG edge returned True?).
- Edge names: does SolidWorks expose persistent edge names/IDs
  (`IEdge::GetID`? persistent IDs? `GetEntityName`?) we can compare after
  rebuilds?
- Fillet feature: `IFeatureManager::InsertFeatureFillet` (or the modern
  equivalent): signature, constant-radius variant, the radius value's
  unit, and how to read the radius back (dimension name pattern
  `D1@FilletN`?). Full `swFeatureFilletType_e` / options enums only as far
  as the constant-radius case needs.
- Face selection for later work (custom planes, shell): "FACE@..." naming
  patterns and stable face identity APIs.

## A5. Sketch curve kinds: arcs, splines, conics, ellipses

Real designs carry arcs (512), conics (854) and splines (3, with 17
interpolation points). Document:

- Arcs: `ISketchManager::CreateArc` / `CreateArcByCenter` - signatures,
  angle convention (radians? sweep direction?), construction-line
  attribute setting on the created arc, endpoints as selectable entities
  for dimensions.
- Splines: `CreateSpline` / `CreateSpline2` / `CreateSpline3` - what each
  accepts (interpolation points? handles? style flags), how to build a
  spline THROUGH given points (our source data is interpolation points +
  degree), closed-spline handling, and point/tangency control.
- Conics and ellipses: does the sketch API expose a conic curve entity at
  all (SketchConic? elliptical arcs?), and if not - what is the documented
  approximation path (spline-through-points? ellipse + trim?) and what
  would a faithful replay refuse instead?
- Dimensioning these entities: which dimension kinds attach to arcs
  (radius/angle/diameter), to spline points (DistanceX/Y to points?), and
  what `AddDimension2` selection requirements apply.
- Relations available on these entities (tangent to spline, equal, etc.) -
  only the names of the relation-add methods and their selection counts.

## A6. Sweeps with guide rails (one design needs it)

- The solid sweep feature API (FeatureSweep / InsertSweptFeature? -
  exact current names): profile + path + GUIDE RAILS arguments, twist and
  orientation options (`swTwistControlType_e`?), path alignment options.
- What must be selected beforehand and with which marks; how the profile
  and path sketches are referenced; whether rails are selected curves or
  sketches.
- Any documented equivalent for a swept surface vs solid and how the
  solid form is chosen.

## A7. Cross-cutting COM/macro facts

- Return-shape discipline: for every object-array-returning API in A1-A6
  (GetBodies2, GetEdges, GetFaces, GetMassProperties, etc.), what the COM
  return looks like in VBA for 0, 1 and N results (Empty? single object?
  Variant array of objects?) and the safe inspection pattern
  (IsArray/IsObject/VarType) - we must never invent a count.
- Selection mark numbers: which calls require specific marks, and the
  documented rules (marks are per-select-call, cleared by ClearSelection2).
- Units: SystemValue in metres/radians (confirm), and any API returning
  document units instead.
- Error handling: which calls return Nothing vs throw vs set a status
  Boolean; the documented rebuild calls (`EditRebuild3` semantics) and
  what "the feature exists but is named 'Imported'"-style failure modes
  look like in the tree (how to detect an import/fallback body).
- Feature naming: `IFeature::Name` assignment, tree order, and reading the
  LAST created feature reliably after a call.

---

# PART B - PROBE DESIGN SUGGESTIONS (short)

For each fact you mark *unverified*, suggest the MINIMAL synthetic probe
geometry that would pin it at the seat (e.g. "plane probe: create plane
offset 12 mm from XY, open a sketch on it, read back the plane transform
and the D1 dimension name"). These become committed probe macros in our
corpus flow - keep each suggestion to a few lines. No parts are needed
from you for this section; it is guidance for the probe author.

---

# DELIVERY

- One markdown report: `SolidWorks_API_Research.md`, tables per topic,
  per-fact status markers, source links, an explicit unknowns table at the
  end (mirroring the SolidCraft report's section 13).
- If a whole topic cannot be established from public documentation, say so
  - it becomes a probe queue rather than a research gap.
