# Research brief 04 - SOLIDWORKS method pages for the probe queue

Follow-up to `research/inputs/03-solidworks-api-research-brief.md`; its
output (`research/outputs/03-solidworks-api-research.md`) fixed the
signatures and mark tables but left the seams below **Unverified**. This
brief asks for the CONCRETE method-page detail behind each, so the seat
probes can be written against documented shapes instead of guesses.

## Ground rules (same as brief 03)

- Public SOLIDWORKS API documentation only (help.solidworks.com API
  reference + SDK guides, release notes, knowledge base). Cite URLs per
  fact. **No doc prose excerpts** - paraphrase; identifiers, signatures
  and enum member names quoted as code are fine and wanted.
- SOLIDWORKS is a Dassault Systèmes mark; nominative reference only.
- Evidence labels per fact: **Documented** / **Inferred** / **Unverified**.
  Version notes where a call changed across releases; target macro host
  is SOLIDWORKS 2026 (seat revision 34.x).
- Deliverable: one markdown report with tables per topic, a compact
  unknowns table at the end, and - where a fact is **Unverified** - the
  minimal probe observation that would settle it (a sentence each).

## T1. The equation manager (blocker for parametric planes)

- `IEquationMgr` complete member list: `Add` (exact signature and return
  semantics - index? error?), `Edit`, `Delete`, `Equation`/`Equations`
  accessors, `Value`/`Values` (evaluated or text?), the global-variable
  vs dimension-equation text forms, and how errors surface (return codes?
  `GetLastError`? silent?).
- How a dimension equation is named/qualified ("D1@Plane1" vs the full
  path), and whether `Add` accepts a dimension name discovered at runtime
  (read back from `GetDimension2.Name`) verbatim.
- Whether editing a global variable's equation re-evaluates dependent
  equations in one rebuild, and what `EditRebuild3` does afterwards.

## T2. Reference planes in detail

- `InsertRefPlane` acceptance table: which reference entity types satisfy
  which constraint bitmasks (single plane + Distance; two planes +
  MidPlane; plane + line + Angle; face + Tangent...). What do the unused
  constraint slots require - 0, or omitted?
- The `OptionFlip`/`OptionReferenceFlip` bits (256/8192): exact semantics
  of each for an offset plane (which side; does 8|256 create the plane on
  the other side, and does the offset sign change?).
- The created feature: type name in the tree (`RefPlane`?), its
  `GetSpecificFeature2` interface (`IRefPlane`), `IRefPlane.Transform`
  matrix layout (is the origin entries 9..11 or 12..14? row-major? units),
  and the display-dimension naming of the offset distance.
- Selecting the created plane again for sketch creation: `SelectByID2`
  name grammar for reference planes, `IRefPlane` object `Select`, mark
  expectations for `ISketchManager.InsertSketch` / `ModelDoc2.CreateSketch`.

## T3. Cut extrusions and booleans (source `cut`/`intersect`)

- The cut-extrusion entry point for a subtractive profile extrude
  (`FeatureCut4`/`FeatureCut5`? full signatures, end-condition args) and
  how it differs from `FeatureExtrusion2`/`3` argument-wise.
- `InsertCombineFeature` in full: operation constants (15901/15902/15903)
  confirmed, `MainBody`/`ToolVar` forms vs the mark-1/mark-2 selection
  form - which is current recommended practice for VBA?
- `ICombineBodiesFeatureData`: the keep-tools (preserve tools) property
  name and semantics; is it feature-editable after creation?
- Body deletion: the current API (`InsertDeleteBody`? `IFeatureManager`?
  `IModelDoc2.DeleteBody2`?) - full signature, what happens to downstream
  features, and the tree result.

## T4. Bodies: naming, selection, transform

- Setting and reading a SOLID BODY's name (`IBody2.Name` settable?), the
  name a body gets from a feature (auto "Boss-Extrude1[1]"?),
  `SelectByID2` grammar for bodies ("SolidBody@..."? exact pattern), and
  `IEntity.Select` for bodies.
- `InsertMoveCopyBody2` per-argument semantics (what TransDist does vs the
  TransX/Y/Z vector; RotAngle* as axis-angle? degrees or radians? which
  reference entities satisfy the 2/4/8 marks) and the resulting feature's
  data interface (`IMoveCopyBodyFeatureData`?) for readback.
- `IBody2.GetMassProperties` confirmed 12-double layout and whether
  `CreateMassProperty` (document-level) and body-level agree.

## T5. Edge identity and the fillet builder

- `IEdge`/`ICurve` geometry members for descriptor building:
  `GetCurveParams`, `GetFirstVertex`/`GetLastVertex`, `ICurve.GetLength`,
  mid-point/tangent evaluation calls - exact names and return shapes.
- Nearest-edge selection: `SelectByRay`, `SelectAtPoint` (does it exist?),
  `ISelectData` mark semantics, and how to verify WHAT got selected
  (`ISelectionMgr.GetSelectedObject6` usage).
- `ISimpleFilletFeatureData2` + `IFeatureManager.CreateDefinition`/
  `CreateFeature` builder sequence for a constant-radius fillet on
  selected edges (which option constants; how edges are supplied; radius
  units), and the resulting feature's dimension naming (D1@FilletN?).
- `GetPersistReference3`/`GetObjectByPersistReference3` lifetimes across
  rebuild and save/reopen.

## T6. Sketch curves: conics, arcs, splines (the R1 blockers)

- `ISketchManager.CreateConic` - FULL signature and parameter meaning:
  how do the arguments map to start point, end point, apex/control, and
  the conic constant rho? Is rho the same definition as a rational
  quadratic's rho (0.5 = parabola)? Which conic types are supported?
  Any related `CreateConicByEqnParams`?
- `CreateArc` confirmed; how arc endpoints and centre read back
  (`GetStartPoint`/`GetEndPoint` on SketchArc? `SketchSegment.GetCurve`?),
  and how dimensions attach to arcs (radius/diameter/angle/arc-length -
  `AddDimension2` selection requirements).
- `CreateSpline3`: the `PointData` array layout (doubles? objects?),
  interpolation guarantees (through all points?), closed-spline handling,
  and how a spline's fit points read back. `CreateSplineByEqnParams`
  input layout (control points + knots + weights?).
- `CreateEllipse`/`CreateEllipticalArc` argument meaning (major endpoint
  and minor radius? axis vectors?) and readback.
- Sketch relations on these entities: which `AddConstraints` relation
  names accept arcs/splines/conics and the selection counts.

## T7. Sweeps for 2026

- The current non-obsolete solid sweep entry point (`InsertProtrusionSwept4`?
  5?) with full signature; guide-curve accepted entity types (2D sketch
  curves? 3D sketch? model edges? composite curves?); must guides
  intersect the profile?; `TwistCtrlOption`/`StartMatchingType`/
  `EndMatchingType`/`PathAlign` enum members with values.
- Keep-tangency and advanced-smoothing semantics briefly; the thin-body
  arguments only insofar as they affect the default solid form.

## T8. The run-time surface (feeds the tooling, keep short)

- `RunMacro` vs `RunMacro2` exact signatures and the common "Type
  mismatch" failure modes (which argument types are rejected); which is
  recommended for headless macro invocation.
- `EditRebuild3` documented return semantics (any reliable failure
  signal?), and `IModelDocExtension.GetLastFeatureAdded` lifetime.
- Any documented statement about VBA macros running with a modal dialog
  open (input-dimension-value, save prompts) blocking automation.

## Unknowns table

Close with the same shape as brief 03: priority / unknown / status /
minimal probe observation / implementation rule.
