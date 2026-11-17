# SolidCraft v0.43.0: nested schema for `solidcraft/57`

**Filed:** 2026-11-17, produced by the owner's research agent against the
SolidCraft repository tag `v0.43.0`. Kept in craft-bridge as the evidence
behind the loader's nested-type handling: the marker cutoff, the
centerline/infinite profile exclusion, the `angle` `flip` refusal, the
expr-wins rule (explicitly required by [S34]-[S36]), the `Position` /
`PointExpr` / `Expr` serialization rules, the Boolean-scope precedence for
Phase B, and the `param_names` correction. Part A of the same round's
brief; the SolidWorks API brief is separate.

---

Research date: 2026-11-17
Source: `tangency-labs/solidcraft`, release tag `v0.43.0` (published 2026-11-16).
Purpose: craft-bridge replay-loader implementation for synthetic test designs.
Status: **source-verified serialization and execution behavior**, not a runtime export test. No access to a SolidCraft MCP server or executable was available in this research environment.

**Do not merge two different representations:** native design JSON contains serialized `Feature`, `Sketch`, `Curve`, and `Constraint` records. MCP command input uses a separate API schema. Values such as `{ "at": [...] }` can be valid command arguments without being valid native `Position` fields.

## A1. Curve kinds and complete curve wrapper

A `Sketch` contains `points: SPoint[]`, `curves: Curve[]`, and `constraints: Constraint[]`. The `Curve` wrapper has `id: string`, the flattened `CurveKind` (`type` and its fields), and optional/defaulted `construction: bool`, `reversed: bool`, `link: string?`, `centerline: bool`, `fixed: bool`, `infinite: bool`, and `spline_handle_of: string?`. Default false flags may serialize as false (`construction`) or be omitted where `skip_serializing_if` is present. Curve kinds are `line`, `circle`, `arc`, `ellipse`, `spline`, and `conic`. Curve indices refer to positions in `sketch.curves`, not curve IDs; point indices refer to positions in `sketch.points`, not point IDs. Each curve also has its stable string `id` (for example `a1`). [S1][S2]

| `type` | Required native fields | Defaulted/optional native fields | Semantics |
|---|---|---|---|
| `line` | `a: usize`, `b: usize` | none | Endpoint indices in `points`. |
| `circle` | `c: usize`, `r: f64` | none | Centre point index and numeric radius in mm. |
| `arc` | `c: usize`, `a: usize`, `b: usize` | wrapper `reversed` | Centre and two endpoint indices in `points`. These are **not** angles. |
| `ellipse` | `c: usize`, `m: usize`, `r: f64` | none | Centre index, end of major-axis vector, and numeric minor radius in mm. |
| `conic` | `a: usize`, `b: usize`, `apex: usize`, `rho: f64` | none | Start/end indices, tangent-intersection control point index, and dimensionless conic shape. |
| `spline` | `pts: usize[]` | `control: bool` (default false), `degree: u8` (default 3), `periodic: bool` (default false), `nurbs: Nurbs?`, `handles: usize[]` (default empty) | Indices into `points`; interpretation depends on `control`. |

**Arc sweep and orientation.** The geometric arc is built counter-clockwise from the position at `a` to the position at `b`, around `c`. The kernel calculates the angular difference and adds full turns until the result is positive. This admits major arcs greater than 180 degrees, up to a sweep short of a full turn. The source provides `sketch.arc.center_point` with `sweep` as a signed number in degrees, strictly between -360 and +360 and not zero. For a clockwise authoring command, it swaps the stored endpoints and sets the curve wrapper's `reversed: true`, preserving the logical authored `.start` and `.end` roles. `arc_sweep` constraints use signed logical angle in radians and preserve major/clockwise arcs. The endpoint `b` is projected onto the circle through `a` when the arc is created. [S1][S3][S4]

**Conic.** `a` and `b` are interpolated endpoints. `apex` is where the endpoint tangent lines meet, not normally an interpolated point. `rho` must be strictly between 0 and 1. `rho < 0.5` is elliptical, `rho == 0.5` is parabolic, and `rho > 0.5` is hyperbolic. The source evaluates a rational quadratic Bezier using `w = rho / (1 - rho)`; the point at parameter 0.5 is `mid(a,b) + rho * (apex - mid(a,b))`. MCP's `sketch.conic` validates `rho` more strictly: above `1e-6` and below `1 - 1e-6`. A conic plus a polygonal closure forms a suitable asymmetric test profile. [S2][S5][S6]

**Spline.** `pts` is an array of point indices. With `control: false` (the default), points are interpolation/fit points; SolidCraft builds a cubic, piecewise-Hermite/natural fit through them. With `control: true`, they are B-spline control points and the curve usually does **not** interpolate interior control points. `degree` is a numeric value (`u8`), with default 3. It is declared with `serde(default = "three")` but no `skip_serializing_if`, so the serializer writes the degree even at its default. `control: false`, `periodic: false`, and empty `handles` can be omitted. [S1][S7]

For a control spline with `nurbs: null`/absent, the curve uses a clamped, uniform B-spline over its control points. For `nurbs` present, its native fields are `knots: f64[]` (required) and `weights: f64[]` (optional/omitted when empty). For `n` points and degree `d`, the knot count is `n + d + 1`, knots must not decrease, both end knots repeat `d + 1` times, and no interior knot repeats more than `d` times. If `weights` is nonempty it must contain one positive number per control point; missing weights mean all weights are 1. The code validates degree 1-9 and limits the weight ratio to `1e12`. Source comments describe explicit knots/weights for projected exact free-form edges. A complete inventory of every producer that emits `nurbs` remains **unverified**. [S8]

`handles` holds point indices for editable tangent-handle endpoints of a **fit** spline. The `sketch.spline.handles` command creates these points and a separate construction line for each handle; each line carries `spline_handle_of` naming its spline. Moving a handle alters the derivative used by the fit spline. This field is not a list of tangent vectors or angle values. Periodic fit splines require an exact closing sample and at least three distinct cyclic samples; they must be cubic (`degree: 3`) and must **not** be control splines. [S1][S7][S9][S10]

**Ellipse.** `m` is a point index for the major-axis end. The major semiaxis vector is `points[m].pos - points[c].pos`; its length is the major radius. `r` is a numeric **minor radius**, not a point index or expression. The major/minor axes and their orientations determine the ellipse in the plane. A dimension parameter may drive the numeric radius through a constraint; this does not turn `r` into an expression string. [S1][S5]

**Construction behavior.** `construction: true` is a wrapper field supported by all curve kinds. The profile detector excludes curves marked `construction`, `centerline`, or `infinite`. This means such curves cannot bound an extrudable closed profile. They remain in the sketch and can serve as reference geometry where permitted. A control curve with `construction: true` is not silently deleted. [S1][S11]

## A2. Exhaustive `ConstraintKind` inventory

`Constraint` is a flattened object with `id: string`, `type: string`, fields from `ConstraintKind`, and the wrapper fields `param?: string`, `driven?: bool` (default false), `text?: [f64,f64]`. The source declares **37 variants**. Unless otherwise stated, `usize` fields are indices of `points` or `curves` in the containing sketch. All `value` fields are numeric `f64`: lengths in mm, angles in **radians**. The wrapper's `param` is the name of the model/user parameter that supplies the driving value. `driven: true` makes a reference dimension: it measures current geometry but does **not** constrain it. [S1][S12]

| Native `type` | Field names and serialized types | Index meaning / remarks |
|---|---|---|
| `pattern_point` | `p: usize`, `q: usize`, `center: usize?`, `angle: f64`, `offset: [f64,f64]`, `directions: [null or [usize,[f64,f64]], null or [usize,[f64,f64]]]` (default `[null,null]`) | p,q,center are point indices. Each direction's usize is a line/curve index. Angle radians. |
| `pattern_radius` | `a: usize`, `b: usize` | Curve indices (patterned radius follows seed). |
| `coincident` | `p: usize`, `q: usize` | Point indices. |
| `point_on_curve` | `p: usize`, `c: usize` | Point index, curve index. |
| `point_along` | `p: usize`, `c: usize`, `at: f64` | Point on curve at length fraction from its start, normally 0..1. |
| `horizontal` | `l: usize` | Line curve index. |
| `vertical` | `l: usize` | Line curve index. |
| `horizontal_points` | `p: usize`, `q: usize` | Point indices. |
| `vertical_points` | `p: usize`, `q: usize` | Point indices. |
| `parallel` | `a: usize`, `b: usize` | Curve indices. |
| `perpendicular` | `a: usize`, `b: usize` | Curve indices. |
| `collinear` | `a: usize`, `b: usize` | Curve indices. |
| `tangent` | `a: usize`, `b: usize` | Curve indices. |
| `equal` | `a: usize`, `b: usize` | Curve indices. |
| `concentric` | `a: usize`, `b: usize` | Curve indices. |
| `midpoint` | `p: usize`, `l: usize` | Point index, line curve index. |
| `symmetric` | `p: usize`, `q: usize`, `l: usize` | Point indices, symmetry-line curve index. |
| `fix` | `p: usize` | Point index. Not the same as `SPoint.fixed`. |
| `pierce` | `p: usize`, `sketch: u64`, `curve: string`, `plane: Plane`, `world: [Vec3]` | Current sketch's point, other sketch feature ID and curve string ID; captured world-space geometry. |
| `distance` | `p: usize`, `q: usize`, `value: f64` | Point indices, mm. |
| `distance_x` | `p: usize`, `q: usize`, `value: f64` | Point indices, signed x difference, mm. |
| `distance_y` | `p: usize`, `q: usize`, `value: f64` | Point indices, signed y difference, mm. |
| `point_line_distance` | `p: usize`, `l: usize`, `value: f64` | Point and line curve, mm. |
| `line_circle_distance` | `l: usize`, `c: usize`, `value: f64`, `inside?: bool` | Line and circle/arc curve indices; optional inside branch. |
| `point_circle_distance` | `p: usize`, `c: usize`, `value: f64`, `inside?: bool` | Point and circle/arc; optional inside branch. |
| `circle_circle_distance` | `a: usize`, `b: usize`, `value: f64`, `circles?: "apart"\|"crossing"\|"nested"` | Two circle/arc curves; `apart` is default. |
| `length` | `l: usize`, `value: f64` | Line curve length, mm. |
| `line_angle` | `l: usize`, `value: f64` | Signed angle of line direction from +X, radians. |
| `arc_sweep` | `c: usize`, `value: f64` | Arc curve, signed logical sweep, radians. |
| `slot_overall_length` | `a: usize`, `b: usize`, `value: f64` | Slot end-arc curve indices, mm. |
| `radius` | `c: usize`, `value: f64` | Circle/arc/ellipse curve, mm. |
| `diameter` | `c: usize`, `value: f64` | Curve index, mm. |
| `angle` | `a: usize`, `b: usize`, `value: f64`, `flip?: bool` | Two line curves, counter-clockwise angle in radians; `flip` selects supplementary branch. |
| `smooth` | `a: usize`, `b: usize` | Curve indices, tangent and equal curvature (G2). |
| `arc_length` | `c: usize`, `value: f64` | Arc curve length, mm. |
| `linear_diameter` | `p: usize`, `l: usize`, `value: f64` | Twice point-to-line distance, mm. |
| `offset_distance` | `output: string`, `value: f64`, `reverse?: bool` | An associative offset's output ID, distance mm, optional chosen side. |

**No `fix_angle`, `fix_length`, or generic `equation_link` is present in `ConstraintKind`.** Fixed angles/lengths are represented by appropriate dimensions or fixed geometry. Separately, `Sketch` carries `links`, `equations`, `patterns`, `offsets`, `freeform_offsets`, and `wires`. Their own nested structures are **outside this specified `ConstraintKind` inventory**. Equations are not an undisclosed constraint variant. [S1][S12]

`Sketch.points[0]` is the fixed sketch origin with `id: "origin"`. The native point record is `SPoint {id:string, pos:[f64,f64], fixed:bool? (default false), link?:string, exprs?: {x?:string,y?:string}}`. Both axes may be expressed, only one, or neither. A point with parameter expressions is fixed by its expressions before solving, even without a `fix` constraint. Conversely, `sketch.constraint.fix` on an ordinary point generates `ConstraintKind::Fix {p}`. [S1][S13][S14]

## A3. `custom` planes, other planes, and `GeoRef`

The exact native custom plane is:

```json
{
  "type": "custom",
  "plane": {
    "origin": [9.5, -7.25, 13.5],
    "x_dir": [0.8, 0.6, 0.0],
    "y_dir": [-0.48, 0.64, 0.6]
  }
}
```

`Plane` is a Rust structure with `origin: Vec3`, `x: Vec3` serialized as `x_dir`, and `y: Vec3` serialized as `y_dir`. Each `Vec3` is a numeric JSON array `[x,y,z]`. The vectors are orthonormal in-plane axes; normal = `x_dir cross y_dir`. This object is a **fixed explicit plane**. It does not contain a face reference or a separate `normal` field. MCP's `sketch.create` with a numeric explicit origin produces `PlaneRef::Custom`; origin expressions referring to parameters produce `PlaneRef::Placed` instead. A face chosen with `fixed: true` also produces `Custom`. [S15][S16]

| PlaneRef `type` | Native nested fields and types | Meaning |
|---|---|---|
| `origin` | `name: string` | `XY`, `XZ`, `YZ` origin datum; world plane conventions depend on name. |
| `custom` | `plane: Plane` | Fixed explicit frame. |
| `placed` | `origin: PointExpr`, `x_dir: Vec3`, `y_dir: Vec3` | Parameter-following origin; axes numerical. |
| `offset` | `base: PlaneRef`, `distance: Expr` | Along the base normal. `distance` is a string expression. |
| `through_point` | `base: PlaneRef`, `point: GeoRef` | Parallel plane through resolved point. |
| `at_angle` | `base: PlaneRef`, `axis_origin: Vec3`, `axis_dir: Vec3`, `angle: Expr`, `line?: GeoRef`, `direction_only?: bool`, `pivot?: GeoRef` | Rotates a plane about a line; defaults and pivot behavior depend on `direction_only`. |
| `midplane` | `a: GeoRef`, `b: GeoRef` | Bisector of two planes or planar faces. |
| `three_points` | `a: GeoRef`, `b: GeoRef`, `c: GeoRef` | Plane through three points. |
| `two_edges` | `a: GeoRef`, `b: GeoRef` | Plane through two coplanar straight edges. |
| `axis_point` | `axis: GeoRef`, `point: GeoRef` | Axis and independent point. |
| `along_path` | `path: GeoRef`, `t: Expr` | Perpendicular to a path at a fraction 0..1 of its length, **not millimetres**. |
| `perpendicular` | `plane: GeoRef`, `line: GeoRef` | Plane square to a line and through it. |
| `tangent` | `face: GeoRef`, `at: Vec3`, `reference?: GeoRef`, `angle?: Expr` | Tangent to curved face. |
| `construction` | `feature: u64` | Construction-plane feature ID. |
| `flipped` | `base: PlaneRef`, `about_y?: bool` | Opposite normal, with selected half-turn convention. |
| `datum` | `datum: DatumRef` | Exact occurrence datum serialized as Browser-key string. |
| `face` | `plane: Plane`, `at: Vec3`, `name?: string`, `anchor?: FaceAnchor` | Planar body face, optionally certified. |

`GeoRef` is tagged by `type`, with the following exhaustive variants from the source:

| `type` | Fields, with native types | How it is resolved |
|---|---|---|
| `point` | `p: Vec3` | Fixed point. |
| `datum` | `datum: string` | Exact occurrence datum key, e.g. `o:2/3/origin/XY`. |
| `origin` | none | Design origin. |
| `vertex` | `at: Vec3`, `edges?: string[]` | Shared vertex of named edges; fallback nearest `at`. |
| `edge` | `at: Vec3`, `name?: string` | Named edge, else nearest edge at point. |
| `face` | `at: Vec3`, `name?: string` | Named face, else face at point. |
| `plane` | `plane: PlaneRef` | Nested plane reference. |
| `world` | `axis: string` | World X, Y or Z axis. |
| `axis` | `name: string` | Construction-axis feature by name. |
| `construct_point` | `name: string` | Construction-point feature by name. |
| `sketch_point` | `sketch: u64`, `point: string` | Sketch feature ID plus point ID, or `wire:index` node. |
| `sketch_curve` | `sketch: u64`, `curve: string` | Sketch feature ID plus curve ID. |
| `snap` | `of: GeoRef`, `kind: "centre"\|"middle"`, `at: Vec3`, `end?: usize` | Live snap location; `end` selects a cylinder end where applicable. |

These native `GeoRef` records are **not** just numeric feature indices. Some identify a named face/edge and retain an `at` fallback. An exact face attachment has the separate `PlaneRef::Face.anchor`, which can carry `FaceAnchor {target: ConfigFaceTarget, component:u64, path:u64[], author_path:u64[], placement?:Mat, coordinates:[f64,f64], x_axis:[f64,f64]}`. `ConfigFaceTarget` contains `body:string`, `origin_feature:u64`, `name:string`, `point:Vec3`, `certificate?: FaceCertificate`. When the certified face is lost, the source documents an error instead of free nearest-face guessing. The exact `FaceCertificate` variants were not enumerated in this round. [S17][S18][S19]

## A4. Numeric values, expressions and pick serialization

**`Expr`** is a transparent newtype around `String`, so an ordinary `Expr` field (such as `extent.distance`, `plane.offset.distance`, `move.angle`, or one element of `move.translate`) serializes as a **JSON string**. Its authored expression text is preserved as text. Examples: `"12 mm"`, `"Depth"`, `"(Span / 3) + 0.7 mm"`. [S20]

**`PointExpr`** (feature `corner`, `center`, `base`, move `origin`, and placed-plane `origin`) uses a **special serializer**. It is `[Expr;3]` at runtime, saved as `[number|string, number|string, number|string]`. A coordinate becomes a JSON number only if it parses as finite `f64` and Rust's shortest decimal formatting exactly equals the original stored text. Thus `"9.5"` becomes `9.5`, while `"Width/2"`, `"10 mm"`, and noncanonical numerical text such as `"1.00"` stay JSON strings. On read, a number becomes a numeric expression string. This explains mixed arrays in actual exports. [S21]

**`Position`, `FacePick`, `Dir`** are transparent wrappers around `Vec3`. Each serializes as **bare `[f64,f64,f64]`**, with no `{"at": ...}` wrapper. This covers fillet `edges`, `rotate_axis`, axis directions and ordinary pick points. An `{"at":[x,y,z]}` object may occur in an MCP command or a tagged `GeoRef`, but is **not** the native `Position` layout. In native Move, `translate: [Expr,Expr,Expr]` therefore saves as three **strings**, while optional `origin: PointExpr` can mix strings and numbers. Native `rotate_axis: Dir` contains three numeric components; it is not an expression triple. [S22][S23]

| Context | Native serialized shape | Unit rule |
|---|---|---|
| Sketch `SPoint.pos` | `[number,number]` | mm, evaluated/cached. |
| Sketch `SPoint.exprs` | `{ "x"?: "...", "y"?: "..." }` | Each present value is a length expression; omitting an axis retains cached numeric coordinate. |
| Constraint `value` | `number` | Length mm or angle radians, **not** an `Expr`. |
| Feature `extent.distance` | `"Depth"` or `"12 mm"` | `Expr`, length. |
| Feature `move.translate` | `["XOffset", "0", "2 mm"]` | Three `Expr`s, length. |
| Feature `move.rotate_axis` | `[0,0,1]` | Numeric direction vector. |
| Feature `move.origin`, primitive `center/base/corner` | `[4.2, "OffsetX", 0]` | Special `PointExpr` serializer; mm for plain numeric values. |
| `Plane.origin/x_dir/y_dir`, `GeoRef.at`, `Position` | `[number,number,number]` | Numeric geometry (origin/picks in mm, axes direction unitless). |

For MCP sketch-point creation, a JSON number is read in mm. A text coordinate that names a parameter becomes an `exprs` coordinate and fixes the point. A string that contains a number only (for example `"5"`) is evaluated but is **not** retained as a driving expression. String expressions use the design's length units for bare quantities; explicit units such as `"5 mm"` or `"0.25 in"` override them. With units `mm`, a negative expression is valid if the result lies within the sketch coordinate range. Direct references to other sketch points are **not** part of the numeric expression grammar; use point IDs/constraints/links for point-to-point relationships. [S13][S14][S24]

## A5. Boolean scope, body names and dimension names

`extrude`, `revolve`, `sweep`, and `loft` serialize `targets` as a JSON array of **body-name strings**. The default is an empty array, which may be omitted on save. A feature also has a separate optional `participants: string[]` at its wrapper level. Source precedence for an operation is: use nonempty `targets`; otherwise use nonempty `participants`; otherwise use bodies in **the feature's component**, plus the feature's own bodies. `participants: ["*"]` is a special explicit all-bodies scope. Consequently, `operation: "join"` with `targets: []` does **not** mean no targets, and does **not** mean indiscriminate union of all components. It may operate on all eligible bodies in the feature component. With no eligible bodies, Join may create a new body. A Cut/Intersect with no eligible bodies is an error. When the UI creates a feature, it can record a specific participant scope to preserve visibility-dependent choices. [S25][S26][S27]

A feature has `body_names: string[]`. New bodies receive a pinned name from this array if available and free. Otherwise the evaluator generates sequential `BodyN` names, or names like `Ribs (4)` for additional bodies of a named feature. `reserved_body_names` prevents reuse of names freed by rename. Later features can name bodies created automatically (those names are real lookup strings), but replay should prefer captured stable names. `new_body` creates a new solid rather than changing eligible target bodies. [S28][S29]

**Correction to the proposed hypothesis:** feature-level `param_names` are **names for input parameter slots**, not output measurement fields. The corresponding expression value is held by that feature's kind fields, such as `extent.distance`. `Document::all_param_exprs()` builds its parameter list from top-level `params` **and** each feature's `param_names` paired with the feature's input expression. These `dN` values can be edited and used in other formulas. Sketch constraints use their own wrapper `param` to link a dimension to a top-level/model parameter, often with `model: true`. `params[].model: true` means the parameter was generated by a feature/dimension, not added as a regular user parameter. [S30][S31]

## A6. Timeline marker, cached positions, expression edge cases and text parameters

**`marker`.** Native `Document.marker` is `usize?`, omitted when `None`. It is a **zero-based index** into `features`. A feature at index `marker` and every feature after it are rolled back and **not evaluated**. Features before it still evaluate. Example: `marker: 3` evaluates `features[0]`, `[1]`, and `[2]`, but not `[3]` or later. The final model used for ordinary STEP export excludes rolled-back feature geometry. This describes the normal evaluated model, not a separately staged preview or an unusual export path. [S32][S33]

**`pos` versus `exprs`.** On evaluation, the engine clones the stored sketch, evaluates its point expressions, updates the clone's `pos`, and solves it. The JSON writer serializes the stored `Document` directly. A parameter edit often calls `Session::settle_value_edit()` and can adopt the solved sketch into the stored document, but the adoption test does **not** explicitly check changes to an expression-bound point's `pos` alone. Therefore source review does **not** guarantee that `points[].pos` is recomputed before every save. An exported file may contain a stale numeric `pos` beside authoritative `exprs` that produces the correct evaluated geometry on load. **Replay must evaluate `exprs` over `pos`; a loader must not treat a disagreement as corruption without testing the exact saved document.** This is a source-derived caution, not a demonstrated exported mismatch. [S20][S34][S35][S36]

**One-axis expressions.** `exprs: {"x":"Offset"}` drives X and leaves Y at stored `pos.y`. `exprs: {"y":"Offset"}` drives Y and leaves X at `pos.x`. With both keys, both values follow parameters. The engine sets `SPoint.fixed = true` when any expression exists. Unfixing such a point through MCP releases its expressions and keeps the evaluated coordinates as ordinary numbers. Zero and negative results are valid as long as the result is finite and in range. Expressions reference named parameters, not arbitrary sketch point positions. [S1][S14][S24]

**`text_params`.** Each entry is `{"name":string,"text":string,"comment"?:string}`. These parameters have no numeric `unit` and cannot be operands in normal numerical `Expr`s. They can feed sketch text strings through separate text expressions, including concatenation of quoted literals/text parameter names and formatted numeric parameters (`number(Width,2)`). Features whose geometry comes from sketch text can therefore respond to edits of `text_params`, but `text_params` do not directly drive an extrude distance or a numeric sketch point coordinate. [S37]

## MCP confirmation: can it create `fixed: true` plus `exprs`?

**Yes, source-confirmed.** The release source includes an automated test that calls `parameters.add`, `sketch.create`, then `sketch.point` with `{ "point": ["W / 2", "H / 3"], "id": "c" }`. The test asserts that the stored point has `fixed: true` and `exprs.x == "W / 2"`, and verifies motion after changing `W`. This tests the engine command API that the MCP calls. It does **not** prove that this research environment exercised a live MCP seat. [S38]

## Open issues and required runtime probes

1. Confirm saved `format` equals exactly `solidcraft/57` using the installed v0.43.0 executable. Source and tag inspection cannot replace this check.
2. Confirm STEP parity by exporting the unchanged evaluated model in the same live session used for the native save.
3. Check the actual native `SPoint.pos` after changing only one coordinate expression and saving. Record any stale baseline numeric value without rejecting the replay.
4. Check `sketch.arc.center_point` major-arc and logical `reversed` references against the saved `arc` curve.
5. For a real sketch's `nurbs` and `handles`, compare its exact source fields against this inventory. The list of all creation paths that emits them is not fully established.
6. Check a certified `PlaneRef::Face` attachment with `anchor` if the loader supports certified face identity. That nested certificate exceeds this narrow task.
7. Measure numeric mass/volume through the SolidCraft engine, not through schema parsing. A shape-parameter expression can evaluate and still fail the solver/kernel.

## Source references (v0.43.0, immutable release tag)

- [S1] [`crates/sketch/src/model.rs` L95-L275, L331-L790, L820-L855](https://github.com/tangency-labs/solidcraft/blob/v0.43.0/crates/sketch/src/model.rs#L95-L275): `SPoint`, `CurveKind`, `Curve`, `ConstraintKind`, wrapper fields, `Sketch`.
- [S2] [`crates/sketch/src/model.rs` L145-L235](https://github.com/tangency-labs/solidcraft/blob/v0.43.0/crates/sketch/src/model.rs#L145-L235): curve field types and point indexing.
- [S3] [`crates/sketch/src/model.rs` L1959-L1990](https://github.com/tangency-labs/solidcraft/blob/v0.43.0/crates/sketch/src/model.rs#L1959-L1990): analytic arc sweep.
- [S4] [`crates/engine/src/cmd/sketch.rs` L188-L198, L2008-L2069](https://github.com/tangency-labs/solidcraft/blob/v0.43.0/crates/engine/src/cmd/sketch.rs#L2008-L2069): MCP arc commands and direction.
- [S5] [`crates/sketch/src/curves.rs` L94-L136](https://github.com/tangency-labs/solidcraft/blob/v0.43.0/crates/sketch/src/curves.rs#L94-L136): ellipse, conic equations.
- [S6] [`crates/engine/src/schema/sketch_create.rs` L1053-L1118](https://github.com/tangency-labs/solidcraft/blob/v0.43.0/crates/engine/src/schema/sketch_create.rs#L1053-L1118): conic command schema and rho bounds.
- [S7] [`crates/sketch/src/curves.rs` L225-L279, L423-L478](https://github.com/tangency-labs/solidcraft/blob/v0.43.0/crates/sketch/src/curves.rs#L225-L279): fit/control spline interpolation.
- [S8] [`crates/sketch/src/nurbs.rs` L1-L70](https://github.com/tangency-labs/solidcraft/blob/v0.43.0/crates/sketch/src/nurbs.rs#L1-L70): explicit NURBS knots/weights and validation.
- [S9] [`crates/sketch/src/model.rs` L2284-L2332](https://github.com/tangency-labs/solidcraft/blob/v0.43.0/crates/sketch/src/model.rs#L2284-L2332): fit-spline handle points and construction lines.
- [S10] [`crates/sketch/src/periodic.rs` L1-L40, L101-L114](https://github.com/tangency-labs/solidcraft/blob/v0.43.0/crates/sketch/src/periodic.rs#L101-L114): periodic spline rules.
- [S11] [`crates/sketch/src/profiles.rs` L883-L920](https://github.com/tangency-labs/solidcraft/blob/v0.43.0/crates/sketch/src/profiles.rs#L883-L920): construction/profile exclusion.
- [S12] [`crates/sketch/src/model.rs` L331-L785](https://github.com/tangency-labs/solidcraft/blob/v0.43.0/crates/sketch/src/model.rs#L331-L785): all 37 constraints, `param`, `driven`.
- [S13] [`crates/engine/src/cmd/sketch_create.rs` L311-L332](https://github.com/tangency-labs/solidcraft/blob/v0.43.0/crates/engine/src/cmd/sketch_create.rs#L311-L332): expressions vs literal point coordinates.
- [S14] [`crates/engine/src/cmd/sketch.rs` L795-L812, L3349-L3418](https://github.com/tangency-labs/solidcraft/blob/v0.43.0/crates/engine/src/cmd/sketch.rs#L795-L812): fixed expression points and explicit Fix constraints.
- [S15] [`solidcore/crates/solidcore-record/src/plane.rs` L1-L50](https://github.com/tangency-labs/solidcraft/blob/v0.43.0/solidcore/crates/solidcore-record/src/plane.rs#L1-L50): `Plane` axes and native keys.
- [S16] [`crates/doc/src/document.rs` L40-L154](https://github.com/tangency-labs/solidcraft/blob/v0.43.0/crates/doc/src/document.rs#L40-L154): all `PlaneRef` variants.
- [S17] [`crates/doc/src/construct.rs` L1-L99, L195-L235](https://github.com/tangency-labs/solidcraft/blob/v0.43.0/crates/doc/src/construct.rs#L1-L99): `GeoRef`, `SnapAt`, `DatumRef`.
- [S18] [`crates/doc/src/face_anchor.rs` L1-L34](https://github.com/tangency-labs/solidcraft/blob/v0.43.0/crates/doc/src/face_anchor.rs#L1-L34): face-certification anchor fields.
- [S19] [`crates/doc/src/config_styles.rs` L31-L41](https://github.com/tangency-labs/solidcraft/blob/v0.43.0/crates/doc/src/config_styles.rs#L31-L41): `ConfigFaceTarget`.
- [S20] [`crates/doc/src/expr.rs` L14-L48, L1071-L1248](https://github.com/tangency-labs/solidcraft/blob/v0.43.0/crates/doc/src/expr.rs#L14-L48): `Expr` as string, units and parameter evaluation.
- [S21] [`crates/doc/src/document.rs` L2854-L2906](https://github.com/tangency-labs/solidcraft/blob/v0.43.0/crates/doc/src/document.rs#L2854-L2906): canonical mixed-number/string `PointExpr` serializer.
- [S22] [`crates/doc/src/points.rs` L1-L62](https://github.com/tangency-labs/solidcraft/blob/v0.43.0/crates/doc/src/points.rs#L1-L62): transparent `Position`, `FacePick`, `Dir`.
- [S23] [`solidcore/crates/solidcore-record/src/vec.rs` L1-L49](https://github.com/tangency-labs/solidcraft/blob/v0.43.0/solidcore/crates/solidcore-record/src/vec.rs#L1-L49): arrays for `Vec2`, `Vec3`.
- [S24] [`crates/doc/src/document.rs` L5033-L5056](https://github.com/tangency-labs/solidcraft/blob/v0.43.0/crates/doc/src/document.rs#L5033-L5056): reevaluation of coordinate `exprs`.
- [S25] [`crates/doc/src/document.rs` L817-L855](https://github.com/tangency-labs/solidcraft/blob/v0.43.0/crates/doc/src/document.rs#L817-L855): extrude `targets` and operation.
- [S26] [`crates/doc/src/eval.rs` L4768-L4806](https://github.com/tangency-labs/solidcraft/blob/v0.43.0/crates/doc/src/eval.rs#L4768-L4806): Boolean fallback scope.
- [S27] [`crates/engine/src/cmd/features.rs` L826-L849](https://github.com/tangency-labs/solidcraft/blob/v0.43.0/crates/engine/src/cmd/features.rs#L826-L849): capture of scope at creation.
- [S28] [`crates/doc/src/document.rs` L3283-L3360](https://github.com/tangency-labs/solidcraft/blob/v0.43.0/crates/doc/src/document.rs#L3283-L3360): body_names, participants, input names.
- [S29] [`crates/doc/src/eval.rs` L4158-L4190](https://github.com/tangency-labs/solidcraft/blob/v0.43.0/crates/doc/src/eval.rs#L4158-L4190): automatic body names.
- [S30] [`crates/doc/src/params.rs` L125-L171, L683-L708](https://github.com/tangency-labs/solidcraft/blob/v0.43.0/crates/doc/src/params.rs#L683-L708): feature input dimension parameters.
- [S31] [`crates/doc/src/document.rs` L1-L35, L4513-L4592](https://github.com/tangency-labs/solidcraft/blob/v0.43.0/crates/doc/src/document.rs#L4513-L4592): model parameter provenance.
- [S32] [`crates/doc/src/document.rs` L3518-L3551](https://github.com/tangency-labs/solidcraft/blob/v0.43.0/crates/doc/src/document.rs#L3518-L3551): marker field.
- [S33] [`crates/doc/src/eval.rs` L3290-L3417](https://github.com/tangency-labs/solidcraft/blob/v0.43.0/crates/doc/src/eval.rs#L3290-L3417): evaluation skips `index >= marker`.
- [S34] [`crates/doc/src/eval.rs` L10910-L10985](https://github.com/tangency-labs/solidcraft/blob/v0.43.0/crates/doc/src/eval.rs#L10910-L10985): evaluates a sketch clone.
- [S35] [`crates/doc/src/document.rs` L4347-L4370](https://github.com/tangency-labs/solidcraft/blob/v0.43.0/crates/doc/src/document.rs#L4347-L4370): raw document JSON writer.
- [S36] [`crates/engine/src/lib.rs` L1761-L1819](https://github.com/tangency-labs/solidcraft/blob/v0.43.0/crates/engine/src/lib.rs#L1761-L1819): parameter change adoption path.
- [S37] [`crates/doc/src/text_params.rs` L1-L43](https://github.com/tangency-labs/solidcraft/blob/v0.43.0/crates/doc/src/text_params.rs#L1-L43): text-only parameter shape.
- [S38] [`crates/engine/src/cmd/sketch_point_expr_tests.rs` L39-L99](https://github.com/tangency-labs/solidcraft/blob/v0.43.0/crates/engine/src/cmd/sketch_point_expr_tests.rs#L39-L99): fixed+exprs API regression test.
