# Research brief 07 - SOLIDWORKS API usage archaeology

Follow-up to briefs 03/04. Those reports pinned signatures from method
pages but several pages are incomplete or contradictory, and a few APIs
have no accessible page at all (`CreateConic` is the critical one - 854
conic curves in a real design gate on it). This brief asks a different
question: **how is each API actually CALLED in real, cited code** - the
official SDK sample pages, the SOLIDWORKS API help "Examples" sections,
the macro recorder's own output, GitHub-hosted macros, and long-lived
forum solutions. Recovered call patterns beat inferred signatures.

## Ground rules (same as briefs 03-04)

- Sources: help.solidworks.com example/SDK pages, the installed-API
  sample packs referenced by docs, GitHub code search (cite repository +
  permalink), SOLIDWORKS forum threads with accepted solutions, API
  conference/training material that is public. Cite URLs per pattern.
- **No documentation prose excerpts.** Code samples may be reproduced as
  code (that is the point of this brief); surrounding prose paraphrased.
  SOLIDWORKS is a Dassault Systèmes mark; nominative reference only.
- Evidence labels per recovered pattern: **Recorded** (macro recorder
  output or official sample), **Community** (cited third-party usage),
  **Conflicting** (multiple incompatible forms seen - describe both).
- Where examples disagree with the method pages of brief 04, say so
  explicitly - the conflicts are the most valuable output.
- Deliverable: one markdown report; per-API sections with the recovered
  call code first, then the parameter-by-parameter meaning, then open
  questions. A short "still needs a seat probe" table at the end.

## T1. `ISketchManager.CreateConic` and `CreateConicByEqnParams` (blocker)

Recover from ANY usage: the complete ordered parameter list, which
arguments are start/end points vs apex vs control vertex vs the conic
constant, the accepted rho range/meaning (does rho=0.5 mean parabola in
this API?), and which conic types can be produced. Sources to try: the
API help's SketchManager example pages, the "Create a conic" sample if
one exists, GitHub search for `CreateConic` in VBA/VB.NET/C# macro repos,
forum threads about conic sketching via API. Also `CreateConicByEqnParams`
- its parameter array record layout. If literally no usage exists
anywhere, say so plainly: that becomes a seat Object Browser probe and
possibly an approximation policy decision.

## T2. `CreateSplineByEqnParams` parameter layout

The flattened `Parameters` array record: knots, weights, control points -
order, count rules, degree placement. Find a real example (SDK samples
have spline sections) and reconstruct the layout exactly. Also confirm
`CreateSpline3`'s `PointData` arrangement from examples (flat xyz triples
vs object array) and the closed-spline convention.

## T3. `ISimpleFilletFeatureData2` builder - one complete worked example

From official samples or community macros: the full sequence
`CreateDefinition(swFmFillet)` -> cast -> `Initialize(...)` -> option
members (circular section, overflow, tangent propagation) -> edge
selection with mark 1 -> `CreateFeature`. Exact member names and the enum
constants used (`swSimpleFilletType_e`? `swConstRadiusFillet`?), radius
units, and any `SetNextCornerRadius`-style calls. Also the variable-radius
variant only insofar as it shares the builder. Include the display-
dimension naming of the resulting radius if any sample records it.

## T4. `InsertMoveCopyBody2` - precedence and marks from usage

The 2013 and 2022 method pages contradict on translation-vs-rotation
precedence. Find real calls supplying BOTH and any forum/KB resolution.
Recover: what TransDist does versus TransX/Y/Z; whether RotAngleX/Y/Z is
axis-angle or Euler; the angle unit in practice; and how marks 2/4/8
references change the interpretation. Any `IMoveCopyBodyFeatureData`
readback usage.

## T5. `InsertCombineFeature` - ToolVar forms in VBA

Recover real VBA call shapes: direct Body2 arguments vs marked-selection
route; how the tool array is built (`Array(body1, body2)`? Variant?);
`Nothing` placements for add/intersect. And any usage of
`ICombineBodiesFeatureData` for readback.

## T6. `SelectByID2` - the macro recorder's name grammar

The recorder writes full selection strings. Collect the RECORDED forms
for: sketch entities (point/line/arc in a sketch), model edges and
vertices, faces, bodies (multibody), planes (origin and reference), and
dimensions. The goal is a table: entity kind -> recorded name pattern ->
what is stable vs regenerated. This feeds the edge resolver and body
registry work. Include what `SelectByID2`'s arguments look like in
recorded output (the mark/append/option values the recorder emits).

## T7. Two small confirmations

- `IRefPlaneFeatureData`: real usage reading `Distance`/`Type2` back from
  a created plane (the readback we need for plane verification).
- `ISweepFeatureData` builder: one worked example of profile + path +
  guide curves assignment (what object forms does `GuideCurves` take in
  practice), plus the recorder's form for sweeps if it records them.

## Output

One report; per-API sections; a conflicts table (example vs method page);
a final "seat probe still required" table with the minimal observation
for each remaining unknown.
