# SolidCraft `solidcraft/57`: format and replay-verification research

**Filed:** 2026-11-17, produced by the owner's research agent against the
SolidCraft repository tag `v0.43.0` and published documentation. Kept in
craft-bridge as the evidence behind the `solidcraft/57` loader decisions
(docs/replay-profile.md, "Recipe shapes"). Source-confirmed findings below
shaped: the `cut` enum spelling, the expr-over-pos precedence rule, the
`fix` constraint no-op, the marker refusal, the id bound, `t` as a path
fraction, and the edge-ref caution.

---

**Research date:** 17 November 2026
**Target:** Native SolidCraft feature-based JSON, with exact-geometry STEP exports
**Purpose:** Synthetic verification fixtures for the craft-bridge replay program. No client designs.
**Source baseline:** SolidCraft repository tag `v0.43.0`, compared with current published user documentation.
**Research method:** Read the Rust serializer, expression evaluator, sketch model, native package writer, model evaluator, format migration code, official feature guide, and MCP tool reference. **No SolidCraft MCP session was available to this research pass.** Therefore, source-derived results are marked *source-confirmed*, but no actual native exports or STEP files were produced or measured.

## 0. Findings that affect the test strategy

| Finding | Status | Consequence |
|---|---|---|
| A native `.json` design has `format: "solidcraft/N"`, top-level `params` and `features`; each serialized feature flattens its `type` and type-specific fields into the record. | Source-confirmed | Verify serialized feature records, not a list of API calls. |
| A command script and a saved native design are **different formats**. | Source-confirmed | Do not infer a `solidcraft/53` command-to-feature conversion. No evidence supports that premise. |
| The code at `v0.43.0` includes the format-57 migration directory. | Source-confirmed | Pin the build to v0.43.0 or verify `format` after a real save. |
| The serialized Boolean enum values are `new_body`, `join`, **`cut`**, and `intersect`. | Source-confirmed | The user's word `subtract` describes a cut, but is **not** the serialized enum value. |
| `sketch.points[].exprs.x` and `.y` contain authored expression strings; a point with expressions is fixed to its evaluated position before solve. | Source-confirmed | Distinguish genuinely parameter-following geometry from numeric coordinates. |
| `plane.type: "along_path"` uses `t` as a **fraction along the path, 0 through 1**, not millimetres. | Source-confirmed | Probe edits such as `t=0.37` to `t=0.63`. |
| `edge_refs[].min/max` describe the picked **body's** bounding box. They are **not** the picked edge's bounding box. | Source-confirmed | Never use this object alone as proof of unique edge identity. |
| `move` has a separate `translate` vector, `rotate_axis`, `origin`, and `angle`. | Source-confirmed | Verify both rotation and translation against the STEP result. |
| `keep_tools` belongs to `combine`; ordinary `extrude` does **not** have that field. | Source-confirmed | Test this only with a combine operation, not an invented extrude field. |
| `.sc` is a versioned design container. Its embedded `document.json` is authoritative; optional geometry/preview caches are not. | Source-confirmed | Treat `.json` as the replay source and `.step` as independent exported geometry. |
| MCP `save` supports native files and `export` supports STEP. | Officially documented | Save and export the same unchanged baseline. |

**Status legend:** *Source-confirmed* means an explicit field, code path, or behavioral comment was found in the version-pinned source. *Documented* means the official guide describes it. *Unverified* means this research pass did not exercise an installed release. Source confirmation does not replace a runtime compatibility test.

## 1. Native document schema

### 1.1 Root object

The `Document` type has the following fields. The stored representation is ordinary JSON for a `.json` design. Some optional fields are omitted when empty. The exact serializer determines whether a field is omitted or present with a default.

| Category | Fields | Notes |
|---|---|---|
| Identity | `format`, `format_pending`, `name`, `units`, `metadata` | `format` is `solidcraft/N`; use `units: "mm"`. |
| Parametric model | `params`, `text_params`, `features`, `marker`, `next_id`, `reserved_body_names`, `param_comments`, `favorites`, `configs` | `marker` is a timeline rollback index, not a suppression flag. |
| Component structure | `components`, `next_component_color_index`, `occurrences`, `assembly`, `body_components`, `body_offsets`, `parts`, `links` | Components and occurrences affect placed geometry. |
| Material and display | `render`, `sheet`, `plastic`, `materials`, `print_faces`, `user_materials`, `appearances`, `visibility`, `named_views` | Display state is not the STEP geometry authority. |
| Organization and extras | `browser_groups`, `timeline_groups`, `browser_order`, `canvases`, `point_clouds`, `notes`, `plugin_data` | Some assets can be stored as container blobs in `.sc`. |

The fields `format`, `name`, `units`, `params`, and `features` are the key replay inputs. A minimal native design should still be **created and saved by SolidCraft**, rather than hand-assembled from this summary. The source defines `MAX_FEATURES = 10,000`, `MAX_PARAMS = 10,000`, and `MAX_FEATURE_ID = 2^52`. A sparse ID such as 9001 is within that bound but must still pass the live reader and timeline tests.

**Source:** [`Document`, `Param`, and size limits](https://github.com/tangency-labs/solidcraft/blob/v0.43.0/crates/doc/src/document.rs#L20-L43), [`Document` root fields](https://github.com/tangency-labs/solidcraft/blob/v0.43.0/crates/doc/src/document.rs#L3518-L3647).

### 1.2 Feature wrapper

Every ordinary feature record includes:

- **Required:** `id` (integer) and `name` (string), plus its flattened `type` tag and required fields of that type.
- **Optional or defaulted:** `suppressed`, `body_names`, `param_names`, `component`, `edge_refs`, `edge_names`, `face_names`, `participants`, `touching`, and compatibility or history fields: `shell_piece_order`, `kernel_piece_order`, `pattern_names_in_order`, `inserted`, `link_faces`, `link_threads`.
- **`body_names`:** Names assigned to bodies generated by this feature, in creation order. A feature can create multiple bodies.
- **`param_names`:** Generated names associated with feature inputs, such as `d3`.
- **`participants`:** Explicit cross-component Boolean scope. An empty list restricts operations to the feature's component. This is distinct from `targets` in the feature's own fields.
- **`component`:** Component ID. The default value 0 is the root.
- **`suppressed`:** The evaluator skips the feature. A downstream reference can still fail if its source output disappears.

**Source:** [`Feature` and flattening of `FeatureKind`](https://github.com/tangency-labs/solidcraft/blob/v0.43.0/crates/doc/src/document.rs#L3283-L3360), [`suppression evaluation`](https://github.com/tangency-labs/solidcraft/blob/v0.43.0/crates/doc/src/eval.rs#L3370-L3425).

### 1.3 Full feature-kind inventory

The table lists every top-level `FeatureKind` variant found at the pinned tag. **Required** means the field has no `serde(default)` on that variant. **Optional** means it has a default or is omitted when absent. A type may still impose conditional constraints, reference requirements, or value ranges. The nested structures are not exhaustively enumerated in this table. A `newer_file_feature` is a compatibility placeholder, not an ordinary modeling command.

| Serialized `type` | Required fields | Optional / defaulted fields |
|---|---|---|
| `newer_file_feature` | `raw` | — |
| `body_transfer` | `bodies`, `from`, `to`, `transform` | — |
| `sketch` | `plane`, `sketch` | — |
| `extrude` | `sketch`, `profiles`, `extent` | `operation`, `targets`, `thin` |
| `revolve` | `sketch`, `profiles`, `axis`, `angle` | `operation`, `targets`, `angle2`, `symmetric`, `convention`, `to` |
| `fillet` | `edges`, `radius` | `body`, `style`, `fit`, `corner`, `setbacks`, `turn`, `tangent_chain` |
| `chamfer` | `edges`, `distance` | `distance2`, `angle`, `flip`, `body`, `tangent_chain` |
| `box` | `corner`, `length`, `width`, `height` | `operation`, `centred` |
| `cylinder` | `base`, `radius`, `height` | `axis`, `operation`, `references` |
| `sphere` | `center`, `radius` | `operation` |
| `torus` | `center`, `major`, `minor` | `operation` |
| `cone` | `base`, `radius`, `top_radius`, `height` | `axis`, `operation` |
| `combine` | `target`, `tools`, `operation` | `keep_tools` |
| `pattern` | `features`, `pattern` | `bodies` |
| `mirror` | `features`, `plane` | `bodies`, `combine`, `placements` |
| `hole` | `position`, `direction`, `diameter` | `at`, `depth`, `hole`, `points`, `thread`, `thread_depth`, `hand`, `thread_class`, `modeled`, `ease_out`, `to`, `references`, `old_surface_start` |
| `thread` | `face` | `designation`, `length`, `modeled`, `continuous_flanks`, `from`, `up_to`, `form`, `hand`, `starts`, `ease_out`, `tip`, `chamfer_angle`, `chamfer_depth`, `steep_flank`, `old_auto_size`, `additional_faces`, `target_bodies`, `single_face_selection` |
| `loft` | `sections` | `operation`, `targets`, `rails`, `rail_fit` |
| `sweep` | `sketch`, `profiles`, `path_sketch`, `path` | `path_body`, `path_edges`, `orientation`, `twist`, `scale`, `rails`, `rail_fit`, `operation`, `targets` |
| `shell` | `faces`, `thickness` | `body`, `direction`, `tangent_chain` |
| `draft` | `faces`, `angle`, `neutral`, `pull` | `body` |
| `construction_plane` | `plane` | — |
| `split` | `body`, `plane` | `tool`, `sketch`, `line` |
| `construction_axis` | `def` | — |
| `construction_point` | `def` | — |
| `split_face` | `faces`, `plane` | `body`, `tool`, `sketch`, `loops` |
| `scale` | `bodies`, `factor` | `origin`, `factors` |
| `offset_face` | `faces`, `distance` | `body` |
| `delete_face` | `faces` | `body`, `heal` |
| `bounding_solid` | `bodies` | `margin` |
| `pipe` | `path_sketch`, `path`, `diameter` | `wall`, `operation`, `targets` |
| `frame_end_cap` | `ends`, `thickness`, `chamfer` | `offset`, `ratio`, `direction` |
| `frame_member` | `path_sketch`, `profile` | `lines`, `anchor`, `locate`, `angle`, `mirror`, `body_name`, `names` |
| `emboss` | `sketch`, `profiles`, `depth` | `deboss`, `face`, `targets` |
| `rib` | `sketch`, `curves`, `thickness` | `depth`, `flip`, `web`, `normal` |
| `replace_face` | `faces`, `target` | `body` |
| `align` | `bodies`, `from`, `to` | `from_normal`, `to_normal`, `flip`, `from_plane`, `to_plane`, `axes`, `from_frame`, `to_frame`, `angle` |
| `cell_grid` | `sketch`, `profiles`, `size`, `web` | `mode`, `shape`, `sides`, `dot_grid`, `angle`, `anchor`, `clip`, `keepout_clip`, `snap_inset`, `margin`, `depth`, `through_all`, `direction`, `targets` |
| `coil` | `base`, `pitch`, `turns` | `axis`, `sketch`, `profiles`, `diameter`, `section_size`, `section`, `position`, `start_angle`, `clockwise`, `operation`, `targets` |
| `sheet_base` | `sketch`, `profiles` | `rule`, `flip` |
| `sheet_contour` | `sketch`, `curves`, `distance` | `rule`, `flip`, `reverse` |
| `sheet_flange` | `edges`, `height`, `angle` | `radius`, `position`, `flip`, `body`, `corner_relief_shape`, `corner_relief`, `height_from` |
| `sheet_hem` | `edges`, `length` | `gap`, `body` |
| `sheet_perforate` | `diameter`, `pitch` | `margin`, `pattern`, `body`, `sketch`, `profiles` |
| `sheet_fold` | `a`, `b`, `angle`, `position` | `sketch`, `curve`, `radius`, `flip`, `fixed`, `body` |
| `sheet_path` | `edge`, `sketch`, `curves` | `radius`, `body` |
| `sheet_unfold` | — | `body`, `refold` |
| `sheet_convert` | `body`, `face` | `rule`, `bends` |
| `boss` | `position`, `diameter`, `height` | `direction`, `hole_diameter`, `hole_depth`, `draft`, `fillet`, `ribs`, `rib_thickness`, `rib_length`, `rib_offset`, `body` |
| `lip` | `face`, `width`, `height` | `groove`, `gap`, `outside`, `body` |
| `snap_fit` | `position`, `hook`, `length`, `thickness`, `width`, `catch_depth`, `catch_length` | `direction`, `base_height`, `new_body`, `body` |
| `rest` | `position`, `width`, `height` | `direction`, `along`, `length`, `draft`, `thickness`, `body` |
| `boundary_fill` | `tools`, `cells` | `operation`, `remove_tools` |
| `remove` | `bodies` | — |
| `patch` | — | `sketch`, `profiles`, `profiles_3d`, `body`, `edges`, `crown`, `continuity`, `points` |
| `stitch` | `bodies` | `tolerance` |
| `thicken` | `bodies`, `thickness` | `symmetric`, `operation`, `targets` |
| `surface_extrude` | `sketch`, `extent` | `profiles`, `curves` |
| `unstitch` | `body` | `faces` |
| `reverse_normal` | `body` | `faces` |
| `surface_untrim` | `face` | `holes`, `outer`, `in_place` |
| `surface_revolve` | `sketch`, `axis`, `angle` | `profiles`, `curves`, `angle2`, `symmetric` |
| `surface_sweep` | `sketch`, `path_sketch`, `path` | `profiles`, `curves`, `path_body`, `path_edges`, `orientation`, `twist`, `scale`, `rails`, `rail_fit` |
| `surface_loft` | `sections` | `continuity`, `crown`, `rails`, `rail_fit` |
| `surface_offset` | `faces`, `distance` | `body`, `chain`, `tolerance` |
| `surface_ruled` | `body`, `edges`, `distance` | `mode`, `angle`, `direction`, `flip`, `corners`, `face`, `far` |
| `surface_extend` | `body`, `edges`, `distance` | `mode`, `up_to`, `neighbours` |
| `surface_blend` | `from`, `to` | `continuity`, `bulge`, `align`, `seam`, `stitch` |
| `surface_boundary` | `sketch`, `curves` | `orders`, `crown`, `samples` |
| `surface_network` | `u` | `v`, `operation` |
| `surface_match` | `body`, `target`, `edges` | `faces`, `order` |
| `surface_fair` | `body`, `tolerance` | `faces`, `strength`, `iterations`, `keep_edges` |
| `surface_trim` | `body`, `plane`, `keep` | `tool`, `keep_more`, `curves`, `with`, `keep_with`, `stitch`, `extend` |
| `move` | `bodies`, `translate` | `rotate_axis`, `origin`, `angle`, `copy`, `copy_frames` |
| `import` | `file`, `step` | `components`, `skip` |
| `mesh_convert` | `body`, `tolerance` | — |
| `mesh_import` | `file`, `meshes` | — |
| `repair` | `body`, `gap` | `patch_holes` |

**Source:** [`FeatureKind` variants](https://github.com/tangency-labs/solidcraft/blob/v0.43.0/crates/doc/src/document.rs#L802-L2250). A full exhaustive schema of every nested custom type, including `PatternKind`, `HoleKind`, sheet-metal rules, assembly data, and all `GeoRef` variants, needs a separate source extraction. For this replay suite the exact nested types needed are described below.

### 1.4 Sketch record, points, and curves

`FeatureKind::Sketch` has fields `type: "sketch"`, `plane` and `sketch`. The nested `sketch` has mandatory `points`, `curves`, and `constraints` arrays. Optional data include `patterns`, `links`, `offsets`, `freeform_offsets`, `wires`, `equations`, counters, and `view`.

A sketch point serializes like this **structural excerpt only**:

```json
{
  "id": "p2",
  "pos": [31.7, 8.4],
  "fixed": true,
  "exprs": {
    "x": "PlaqueWidth - 3.4 mm",
    "y": "PlaqueHeight / 2"
  }
}
```

`pos` is the stored numeric position and must not be treated as the only driving source when `exprs` exists. Point 0 is the fixed origin `id: "origin"`. In `PointExprs`, `x` and `y` are optional independent expression strings; a coordinate without an expression retains its numeric position in millimetres. The engine evaluates expression-controlled points before sketch solving and fixes them at the resulting positions.

`CurveKind` is also tagged by `type`. Its variants are `line {a,b}`, `circle {c,r}`, `arc {c,a,b}`, `ellipse {c,m,r}`, `spline {pts,control?,degree?,periodic?,nurbs?,handles?}`, and `conic {a,b,apex,rho}`. Point references are **indices into the sketch's points array**, not point names. Each curve has `id` plus its flattened variant data, and optional `construction` or other view/history fields. A line can serialize as `{"id":"l1","type":"line","a":1,"b":2}` in its parent `curves` array. Do not hand-construct a full sketch from this excerpt without testing the reader.

**Source:** [`SPoint`, `PointExprs`, `CurveKind`, `Curve`](https://github.com/tangency-labs/solidcraft/blob/v0.43.0/crates/sketch/src/model.rs#L95-L258), [`Sketch` structure](https://github.com/tangency-labs/solidcraft/blob/v0.43.0/crates/sketch/src/model.rs#L820-L868).

### 1.5 Profile selection and extrude extent

`profiles.type` has six supported serialized variants:

| Value | Additional fields | Meaning |
|---|---|---|
| `all` | — | All closed profiles. |
| `indices` | `indices` | Indexes of evaluated sketch profiles. |
| `curves` | `loops` | Exact source curve sets; zero or multiple matches cause an error. |
| `offset_boundaries` | `associations` | Persistent offset associations. |
| `whole_boundaries` | `profiles`, each with `outer` and `holes` | Entire source boundaries including holes. |
| `points` | `points` | Select profile containing each point. |

`extent` is a structured value. It always has `distance: Expr` and can include `convention`, `direction`, `distance2`, `start_offset`, `through_all`, `second_follows_first`, `taper`, `corner`, `to`, `to_offset`, `from`, `to_body_apart`, and `picked_placements`. The accepted `direction` values are `positive`, `negative`, `symmetric`. The accepted `convention` values are `legacy_per_side` and `total_distance`. For symmetric extrusion, `legacy_per_side` uses the stored distance on **each** side, while `total_distance` splits the distance between both sides. For the tests, prefer one-sided positive extrusion to avoid convention ambiguity, while preserving whichever convention the exporter writes.

**Source:** [`ProfileSel`, `Direction`, `ExtrudeConvention`, `Extent`](https://github.com/tangency-labs/solidcraft/blob/v0.43.0/crates/doc/src/document.rs#L174-L405).

## 2. Parameter expression language

The parameter record is `{name: string, expr: string, unit: string, comment?: string, model?: boolean}`. `model: false` is a user-created parameter. `model: true` marks a parameter created by a dimension or feature, such as `d1` or `d2`. The boolean is **provenance**, not an instruction to omit the parameter from modeling. The evaluator works with `expr` and `unit` in both cases.

**Operators and precedence:** parentheses, unary `+` and `-`, power `^`, multiplication `*`, division `/`, remainder `%`, addition `+`, subtraction `-`. Implicit multiplication is accepted, so `2 Width` and `10 mm` are meaningful. Number-and-unit grouping binds tightly, including compound expressions. Decimal scientific notation is supported. A leading `=` can mark a formula. Operators `×`, `·`, `÷`, and the Unicode minus are normalized by the lexer. A semicolon may separate function arguments. The parser also permits locale decimal commas in defined contexts. For repeatable fixtures, prefer period decimals and a normal comma between function arguments.

**Functions:** `sin`, `cos`, `tan`, `asin`, `acos`, `atan`, `atan2`, `sqrt`, `abs`, `min`, `max`, `floor`, `ceil`, `round`, `exp`, `sinh`, `cosh`, `tanh`, `asinh`, `acosh`, `atanh`, `ln`, `log`, `log10`, `sign`, `hypot`, `pow`, `trunc`, `mod`. Constants: `pi` and `PI`.

**Units:** `mm`, `cm`, `m`, `um`, `µm`, `in`, `inch`, `ft`, `yd`, `km`, `mil`, `nm`, `deg`, `rad`, plus accepted degree spellings and inch/foot glyphs. Internal values use millimetres and radians. The evaluator checks dimensions. A bare number beside a dimensioned value can inherit the relevant unit. A bare trigonometric argument is interpreted as degrees. Source expressions remain strings and are not normalized on save.

**Evaluation:** Parameter dependencies are resolved recursively in dependency order. Unknown names and circular dependencies generate errors. The evaluator reports a circular path such as `a → b → a`. Limits in the source include 4,096 bytes for one expression, 64 parser nesting levels, and 64 function arguments.

A suitable bonus expression is `(PlaqueWidth / 3 + 2.1 mm) / 2`. This evaluates to a length, keeps parentheses and units in the JSON expression, and changes when `PlaqueWidth` changes. Avoid expressions with ambiguous mixed units for basic test fixtures.

**Source:** [`expression syntax, functions, units`](https://github.com/tangency-labs/solidcraft/blob/v0.43.0/crates/doc/src/expr.rs#L1-L190), [`lexer and precedence`](https://github.com/tangency-labs/solidcraft/blob/v0.43.0/crates/doc/src/expr.rs#L360-L805), [`evaluation and dependencies`](https://github.com/tangency-labs/solidcraft/blob/v0.43.0/crates/doc/src/expr.rs#L1070-L1260).

## 3. All serialized plane reference types

Plane references use a tagged `type`. The following inventory comes directly from `PlaneRef`. Nested `GeoRef`, `DatumRef`, and `Plane` have separate definitions and should be acquired from source or live output if used.

| `plane.type` | Main fields | Meaning / caution |
|---|---|---|
| `origin` | `name` | Exact values `XY`, `XZ`, `YZ`. Their handedness is not interchangeable. |
| `custom` | `plane` | Explicit geometric plane, such as a face-derived plane. |
| `datum` | `datum` | Plane of a placement datum, re-resolved on rebuild. |
| `placed` | `origin`, `x_dir`, `y_dir` | Parameterized 3-D origin and two orientation axes. |
| `offset` | `base`, `distance` | Offset by signed expression distance along `base` normal. **No separate `flip` field on this variant.** |
| `through_point` | `base`, `point` | Parallel to base through a referenced point. |
| `at_angle` | `base`, `axis_origin`, `axis_dir`, `angle`; optional `line`, `direction_only`, `pivot` | Rotate base around referenced or fixed axis. |
| `construction` | `feature` | Refer to a separate `construction_plane` feature by feature ID. |
| `flipped` | `base`, optional `about_y` | Reverse normal by half-turn about the base x or y axis. |
| `midplane` | `a`, `b` | Midway between two referenced planes or faces. |
| `two_edges` | `a`, `b` | Plane through two coplanar straight edge references. |
| `axis_point` | `axis`, `point` | Plane through axis and separate point. |
| `three_points` | `a`, `b`, `c` | Plane through three points. |
| `tangent` | `face`, `at`; optional `reference`, `angle` | Tangent to curved face. |
| `along_path` | `path`, `t` | Plane perpendicular to a referenced curve/edge at **fraction** `t` in `[0,1]`. |
| `perpendicular` | `plane`, `line` | Plane through line and perpendicular to a plane reference. |
| `face` | `plane`, `at`; optional `name`, `anchor` | Legacy or certified face attachment. |

**Offset test requirement:** A top-level `construction_plane` feature must have `plane.type: "offset"`, `base: {"type":"origin","name":"XY"}`, and a distance expression referring to the named parameter `OffsetDistance`. The later sketch must use a `plane.type: "construction"` reference to the offset feature, or another emitted associative reference proving it follows that feature. A visually correct sketch on a fixed `placed` plane does **not** meet that test.

**Along-path test requirement:** `t` must be driven by a parameter and the selected path must be a real referenced sketch curve or model edge. The dependent sketch/extrude must make the plane's changed position and orientation measurable in STEP.

**Source:** [`PlaneRef` source and field comments](https://github.com/tangency-labs/solidcraft/blob/v0.43.0/crates/doc/src/document.rs#L41-L151), [official Construct feature list](https://docs.solidcraft.io/guide/features.html).

## 4. Move semantics

Serialized `move` fields:

| Field | Status | Description |
|---|---|---|
| `bodies` | required | Existing named bodies to transform. |
| `translate` | required | Exactly three length expressions `[dx, dy, dz]`. |
| `rotate_axis` | optional | 3-D direction. Default Z axis. |
| `origin` | optional | 3-D pivot, expression-capable. Default `[0,0,0]`. |
| `angle` | optional | Angle expression. Default zero angle. |
| `copy` | optional | Default false: update original body. True: preserve original and make moved copy. |
| `copy_frames` | optional | Source-frame information for copied bodies in components; advanced behavior. |

The evaluator computes the translation vector and angle, then passes translation, pivot, axis and angle to the kernel transform. The exact matrix-order implementation inside the kernel was **not independently calculated in this pass**. Test its output with a non-origin pivot, asymmetric body, and measured STEP bounding box. The `solid.move` **command API** may use parameter names different from serialized `move`; always call `describe_command` rather than reusing the serialized field names in an MCP command call.

`remove` has only `bodies`. It requires each listed body to exist at that history point. It removes those bodies from the evaluated result, not their historical creation records. The earlier feature remains in the timeline.

**Source:** [`Move` and `Remove` serialization](https://github.com/tangency-labs/solidcraft/blob/v0.43.0/crates/doc/src/document.rs#L1844-L1851), [`Move` serialization](https://github.com/tangency-labs/solidcraft/blob/v0.43.0/crates/doc/src/document.rs#L2181-L2205), [`Move` evaluation](https://github.com/tangency-labs/solidcraft/blob/v0.43.0/crates/doc/src/eval.rs#L10781-L10834), [`Remove` evaluation](https://github.com/tangency-labs/solidcraft/blob/v0.43.0/crates/doc/src/eval_more.rs#L193-L205).

## 5. Fillet edge selection and stability

A fillet feature has `edges: Vec<Position>` and `radius: Expr`, plus optional `body`, `style`, `fit`, `corner`, `setbacks`, `turn`, and `tangent_chain`. The `edges` are 3-D pick points, not numeric edge indexes. The feature wrapper can add `edge_refs` and `edge_names` for the corresponding picks.

`edge_refs` entries have exactly `{dir, min, max}`:

- `dir`: Unit direction of a straight picked edge. It is `[0,0,0]` for a curved edge.
- `min` and `max`: Bounds of the **whole picked body** when the edge was selected. These are **not** bounds of an edge, and multiple edges of one body can have identical `min/max` values.
- `edge_names`: Persistent edge names, resolved first when possible. These can survive geometry changes but are not universal proof of stability.
- `edges`: Pick coordinates, which the evaluator can use to find an edge again if names or references cannot resolve.

For each fillet isolate, query `body_topology` immediately before applying that fillet. Record the candidate edge's name/identity if exposed, endpoints, midpoint, length, direction, and body. Choose the pick at the interior midpoint, far from neighboring vertices and edges. Assert that **exactly one** edge matches the geometric descriptor within a documented tolerance. Check that the exported fillet contains one edge pick and that the resulting body is still valid. Run the same checks after an upstream parameter edit and after reopening native JSON. For a negative probe, deliberately choose a point equidistant from two edges in an **independent discarded test copy** and require the verifier to flag ambiguity, rather than accepting an arbitrary nearest edge.

The source says saved names are resolved first and documents their fallback behavior. It does **not** guarantee that every possible imported or re-topologized edge keeps its name across all edits. A detector must distinguish `missing`, `ambiguous`, and `unique`.

**Source:** [`Feature` references](https://github.com/tangency-labs/solidcraft/blob/v0.43.0/crates/doc/src/document.rs#L3290-L3315), [`EdgeRef` and generation](https://github.com/tangency-labs/solidcraft/blob/v0.43.0/crates/doc/src/eval.rs#L5475-L5570).

## 6. Boolean operations, names, targets and tool retention

The serialized `Operation` enum is `new_body`, `join`, `cut`, `intersect`. A command parser accepts alternative spellings, including `subtract`, but the native enum serializer emits `cut`.

- `new_body`: Create independent body geometry. Check output `body_names` and model body count.
- `join`: Add new material to selected target bodies. A disjoint join can still result in a multi-piece body, with a warning; avoid this in the matrix fixture.
- `cut`: Subtract extruded or primitive tool volume from the selected target bodies. For a controlled test, require measurable positive volume reduction.
- `intersect`: Keep the common volume of the tool and specified target(s). Use overlapping geometry that produces a nonempty closed solid.
- `targets` (inside `extrude`, `sweep`, etc.): Names of bodies the operation may alter. Empty can mean the available bodies in the feature's own component, subject to its execution rules.
- `participants` (feature wrapper): Scope outside the feature's own component. Do not confuse it with `targets`.
- `body_names` (feature wrapper): Output body-name assignment, not Boolean targets.
- `tools` (inside `combine`): Names of **existing** bodies used in the Boolean, distinct from the named `target` body.
- `keep_tools` (only `combine` here): False removes the used tool bodies; true preserves them, while the target gets the Boolean result.

The accepted `combine.operation` is the same enum, but a meaningful combine fixture uses `join`, `cut`, or `intersect`, not a `new_body` semantic assumption. A `combine` test must check both final volume and final body count to distinguish retained tools from non-retained tools.

**Source:** [`Operation` parser and enum](https://github.com/tangency-labs/solidcraft/blob/v0.43.0/crates/doc/src/document.rs#L206-L243), [`Extrude` and `Combine`](https://github.com/tangency-labs/solidcraft/blob/v0.43.0/crates/doc/src/document.rs#L818-L844), [`Combine` fields](https://github.com/tangency-labs/solidcraft/blob/v0.43.0/crates/doc/src/document.rs#L984-L995), [official operation semantics](https://docs.solidcraft.io/guide/features.html).

## 7. Suppression and history

`Feature.suppressed` defaults to `false`. When `true`, the evaluator records a result entry but **does not apply the feature's geometry**. Names formerly assigned to suppressed features can remain reserved. The feature still exists on the timeline and is still serialized. It does not automatically provide bodies to downstream steps. Any later feature depending on a suppressed source must be independently checked for errors.

**Test rule:** Put a suppressible independent feature between two supported ones. The later supported feature must depend only on geometry that survives the suppression. Compare `suppressed=true` native JSON, no output from the suppressed feature, and final STEP parity. Do not treat a suppressed feature as removed from the feature array.

**Source:** [`suppression and evaluation`](https://github.com/tangency-labs/solidcraft/blob/v0.43.0/crates/doc/src/eval.rs#L3370-L3435), [official timeline behavior](https://docs.solidcraft.io/guide/features.html).

## 8. Sidecars, native `.sc`, `.json` and STEP

The official importer/exporter recognizes:

- `.json`: Plain native SolidCraft JSON design. It retains editable parameters and features.
- `.sc`: Versioned SolidCraft design container. Current writer declares `solidcraft-container/1`. It stores `document.json` and `container.json`. It may contain `blobs/` for imported STEP and mesh data, `plugins/` records, optional cached evaluated bodies and preview meshes, a thumbnail, and optional provenance information.
- `.step`/`.stp`: Exported AP242 B-rep geometry, body names, colours, and component tree. STEP is a **geometry reference**, not a replacement for parameter or timeline tests.

The `.sc` writer can store `cache/model.json`, `cache/bodies.bin`, `cache/preview.json`, and `cache/preview.bin`. These caches are optional. They are not the authoritative edit/replay source and can be omitted when space or compatibility prevents caching. Large imported STEP sources may appear as referenced `blobs/*.step` entries in the container. The source also permits plugin-owned authoritative records and provenance metadata.

**Warnings:** No source reviewed here establishes an automatic external `Name.warnings` file emitted beside a native JSON or STEP export. The agent should capture command replies, timeline errors, export results, and QA data in an **agent-created** `Name.validation.json` or `Name.validation.md`. Do not label such a file an exporter-provided sidecar.

**Source:** [official file formats](https://docs.solidcraft.io/guide/files.html), [native container implementation](https://github.com/tangency-labs/solidcraft/blob/v0.43.0/crates/io/src/design.rs#L188-L375).

## 9. Versioning and compatibility: correcting the `53 → 57` premise

A native `solidcraft/N` file has a format number. The source documents sequential migrations from older numbers to the current one. A later design can open in an older build with warnings and unknown records preserved, but the behavior is **not** a blanket guarantee that the older build can reconstruct or safely modify every new feature.

`solidcraft/53` was **not verified as a command-list format**. The feature-history schema exists in older designs. Commands are the engine's imperative API and command scripts; saved native designs are declarative document and feature records. **Do not call 53 a command-based format until an actual format-53 sample proves this.**

From the pinned source's migration directories:

| Format | Representative new saved structures or behavior |
|---|---|
| 53 | Configuration row identities; hole start on sketch plane; sweep paths on body edges. |
| 54 | Centered box placement semantics. |
| 55 | Definition-face blend references; construction snap points; plugin namespaces; sketch link names; move pivot; spline handles; split-face loops. |
| 56 | Cell grid feature; frame member location; standard frame sizes; occurrence move expressions; names of patterned copies. |
| 57 | Cell-grid clipping/margin/snap; retained To-Object picks; thread class and multiple-face states; linked configuration and cosmetic threads; move-copy placement; placement expressions; named construction-axis following; saved face picks; sweep-wall naming. |

The `v57` directory exists at the pinned tag and contains fourteen change files. The writer computes its current format from folded migrations plus pending changes. This means **a newer development build may write `solidcraft/58` or later**, even if it identifies as close to v0.43.0. Always inspect the saved `format` string and stop if it does not match the requested 57 fixture.

For this suite, require the final saved JSON to say `"format": "solidcraft/57"`. If the installed app writes something else, either use a matching release or record the blocker. Do **not** change the `format` string by hand to 57.

**Source:** [versioning and upgrade code](https://github.com/tangency-labs/solidcraft/blob/v0.43.0/crates/doc/src/format.rs#L1-L15), [`v53` directory](https://github.com/tangency-labs/solidcraft/tree/v0.43.0/crates/doc/src/format/v53), [`v57` directory](https://github.com/tangency-labs/solidcraft/tree/v0.43.0/crates/doc/src/format/v57), [MCP build-version tool](https://docs.solidcraft.io/guide/mcp.html).

## 10. Typical designer edits, feature combinations and real-world frequency

The official feature guide explicitly demonstrates editing extrude depth from 12 mm to 20 mm and rebuilding downstream shell, fillet, hole, and pattern features. Other established editable input categories include hole diameter and position, fillet radius, shell wall thickness, pattern quantity, plane offset, sketch dimensions, and target selection. These are suitable **plausible edit tests**, but not a statistical description of how often users edit them.

An official, representative feature sequence is `sketch → extrude → shell → fillet → hole → circular pattern → extrude-depth edit`. The guide also identifies `sketch → extrude`, `sketch → sweep`, `plane along path → sketch`, and primitive → Boolean combinations. For replay verification, limit each design to only the features needed for its target capability.

**Unverified:** The median and range of feature counts in actual user designs, the observed frequency of each edit type, and the distribution of typical feature combinations. No real customer dataset was provided or inspected. An anecdotal estimate would not be evidence. Collect anonymized feature-count histograms later if these metrics are necessary to weight test coverage.

**Source:** [official feature sequence and edits](https://docs.solidcraft.io/guide/features.html#step-by-step), [MCP testing and verification](https://docs.solidcraft.io/guide/mcp.html).

## 11. MCP behavior needed by the builder agent

The official MCP tool reference documents these capabilities:

| Tool | Use in the verification workflow |
|---|---|
| `version` | Read actual app or CLI build and mode. |
| `skill_reference` | Load built-in modeling, editing, and checking skills before authoring. |
| `list_commands` and `describe_command` | Find **live command** names, arguments and JSON Schema. |
| `new_design` | Begin an isolated synthetic design. |
| `execute` | Execute one modeling or inspection engine command. |
| `batch` | Execute multiple commands in order, with rollback on failure. |
| `inspect_design` | Read native feature timeline, parameters, sketches, errors, warnings, and bodies. |
| `measure` | Get closed-body volume, bounding boxes, body count, centre of mass and related metrics. |
| `body_topology` | Read candidate face/edge descriptors for stable picks. |
| `set_parameter` | Make a persistent parameter edit, if required. |
| `check_parameters` | Temporarily edit parameters and check rebuilt design, then restore baseline. |
| `save` | Save editable `.json` or `.sc` design. |
| `export` | Export `.step` geometry from the current model. |
| `open` | Open saved native file or STEP for an independent round-trip check. |
| `screenshot` | Optional visual verification of asymmetric orientation. |

MCP `batch` supports `rollback: true` for a failing command. It also accepts a tool-level `expect` assertion with `volume_mm3`, `bodies`, and `bbox`. The documented default volume tolerance is relative 0.002 (0.2%). Bounding-box coordinates are tested within 0.05 mm. This is useful as a build-stage guard but **not sufficient** for final STEP parity: two different shapes can have equal volume and bounds.

With `--param-check strict`, typed command contracts reject unknown or malformed keys when the build supports that check. Older commands without typed schemas can remain unchecked. In connected mode, the app and server should be on compatible versions.

**Access constraint:** On Windows the MCP file sandbox accepts local-drive paths in allowed directories. The directory must exist before starting the MCP server with `--allow-path`. UNC paths and hidden/unauthorized directories are refused. `--allow-any-path` bypasses sandbox limits and is **not required** for these tests. Headless `view.pick` is unavailable; use `body_topology` and named references instead.

**Source:** [official MCP guide, tool list and batch expectations](https://docs.solidcraft.io/guide/mcp.html).

## 12. Verification measurements and thresholds

These are **proposed acceptance thresholds**, not guarantees from the exporter:

1. **Native format:** The JSON parses and says exactly `solidcraft/57`. All expected feature types, parameter names, expressions and IDs appear. Zero unexpected active errors.
2. **Geometry:** Every expected final body is closed, nonempty, and has positive volume. Check the expected count, distinct body names, each body volume and bounds, total volume, and total bounds.
3. **STEP parity:** Save native JSON and export STEP from the **same baseline state**. Open STEP in a separate test session. Compare total closed-body volume (relative error ≤ 0.001 unless machine measurement requires a documented alternative), body count, and all six bounding coordinates (absolute difference ≤ 0.05 mm). Match named bodies if the import preserves names. Centroid and inertia, when available, provide stronger additional evidence.
4. **Edit/follow:** Use `check_parameters` or controlled edits to change one named parameter. Assert the specified dimensional/positional changes and record changed metrics. Restore original values **before final saves and exports**. Assert restored baseline metrics equal pre-edit measurements within measurement noise.
5. **Feature-specific proof:** A JSON assertion confirms the target feature and field semantics. Examples: offset reference chain, `PointExprs`, `cut` target list, rotated Move, `along_path.t`, `keep_tools`, and three fillets with distinct resolved edges.
6. **Source integrity:** Hash the saved JSON and STEP files. Record tool version/commit, parameter values, feature IDs/types, build status, warnings, body count, bounds, volume, edit results and STEP parity in a separate validation manifest.
7. **Failure behavior:** If a feature fails, do not silently replace it with a frozen primitive, static custom plane, tessellated shape, edited STEP, or hand-patched export. Mark that case `BLOCKED` or `FAIL` with the exact reason.

Volume alone is not mass. STEP geometry can be compared by volume without assigned material density. If density is assigned, mass is volume × density, and both models must use the same density for a mass comparison.

## 13. Remaining unknowns and required live probes

| Question | What is known | Live probe required |
|---|---|---|
| Exact version loaded at the builder's seat | v0.43.0 source contains format 57 | Call `version`, make a native save, inspect `format`. |
| Exact MCP JSON arguments for every command | `describe_command` exposes contracts where available | Inspect each command before invoking it. Do not copy native serialized keys blindly. |
| Raw `fixed:true + exprs` creation through MCP | Serializer supports it | Create actual point, export JSON, edit parameters. If unsupported via MCP, report blocker. |
| Distinct edge resolution after upstream edits | Names and fallback references exist | Query topology before/after edits and reopen. Distinguish ambiguous from missing. |
| Rotation/translation transform order | Evaluator delegates to kernel | Measure non-origin pivot result and compare to expected transformed vertices. |
| Sweep guide-rail encoding and exact constraints | `rails`, `rail_fit` are serialized fields | Create minimal rail variant only if live commands emit it. |
| `combine.keep_tools` behavior across each operation | Serialization and generic semantics are known | Test `keep_tools=true` and `false` with body count, volume and name checks. |
| Sparse IDs via a public MCP command | IDs support sparse values up to bound | Inspect timeline APIs; do not hand-edit a saved output and call it an exporter fixture. |
| Auto-generated external warnings files | No evidence of such exporter sidecars | Observe export directory; create an explicit agent validation report instead. |
| Reproducibility across versions | Older formats migrate; newer formats can carry unknowns | Open/save in exact target build; compare JSON structure and STEP geometry. |
| Population-level edit frequency / typical feature count | Official tutorial supplies examples | Requires representative, permitted telemetry or sample corpus. |

## 14. Source index

The links below identify the **version-pinned** source used for the report.

1. [Native feature/document schema, SolidCraft v0.43.0](https://github.com/tangency-labs/solidcraft/blob/v0.43.0/crates/doc/src/document.rs)
2. [Sketch point and curve serializer, SolidCraft v0.43.0](https://github.com/tangency-labs/solidcraft/blob/v0.43.0/crates/sketch/src/model.rs)
3. [Expression parser and evaluator, SolidCraft v0.43.0](https://github.com/tangency-labs/solidcraft/blob/v0.43.0/crates/doc/src/expr.rs)
4. [Model evaluation, edge picks, transform and suppression, SolidCraft v0.43.0](https://github.com/tangency-labs/solidcraft/blob/v0.43.0/crates/doc/src/eval.rs)
5. [Body removal evaluator, SolidCraft v0.43.0](https://github.com/tangency-labs/solidcraft/blob/v0.43.0/crates/doc/src/eval_more.rs)
6. [Native `.sc` package writer, SolidCraft v0.43.0](https://github.com/tangency-labs/solidcraft/blob/v0.43.0/crates/io/src/design.rs)
7. [Versioned design migrations, SolidCraft v0.43.0](https://github.com/tangency-labs/solidcraft/blob/v0.43.0/crates/doc/src/format.rs)
8. [Format 53 changes](https://github.com/tangency-labs/solidcraft/tree/v0.43.0/crates/doc/src/format/v53) and [format 57 changes](https://github.com/tangency-labs/solidcraft/tree/v0.43.0/crates/doc/src/format/v57)
9. [Official MCP guide: commands, session management, export and verification](https://docs.solidcraft.io/guide/mcp.html)
10. [Official native/import/export format guide](https://docs.solidcraft.io/guide/files.html)
11. [Official solid features and edit workflow](https://docs.solidcraft.io/guide/features.html)

**Research limitation:** All source links describe the tagged source or official documentation as accessed on the research date. Runtime exports, body volumes, edge-resolution counts, and parameter-follow behavior remain **unverified until the builder agent runs SolidCraft MCP**. No CAD file in this research package should be described as verified or produced.
