//! The SWBasic/VBA emitter: a replay plan becomes a plain-text `.swb` macro.
//!
//! The generated macro mirrors `corpus/compat-check/minimal.swb`, the
//! hand-reviewed compatibility checkpoint: same entry point, same fresh-part
//! creation, same report contract (header first, one line per recipe step,
//! summary last, every line flushed). All untrusted text passes through the
//! VBA-literal escaper; report text gets its own JSON escaper inside the
//! macro. Numbers are emitted locale-independent.

use crate::replay::{
    AxisSel, DimKind, DimensionT, ExtrudeStep, OriginPlane, PlannedStep, RelationT, RevolveStep,
    SketchStep, Work,
};

/// VBA string-literal escaping: quotes doubled, control characters removed,
/// length capped. This is not JSON escaping; the macro has its own.
pub fn vb_lit(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars().take(200) {
        match c {
            '"' => out.push_str("\"\""),
            '\r' | '\n' | '\t' => out.push(' '),
            c if (c as u32) < 0x20 => out.push(' '),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// A locale-independent VBA numeric literal (Rust's Display never uses
/// exponent notation; every value is validated finite upstream).
pub fn vba_num(v: f64) -> String {
    let s = v.to_string();
    if s.contains('.') || s.contains('e') || s.contains("inf") || s.contains("NaN") {
        s
    } else {
        format!("{s}.0")
    }
}

/// JSON string *content* escaping (backslash, quote, controls) for values
/// baked into generated JSON text. The result then goes through `vb_lit`
/// when it becomes part of a VBA literal: two separate escapes, never one.
pub fn json_str_content(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\r' => out.push_str("\\r"),
            '\n' => out.push_str("\\n"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => {}
            c => out.push(c),
        }
    }
    out
}

fn plane_code(plane: OriginPlane) -> &'static str {
    match plane {
        OriginPlane::Xy => "XY",
        OriginPlane::Xz => "XZ",
        OriginPlane::Yz => "YZ",
    }
}

/// The sketch frame each SolidCraft origin plane must produce in the target
/// part: `u` along the first named axis, `v` along the second, and the
/// right-handed normal `u x v`. XY: u=X, v=Y, n=+Z. XZ: u=X, v=-Z, n=+Y.
/// YZ: u=Y, v=Z, n=+X. The frame is what "verbatim (u, v)" coordinates need
/// to land in model space unchanged, so the probe verifies it exactly -
/// axes and signs - and fails closed on anything else (docs/mapping.md).
/// Source-pinned from SolidCraft's own plane table
/// (solidcore/crates/solidcore-record/src/plane.rs:29-34, in the SolidCraft
/// repository), never inferred from the plane-name letters; the XY row is
/// what the corpus exercises, the XZ/YZ rows await their first seat run.
pub fn solidcraft_frame(plane: OriginPlane) -> [[f64; 3]; 3] {
    let (u, v) = match plane {
        OriginPlane::Xy => ([1.0, 0.0, 0.0], [0.0, 1.0, 0.0]),
        OriginPlane::Xz => ([1.0, 0.0, 0.0], [0.0, 0.0, -1.0]),
        OriginPlane::Yz => ([0.0, 1.0, 0.0], [0.0, 0.0, 1.0]),
    };
    let n = [
        u[1] * v[2] - u[2] * v[1],
        u[2] * v[0] - u[0] * v[2],
        u[0] * v[1] - u[1] * v[0],
    ];
    [u, v, n]
}

/// The sketch frame in model coordinates, read from one
/// `ModelToSketchTransform.ArrayData` value (16 doubles: rotation 0-8,
/// translation 9-11, scale 12, row-major, row-vector convention).
///
/// With `p_sketch = p_model * R + t`, row i of R is the model basis vector
/// `e_i` expressed in sketch coordinates, so the sketch basis expressed in
/// MODEL coordinates is the columns of R: u = (a0, a3, a6),
/// v = (a1, a4, a7), normal = (a2, a5, a8). Equivalently: the sketch normal
/// is sketch-space (0, 0, 1) transformed by the inverse transform. The
/// translation is the model origin in sketch coordinates, so a zero
/// translation means the sketch - and therefore the plane - runs through the
/// model origin. This is the arithmetic the generated probe performs; the
/// tests pin it with non-symmetric frames.
pub fn sketch_frame_from_array_data(a: &[f64; 16]) -> [[f64; 3]; 3] {
    [
        [a[0], a[3], a[6]], // sketch u axis, model coordinates
        [a[1], a[4], a[7]], // sketch v axis, model coordinates
        [a[2], a[5], a[8]], // sketch normal, model coordinates
    ]
}

/// The model origin in sketch coordinates (translation of the same
/// transform): nonzero means the sketch origin is offset from the model
/// origin, so verbatim coordinates would land shifted.
pub fn sketch_origin_from_array_data(a: &[f64; 16]) -> [f64; 3] {
    [a[9], a[10], a[11]]
}

/// A frame vector as report text, e.g. `u=(1,0,0)`.
fn frame_desc(frame: [[f64; 3]; 3]) -> String {
    fn one(v: [f64; 3]) -> String {
        let f = |x: f64| {
            if x == 0.0 {
                "0".to_string()
            } else if x == 1.0 {
                "1".to_string()
            } else if x == -1.0 {
                "-1".to_string()
            } else {
                x.to_string()
            }
        };
        format!("{},{},{}", f(v[0]), f(v[1]), f(v[2]))
    }
    format!(
        "u=({}) v=({}) n=({})",
        one(frame[0]),
        one(frame[1]),
        one(frame[2])
    )
}

fn mm(v: f64) -> f64 {
    v / 1000.0
}

/// Emit the whole macro for a plan. `steps` is the full plan in recipe order.
pub fn emit(
    steps: &[PlannedStep],
    artifact: &str,
    bridge_version: &str,
    snapshots: &std::collections::BTreeMap<usize, crate::profile::SnapshotScope>,
) -> String {
    let sketches: Vec<(usize, &SketchStep)> = steps
        .iter()
        .filter_map(|s| match &s.work {
            Work::Sketch(sk) => Some((s.index, sk)),
            _ => None,
        })
        .collect();
    let extrudes: Vec<(usize, &ExtrudeStep)> = steps
        .iter()
        .filter_map(|s| match &s.work {
            Work::Extrude(ex) => Some((s.index, ex)),
            _ => None,
        })
        .collect();

    let mut out = String::new();
    let final_slot = snapshots
        .iter()
        .find(|(_, s)| **s == crate::profile::SnapshotScope::Final)
        .map(|(k, _)| *k);
    out.push_str(&format!(
        "' Generated by craft-bridge {bridge_version} from {} - review before running.\n",
        vb_lit(artifact)
    ));
    out.push_str("' Units: the SolidWorks API always takes metres and radians.\n");
    out.push_str("Option Explicit\n\n");
    out.push_str("Dim swApp As SldWorks.SldWorks\n");
    out.push_str("Dim swDoc As SldWorks.ModelDoc2\n");
    out.push_str("Dim swSketchMgr As SketchManager\n");
    out.push_str("Dim reportPath As String\n");
    out.push_str("Dim gMsg As String\n");
    out.push_str("Dim gPlane As String\n");
    out.push_str("Dim gProbeLog As String\nDim gProbeReason As String\n");
    let max_step = steps.iter().map(|s| s.index).max().unwrap_or(0);
    out.push_str(&format!(
        "Dim gMap00(0 To {max_step}) As Double\n\
         Dim gMap01(0 To {max_step}) As Double\n\
         Dim gMap10(0 To {max_step}) As Double\n\
         Dim gMap11(0 To {max_step}) As Double\n\
         Dim gMapValid(0 To {max_step}) As Boolean\n\
         Dim gMapSide(0 To {max_step}) As Long\n\
         Dim gFrame(0 To {max_step}, 0 To 11) As Double\n\
         Dim gOmit As String\n         Dim gCause As String\n\
         Dim gDimToggle As Boolean\n\
         Dim gDimToggleSaved As Boolean\n"
    ));
    out.push_str("Dim gDoc As Object\n");
    out.push_str("Dim gFeatMgr As Object\n");
    let max_slot = steps
        .iter()
        .filter_map(|s| match &s.work {
            Work::Sketch(sk) => sk.dim_equations.iter().map(|(_, e)| e.slot).max(),
            Work::Extrude(ex) => ex.equations.iter().map(|e| e.slot).max(),
            Work::Revolve(rv) => rv.equations.iter().map(|e| e.slot).max(),
            Work::Primitive(pr) => pr
                .sizes
                .iter()
                .filter_map(|sz| sz.equation.as_ref().map(|e| e.slot))
                .max(),
            _ => None,
        })
        .max();
    // Always declared: AddDimEquation references regName even when the
    // recipe carries no equations (plan review).
    out.push_str(&format!(
        "Dim regName(0 To {}) As String\n",
        max_slot.unwrap_or(0)
    ));
    // Each registered slot also records the sign its dimension carries
    // relative to the source value: downstream equation references
    // normalize through SlotRef so source semantics hold across edits.
    out.push_str(&format!(
        "Dim regSign(0 To {}) As Double\n",
        max_slot.unwrap_or(0)
    ));
    out.push_str(&format!(
        "Dim regScale(0 To {}) As Double\n",
        max_slot.unwrap_or(0)
    ));
    // ---- extrude/revolve/primitive direction truth table (Phase 1 batch 2/3) ----
    // Every constant is measured on the seat from a direction probe's
    // runtime report; 0 means UNMEASURED and every decision that consults
    // it refuses with a named reason - a step whose direction semantics
    // cannot be established fails and never reports created
    // (docs/mapping.md, "Direction truth table").
    out.push_str(
        "\n' ---- extrude/revolve/primitive direction truth table (measured on the seat) ----\n",
    );
    out.push_str("' Each constant comes from a direction probe's runtime report (corpus/\n");
    out.push_str("' compat-probe); 0 = UNMEASURED and every decision that consults it\n");
    out.push_str("' refuses with a named reason - never a silent guess, never 'created'.\n");
    out.push_str(&format!(
        "Const TT_EX_DIR_TRUE_SIDE As Long = {}      ' probe-extrudedir: Dir=True sends material toward TT * the sketch normal\n",
        crate::truth::TT_EX_DIR_TRUE_SIDE
    ));
    out.push_str(&format!(
        "Const TT_EX_FIRST_SIDE As Long = {}         ' probe-twosided: the first depth's end sits toward TT * the sketch normal (Dir=False call shape)\n",
        crate::truth::TT_EX_FIRST_SIDE
    ));
    out.push_str(&format!(
        "Const TT_EX_THROUGH_ALL_DIR As Long = {}    ' probe-throughall: the through-all Dir=True reading (measured directly; its inconclusive branch did not occur)\n",
        crate::truth::TT_EX_THROUGH_ALL_DIR
    ));
    out.push_str(&format!(
        "Const TT_REV_SENSE As Long = {}             ' probe-revolvedir: ReverseDir=False sweeps the right-hand +angle sense about the drawn axis when +1\n",
        crate::truth::TT_REV_SENSE
    ));
    out.push_str(&format!(
        "Const TT_REV_TWO_FIRST_SIDE As Long = {}    ' probe-revolvedir: the two-sided first angle sweeps right-hand sense TT about the drawn axis\n",
        crate::truth::TT_REV_TWO_FIRST_SIDE
    ));
    out.push_str(&format!(
        "Const TT_REV_PROBE_CHIRALITY As Long = {}   ' the configuration chirality the revolve evidence establishes (probe-revolvedir: one directed axis and one profile side)\n",
        crate::truth::TT_REV_PROBE_CHIRALITY
    ));
    out.push_str(&format!(
        "Const TT_PRIM_DIR_TRUE_SIDE As Long = {}   ' probe-primorient: the primitive height extends toward TT * the sketch normal for Dir=True (0 = unmeasured)\n",
        crate::truth::TT_PRIM_DIR_TRUE_SIDE
    ));
    for s in steps {
        if matches!(
            s.work,
            Work::Extrude(_) | Work::Revolve(_) | Work::Primitive(_) | Work::Modifier(_)
        ) {
            out.push_str(&format!("Dim feat{} As SldWorks.Feature\n", s.index));
        }
    }
    for (index, sk) in &sketches {
        out.push_str(&format!("Dim skName{index} As String\n"));
        out.push_str(&format!("Dim skFeat{index} As SldWorks.Feature\n"));
        out.push_str(&format!(
            "Dim pt{index}(0 To {}) As Object\n",
            sk.slot_count().saturating_sub(1)
        ));
        if !sk.lines.is_empty() {
            out.push_str(&format!(
                "Dim sl{index}(0 To {}) As Object\n",
                sk.lines.len() - 1
            ));
        }
        if !sk.circles.is_empty() {
            out.push_str(&format!(
                "Dim ci{index}(0 To {}) As Object\n",
                sk.circles.len() - 1
            ));
        }
    }
    out.push('\n');

    // ---- main ----
    out.push_str("Sub main()\n");
    out.push_str("    Dim template As String\n    Dim ok As Boolean\n    Dim st As String\n    Dim stopModel As Boolean\n    Dim stepIdx As Long\n    Dim closedOk As Boolean\n    Dim cleanupMode As String\n");
    out.push_str("    stepIdx = -1\n    On Error GoTo Fail\n\n");
    out.push_str("    ' ---- report setup (before any model mutation) ----\n");
    out.push_str("    reportPath = ReportPathFromHost()\n");
    out.push_str("    If reportPath = \"\" Then Err.Raise vbObjectError + 513, \"craft-bridge\", \"no report path: CRAFT_BRIDGE_REPORT is unset and no path file exists in TEMP\"\n");
    out.push_str("    StartReport\n");
    let header_json = format!(
        "{{\"type\":\"header\",\"macro\":\"{}\",\"bridge_version\":\"{}\"}}",
        json_str_content(artifact),
        json_str_content(bridge_version)
    );
    out.push_str(&format!("    WriteLine {}\n", vb_lit(&header_json)));

    // The fresh-part bootstrap is not a recipe step: it runs with stepIdx
    // still -1 so recipe step numbers stay aligned with the plan report. The
    // checkpoint proved this shape on the seat (docs/mapping.md, Document
    // handling); the enum is spelled fully qualified per the checkpoint.
    out.push_str(
        "\n    ' ---- fresh part from the default template (before recipe step 0) ----\n\
         Set swApp = Application.SldWorks\n\
         gDimToggle = swApp.GetUserPreferenceToggle(swUserPreferenceToggle_e.swInputDimValOnCreate)\n\
         gDimToggleSaved = True\n\
         swApp.SetUserPreferenceToggle swUserPreferenceToggle_e.swInputDimValOnCreate, False\n\
         If swApp.GetUserPreferenceToggle(swUserPreferenceToggle_e.swInputDimValOnCreate) Then Err.Raise vbObjectError + 528, \"craft-bridge\", \"the input-dimension-value dialog could not be disabled\"\n\
         template = swApp.GetUserPreferenceStringValue(swUserPreferenceStringValue_e.swDefaultTemplatePart)\n\
         If template = \"\" Then Err.Raise vbObjectError + 514, \"craft-bridge\", \"no default part template is configured\"\n\
         Set swDoc = swApp.NewDocument(template, 0, 0, 0)\n\
         If swDoc Is Nothing Then Err.Raise vbObjectError + 515, \"craft-bridge\", \"NewDocument returned nothing\"\n\
         Set swSketchMgr = swDoc.SketchManager\n\
         If swSketchMgr Is Nothing Then Err.Raise vbObjectError + 521, \"craft-bridge\", \"the sketch manager is unavailable\"\n\
         Set gFeatMgr = swDoc.FeatureManager\n\
         If gFeatMgr Is Nothing Then Err.Raise vbObjectError + 522, \"craft-bridge\", \"the feature manager is unavailable\"\n\
         Set gDoc = swDoc\n\
         WriteStep stepIdx, \"document.new\", \"created\", \"fresh part from the default template\"\n\n",
    );

    for step in steps {
        out.push_str(&format!(
            "    ' ---- step {}: {} ----\n",
            step.index, step.command
        ));
        out.push_str(&format!("    stepIdx = {}\n", step.index));
        match &step.work {
            Work::Units { unit } => {
                out.push_str(&format!(
                    "    WriteStep stepIdx, {}, \"not-executed\", {}\n\n",
                    vb_lit(&step.command),
                    vb_lit(&format!(
                        "units recorded ({unit}); values replay in millimetres"
                    ))
                ));
            }
            Work::UserParam {
                name,
                equation_rhs,
                expect_si,
                dim,
                ..
            } => {
                out.push_str("    If Not stopModel Then\n");
                let eval_scale = match dim {
                    crate::replay::expr::Dim::Length => "1000#",
                    crate::replay::expr::Dim::Angle => "0#",
                    crate::replay::expr::Dim::Count => "1#",
                };
                out.push_str(&format!(
                    "        st = AddGlobalEquation({}, {}, {}, {})\n",
                    vb_lit(name),
                    vb_lit(equation_rhs),
                    vba_num(*expect_si),
                    eval_scale
                ));
                out.push_str("    Else\n        st = \"not-executed\"\n        gMsg = \"after the first stopped step\"\n    End If\n");
                out.push_str(&format!(
                    "    WriteStep stepIdx, {}, st, gMsg\n    If st = \"failed\" Then stopModel = True\n\n",
                    vb_lit(&step.command)
                ));
            }
            Work::Note => {
                out.push_str(&format!(
                    "    WriteStep stepIdx, {}, \"not-executed\", {}\n\n",
                    vb_lit(&step.command),
                    vb_lit(&step.detail)
                ));
            }
            Work::Sketch(_) => {
                out.push_str("    If Not stopModel Then\n");
                out.push_str(&format!("        st = BuildSketch{}()\n", step.index));
                out.push_str("    Else\n        st = \"not-executed\"\n        gMsg = \"after the first stopped step\"\n    End If\n");
                out.push_str(&format!(
                    "    WriteStep stepIdx, {}, st, gMsg\n",
                    vb_lit(&step.command)
                ));
                out.push_str("    If st = \"failed\" Then stopModel = True\n\n");
            }
            Work::Extrude(_)
            | Work::Revolve(_)
            | Work::Primitive(_)
            | Work::Modifier(_)
            | Work::Pattern(_) => {
                out.push_str("    If Not stopModel Then\n");
                let kind = match &step.work {
                    Work::Extrude(_) => "Extrude",
                    Work::Revolve(_) => "Revolve",
                    Work::Primitive(_) => "Prim",
                    Work::Pattern(_) => "Pat",
                    _ => "Mod",
                };
                out.push_str(&format!("        st = Build{}{}()\n", kind, step.index));
                out.push_str("    Else\n        st = \"not-executed\"\n        gMsg = \"after the first stopped step\"\n    End If\n");
                out.push_str(&format!(
                    "    WriteStep stepIdx, {}, st, gMsg\n",
                    vb_lit(&step.command)
                ));
                out.push_str("    If st = \"failed\" Then stopModel = True\n\n");
            }
            Work::Unmappable => {
                let boundary =
                    snapshots.get(&step.index) == Some(&crate::profile::SnapshotScope::Boundary);
                let sidecar = if boundary {
                    Some(step.index)
                } else {
                    final_slot
                };
                match sidecar {
                    Some(_) => {
                        out.push_str("    If Not stopModel Then\n");
                        out.push_str(&format!("        st = Fallback{}()\n", step.index));
                        out.push_str(
                        "    Else\n        st = \"not-executed\"\n        gMsg = \"after the first stopped step\"\n    End If\n",
                    );
                        out.push_str(&format!(
                            "    WriteStep stepIdx, {}, st, gMsg\n",
                            vb_lit(&step.command)
                        ));
                        if !boundary {
                            // The coarse rule: a final snapshot terminates replay.
                            out.push_str("    If st = \"fallback\" Then stopModel = True\n\n");
                        } else {
                            out.push('\n');
                        }
                    }
                    None => {
                        out.push_str(&format!(
                            "    WriteStep stepIdx, {}, \"not-executed\", {}\n",
                            vb_lit(&step.command),
                            vb_lit(&step.detail)
                        ));
                        out.push_str("    stopModel = True\n\n");
                    }
                }
            }
            Work::AfterStop => {
                out.push_str(&format!(
                    "    WriteStep stepIdx, {}, \"not-executed\", {}\n\n",
                    vb_lit(&step.command),
                    vb_lit("after the first unmapped step")
                ));
            }
        }
    }

    out.push_str("    If gDimToggleSaved And Not swApp Is Nothing Then swApp.SetUserPreferenceToggle swUserPreferenceToggle_e.swInputDimValOnCreate, gDimToggle
    If gDimToggleSaved And Not swApp Is Nothing Then If swApp.GetUserPreferenceToggle(swUserPreferenceToggle_e.swInputDimValOnCreate) <> gDimToggle Then WriteStep stepIdx, \"toggle-restore\", \"failed\", \"the input-dimension-value preference did not restore\"
    cleanupMode = CloseMode()\n    If cleanupMode = \"close\" Then\n    CloseOwnedDoc closedOk\n    If closedOk Then\n    WriteLine \"{\"\"type\"\":\"\"summary\"\",\"\"status\"\":\"\"ok\"\"}\"\n    Else\n    WriteStep stepIdx, \"close\", \"failed\", \"the owned document could not be confirmed closed\"\n    WriteLine \"{\"\"type\"\":\"\"summary\"\",\"\"status\"\":\"\"failed\"\"}\"\n    End If\n    ElseIf cleanupMode = \"keep\" Then\n    WriteStep stepIdx, \"owned-document\", \"kept-open\", OwnedDocTitle()\n    WriteLine \"{\"\"type\"\":\"\"summary\"\",\"\"status\"\":\"\"ok\"\"}\"\n    Else\n    WriteStep stepIdx, \"close-mode\", \"failed\", \"the cleanup mode could not be established\"\n    WriteLine \"{\"\"type\"\":\"\"summary\"\",\"\"status\"\":\"\"failed\"\"}\"\n    End If\n");
    out.push_str("    Exit Sub\n\n");
    out.push_str("Fail:\n");
    out.push_str("    ' Capture the error FIRST: executing On Error resets Err.\n");
    out.push_str("    Dim errNum As Long, errMsg As String\n");
    out.push_str("    errNum = Err.Number\n    errMsg = Err.Description\n");
    out.push_str("    ' The report must survive failure: open it if needed, write what happened\n");
    out.push_str("    ' and the completion signal, then leave the model as it is.\n");
    out.push_str("    On Error Resume Next\n");
    out.push_str("    If reportPath = \"\" Then reportPath = ReportPathFromHost()\n");
    out.push_str("    If reportPath <> \"\" Then\n");
    out.push_str("        WriteStep stepIdx, \"macro\", \"failed\", \"err \" & Trim(Str(errNum)) & \": \" & errMsg\n");

    out.push_str("    End If\n        If gDimToggleSaved And Not swApp Is Nothing Then swApp.SetUserPreferenceToggle swUserPreferenceToggle_e.swInputDimValOnCreate, gDimToggle
    If gDimToggleSaved And Not swApp Is Nothing Then If swApp.GetUserPreferenceToggle(swUserPreferenceToggle_e.swInputDimValOnCreate) <> gDimToggle Then WriteStep stepIdx, \"toggle-restore\", \"failed\", \"the input-dimension-value preference did not restore\"
    cleanupMode = CloseMode()\n    If cleanupMode = \"close\" Then\n    CloseOwnedDoc closedOk\n    End If\n    If reportPath <> \"\" Then\n        WriteLine \"{\"\"type\"\":\"\"summary\"\",\"\"status\"\":\"\"failed\"\"}\"\n    End If\nEnd Sub\n\n");

    // ---- per-sketch builders ----
    for (index, sk) in &sketches {
        out.push_str(&emit_sketch_subs(*index, sk));
    }
    for (index, ex) in &extrudes {
        out.push_str(&emit_extrude_subs(*index, ex));
    }
    let revolves: Vec<(usize, &crate::replay::RevolveStep)> = steps
        .iter()
        .filter_map(|s| match &s.work {
            Work::Revolve(rv) => Some((s.index, rv)),
            _ => None,
        })
        .collect();
    for (index, rv) in &revolves {
        let rv_sketch = steps.iter().find_map(|s| match &s.work {
            Work::Sketch(sk) if s.index == rv.sketch_step => Some(sk),
            _ => None,
        });
        out.push_str(&emit_revolve_subs(*index, rv, rv_sketch));
    }
    let prims: Vec<(usize, &crate::replay::PrimStep)> = steps
        .iter()
        .filter_map(|s| match &s.work {
            Work::Primitive(p) => Some((s.index, p)),
            _ => None,
        })
        .collect();
    for (index, pr) in &prims {
        out.push_str(&emit_prim_subs(*index, pr));
    }
    let mods: Vec<(usize, &crate::replay::ModStep)> = steps
        .iter()
        .filter_map(|s| match &s.work {
            Work::Modifier(m) => Some((s.index, m)),
            _ => None,
        })
        .collect();
    for (index, md) in &mods {
        out.push_str(&emit_mod_subs(*index, md));
    }
    let pats: Vec<(usize, &crate::replay::PatStep)> = steps
        .iter()
        .filter_map(|s| match &s.work {
            Work::Pattern(p) => Some((s.index, p)),
            _ => None,
        })
        .collect();
    for (index, pt) in &pats {
        out.push_str(&emit_pat_subs(*index, pt));
    }
    // Fallback builders for snapshot-covered steps, and the transaction
    // helpers they share: restore the pre-step state (delete everything
    // created so far), import the snapshot body, and invalidate every live
    // binding the replacement made stale.
    for index in snapshots.keys() {
        if steps
            .iter()
            .any(|s| s.index == *index && matches!(s.work, Work::Unmappable))
        {
            let sidecar = snapshots
                .get(index)
                .map(|s| *s == crate::profile::SnapshotScope::Boundary)
                .map(|b| {
                    if b {
                        *index
                    } else {
                        final_slot.unwrap_or(*index)
                    }
                })
                .unwrap_or(*index);
            out.push_str(&emit_fallback_subs(*index, sidecar));
        }
    }
    if !snapshots.is_empty() {
        out.push_str(&emit_delete_created(steps));
    }
    // Always defined: AddDimEquation and every fallback builder reference it
    // regardless of the recipe's equation slots (plan review).
    out.push_str(
        "Sub ClearSlotsAll()\n    Dim i As Long\n    For i = 0 To UBound(regName)\n        regName(i) = \"\"\n        regSign(i) = 1#
        regScale(i) = 1#\n    Next i\nEnd Sub\n\n",
    );
    // A registered dimension may carry its source value with the opposite
    // sign (the map is reflected on its axis). Downstream equation text
    // references such a slot normalized back to source semantics, so
    // parameter edits keep meaning what the source meant.
    out.push_str(
        "Function SlotRef(ByVal slot As Long) As String\n    Dim f As Double\n    f = regSign(slot) * regScale(slot)\n    If Abs(f - 1#) <= 0.000001 Then\n    SlotRef = \"\"\"\" & regName(slot) & \"\"\"\"\n    Else\n    SlotRef = \"(\" & Trim(Str(1# / f)) & \"*\" & \"\"\"\" & regName(slot) & \"\"\"\" & \")\"\n    End If\nEnd Function\n\n",
    );

    // ---- shared helpers (the checkpoint's, unchanged in spirit) ----
    out.push_str(
        "' Write one step line and flush it to disk immediately.\n\
         ' The plane whose sketch frame exactly matches the SolidCraft frame for the\n\
         ' requested plane: axes, signs, and the sketch origin at the model origin.\n\
         ' Plane names and tree positions are per config (one seat maps Front to the\n\
         ' ZY plane), so no name or position is trusted; and verbatim (u, v) replay\n\
         ' only lands coordinates where they belong on the frame the recipe implies.\n\
         ' Anything else is refused and fails closed - coordinates are never\n\
         ' silently reinterpreted (docs/mapping.md, Orientation). The probe leaves\n\
         ' the sketch open on success. On failure the probe sketch is closed,\n\
         ' deleted as one owned feature, and the deletion is verified.\n\
         Function OpenSketchOnPlane(ByVal idx As Long, ByVal which As String) As Boolean\n\
             Dim feat As SldWorks.Feature, skFeat As SldWorks.Feature, ok As Boolean\n\
             Set feat = swDoc.FirstFeature\n\
             Do While Not feat Is Nothing\n\
                 If InStr(1, feat.GetTypeName2(), \"lane\", vbTextCompare) > 0 Then\n\
                     ok = feat.Select(False)\n\
                     If ok Then\n\
                         swSketchMgr.InsertSketch True\n\
                         Set skFeat = swSketchMgr.ActiveSketch\n\
                         If skFeat Is Nothing Then\n\
                             gProbeLog = gProbeLog & feat.Name & \": no sketch opened; \"\n\
                         ElseIf PlaneIsSource(which) Then\n\
                             ClearMap idx
If AcceptAndPublish(idx, which, gProbeReason) Then
OpenSketchOnPlane = True
Exit Function
End If
gProbeLog = gProbeLog & feat.Name & \": map refused: \" & gProbeReason & \" strict-match=\" & CStr(SketchFrameIs(which)) & \"; \"
If Not DiscardProbeSketch(skFeat) Then Err.Raise vbObjectError + 520, \"craft-bridge\", \"the refused probe sketch could not be confirmed deleted; probed: \" & gProbeLog\n\
                         Else\n\
                             gProbeLog = gProbeLog & feat.Name & \": \" & FrameText() & \" strict-match=\" & CStr(SketchFrameIs(which)) & \"; \"\n\
                             If Not DiscardProbeSketch(skFeat) Then Err.Raise vbObjectError + 520, \"craft-bridge\", \"the probe sketch could not be confirmed deleted; probed: \" & gProbeLog\n\
                         End If\n\
                     End If\n\
                 End If\n\
                 Set feat = feat.GetNextFeature\n\
             Loop\n\
             OpenSketchOnPlane = False\n\
         End Function\n\
\n\
         ' The plane feature's own runtime name (never assumed) for call shapes that\n\
         ' need the plane selected as a FEATURE, like mirrors. The probe sketch is\n\
         ' discarded: the mirror needs the plane, not a sketch.\n\
         Function PlaneNameFor(ByVal which As String) As String\n\
             Dim feat As SldWorks.Feature, skFeat As SldWorks.Feature, ok As Boolean\n\
             Set feat = swDoc.FirstFeature\n\
             Do While Not feat Is Nothing\n\
                 If InStr(1, feat.GetTypeName2(), \"lane\", vbTextCompare) > 0 Then\n\
                     ok = feat.Select(False)\n\
                     If ok Then\n\
                         swSketchMgr.InsertSketch True\n\
                         Set skFeat = swSketchMgr.ActiveSketch\n\
                         If skFeat Is Nothing Then\n\
                             gProbeLog = gProbeLog & feat.Name & \": no sketch opened; \"\n\
                         ElseIf PlaneIsSource(which) Then\n\
                             gProbeLog = gProbeLog & feat.Name & \": \" & FrameText() & \" strict-match=\" & CStr(SketchFrameIs(which)) & \"; \"\n\
                             If Not DiscardProbeSketch(skFeat) Then Err.Raise vbObjectError + 520, \"craft-bridge\", \"the probe sketch could not be confirmed deleted; probed: \" & gProbeLog\n\
                             PlaneNameFor = feat.Name\n\
                             Exit Function\n\
                         Else\n\
                             gProbeLog = gProbeLog & feat.Name & \": \" & FrameText() & \" strict-match=\" & CStr(SketchFrameIs(which)) & \"; \"\n\
                             If Not DiscardProbeSketch(skFeat) Then Err.Raise vbObjectError + 520, \"craft-bridge\", \"the probe sketch could not be confirmed deleted; probed: \" & gProbeLog\n\
                         End If\n\
                     End If\n\
                 End If\n\
                 Set feat = feat.GetNextFeature\n\
             Loop\n\
         End Function\n\
\n\
         ' The strict frame predicate (DIAGNOSTIC ONLY): the sketch frame exactly
' equal to the SolidCraft frame for the plane (its plane.rs 29-34): XZ has
' v = -Z, n = +Y; the sketch origin at the model origin. RETIRED from the
' acceptance path (owner decision 2026-11-13, frame adaptation): acceptance
' is PlaneIsSource plus the fail-closed map validation in AcceptAndPublish.
' This predicate remains to report how close a probed frame came to strict
' equality. The model-to-sketch transform is row-major with rows = model
' axes in sketch coordinates, so the sketch axes in model coordinates are
' its columns: u = (a0, a3, a6), v = (a1, a4, a7), n = (a2, a5, a8).
Function SketchFrameIs(ByVal which As String) As Boolean
Dim tr As Variant, a As Variant, ok As Boolean
On Error Resume Next
Set tr = swSketchMgr.ActiveSketch.ModelToSketchTransform
If Err.Number <> 0 Then Exit Function
a = tr.ArrayData
If Abs(a(9)) > 0.000001 Or Abs(a(10)) > 0.000001 Or Abs(a(11)) > 0.000001 Then Exit Function
Select Case which
Case \"XY\"
ok = VecIs(a(0), a(3), a(6), 1#, 0#, 0#) And VecIs(a(1), a(4), a(7), 0#, 1#, 0#) And VecIs(a(2), a(5), a(8), 0#, 0#, 1#)
Case \"XZ\"
ok = VecIs(a(0), a(3), a(6), 1#, 0#, 0#) And VecIs(a(1), a(4), a(7), 0#, 0#, -1#) And VecIs(a(2), a(5), a(8), 0#, 1#, 0#)
Case \"YZ\"
ok = VecIs(a(0), a(3), a(6), 0#, 1#, 0#) And VecIs(a(1), a(4), a(7), 0#, 0#, 1#) And VecIs(a(2), a(5), a(8), 1#, 0#, 0#)
End Select
SketchFrameIs = ok
End Function

' The acceptance predicate (frame adaptation): the probe sketch must sit on
' the geometric source plane - translation at the model origin and the
' normal parallel to the source normal (either sign: the same geometric
' plane). Strict frame equality is no longer required (owner decision
' 2026-11-13); the adaptation map published by AcceptAndPublish is what
' makes that safe. Tolerance 1e-6.
Function PlaneIsSource(ByVal which As String) As Boolean
Dim tr As Object
Dim a As Variant
Dim nx As Double
Dim ny As Double
Dim nz As Double
On Error Resume Next
PlaneIsSource = False
Err.Clear
Set tr = swSketchMgr.ActiveSketch.ModelToSketchTransform
If Err.Number <> 0 Then Exit Function
Err.Clear
a = tr.ArrayData
If Err.Number <> 0 Then Exit Function
If Abs(a(9)) > 0.000001 Or Abs(a(10)) > 0.000001 Or Abs(a(11)) > 0.000001 Then Exit Function
nx = a(2)
ny = a(5)
nz = a(8)
Select Case which
Case \"XY\"
If Abs(Abs(nz) - 1#) > 0.000001 Then Exit Function
Case \"XZ\"
If Abs(Abs(ny) - 1#) > 0.000001 Then Exit Function
Case \"YZ\"
If Abs(Abs(nx) - 1#) > 0.000001 Then Exit Function
Case Else
Exit Function
End Select
PlaneIsSource = True
End Function\n\
\n\
         ' Direction equality with sign, on normalized vectors: the transform may\n\
         ' carry a uniform scale, the axes must still be exactly the expected ones.\n\
         Function VecIs(ByVal ax As Double, ByVal ay As Double, ByVal az As Double, ByVal bx As Double, ByVal by As Double, ByVal bz As Double) As Boolean\n\
             Dim L As Double\n\
             L = Sqr(ax * ax + ay * ay + az * az)\n\
             If Abs(L) < 0.000001 Then Exit Function\n\
             VecIs = Abs(ax / L - bx) < 0.000001 And Abs(ay / L - by) < 0.000001 And Abs(az / L - bz) < 0.000001\n\
         End Function\n\
\n\
         ' The active sketch's frame as report text: the fail-closed messages name\n\
         ' exactly what each probed plane offered.\n\
         Function FrameText() As String\n\
             Dim tr As Variant, a As Variant\n\
             On Error Resume Next\n\
             Set tr = swSketchMgr.ActiveSketch.ModelToSketchTransform\n\
             If Err.Number <> 0 Then FrameText = \"(no frame)\": Exit Function\n\
             a = tr.ArrayData\n\
             FrameText = \"u=(\" & Vec3Text(a(0), a(3), a(6)) & \") v=(\" & Vec3Text(a(1), a(4), a(7)) & \") n=(\" & Vec3Text(a(2), a(5), a(8)) & \") o=(\" & Vec3Text(a(9), a(10), a(11)) & \")\"\n\
         End Function\n\
\n\
         Function Vec3Text(ByVal x As Double, ByVal y As Double, ByVal z As Double) As String\n\
             Vec3Text = Trim(Str(x)) & \",\" & Trim(Str(y)) & \",\" & Trim(Str(z))\n\
         End Function\n\
\n\
         ' A live feature-tree lookup by name. Returns the feature when a complete\n\
         ' walk found it; returns Nothing with uncertain = False only when a\n\
         ' complete walk proves it absent; uncertain = True whenever the walk or\n\
         ' the name failed. Absence is never inferred from a failed query, and an\n\
         ' empty name is never sent to any API.\n\
         Function FindFeatureByName(ByVal nm As String, ByRef uncertain As Boolean) As SldWorks.Feature\n\
             Dim feat As SldWorks.Feature, fn As String\n\
             Set FindFeatureByName = Nothing\n\
             uncertain = True\n\
             If nm = \"\" Then Exit Function\n\
             On Error Resume Next\n\
             Err.Clear\n\
             Set feat = swDoc.FirstFeature\n\
             Do While Not feat Is Nothing\n\
                 If Err.Number <> 0 Then Exit Function\n\
                 Err.Clear\n\
                 fn = feat.Name\n\
                 If Err.Number <> 0 Then Exit Function\n\
                 If fn = nm Then\n\
                     Set FindFeatureByName = feat\n\
                     uncertain = False\n\
                     Exit Function\n\
                 End If\n\
                 Err.Clear\n\
                 Set feat = feat.GetNextFeature\n\
             Loop\n\
             If Err.Number = 0 Then uncertain = False\n\
         End Function\n\
 \n\
         ' Close the probe sketch and delete exactly the feature that was created\n\
         ' for it - never a blind EditDelete, never a stale-object test. The name\n\
         ' is captured while the object is alive and validated; the close is\n\
         ' confirmed; the delete runs from a clean selection of the one feature the\n\
         ' live tree returns; and the deletion is verified the same way. Closing an\n\
         ' empty sketch may already have removed it - that is a clean result. True\n\
         ' = the probe sketch is proven gone; every uncertain state fails closed.\n\
         Function DiscardProbeSketch(ByVal skFeat As SldWorks.Feature) As Boolean\n\
             Dim nm As String, cur As Object, live As SldWorks.Feature\n\
             Dim uncertain As Boolean\n\
             DiscardProbeSketch = False\n\
             On Error Resume Next\n\
             Err.Clear\n\
             nm = skFeat.Name\n\
             If Err.Number <> 0 Then Exit Function\n\
             If nm = \"\" Then Exit Function\n\
             Err.Clear\n\
             Set cur = swSketchMgr.ActiveSketch\n\
             If Err.Number <> 0 Then Exit Function\n\
             If Not cur Is Nothing Then\n\
                 Err.Clear\n\
                 swSketchMgr.InsertSketch True\n\
                 If Err.Number <> 0 Then Exit Function\n\
                 Err.Clear\n\
                 Set cur = swSketchMgr.ActiveSketch\n\
                 If Err.Number <> 0 Then Exit Function\n\
                 If Not cur Is Nothing Then Exit Function\n\
             End If\n\
             Set live = FindFeatureByName(nm, uncertain)\n\
             If live Is Nothing Then\n\
                 DiscardProbeSketch = Not uncertain\n\
                 Exit Function\n\
             End If\n\
             Err.Clear\n\
             swDoc.ClearSelection2 True\n\
             If Err.Number <> 0 Then Exit Function\n\
             Err.Clear\n\
             If Not live.Select(False) Then Exit Function\n\
             Err.Clear\n\
             swDoc.EditDelete\n\
             If Err.Number <> 0 Then Exit Function\n\
             Err.Clear\n\
             Set live = FindFeatureByName(nm, uncertain)\n\
             DiscardProbeSketch = (live Is Nothing) And (Not uncertain)\n\
         End Function\n\
 \n\
         ' Close the sketch only if one is actually open: InsertSketch toggles, so a\n\
         ' blind call would open a new sketch when none is active.\n\
         Sub CloseSketchIfOpen()\n\
             Dim sk As Object\n\
             On Error Resume Next\n\
             Set sk = swSketchMgr.ActiveSketch\n\
             If Not sk Is Nothing Then swSketchMgr.InsertSketch True\n\
         End Sub\n\
\n\
         ' The first feature type names in the tree: the report names what the scan\n\
         ' actually saw when plane selection fails.\n\
         Function TreeSummary() As String\n\
             Dim feat As SldWorks.Feature, s As String, planes As String, i As Long\n\
             On Error Resume Next\n\
             Set feat = swDoc.FirstFeature\n\
             Do While Not feat Is Nothing And i < 300\n\
                 If InStr(1, feat.GetTypeName2(), \"lane\", vbTextCompare) > 0 Then\n\
                     planes = planes & feat.GetTypeName2() & \"/\" & feat.Name & \" \"\n\
                 End If\n\
                 If i < 10 Then s = s & feat.GetTypeName2() & \" \"\n\
                 i = i + 1\n\
                 Set feat = feat.GetNextFeature\n\
             Loop\n\
             TreeSummary = Trim(Str(i)) & \" features; plane-like: [\" & Trim(planes) & \"]; first: \" & s\n\
         End Function\n\
\n\
         ' Truncate the report so this run never inherits a previous run's lines.\n\
         Sub StartReport()\n\
             On Error Resume Next\n\
             Dim f As Integer\n\
             f = FreeFile\n\
             Open reportPath For Output As #f\n\
             Close #f\n\
         End Sub\n\
\n\
         ' One line, one fresh handle: open, print, close. Nothing to leak,\n\
         ' nothing to collide with, and a crash can never hide earlier lines.\n\
         Sub WriteLine(ByVal text As String)\n\
             On Error Resume Next\n\
             Dim f As Integer\n\
             f = FreeFile\n\
             Open reportPath For Append As #f\n\
             Print #f, text\n\
             Close #f\n\
         End Sub\n\
\n\
         Sub WriteStep(ByVal idx As Long, ByVal command As String, ByVal status As String, ByVal message As String)\n\
             On Error Resume Next\n\
             WriteLine \"{\"\"type\"\":\"\"step\"\",\"\"index\"\":\" & Trim(Str(idx)) & \",\"\"command\"\":\"\"\" & JsonStr(command) & \"\"\",\"\"status\"\":\"\"\" & JsonStr(status) & \"\"\",\"\"message\"\":\"\"\" & JsonStr(message) & \"\"\"}\"\n\

         End Sub\n\
\n\
\n\
                 ' Close exactly the document this macro created - never any other - and\n\
        ' verify it by a live enumeration of open documents: before the close, the\n\
        ' title must identify exactly our object among open documents (an empty\n\
        ' title is refused); after it, no open document may answer to the title.\n\
        ' A failed query proves nothing and leaves the run failed. The owner rule:\n\
        ' runs leave no documents open.\n\
        Sub CloseOwnedDoc(ByRef closedOk As Boolean)
Dim exact As String
Dim d As Object
Dim t2 As String
Dim cnt As Long
Dim isOurs As Boolean
On Error Resume Next
closedOk = False
If swDoc Is Nothing Then
closedOk = True
Exit Sub
End If
Err.Clear
exact = swDoc.GetTitle
If Err.Number <> 0 Then Exit Sub
If Trim(exact) = \"\" Then Exit Sub
Err.Clear
Set d = swApp.GetFirstDocument
If Err.Number <> 0 Then Exit Sub
Do While Not d Is Nothing
t2 = d.GetTitle
If Err.Number <> 0 Then Exit Sub
If t2 = exact Then
cnt = cnt + 1
If d Is swDoc Then isOurs = True
End If
Err.Clear
Set d = d.GetNext
If Err.Number <> 0 Then Exit Sub
Loop
If cnt <> 1 Or Not isOurs Then Exit Sub
Err.Clear
swApp.CloseDoc exact
If Err.Number <> 0 Then Exit Sub
Err.Clear
Set d = swApp.GetFirstDocument
If Err.Number <> 0 Then Exit Sub
cnt = 0
Do While Not d Is Nothing
t2 = d.GetTitle
If Err.Number <> 0 Then Exit Sub
If t2 = exact Then cnt = cnt + 1
Err.Clear
Set d = d.GetNext
If Err.Number <> 0 Then Exit Sub
Loop
If cnt = 0 Then closedOk = True
End Sub

Function CloseMode() As String
Dim p As String, f As Integer, oneLine As String
Dim ok As Boolean
Dim e3 As Long
CloseMode = \"unknown\"
On Error Resume Next
p = Environ$(\"TEMP\")
If p = \"\" Then Exit Function
p = p & \"\\craft-bridge-report-path.txt\"
Err.Clear
f = FreeFile
Open p For Input As #f
If Err.Number <> 0 Then Exit Function
Err.Clear
Line Input #f, oneLine
ok = (Err.Number = 0)
Err.Clear
If ok Then Line Input #f, oneLine
ok = ok And (Err.Number = 0)
Err.Clear
If ok Then Line Input #f, oneLine
e3 = Err.Number
Close #f
If Not ok Then Exit Function
If e3 = 0 Then
If Trim(oneLine) = \"close-after-run\" Then CloseMode = \"close\" Else CloseMode = \"unknown\"
ElseIf e3 = 62 And ok Then
CloseMode = \"keep\"
End If
End Function

Function OwnedDocTitle() As String
On Error Resume Next
OwnedDocTitle = \"\"
If swDoc Is Nothing Then Exit Function
OwnedDocTitle = \"\" & swDoc.GetTitle
End Function\n\

' Accept a probed origin plane and publish its adaptation map for sketch
' slot idx: the probe-sketch translation must vanish and the normal must
' be parallel to the source normal (either sign) - the map and side come
' from PublishMap's fail-closed validation. Every read is trapped; any
' refusal carries its reason.
Function AcceptAndPublish(ByVal idx As Long, ByVal which As String, ByRef reason As String) As Boolean
Dim tr As Object
Dim a As Variant
Dim uPx As Double
Dim uPy As Double
Dim uPz As Double
Dim vPx As Double
Dim vPy As Double
Dim vPz As Double
Dim nPx As Double
Dim nPy As Double
Dim nPz As Double
On Error GoTo AccFail
AcceptAndPublish = False
reason = \"\"
Select Case which
Case \"XY\"
uPx = 1#: uPy = 0#: uPz = 0#
vPx = 0#: vPy = 1#: vPz = 0#
nPx = 0#: nPy = 0#: nPz = 1#
Case \"XZ\"
uPx = 1#: uPy = 0#: uPz = 0#
vPx = 0#: vPy = 0#: vPz = -1#
nPx = 0#: nPy = 1#: nPz = 0#
Case \"YZ\"
uPx = 0#: uPy = 1#: uPz = 0#
vPx = 0#: vPy = 0#: vPz = 1#
nPx = 1#: nPy = 0#: nPz = 0#
Case Else
reason = \"unknown source plane \" & which
Exit Function
End Select
Set tr = swSketchMgr.ActiveSketch.ModelToSketchTransform
If tr Is Nothing Then
reason = \"the transform is missing\"
Exit Function
End If
a = tr.ArrayData
If Abs(a(9)) > 0.000001 Or Abs(a(10)) > 0.000001 Or Abs(a(11)) > 0.000001 Then
reason = \"the probe sketch origin is \" & Vec3Text(a(9), a(10), a(11)) & \", not the model origin\"
Exit Function
End If
If Not PublishMap(idx, uPx, uPy, uPz, vPx, vPy, vPz, nPx, nPy, nPz, reason) Then Exit Function
AcceptAndPublish = True
Exit Function
AccFail:
reason = \"the frame read failed: err \" & Trim(Str(Err.Number)) & \": \" & Err.Description
End Function

' Invalidate one sketch slot's map (sketch deletion or re-probe). The
' publication is transactional: nothing reads a map that is not valid.
Sub ClearMap(ByVal idx As Long)
gMapValid(idx) = False
gMapSide(idx) = 0
End Sub

' Runtime axis classification (frame adaptation): the SC u direction maps
' to sketch (s,t) as (gMap00, gMap01), the SC v direction as (gMap10,
' gMap11). For a signed-permutation map each lands axis-aligned: AxisClass
' returns 0 when it lands on the s axis, 1 on the t axis, and -1 when the
' map is not a permutation (bounded support: this seat only exhibits
' permutation frames). AxisSign is the sign of the landed component.
Function AxisClass(ByVal idx As Long, ByVal isHoriz As Boolean) As Long
Dim c0 As Double
Dim c1 As Double
If isHoriz Then
c0 = gMap00(idx)
c1 = gMap01(idx)
Else
c0 = gMap10(idx)
c1 = gMap11(idx)
End If
AxisClass = -1
If Abs(Abs(c0) - 1#) <= 0.000001 And Abs(c1) <= 0.000001 Then AxisClass = 0
If Abs(Abs(c1) - 1#) <= 0.000001 And Abs(c0) <= 0.000001 Then AxisClass = 1
End Function

Function AxisSign(ByVal idx As Long, ByVal isHoriz As Boolean, ByVal ax As Long) As Double
Dim c As Double
If isHoriz Then
If ax = 0 Then c = gMap00(idx) Else c = gMap01(idx)
Else
If ax = 0 Then c = gMap10(idx) Else c = gMap11(idx)
End If
AxisSign = 1#
If c < 0 Then AxisSign = -1#
End Function

' The dimension text position is MODEL space (Phase 0, owner-verified):
' convert a sketch (s,t) position through the stored frame to (X,Y,Z).
Function SketchToModelX(ByVal idx As Long, ByVal s As Double, ByVal t As Double) As Double
SketchToModelX = gFrame(idx, 9) + s * gFrame(idx, 0) + t * gFrame(idx, 3)
End Function

Function SketchToModelY(ByVal idx As Long, ByVal s As Double, ByVal t As Double) As Double
SketchToModelY = gFrame(idx, 10) + s * gFrame(idx, 1) + t * gFrame(idx, 4)
End Function

Function SketchToModelZ(ByVal idx As Long, ByVal s As Double, ByVal t As Double) As Double
SketchToModelZ = gFrame(idx, 11) + s * gFrame(idx, 2) + t * gFrame(idx, 5)
End Function

' Frame adaptation (batch 3): primitive replay supports signed-
' permutation maps (bounded support - this seat only exhibits permutation
' frames): each map entry is 0 or +-1 with exactly one nonzero per row and
' per column. The map's orientation (gMapSide) is unconstrained - the
' direction truth table composes it. Anything else is refused with a named
' reason and never reports created (docs/mapping.md, Primitives).
Function MapIsPermutation(ByVal idx As Long) As Boolean
Dim m00 As Double
Dim m01 As Double
Dim m10 As Double
Dim m11 As Double
MapIsPermutation = False
If Not gMapValid(idx) Then Exit Function
m00 = Abs(gMap00(idx))
m01 = Abs(gMap01(idx))
m10 = Abs(gMap10(idx))
m11 = Abs(gMap11(idx))
If m00 > 0.000001 And Abs(m00 - 1#) > 0.000001 Then Exit Function
If m01 > 0.000001 And Abs(m01 - 1#) > 0.000001 Then Exit Function
If m10 > 0.000001 And Abs(m10 - 1#) > 0.000001 Then Exit Function
If m11 > 0.000001 And Abs(m11 - 1#) > 0.000001 Then Exit Function
If Abs(m00 + m10 - 1#) > 0.000001 Then Exit Function
If Abs(m01 + m11 - 1#) > 0.000001 Then Exit Function
If Abs(m00 + m01 - 1#) > 0.000001 Then Exit Function
If Abs(m10 + m11 - 1#) > 0.000001 Then Exit Function
MapIsPermutation = True
End Function

' The just-created sketch as a TREE FEATURE (seat-proven 2026-11-17:
' ActiveSketch.Name is an internal name that does NOT match the tree when
' probe-sketch deletion recycled the name - SelectByID2 by it fails and
' equations qualified by it drive nothing). A fresh part always appends the
' new sketch as the last tree feature; its Name is the true tree name and
' its Select is the pinned-working selection.
Function LastFeature() As SldWorks.Feature
Dim f As SldWorks.Feature
Dim last As SldWorks.Feature
On Error Resume Next
Err.Clear
Set f = swDoc.FirstFeature
If Err.Number <> 0 Then Exit Function
Do While Not f Is Nothing
Set last = f
Err.Clear
Set f = f.GetNextFeature
If Err.Number <> 0 Then Exit Function
Loop
Set LastFeature = last
End Function

' Rebuild and stay in sketch edit (seat-proven 2026-11-16: EditRebuild3
' EXITS sketch edit - sketch API calls then silently do nothing, and its
' Boolean return is NOT a gate: it returns False in working flows). The
' rebuild is required before any behavioural readback of a constraint;
' re-entry is required before any further sketch API call. The final state
' is verified; the refusal is raised only AFTER error handling is restored,
' so it cannot be swallowed.
Sub SolveSketch(ByVal nm As String, ByVal skObj As SldWorks.Feature)
Dim sk As Object
Dim okSel As Boolean
swDoc.EditRebuild3
On Error Resume Next
Set sk = swSketchMgr.ActiveSketch
If sk Is Nothing Then
Err.Clear
okSel = False
If Not skObj Is Nothing Then okSel = skObj.Select(False)
If Not okSel Then swDoc.Extension.SelectByID2 nm, \"SKETCH\", 0#, 0#, 0#, False, 0, Nothing, swSelectOption_e.swSelectOptionDefault
swSketchMgr.InsertSketch True
Set sk = swSketchMgr.ActiveSketch
End If
On Error GoTo 0
If sk Is Nothing Then Err.Raise vbObjectError + 529, \"craft-bridge\", \"sketch edit could not be re-entered for \" & nm
End Sub

' The closed-context half of verification (F7): after closing, no sketch
' may be active. The geometry readback runs in both contexts - the seat
' pinned handle readback as identical in active and closed contexts.
Function NoSketchIsOpen() As Boolean
Dim sk As Object
On Error Resume Next
Err.Clear
Set sk = swSketchMgr.ActiveSketch
If Err.Number <> 0 Then Exit Function
NoSketchIsOpen = (sk Is Nothing)
End Function

' Frame adaptation map (owner decision 2026-11-13): each sketch step
' publishes its own 2x2 map after its probe validates the frame; features
' referencing an earlier sketch restore that sketch's state first. MapU/
' MapV express SC (u,v) in the sketch's own (s,t): s = u(u_P.u') + v(v_P.u'),
' t = u(u_P.v') + v(v_P.v'), as stored by PublishMap.
Function MapU(ByVal idx As Long, ByVal u As Double, ByVal v As Double) As Double
MapU = u * gMap00(idx) + v * gMap10(idx)
End Function

Function MapV(ByVal idx As Long, ByVal u As Double, ByVal v As Double) As Double
MapV = u * gMap01(idx) + v * gMap11(idx)
End Function

' Publish the map for sketch slot idx from the probed frame. Fail-closed
' and transactional: validity is cleared first, every read and conversion
' is trapped, values are computed into locals, and the map is stored only
' after the complete validation passes - the pinned 16-entry ArrayData
' shape, finite entries, scale ~ 1 (entry 12), unit axes, mutual
' orthogonality, and a right-handed 3D frame (u x v = n). A proper 3D frame
' may still give an improper 2D map when the normal opposes the source
' normal - that is supported and recorded in gMapSide. Tolerances: 1e-6.
Function PublishMap(ByVal idx As Long, ByVal uPx As Double, ByVal uPy As Double, ByVal uPz As Double, ByVal vPx As Double, ByVal vPy As Double, ByVal vPz As Double, ByVal nPx As Double, ByVal nPy As Double, ByVal nPz As Double, ByRef reason As String) As Boolean
Dim tr As Object
Dim a As Variant
Dim ux As Double
Dim uy As Double
Dim uz As Double
Dim vx As Double
Dim vy As Double
Dim vz As Double
Dim nx As Double
Dim ny As Double
Dim nz As Double
Dim sc As Double
Dim ub As Long
Dim i As Long
Dim x As Double
Dim m00 As Double
Dim m01 As Double
Dim m10 As Double
Dim m11 As Double
Dim dot As Double
Dim side As Long
PublishMap = False
reason = \"\"
gMapValid(idx) = False
On Error Resume Next
Err.Clear
Set tr = swSketchMgr.ActiveSketch.ModelToSketchTransform
If Err.Number <> 0 Then
reason = \"the transform could not be read: err \" & Trim(Str(Err.Number))
Exit Function
End If
If tr Is Nothing Then
reason = \"the transform is missing\"
Exit Function
End If
Err.Clear
a = tr.ArrayData
If Err.Number <> 0 Then
reason = \"ArrayData could not be read: err \" & Trim(Str(Err.Number))
Exit Function
End If
Err.Clear
ub = UBound(a)
If Err.Number <> 0 Then
reason = \"ArrayData is not an array: err \" & Trim(Str(Err.Number))
Exit Function
End If
If ub <> 15 Then
reason = \"ArrayData has \" & Trim(Str(ub + 1)) & \" entries; the pinned shape is 16\"
Exit Function
End If
For i = 0 To 15
Err.Clear
x = CDbl(a(i))
If Err.Number <> 0 Then
reason = \"entry \" & Trim(Str(i)) & \" is not numeric: err \" & Trim(Str(Err.Number))
Exit Function
End If
If x <> x Then
reason = \"entry \" & Trim(Str(i)) & \" is not finite\"
Exit Function
End If
If Abs(x) > 1E+100 Then
reason = \"entry \" & Trim(Str(i)) & \" is not finite\"
Exit Function
End If
Next i
' Arithmetic is trapped: an overflow must fail the publication, never skip
' a check and reach the store below.
On Error GoTo MapFail
ux = a(0)
uy = a(3)
uz = a(6)
vx = a(1)
vy = a(4)
vz = a(7)
nx = a(2)
ny = a(5)
nz = a(8)
sc = a(12)
If Abs(sc - 1#) > 0.000001 Then
reason = \"scale is \" & Trim(Str(sc)) & \", not 1\"
Exit Function
End If
If Abs(Sqr(ux * ux + uy * uy + uz * uz) - 1#) > 0.000001 Then reason = \"u is not unit\": Exit Function
If Abs(Sqr(vx * vx + vy * vy + vz * vz) - 1#) > 0.000001 Then reason = \"v is not unit\": Exit Function
If Abs(Sqr(nx * nx + ny * ny + nz * nz) - 1#) > 0.000001 Then reason = \"n is not unit\": Exit Function
If Abs(ux * vx + uy * vy + uz * vz) > 0.000001 Then reason = \"u and v are not orthogonal\": Exit Function
If Abs(ux * nx + uy * ny + uz * nz) > 0.000001 Then reason = \"u and n are not orthogonal\": Exit Function
If Abs(vx * nx + vy * ny + vz * nz) > 0.000001 Then reason = \"v and n are not orthogonal\": Exit Function
If Abs((uy * vz - uz * vy) - nx) > 0.000001 Then reason = \"u x v does not equal n (x) - the 3D frame is not right-handed\": Exit Function
If Abs((uz * vx - ux * vz) - ny) > 0.000001 Then reason = \"u x v does not equal n (y) - the 3D frame is not right-handed\": Exit Function
If Abs((ux * vy - uy * vx) - nz) > 0.000001 Then reason = \"u x v does not equal n (z) - the 3D frame is not right-handed\": Exit Function
m00 = uPx * ux + uPy * uy + uPz * uz
m10 = vPx * ux + vPy * uy + vPz * uz
m01 = uPx * vx + uPy * vy + uPz * vz
m11 = vPx * vx + vPy * vy + vPz * vz
dot = nx * nPx + ny * nPy + nz * nPz
If Abs(dot) < 0.999999 Then
If Abs(dot) <= 0.000001 Then
reason = \"the normal is perpendicular to the source normal\"
Else
reason = \"the normal is only \" & Trim(Str(dot)) & \" parallel to the source normal\"
End If
Exit Function
End If
side = 1
If dot < 0 Then side = -1
' Publish only now: nothing above has touched persistent state.
gMap00(idx) = m00
gMap10(idx) = m10
gMap01(idx) = m01
gMap11(idx) = m11
gMapSide(idx) = side
gFrame(idx, 0) = ux
gFrame(idx, 1) = uy
gFrame(idx, 2) = uz
gFrame(idx, 3) = vx
gFrame(idx, 4) = vy
gFrame(idx, 5) = vz
gFrame(idx, 6) = nx
gFrame(idx, 7) = ny
gFrame(idx, 8) = nz
gFrame(idx, 9) = a(9)
gFrame(idx, 10) = a(10)
gFrame(idx, 11) = a(11)
gMapValid(idx) = True
PublishMap = True
Exit Function
MapFail:
reason = \"the frame computation failed: err \" & Trim(Str(Err.Number)) & \": \" & Err.Description
End Function

' The report path travels in a one-line file under the TEMP directory that
' the host rewrites before every run: preferred over CRAFT_BRIDGE_REPORT,
' because the macro runs inside the SolidWorks process, which may be an
' instance that started long ago with a stale environment. The environment
' variable is only the fallback for when the path file is missing or blank.
Function ReportPathFromHost() As String
Dim p As String, f As Integer, oneLine As String
Dim got As Boolean
ReportPathFromHost = \"\"
On Error Resume Next
p = Environ$(\"TEMP\")
If p <> \"\" Then
p = p & \"\\craft-bridge-report-path.txt\"
Err.Clear
f = FreeFile
Open p For Input As #f
If Err.Number = 0 Then
Err.Clear
Line Input #f, oneLine
got = (Err.Number = 0)
Err.Clear
Close #f
If got Then
oneLine = Trim(oneLine)
If oneLine <> \"\" Then
ReportPathFromHost = oneLine
Exit Function
End If
End If
End If
Err.Clear
End If
ReportPathFromHost = Trim(Environ$(\"CRAFT_BRIDGE_REPORT\"))
End Function\n\
\n\
         ' The directory of the macro (its sidecar snapshots live there): the\n\
         ' second line of the host's path file.\n\
         Function MacroDirFromHost() As String\n\
             Dim p As String, f As Integer, oneLine As String\n\
             p = Environ$(\"TEMP\")\n\
             If p = \"\" Then Exit Function\n\
             p = p & \"\\craft-bridge-report-path.txt\"\n\
             Err.Clear\n\
             On Error Resume Next\n\
             f = FreeFile\n\
             Open p For Input As #f\n\
             If Err.Number = 0 Then\n\
                 Line Input #f, oneLine\n\
                 Line Input #f, oneLine\n\
                 Close #f\n\
                 If Err.Number = 0 Then MacroDirFromHost = Trim(oneLine)\n\
             End If\n\
         End Function\n\
         ' JSON string escaping for the report file only.\n\
         Function JsonStr(ByVal s As String) As String\n\
             Dim t As String\n\
             t = Replace(s, \"\\\", \"\\\\\")\n\
             t = Replace(t, \"\"\"\", \"\\\"\"\")\n\
             t = Replace(t, vbCrLf, \"\\n\")\n\
             t = Replace(t, vbCr, \"\\n\")\n\
             t = Replace(t, vbLf, \"\\n\")\n\
             t = Replace(t, vbTab, \"\\t\")\n\
             JsonStr = t\n\
         End Function\n\
\n\
         ' Add a global-variable equation and rebuild. The name is a global, the\n\
         ' right-hand side is equation syntax. Globals are verified through the\n\
         ' dimensions that reference them.\n\
         Function AddGlobalEquation(ByVal nm As String, ByVal rhs As String, ByVal expect As Double, ByVal evalScale As Double) As String
On Error GoTo EqFail
Dim eqMgr As Object
Dim eqText As String
Dim idx As Long
Set eqMgr = swDoc.GetEquationMgr
If eqMgr Is Nothing Then gMsg = \"the equation manager is missing\": AddGlobalEquation = \"failed\": Exit Function
eqText = \"\"\"\" & nm & \"\"\"=\" & rhs
idx = eqMgr.Add(-1, eqText)
If idx < 0 Then gMsg = \"equation failed: the equation was not accepted\": GoTo EqFail
swDoc.EditRebuild3

If eqMgr.Equation(idx) <> eqText Then gMsg = \"equation failed: the stored equation reads back differently\": GoTo EqFail
If evalScale <= 0 Then gMsg = \"equation failed: the evaluator unit semantics are not established for this parameter\": GoTo EqFail\n\
If Abs(eqMgr.Value(idx) - expect * evalScale) > 0.001 Then gMsg = \"equation failed: the equation value does not match its written right-hand side\": GoTo EqFail
gMsg = \"equation \" & eqText
AddGlobalEquation = \"created\"
Exit Function
EqFail:
If Err.Number <> 0 Then gMsg = \"equation failed: \" & Err.Description
AddGlobalEquation = \"failed\"
End Function\n\
\n\
         ' Add an equation driving a dimension and verify it evaluated: the\n\
         ' dimension name is captured at runtime, the value must match the\n\
         ' expectation after a rebuild. Never presence alone.\n\
         Function AddDimEquation(ByVal dm As SldWorks.Dimension, ByVal rhs As String, ByVal expect As Double, ByVal slot As Long, ByVal signVal As Double, ByVal qual As String, ByVal dimScale As Double) As Boolean
On Error GoTo EqFail
Dim eqMgr As Object
Dim eqText As String
Dim idx As Long
Dim qName As String
Set eqMgr = swDoc.GetEquationMgr
If eqMgr Is Nothing Then gMsg = \"the equation manager is missing\": GoTo EqFail
qName = dm.Name
If InStr(qName, \"@\") = 0 Then qName = qName & \"@\" & qual
If signVal < 0 Then
eqText = \"\"\"\" & qName & \"\"\"=-1 * (\" & rhs & \")\"
Else
eqText = \"\"\"\" & qName & \"\"\"=\" & rhs
End If
idx = eqMgr.Add(-1, eqText)
If idx < 0 Then gMsg = \"the dimension equation was not accepted: \" & eqText: GoTo EqFail
swDoc.EditRebuild3
If Abs(dm.SystemValue - expect) > 0.000001 Then gMsg = \"the driven value does not match: equation <\" & eqText & \"> drove \" & Trim(Str(dm.SystemValue)) & \" but the expectation is \" & Trim(Str(expect)): GoTo EqFail

If eqMgr.Equation(idx) <> eqText Then gMsg = \"the stored equation reads back differently\": GoTo EqFail
If slot >= 0 Then regName(slot) = qName
If slot >= 0 Then regSign(slot) = signVal
If slot >= 0 Then regScale(slot) = dimScale
AddDimEquation = True
Exit Function
EqFail:
If Err.Number <> 0 Then gMsg = \"err \" & Trim(Str(Err.Number)) & \": \" & Err.Description
AddDimEquation = False
End Function\n",
    );
    out
}

/// Per-sketch subs: BuildSketchN (orchestrator with the restore path),
/// DrawGeometryN, RelationsN, DimsN, VerifySketchN, CleanSketchN.
fn emit_sketch_subs(index: usize, sk: &SketchStep) -> String {
    let mut out = String::new();
    let initial_status = if sk.dropped_constraints.is_empty() {
        "created"
    } else {
        "degraded"
    };
    let initial_msg = if sk.dropped_constraints.is_empty() {
        format!(
            "{} relations and {} dimensions replayed",
            sk.relations.len(),
            sk.dimensions.len()
        )
    } else {
        format!(
            "constraints not replayed: {}",
            sk.dropped_constraints.join(", ")
        )
    };
    out.push_str(&format!("' Sketch step {index}: {}\n", vb_lit(&sk.name)));
    out.push_str(&format!(
        "Function BuildSketch{index}() As String\n    On Error GoTo HardFail\n    Dim ok As Boolean\n    Dim skFeat As SldWorks.Feature\n    Dim uncertain As Boolean\n    gMsg = \"\"\n    gOmit = \"\"\n"
    ));
    out.push_str(&format!(
        "    ok = OpenSketchOnPlane({}, {})\n",
        index,
        vb_lit(plane_code(sk.plane))
    ));
    out.push_str(&format!(
        "    If ok = False Then gMsg = \"no origin plane carries the {} frame {}; probed: \" & gProbeLog & \"; tree scan saw: \" & TreeSummary(): GoTo SketchFailed\n",
        plane_code(sk.plane),
        frame_desc(solidcraft_frame(sk.plane))
    ));
    out.push_str(&format!(
        "    Set skFeat = swSketchMgr.ActiveSketch\n    Set skFeat{index} = LastFeature()\n    If skFeat{index} Is Nothing Then gMsg = \"the new sketch feature is missing from the tree\": GoTo SketchFailed\n    skName{index} = skFeat{index}.Name\n    Set skFeat{index} = FindFeatureByName(skName{index}, uncertain)\n    If skFeat{index} Is Nothing Then gMsg = \"the sketch feature object could not be captured for later selection: name=<\" & skName{index} & \">, tree scan saw: \" & TreeSummary(): GoTo SketchFailed\n",
    ));
    out.push_str(&format!("    DrawGeometry{index}\n"));
    out.push_str(&format!(
        "    If Not Relations{index}() Then GoTo Degrade\n    If Not Dims{index}() Then GoTo Degrade\n    If Not VerifySketch{index}() Then GoTo Degrade\n"
    ));
    out.push_str("    swSketchMgr.InsertSketch True\n");
    out.push_str(&format!(
        "    If Not NoSketchIsOpen() Then gMsg = \"the sketch context did not close\": GoTo SketchFailed\n    If Not VerifySketch{index}() Then gMsg = \"closed-context verification failed: the solved geometry differs after closing\": GoTo SketchFailed\n"
    ));
    out.push_str(&format!(
        "    gMsg = {}\n    BuildSketch{index} = \"{}\"\n    If gOmit <> \"\" Then\n        gMsg = gMsg & \" | omitted: \" & gOmit\n        BuildSketch{index} = \"degraded\"\n    End If\n    Exit Function\n\n",
        vb_lit(&initial_msg),
        initial_status
    ));
    out.push_str(
        "Degrade:\n\
             ' Restore, do not merely delete relations: wipe the sketch contents,\n\
             ' redraw at the stored coordinates, reapply driving dimensions, verify again.\n\
             ' CleanSketch revalidates the whole cycle: map invalidated first, the\n\
             ' deletion verified, the frame re-probed and the sketch identity\n\
             ' reacquired; its refusal fails the sketch with the named reason.\n",
    );
    out.push_str(&format!(
        "    If Not CleanSketch{index}() Then GoTo SketchFailed\n    DrawGeometry{index}\n    If Not Dims{index}() Then GoTo SketchFailed\n    If Not VerifySketch{index}() Then GoTo SketchFailed\n"
    ));
    out.push_str("    swSketchMgr.InsertSketch True\n");
    out.push_str(&format!(
        "    If Not NoSketchIsOpen() Then gMsg = \"the sketch context did not close\": GoTo SketchFailed\n    If Not VerifySketch{index}() Then gMsg = \"closed-context verification failed after recreation\": GoTo SketchFailed\n"
    ));
    out.push_str(&format!(
        "    gMsg = \"rebuilt as geometry plus driving dimensions\"\n    If gOmit <> \"\" Then gMsg = gMsg & \" | omitted: \" & gOmit\n    BuildSketch{index} = \"degraded\"\n    Exit Function\n\n"
    ));
    out.push_str("SketchFailed:\n    On Error Resume Next\n    CloseSketchIfOpen\n");
    out.push_str(&format!(
        "    BuildSketch{index} = \"failed\"\n    Exit Function\n\nHardFail:\n    ' Capture the error FIRST: executing an On Error statement resets Err.\n    Dim errNum As Long, errMsg As String\n    errNum = Err.Number\n    errMsg = Err.Description\n    On Error Resume Next\n    CloseSketchIfOpen\n    gMsg = \"err \" & Trim(Str(errNum)) & \": \" & errMsg\n    BuildSketch{index} = \"failed\"\nEnd Function\n\n"
    ));

    // DrawGeometry
    out.push_str(&format!("Sub DrawGeometry{index}()\n"));
    // Coordinates go through the sketch's own adaptation map (frame
    // adaptation): SC (u,v) -> (s,t) at runtime. Point handles use the
    // object-returning GetStartPoint2/GetEndPoint2/GetCenterPoint2 forms
    // the seat pinned - the legacy forms return Double() arrays and cannot
    // be selected.
    for (i, l) in sk.lines.iter().enumerate() {
        let a = sk.points[l.a];
        let b = sk.points[l.b];
        out.push_str(&format!(
            "    Set sl{index}({i}) = swSketchMgr.CreateLine(MapU({}, {}, {}), MapV({}, {}, {}), 0#, MapU({}, {}, {}), MapV({}, {}, {}), 0#)\n",
            index,
            vba_num(mm(a[0])),
            vba_num(mm(a[1])),
            index,
            vba_num(mm(a[0])),
            vba_num(mm(a[1])),
            index,
            vba_num(mm(b[0])),
            vba_num(mm(b[1])),
            index,
            vba_num(mm(b[0])),
            vba_num(mm(b[1])),
        ));
        if l.construction {
            out.push_str(&format!("    sl{index}({i}).ConstructionGeometry = True\n"));
        }
        out.push_str(&format!(
            "    Set pt{index}({}) = sl{index}({i}).GetStartPoint2\n    Set pt{index}({}) = sl{index}({i}).GetEndPoint2\n",
            2 * i,
            2 * i + 1
        ));
    }
    for (j, c) in sk.circles.iter().enumerate() {
        let ctr = sk.points[c.c];
        out.push_str(&format!(
            "    Set ci{index}({j}) = swSketchMgr.CreateCircleByRadius(MapU({}, {}, {}), MapV({}, {}, {}), 0#, {})\n",
            index,
            vba_num(mm(ctr[0])),
            vba_num(mm(ctr[1])),
            index,
            vba_num(mm(ctr[0])),
            vba_num(mm(ctr[1])),
            vba_num(mm(c.r))
        ));
        if c.construction {
            out.push_str(&format!("    ci{index}({j}).ConstructionGeometry = True\n"));
        }
        out.push_str(&format!(
            "    Set pt{index}({}) = ci{index}({j}).GetCenterPoint2\n",
            2 * sk.lines.len() + j
        ));
    }
    out.push_str("    swDoc.ClearSelection2 True\nEnd Sub\n\n");

    // Relations
    out.push_str(&format!(
        "Function Relations{index}() As Boolean\n    On Error GoTo RelFail\n    Const TOL As Double = 0.000001\n    Dim vA As Variant\n    Dim vB As Variant\n    Dim vC As Variant\n    Dim vD As Variant\n    Dim chk As Double\n"
    ));
    for rel in &sk.relations {
        out.push_str(&emit_relation(index, rel));
    }
    out.push_str(&format!(
        "    Relations{index} = True\n    Exit Function\nRelFail:\n    If Err.Number <> 0 Then gMsg = \"err \" & Trim(Str(Err.Number)) & \": \" & Err.Description\n    If gMsg = \"\" Then gMsg = \"a sketch relation could not be applied\"\n    Relations{index} = False\nEnd Function\n\n"
    ));

    // Dimensions
    out.push_str(&format!(
        "Function Dims{index}() As Boolean\n    On Error GoTo DimFail\n    Const TOL As Double = 0.000001\n    gMsg = \"\"\n    Dim dd As SldWorks.DisplayDimension\n    Dim dm As SldWorks.Dimension\n    Dim ax As Double\n    Dim ay As Double\n    Dim bx As Double\n    Dim by As Double\n    Dim ds As Double\n    Dim dt As Double\n    Dim proj As Double\n    Dim other As Double\n    Dim created As Double\n    Dim signVal As Double\n    Dim sT As Double\n    Dim tT As Double\n    Dim mx As Double\n    Dim my As Double\n    Dim mz As Double\n    Dim axClass As Long\n    Dim dimScale As Double\n    Dim v As Variant\n    Dim v2 As Variant\n    Dim nax As Double\n    Dim nay As Double\n    Dim nbx As Double\n    Dim nby As Double\n    Dim chk As Double\n    Dim chk2 As Double\n"
    ));
    for (di, dim) in sk.dimensions.iter().enumerate() {
        out.push_str(&emit_dimension(index, dim, sk, equation_for(sk, di)));
    }
    out.push_str(&format!(
        "    Dims{index} = True\n    Exit Function\nDimFail:\n    If Err.Number <> 0 Then gMsg = \"err \" & Trim(Str(Err.Number)) & \": \" & Err.Description\n    If gMsg = \"\" Then gMsg = \"a driving dimension could not be added\"\n    Dims{index} = False\nEnd Function\n\n"
    ));

    // Verify
    out.push_str(&format!(
        "Function VerifySketch{index}() As Boolean\n    Const TOL As Double = 0.000001\n    Dim v As Variant\n    Dim cph As Object\n    On Error GoTo VerifyFail\n"
    ));
    // Fresh three-component readback in sketch space (the seat pinned the
    // legacy forms as Double() arrays of (u,v,0) here): all three
    // components are checked - z must vanish - against the mapped values.
    for (i, l) in sk.lines.iter().enumerate() {
        for (getter, p) in [
            ("GetStartPoint", sk.points[l.a]),
            ("GetEndPoint", sk.points[l.b]),
        ] {
            out.push_str(&format!(
                "    v = sl{index}({i}).{getter}\n    If Abs(v(0) - MapU({index}, {u}, {vv})) > TOL Then GoTo VerifyFail\n    If Abs(v(1) - MapV({index}, {u}, {vv})) > TOL Then GoTo VerifyFail\n    If Abs(v(2)) > TOL Then GoTo VerifyFail\n",
                getter = getter,
                index = index,
                u = vba_num(mm(p[0])),
                vv = vba_num(mm(p[1])),
            ));
        }
    }
    for (j, c) in sk.circles.iter().enumerate() {
        let ctr = sk.points[c.c];
        // The centre comes from the pinned GetCenterPoint2 handle (an
        // endpoint accessor does not establish centre coordinates): a fresh
        // handle, all three components checked.
        out.push_str(&format!(
            "    Set cph = ci{index}({j}).GetCenterPoint2\n    If cph Is Nothing Then GoTo VerifyFail\n    If Abs(cph.X - MapU({index}, {u}, {vv})) > TOL Then GoTo VerifyFail\n    If Abs(cph.Y - MapV({index}, {u}, {vv})) > TOL Then GoTo VerifyFail\n    If Abs(cph.Z) > TOL Then GoTo VerifyFail\n",
            index = index,
            u = vba_num(mm(ctr[0])),
            vv = vba_num(mm(ctr[1])),
        ));
        out.push_str(&format!(
            "    If Abs(ci{index}({j}).GetRadius - {}) > TOL Then GoTo VerifyFail\n",
            vba_num(mm(c.r))
        ));
    }
    out.push_str(&format!(
        "    VerifySketch{index} = True\n    Exit Function\nVerifyFail:\n    VerifySketch{index} = False\nEnd Function\n\n"
    ));

    // CleanSketch: verified deletion-and-recreation (folded design 35f04a3f
    // item 4). The map is invalidated first, every created entity joins one
    // deletion whose result is verified through the pinned point
    // enumeration, the frame is re-probed and republished before any
    // redraw, and the sketch identity is reacquired. The dimension slots
    // this sketch registered are cleared so the redraw rebinds them under
    // fresh runtime names.
    out.push_str(&format!(
        "Function CleanSketch{index}() As Boolean\n    On Error GoTo CleanFail\n    Dim anySel As Boolean\n    Dim okSel As Boolean\n    Dim cleanOk As Boolean\n    Dim pts As Variant\n    Dim ub As Long\n    Dim skFeat As SldWorks.Feature\n    Dim uncertain As Boolean\n    CleanSketch{index} = False\n    gCause = gMsg\n    gMsg = \"\"\n    SolveSketch skName{index}, skFeat{index}\n    ClearMap {index}\n"
    ));
    for (_, eq) in &sk.dim_equations {
        out.push_str(&format!(
            "    regName({}) = \"\"\n    regSign({}) = 1#
    regScale({}) = 1#\n",
            eq.slot, eq.slot, eq.slot
        ));
    }
    for i in 0..sk.lines.len() {
        out.push_str(&format!(
            "    If Not sl{index}({i}) Is Nothing Then\n    okSel = sl{index}({i}).Select(anySel)\n    If Not okSel Then gMsg = \"a sketch entity would not select for deletion\": GoTo CleanFail\n    anySel = True\n    End If\n"
        ));
    }
    for j in 0..sk.circles.len() {
        out.push_str(&format!(
            "    If Not ci{index}({j}) Is Nothing Then\n    okSel = ci{index}({j}).Select(anySel)\n    If Not okSel Then gMsg = \"a sketch entity would not select for deletion\": GoTo CleanFail\n    anySel = True\n    End If\n"
        ));
    }
    out.push_str("    If anySel Then swDoc.EditDelete\n    swDoc.ClearSelection2 True\n");
    out.push_str(
        "    ' Verify the deletion through the pinned point enumeration (every\n    ' line endpoint and circle centre is a sketch point): only a PROVEN-\n    ' EMPTY collection counts. A fetch, type or bounds failure refuses -\n    ' a failed query is never proof of emptiness.\n    On Error Resume Next\n    Err.Clear\n    pts = swSketchMgr.ActiveSketch.GetSketchPoints2\n    If Err.Number <> 0 Then\n    On Error GoTo CleanFail\n    gMsg = \"the point collection could not be read after deletion: err \" & Trim(Str(Err.Number))\n    GoTo CleanFail\n    End If\n    If IsArray(pts) Then\n    Err.Clear\n    ub = UBound(pts)\n    If Err.Number = 0 Then\n    On Error GoTo CleanFail\n    gMsg = \"sketch contents deletion is not verified: \" & Trim(Str(ub + 1)) & \" points remain\"\n    GoTo CleanFail\n    End If\n    If Err.Number <> 9 Then\n    On Error GoTo CleanFail\n    gMsg = \"the point collection bounds could not be established: err \" & Trim(Str(Err.Number))\n    GoTo CleanFail\n    End If\n    Else\n    ' The seat-proven empty signature (2026-11-16, revolve-axes run): a\n    ' successful fetch on a deleted sketch returns Empty, not an empty\n    ' array. Any other non-array value refuses.\n    If Not IsEmpty(pts) Then\n    On Error GoTo CleanFail\n    gMsg = \"the point collection came back as an unexpected non-array value\"\n    GoTo CleanFail\n    End If\n    End If\n    On Error GoTo CleanFail\n",
    );
    out.push_str(&format!(
        "    If Not AcceptAndPublish({index}, {}, gProbeReason) Then gMsg = \"the sketch frame could not be re-accepted after recreation: \" & gProbeReason: GoTo CleanFail\n",
        vb_lit(plane_code(sk.plane))
    ));
    out.push_str(&format!(
        "    Set skFeat{index} = LastFeature()\n    If skFeat{index} Is Nothing Then gMsg = \"the recreated sketch is missing from the tree\": GoTo CleanFail\n",
    ));
    out.push_str(&format!(
        "    skName{index} = skFeat{index}.Name\n    Set skFeat{index} = FindFeatureByName(skName{index}, uncertain)\n    If skFeat{index} Is Nothing Then gMsg = \"the recreated sketch feature object could not be captured for selection: name=<\" & skName{index} & \">, tree scan saw: \" & TreeSummary(): GoTo CleanFail\n    CleanSketch{index} = True\n    Exit Function\nCleanFail:\n    If Err.Number <> 0 Then gMsg = \"err \" & Trim(Str(Err.Number)) & \": \" & Err.Description\n    gMsg = gCause & \" | recreate refused: \" & gMsg\n    CleanSketch{index} = False\nEnd Function\n\n"
    ));
    out
}

fn emit_relation(index: usize, rel: &RelationT) -> String {
    let select_point = |slot: usize, append: bool| {
        format!(
            "    If Not pt{index}({slot}).Select({}) Then gMsg = \"the point slot {slot} would not select\": GoTo RelFail\n",
            if append { "True" } else { "False" }
        )
    };
    let select_line = |li: usize, append: bool| {
        format!(
            "    If Not sl{index}({li}).Select({}) Then gMsg = \"the line {li} would not select\": GoTo RelFail\n",
            if append { "True" } else { "False" }
        )
    };
    // SketchAddConstraints has no return value (API doc: IModelDoc2.
    // SketchAddConstraints), so application is established behaviourally:
    // the constraint's own predicate must hold on the solved geometry.
    let apply = |c: &str| {
        format!("    swDoc.SketchAddConstraints \"{c}\"\n    swDoc.ClearSelection2 True\n    SolveSketch skName{index}, skFeat{index}\n")
    };
    // Level checks per classified axis: 0 = the pair shares t (Y), 1 = s (X).
    let level_pts = |a: usize, b: usize, axis0: bool, label: &str| {
        let c = if axis0 { "Y" } else { "X" };
        format!(
            "    If Abs(pt{index}({a}).{c} - pt{index}({b}).{c}) > TOL Then gMsg = \"the {label} relation did not level the classified axis\": GoTo RelFail\n"
        )
    };
    let level_line = |l: usize, axis0: bool, label: &str| {
        let ix = if axis0 { 1 } else { 0 };
        format!(
            "    vA = sl{index}({l}).GetStartPoint\n    vB = sl{index}({l}).GetEndPoint\n    If Abs(vA({ix}) - vB({ix})) > TOL Then gMsg = \"the {label} relation did not level the classified axis\": GoTo RelFail\n"
        )
    };
    // Axis-dependent relations classify at runtime (folded design 35f04a3f):
    // the SC direction lands on the sketch s or t axis for a signed
    // permutation and the constraint kind follows it there. A non-permutation
    // map OMITS the relation with a named reason - terminating by
    // construction, never re-attempted as a failure.
    let classified = |is_horiz: &str,
                      sel: &str,
                      label: &str,
                      dir: &str,
                      check0: &str,
                      check1: &str,
                      name0: &str,
                      name1: &str| {
        format!(
            "    Select Case AxisClass({index}, {is_horiz})\n    Case 0\n{sel}    swDoc.SketchAddConstraints \"{name0}\"\n    swDoc.ClearSelection2 True\n    SolveSketch skName{index}, skFeat{index}\n{check0}    Case 1\n{sel}    swDoc.SketchAddConstraints \"{name1}\"\n    swDoc.ClearSelection2 True\n    SolveSketch skName{index}, skFeat{index}\n{check1}    Case Else\n    gOmit = gOmit & \"relation {label}: the SC {dir} axis is not axis-aligned in this sketch frame (non-permutation map); \"\n    End Select\n"
        )
    };
    match *rel {
        RelationT::PointsCoincident(a, b) => {
            let label = format!("points-coincident {a}-{b}");
            select_point(a, false)
                + &select_point(b, true)
                + &apply("sgCoincident")
                + &format!(
                    "    If Abs(pt{index}({a}).X - pt{index}({b}).X) > TOL Or Abs(pt{index}({a}).Y - pt{index}({b}).Y) > TOL Then gMsg = \"the {label} relation did not coincide the points\": GoTo RelFail\n"
                )
        }
        RelationT::PointsHorizontal(a, b) => {
            let label = format!("points-horizontal {a}-{b}");
            let sel = select_point(a, false) + &select_point(b, true);
            classified(
                "True",
                &sel,
                &label,
                "u",
                &level_pts(a, b, true, &label),
                &level_pts(a, b, false, &label),
                "sgHORIZONTALPOINTS2D",
                "sgVERTPOINTS2D",
            )
        }
        RelationT::PointsVertical(a, b) => {
            let label = format!("points-vertical {a}-{b}");
            let sel = select_point(a, false) + &select_point(b, true);
            classified(
                "False",
                &sel,
                &label,
                "v",
                &level_pts(a, b, true, &label),
                &level_pts(a, b, false, &label),
                "sgHORIZONTALPOINTS2D",
                "sgVERTPOINTS2D",
            )
        }
        RelationT::LineHorizontal(l) => {
            let label = format!("line-horizontal {l}");
            let sel = select_line(l, false);
            classified(
                "True",
                &sel,
                &label,
                "u",
                &level_line(l, true, &label),
                &level_line(l, false, &label),
                "sgHORIZONTAL2D",
                "sgVERTICAL2D",
            )
        }
        RelationT::LineVertical(l) => {
            let label = format!("line-vertical {l}");
            let sel = select_line(l, false);
            classified(
                "False",
                &sel,
                &label,
                "v",
                &level_line(l, true, &label),
                &level_line(l, false, &label),
                "sgHORIZONTAL2D",
                "sgVERTICAL2D",
            )
        }
        RelationT::LinesParallel(a, b) => {
            let label = format!("lines-parallel {a}-{b}");
            select_line(a, false)
                + &select_line(b, true)
                + &apply("sgParallel")
                + &format!(
                    "    vA = sl{index}({a}).GetStartPoint\n    vB = sl{index}({a}).GetEndPoint\n    vC = sl{index}({b}).GetStartPoint\n    vD = sl{index}({b}).GetEndPoint\n    chk = (vB(0) - vA(0)) * (vD(1) - vC(1)) - (vB(1) - vA(1)) * (vD(0) - vC(0))\n    If Abs(chk) > TOL Then gMsg = \"the {label} relation did not parallel the lines\": GoTo RelFail\n"
                )
        }
        RelationT::LinesPerpendicular(a, b) => {
            let label = format!("lines-perpendicular {a}-{b}");
            select_line(a, false)
                + &select_line(b, true)
                + &apply("sgPerpendicular")
                + &format!(
                    "    vA = sl{index}({a}).GetStartPoint\n    vB = sl{index}({a}).GetEndPoint\n    vC = sl{index}({b}).GetStartPoint\n    vD = sl{index}({b}).GetEndPoint\n    chk = (vB(0) - vA(0)) * (vD(0) - vC(0)) + (vB(1) - vA(1)) * (vD(1) - vC(1))\n    If Abs(chk) > TOL Then gMsg = \"the {label} relation did not square the lines\": GoTo RelFail\n"
                )
        }
        RelationT::LinesTangent(a, b) => {
            // Tangency has no cheap solved-geometry predicate over two lines;
            // the selection checks and the coordinate verification are the
            // evidence here (documented limitation, docs/mapping.md).
            select_line(a, false) + &select_line(b, true) + &apply("sgTangent")
        }
        RelationT::LinesEqual(a, b) => {
            let label = format!("lines-equal {a}-{b}");
            select_line(a, false)
                + &select_line(b, true)
                + &apply("sgEqual")
                + &format!(
                    "    vA = sl{index}({a}).GetStartPoint\n    vB = sl{index}({a}).GetEndPoint\n    vC = sl{index}({b}).GetStartPoint\n    vD = sl{index}({b}).GetEndPoint\n    chk = Sqr((vB(0) - vA(0)) ^ 2 + (vB(1) - vA(1)) ^ 2) - Sqr((vD(0) - vC(0)) ^ 2 + (vD(1) - vC(1)) ^ 2)\n    If Abs(chk) > TOL Then gMsg = \"the {label} relation did not equalize the lines\": GoTo RelFail\n"
                )
        }
    }
}

fn equation_for(sk: &SketchStep, di: usize) -> Option<&crate::replay::DimEquation> {
    sk.dim_equations
        .iter()
        .find(|(i, _)| *i == di)
        .map(|(_, e)| e)
}

fn emit_dimension(
    index: usize,
    dim: &DimensionT,
    sk: &SketchStep,
    equation: Option<&crate::replay::DimEquation>,
) -> String {
    let mut out = String::new();
    let si = match dim.kind {
        DimKind::Angle => dim.value, // already radians
        _ => mm(dim.value),
    };
    let label = match dim.kind {
        DimKind::Length => format!("length line {}", dim.a),
        DimKind::Distance => format!("distance {}-{}", dim.a, dim.b.unwrap_or(0)),
        DimKind::DistanceX => format!("DistanceX {}-{}", dim.a, dim.b.unwrap_or(0)),
        DimKind::DistanceY => format!("DistanceY {}-{}", dim.a, dim.b.unwrap_or(0)),
        DimKind::Radius => format!("radius circle {}", dim.a),
        DimKind::Diameter => format!("diameter circle {}", dim.a),
        DimKind::Angle => format!("angle lines {}-{}", dim.a, dim.b.unwrap_or(0)),
    };
    out.push_str(&format!("    ' dimension: {label}\n"));
    out.push_str(if matches!(dim.kind, DimKind::Radius) {
        "    dimScale = 2#\n"
    } else {
        "    dimScale = 1#\n"
    });
    // Referent coordinates come from the live handles at runtime, so the
    // placement and the kind check follow the actual sketch, not the source
    // coordinates. The text position is MODEL space (Phase 0): SketchToModel
    // converts the sketch-space heuristic position through the stored frame.
    let mut coords = String::new();
    let select;
    let placement;
    let mut kindcheck = String::new();
    match dim.kind {
        DimKind::Length => {
            let (a, b) = (2 * dim.a, 2 * dim.a + 1);
            coords = format!(
                "    ax = pt{index}({a}).X\n    ay = pt{index}({a}).Y\n    bx = pt{index}({b}).X\n    by = pt{index}({b}).Y\n    ds = bx - ax\n    dt = by - ay\n"
            );
            select = format!(
                "    If Not sl{index}({}).Select(False) Then gMsg = \"the line {} would not select\": GoTo DimFail\n",
                dim.a, dim.a
            );
            placement =
                "    sT = (ax + bx) / 2# + 0.01\n    tT = (ay + by) / 2# + 0.01\n".to_string();
            kindcheck = format!(
                "    proj = Sqr(ds * ds + dt * dt)\n    If Abs(Abs(created) - proj) > TOL Then\n    gMsg = \"dimension {label} measures \" & Trim(Str(created)) & \" but the line length is \" & Trim(Str(proj))\n    GoTo DimFail\n    End If\n"
            );
        }
        DimKind::Radius | DimKind::Diameter => {
            let cslot = 2 * sk.lines.len() + dim.a;
            coords = format!("    ax = pt{index}({cslot}).X\n    ay = pt{index}({cslot}).Y\n");
            select = format!(
                "    If Not ci{index}({}).Select(False) Then gMsg = \"the circle {} would not select\": GoTo DimFail\n",
                dim.a, dim.a
            );
            placement = format!(
                "    sT = ax + 0.75 * ci{index}({}).GetRadius\n    tT = ay + 0.75 * ci{index}({}).GetRadius\n",
                dim.a, dim.a
            );
            let (what, expr) = if matches!(dim.kind, DimKind::Radius) {
                (
                    "twice the circle radius (the API circle dimension is diameter-valued)",
                    format!("2# * ci{index}({}).GetRadius", dim.a),
                )
            } else {
                (
                    "twice the circle radius",
                    format!("2# * ci{index}({}).GetRadius", dim.a),
                )
            };
            kindcheck = format!(
                "    proj = {expr}\n    If Abs(Abs(created) - proj) > TOL Then\n    gMsg = \"dimension {label} measures \" & Trim(Str(created)) & \" but {what} is \" & Trim(Str(proj))\n    GoTo DimFail\n    End If\n"
            );
        }
        DimKind::Angle => {
            select = format!(
                "    If Not sl{index}({}).Select(False) Then gMsg = \"the line {} would not select\": GoTo DimFail\n    If Not sl{index}({}).Select(True) Then gMsg = \"the line {} would not select\": GoTo DimFail\n",
                dim.a,
                dim.a,
                dim.b.unwrap_or(0),
                dim.b.unwrap_or(0)
            );
            placement = "    sT = 0.01\n    tT = 0.01\n".to_string();
            // No projection check applies to an angle; the value match below
            // is the observable contract.
        }
        _ => {
            // Distance, DistanceX, DistanceY: two point slots.
            let (a, b) = (dim.a, dim.b.unwrap_or(0));
            coords = format!(
                "    ax = pt{index}({a}).X\n    ay = pt{index}({a}).Y\n    bx = pt{index}({b}).X\n    by = pt{index}({b}).Y\n    ds = bx - ax\n    dt = by - ay\n"
            );
            select = format!(
                "    If Not pt{index}({a}).Select(False) Then gMsg = \"the point slot {a} would not select\": GoTo DimFail\n    If Not pt{index}({b}).Select(True) Then gMsg = \"the point slot {b} would not select\": GoTo DimFail\n"
            );
            match dim.kind {
                DimKind::DistanceX | DimKind::DistanceY => {
                    // The placed side decides the dimension kind: offset the
                    // text perpendicular to the classified axis so the kind
                    // measures that axis. Sign handling is below, in the
                    // equation RHS and the expected value.
                    placement = "    If axClass = 0 Then\n    proj = ds\n    other = dt\n    sT = (ax + bx) / 2#\n    tT = ay - 0.01\n    Else\n    proj = dt\n    other = ds\n    sT = ax + 0.01\n    tT = (ay + by) / 2#\n    End If\n".to_string();
                }
                _ => {
                    placement = "    sT = (ax + bx) / 2# + 0.01\n    tT = (ay + by) / 2# + 0.01\n"
                        .to_string();
                }
            }
            kindcheck = format!(
                "    proj = Sqr(ds * ds + dt * dt)\n    If Abs(Abs(created) - proj) > TOL Then\n    gMsg = \"dimension {label} measures \" & Trim(Str(created)) & \" but the point distance is \" & Trim(Str(proj))\n    GoTo DimFail\n    End If\n"
            );
            if matches!(dim.kind, DimKind::DistanceX | DimKind::DistanceY) {
                // Replace the aligned check with the classified-axis check:
                // |SystemValue| must match the classified-axis projection
                // (|ds| horizontal, |dt| vertical), and must NOT be the other
                // axis - that would mean the placed kind came out wrong.
                kindcheck = format!(
                    "    If Abs(Abs(created) - Abs(proj)) > TOL Then\n    gMsg = \"dimension {label} measures \" & Trim(Str(created)) & \" but the classified-axis displacement is \" & Trim(Str(proj))\n    GoTo DimFail\n    End If\n    If Abs(other) > TOL Then\n    If Abs(Abs(proj) - Abs(other)) <= TOL Then\n    gMsg = \"dimension {label}: the projection magnitudes coincide; the kind cannot be established\"\n    GoTo DimFail\n    End If\n    If Abs(Abs(created) - Abs(other)) <= TOL Then\n    gMsg = \"dimension {label} landed on the wrong axis kind (measures \" & Trim(Str(created)) & \")\"\n    GoTo DimFail\n    End If\n    End If\n"
                );
            }
        }
    }
    // The create/check/equation tail is common. The sign is resolved from
    // what the dimension actually measures against the source value (+-si):
    // it is written into the equation RHS and the expected value so later
    // parameter edits keep tracking (never only the initial SystemValue).
    let mut tail = String::new();
    tail.push_str(&format!(
        "    mx = SketchToModelX({index}, sT, tT)\n    my = SketchToModelY({index}, sT, tT)\n    mz = SketchToModelZ({index}, sT, tT)\n    Set dd = swDoc.AddDimension2(mx, my, mz)\n    If dd Is Nothing Then gMsg = \"the {label} dimension did not appear\": GoTo DimFail\n    Set dm = dd.GetDimension2(0)\n    If dm Is Nothing Then gMsg = \"the {label} dimension has no core dimension\": GoTo DimFail\n    created = dm.SystemValue\n"
    ));
    tail.push_str(&kindcheck);
    tail.push_str(&format!(
        "    If Abs(created - dimScale * {}) <= TOL Then\n    signVal = 1#\n    ElseIf Abs(created + dimScale * {}) <= TOL Then\n    signVal = -1#\n    Else\n    gMsg = \"dimension {label} measures \" & Trim(Str(created)) & \" but the source value is {}\"\n    GoTo DimFail\n    End If\n",
        vba_num(si),
        vba_num(si),
        vba_num(si)
    ));
    tail.push_str(&format!(
        "    dm.SystemValue = signVal * dimScale * {}\n",
        vba_num(si)
    ));
    // Behavioural kind evidence: setting the value must move exactly the
    // measured quantity. A wrong-kind dimension provably moves the wrong
    // axis (or the wrong measure) and fails here with both numbers named.
    let behaviour = match dim.kind {
        DimKind::Length => {
            let l = dim.a;
            format!(
                "    v = sl{index}({l}).GetStartPoint\n    v2 = sl{index}({l}).GetEndPoint\n    chk = Sqr((v2(0) - v(0)) * (v2(0) - v(0)) + (v2(1) - v(1)) * (v2(1) - v(1)))\n    If Abs(chk - Abs({si})) > TOL Then gMsg = \"the value edit did not carry the {label} geometry (length \" & Trim(Str(chk)) & \" after driving \" & Trim(Str(Abs({si}))) & \")\": GoTo DimFail\n",
                si = vba_num(si)
            )
        }
        DimKind::Radius | DimKind::Diameter => {
            let j = dim.a;
            let measure = if matches!(dim.kind, DimKind::Radius) {
                format!("ci{index}({j}).GetRadius")
            } else {
                format!("2# * ci{index}({j}).GetRadius")
            };
            format!(
                "    chk = {measure}\n    If Abs(chk - Abs({si})) > TOL Then gMsg = \"the value edit did not carry the {label} geometry (measure \" & Trim(Str(chk)) & \" after driving \" & Trim(Str(Abs({si}))) & \")\": GoTo DimFail\n",
                si = vba_num(si)
            )
        }
        DimKind::Angle => format!(
            "    If Abs(dm.SystemValue - signVal * dimScale * {si}) > TOL Then gMsg = \"the {label} value edit did not stick\": GoTo DimFail\n",
            si = vba_num(si)
        ),
        DimKind::DistanceX | DimKind::DistanceY => {
            let (a, b) = (dim.a, dim.b.unwrap_or(0));
            format!(
                "    nax = pt{index}({a}).X\n    nay = pt{index}({a}).Y\n    nbx = pt{index}({b}).X\n    nby = pt{index}({b}).Y\n    If axClass = 0 Then\n    chk = nbx - nax\n    chk2 = nby - nay\n    Else\n    chk = nby - nay\n    chk2 = nbx - nax\n    End If\n    If Abs(Abs(chk) - Abs({si})) > TOL Then gMsg = \"the value edit did not carry the {label} geometry (classified-axis \" & Trim(Str(chk)) & \" after driving \" & Trim(Str(Abs({si}))) & \")\": GoTo DimFail\n    If Abs(Abs(chk2) - Abs(other)) > TOL Then\n    gMsg = \"the value edit moved the perpendicular axis - the {label} kind is wrong\"\n    GoTo DimFail\n    End If\n",
                si = vba_num(si)
            )
        }
        _ => {
            let (a, b) = (dim.a, dim.b.unwrap_or(0));
            format!(
                "    nax = pt{index}({a}).X\n    nay = pt{index}({a}).Y\n    nbx = pt{index}({b}).X\n    nby = pt{index}({b}).Y\n    chk = Sqr((nbx - nax) * (nbx - nax) + (nby - nay) * (nby - nay))\n    If Abs(chk - Abs({si})) > TOL Then gMsg = \"the value edit did not carry the {label} geometry (distance \" & Trim(Str(chk)) & \" after driving \" & Trim(Str(Abs({si}))) & \")\": GoTo DimFail\n",
                si = vba_num(si)
            )
        }
    };
    tail.push_str(&behaviour);
    if let Some(equation) = equation {
        tail.push_str(&slot_guards(equation, "DimFail"));
        // The API circle dimension is diameter-valued: a radius source is
        // expressed as twice its value in the equation text, so parameter
        // edits keep meaning the radius (slot references divide it back).
        let rhs_expr = if matches!(dim.kind, DimKind::Radius) {
            format!("\"2*(\" & {} & \")\"", rhs_vba(equation))
        } else {
            rhs_vba(equation)
        };
        tail.push_str(&format!(
            "    If Not AddDimEquation(dm, {}, signVal * {} * dimScale, {}, signVal, skName{index}, dimScale) Then GoTo DimFail\n    SolveSketch skName{index}, skFeat{index}\n",
            rhs_expr,
            vba_num(equation.expect_si),
            equation.slot
        ));
    }
    tail.push_str("    swDoc.ClearSelection2 True\n");

    out.push_str(&coords);
    match dim.kind {
        DimKind::DistanceX | DimKind::DistanceY => {
            let (is_horiz, dir_name) = if matches!(dim.kind, DimKind::DistanceX) {
                ("True", "u")
            } else {
                ("False", "v")
            };
            out.push_str(&format!("    axClass = AxisClass({index}, {is_horiz})\n"));
            out.push_str(&format!(
                "    If axClass < 0 Then\n    gOmit = gOmit & \"dimension {label}: the SC {dir_name} axis is not axis-aligned in this sketch frame (non-permutation map); \"\n    Else\n"
            ));
            out.push_str(&select);
            out.push_str(&placement);
            out.push_str(&tail);
            out.push_str("    End If\n");
        }
        _ => {
            out.push_str(&select);
            out.push_str(&placement);
            out.push_str(&tail);
        }
    }
    out
}

/// The sign contract on a feature dimension equation: the dimension carries
/// |expect| and the equation right-hand side keeps source semantics; the
/// sign travels through AddDimEquation's signVal and the registered regSign,
/// so downstream SlotRef references normalize back to source semantics.
fn sign_lit(expect_si: f64) -> &'static str {
    if expect_si < 0.0 {
        "-1#"
    } else {
        "1#"
    }
}

fn emit_extrude_subs(index: usize, ex: &ExtrudeStep) -> String {
    use crate::replay::ExtrudeKind;
    let sk = ex.sketch_step;
    let mut out = String::new();
    out.push_str(&format!("' Extrude step {index}: {}\n", vb_lit(&ex.name)));
    let mut locals = String::from(
        "    Dim ok As Boolean\n    Dim feat As SldWorks.Feature\n    Dim dm As SldWorks.Dimension\n    Dim objState As String\n",
    );
    let mut body = String::new();
    // The map guard precedes every direction decision: the side decisions
    // run through the sketch's map side (any valid adaptation map - the
    // side is the map's orientation, not a permutation property).
    body.push_str(&format!(
        "    If Not gMapValid({sk}) Then gMsg = \"frame adaptation: the profile sketch carries no valid adaptation map; refusing to report created\": GoTo ExFail\n"
    ));
    let select_sketch = format!(
        "    ok = False\n    objState = \"missing\"\n    If Not skFeat{sk} Is Nothing Then\n    objState = \"name=<\" & skFeat{sk}.Name & \">\"\n    ok = skFeat{sk}.Select(False)\n    objState = objState & \" select-result=\" & CStr(ok)\n    End If\n    If ok = False Then ok = swDoc.Extension.SelectByID2(skName{sk}, \"SKETCH\", 0#, 0#, 0#, False, 0, Nothing, swSelectOption_e.swSelectOptionDefault)\n    If ok = False Then gMsg = \"could not select the sketch for the extrude: \" & objState & \" name=<\" & skName{sk} & \">; tree scan saw: \" & TreeSummary(): GoTo ExFail\n"
    );
    let no_depth_note =
        "    ' through-all: the end condition is the geometry; depth readback does not apply\n";
    let message = match &ex.kind {
        ExtrudeKind::Blind { depth_m, negative } => {
            locals.push_str("    Dim wantSide As Long\n    Dim dirFlag As Boolean\n");
            body.push_str("    If TT_EX_DIR_TRUE_SIDE = 0 Then gMsg = \"the extrude direction truth table is unmeasured: probe-extrudedir has not established which physical side the Dir flag sends material to; refusing to report created\": GoTo ExFail\n");
            body.push_str(&format!(
                "    wantSide = {} * gMapSide({sk})\n    dirFlag = (wantSide = TT_EX_DIR_TRUE_SIDE)\n",
                if *negative { "-1" } else { "1" }
            ));
            body.push_str(&select_sketch);
            body.push_str(&format!(
                "    Set feat = gFeatMgr.FeatureExtrusion2(True, False, dirFlag, swEndConditions_e.swEndCondBlind, swEndConditions_e.swEndCondBlind, {}, 0#, False, False, False, False, 0#, 0#, False, False, False, False, True, True, True, 0, 0#, False)\n    If feat Is Nothing Then gMsg = \"the extrude was not created\": GoTo ExFail\n",
                vba_num(*depth_m)
            ));
            body.push_str(&format!(
                "    Set dm = swDoc.Parameter(\"D1@\" & feat.Name)\n    If dm Is Nothing Then gMsg = \"the extrude dimension D1 is missing\": GoTo ExFail\n    If Abs(dm.SystemValue - {}) > 0.000001 Then gMsg = \"the extrude dimension D1 does not match\": GoTo ExFail\n",
                vba_num(*depth_m)
            ));
            for eq in &ex.equations {
                body.push_str(&slot_guards(eq, "ExFail"));
                body.push_str(&format!(
                    "    Set dm = swDoc.Parameter(\"D1@\" & feat.Name)\n    If dm Is Nothing Then gMsg = \"the extrude dimension D1 is missing\": GoTo ExFail\n    If Not AddDimEquation(dm, {}, {}, {}, {}, feat.Name, 1#) Then gMsg = \"the extrude depth equation did not evaluate\": GoTo ExFail\n",
                    rhs_vba(eq),
                    vba_num(eq.expect_si.abs()),
                    eq.slot,
                    sign_lit(eq.expect_si)
                ));
            }
            format!("depth {} m", vba_num(*depth_m))
        }
        ExtrudeKind::Symmetric { total_m } => {
            // Direction-free: a mid-plane extrude spans both sides, so no
            // truth-table cell is consulted and any valid map works. The
            // depth is the TOTAL (the documented mid-plane call shape,
            // checkpoint-pending) and is verified by the D1 readback.
            body.push_str(&select_sketch);
            body.push_str(&format!(
                "    Set feat = gFeatMgr.FeatureExtrusion2(True, False, False, swEndConditions_e.swEndCondMidPlane, swEndConditions_e.swEndCondBlind, {}, 0#, False, False, False, False, 0#, 0#, False, False, False, False, True, True, True, 0, 0#, False)\n    If feat Is Nothing Then gMsg = \"the extrude was not created\": GoTo ExFail\n",
                vba_num(*total_m)
            ));
            body.push_str(&format!(
                "    Set dm = swDoc.Parameter(\"D1@\" & feat.Name)\n    If dm Is Nothing Then gMsg = \"the extrude dimension D1 is missing\": GoTo ExFail\n    If Abs(dm.SystemValue - {}) > 0.000001 Then gMsg = \"the extrude dimension D1 does not match\": GoTo ExFail\n",
                vba_num(*total_m)
            ));
            for eq in &ex.equations {
                body.push_str(&slot_guards(eq, "ExFail"));
                body.push_str(&format!(
                    "    Set dm = swDoc.Parameter(\"D1@\" & feat.Name)\n    If dm Is Nothing Then gMsg = \"the extrude dimension D1 is missing\": GoTo ExFail\n    If Not AddDimEquation(dm, {}, {}, {}, {}, feat.Name, 1#) Then gMsg = \"the extrude depth equation did not evaluate\": GoTo ExFail\n",
                    rhs_vba(eq),
                    vba_num(eq.expect_si.abs()),
                    eq.slot,
                    sign_lit(eq.expect_si)
                ));
            }
            format!("symmetric, total {} m", vba_num(*total_m))
        }
        ExtrudeKind::TwoSided {
            first_m,
            second_m,
            first_negative,
            second_negative,
        } => {
            let same_side = first_negative == second_negative && *first_m > 0.0 && *second_m > 0.0;
            if same_side {
                // The body spans between the two ends (#2700); ends on the
                // same side of the sketch plane have no supported call
                // shape - refused before anything is created.
                body.push_str("    gMsg = \"a two-sided extrude whose ends lie on the same side of the sketch plane is not representable as a SolidWorks two-sided extrude in this phase; refusing to report created\": GoTo ExFail\n");
            } else {
                locals.push_str(
                    "    Dim wantFirst As Long\n    Dim swap2 As Boolean\n    Dim dA As Double\n    Dim dB As Double\n    Dim wFirst As Long\n    Dim wSecond As Long\n",
                );
                body.push_str("    If TT_EX_FIRST_SIDE = 0 Then gMsg = \"the two-sided extrude direction truth table is unmeasured: probe-twosided has not established which physical side receives the first depth; refusing to report created\": GoTo ExFail\n");
                body.push_str(&format!(
                    "    wantFirst = {} * gMapSide({sk})\n    swap2 = (wantFirst <> TT_EX_FIRST_SIDE)\n    If swap2 Then\n    dA = {}\n    dB = {}\n    wFirst = 2\n    wSecond = 1\n    Else\n    dA = {}\n    dB = {}\n    wFirst = 1\n    wSecond = 2\n    End If\n",
                    if *first_negative { "-1" } else { "1" },
                    vba_num(*second_m),
                    vba_num(*first_m),
                    vba_num(*first_m),
                    vba_num(*second_m),
                ));
                body.push_str(&select_sketch);
                body.push_str("    Set feat = gFeatMgr.FeatureExtrusion2(False, False, False, swEndConditions_e.swEndCondBlind, swEndConditions_e.swEndCondBlind, dA, dB, False, False, False, False, 0#, 0#, False, False, False, False, True, True, True, 0, 0#, False)\n    If feat Is Nothing Then gMsg = \"the extrude was not created\": GoTo ExFail\n");
                body.push_str("    Set dm = swDoc.Parameter(\"D\" & Trim(Str(wFirst)) & \"@\" & feat.Name)\n    If dm Is Nothing Then gMsg = \"the extrude dimension carrying the first depth is missing\": GoTo ExFail\n");
                body.push_str(&format!(
                    "    If Abs(dm.SystemValue - {}) > 0.000001 Then gMsg = \"the extrude first depth does not match\": GoTo ExFail\n",
                    vba_num(*first_m)
                ));
                body.push_str("    Set dm = swDoc.Parameter(\"D\" & Trim(Str(wSecond)) & \"@\" & feat.Name)\n    If dm Is Nothing Then gMsg = \"the extrude dimension carrying the second depth is missing\": GoTo ExFail\n");
                body.push_str(&format!(
                    "    If Abs(dm.SystemValue - {}) > 0.000001 Then gMsg = \"the extrude second depth does not match\": GoTo ExFail\n",
                    vba_num(*second_m)
                ));
                for eq in &ex.equations {
                    // The equation binds to the dimension carrying this
                    // source side - after the swap, never by literal D number.
                    let which = if eq.which == 1 { "wFirst" } else { "wSecond" };
                    body.push_str(&slot_guards(eq, "ExFail"));
                    body.push_str(&format!(
                        "    Set dm = swDoc.Parameter(\"D\" & Trim(Str({which})) & \"@\" & feat.Name)\n    If dm Is Nothing Then gMsg = \"the extrude depth dimension is missing\": GoTo ExFail\n    If Not AddDimEquation(dm, {}, {}, {}, {}, feat.Name, 1#) Then gMsg = \"the extrude depth equation did not evaluate\": GoTo ExFail\n",
                        rhs_vba(eq),
                        vba_num(eq.expect_si.abs()),
                        eq.slot,
                        sign_lit(eq.expect_si)
                    ));
                }
            }
            format!("two-sided {} + {} m", vba_num(*first_m), vba_num(*second_m))
        }
        ExtrudeKind::ThroughAll { negative } => {
            locals.push_str("    Dim wantSide As Long\n    Dim dirFlag As Boolean\n");
            body.push_str("    If TT_EX_THROUGH_ALL_DIR = 0 Then gMsg = \"the through-all extrude direction semantics are not established: probe-throughall cannot pin a physical side without bounded material and its interpretation contract forbids inferring it from the blind cases; refusing to report created\": GoTo ExFail\n");
            body.push_str(&format!(
                "    wantSide = {} * gMapSide({sk})\n    dirFlag = (wantSide = TT_EX_THROUGH_ALL_DIR)\n",
                if *negative { "-1" } else { "1" }
            ));
            body.push_str(&select_sketch);
            body.push_str("    Set feat = gFeatMgr.FeatureExtrusion2(True, False, dirFlag, swEndConditions_e.swEndCondThroughAll, swEndConditions_e.swEndCondBlind, 0#, 0#, False, False, False, False, 0#, 0#, False, False, False, False, True, True, True, 0, 0#, False)\n    If feat Is Nothing Then gMsg = \"the extrude was not created\": GoTo ExFail\n");
            body.push_str(no_depth_note);
            "through all".to_string()
        }
    };
    let mut tail = String::new();
    tail.push_str(&format!(
        "    Set feat{index} = feat\n    gMsg = {}\n    BuildExtrude{index} = \"created\"\n    Exit Function\n\nExFail:\n    BuildExtrude{index} = \"failed\"\n    Exit Function\n\nHardFail:\n    gMsg = \"err \" & Trim(Str(Err.Number)) & \": \" & Err.Description\n    On Error Resume Next\n    BuildExtrude{index} = \"failed\"\nEnd Function\n\n",
        vb_lit(&message)
    ));
    out.push_str(&format!(
        "Function BuildExtrude{index}() As String\n    On Error GoTo HardFail\n{locals}"
    ));
    out.push_str(&body);
    out.push_str(&tail);
    out
}

/// The source-side sweep factor for a revolve: the SolidCraft kernel's own
/// canonical factor (`canonical_revolve_axis` / `revolved`: partial turns go
/// toward the side the sketch normal points to) from the sketch's
/// NON-CONSTRUCTION profile geometry. Computed at emit time; a named Err
/// means the side cannot be established and the builder fails closed.
fn axis_sigma(sk: &SketchStep, axis: &AxisSel) -> Result<i32, String> {
    let (o, d) = match axis {
        AxisSel::Emitted { origin, dir } => (*origin, *dir),
        AxisSel::Line(li) => {
            let l = sk
                .lines
                .get(*li)
                .ok_or_else(|| "the revolve axis line is not in the sketch".to_string())?;
            let a = sk.points[l.a];
            let b = sk.points[l.b];
            (a, [b[0] - a[0], b[1] - a[1]])
        }
    };
    let lines: Vec<(usize, usize, bool)> = sk
        .lines
        .iter()
        .map(|l| (l.a, l.b, l.construction))
        .collect();
    let circles: Vec<(usize, f64, bool)> = sk
        .circles
        .iter()
        .map(|c| (c.c, c.r, c.construction))
        .collect();
    crate::truth::profile_side(&sk.points, &lines, &circles, o, d)
}

/// Revolve builder: the axis is mapped through the sketch's adaptation map
/// "like geometry" (both helper-axis endpoints go through MapU/MapV; a
/// profile-line axis is already drawn mapped, so the drawn direction is the
/// mapped source direction). The sweep direction runs through the direction
/// truth table: the evidence-scope gate (TT_REV_PROBE_CHIRALITY - the
/// chirality where every candidate reading of the probe evidence agrees),
/// the measured sense constants, and the source-side factor from the
/// profile geometry.
/// Full turns and mid-plane forms are direction-free and consult no
/// constant. Everything unestablished fails with a named reason before the
/// feature call - never "created". A refusal emits ONLY the refusal: every
/// emitted identifier is declared on its own path (Option Explicit makes a
/// stray reference a whole-module compile failure).
fn emit_revolve_subs(index: usize, rv: &RevolveStep, sk: Option<&SketchStep>) -> String {
    use crate::replay::{DimEquation as DE, RevolveKind as RK};
    let sidx = rv.sketch_step;
    let mut out = String::new();
    out.push_str(&format!("' Revolve step {index}: {}\n", vb_lit(&rv.name)));
    let full_turn = matches!(
        &rv.kind,
        RK::Blind { angle_rad, .. } if (*angle_rad - 2.0 * std::f64::consts::PI).abs() < 1e-9
    );
    // The source-side factor is needed by directional forms only; the
    // direction-free paths (full turn, mid-plane) never consult it.
    let need_sigma = match &rv.kind {
        RK::TwoSided { .. } => true,
        RK::Blind { .. } => !full_turn,
        RK::Symmetric { .. } => false,
    };
    let sigma = if need_sigma {
        match sk {
            Some(sk) => axis_sigma(sk, &rv.axis),
            None => Err("the profile sketch is not in the plan".to_string()),
        }
    } else {
        Ok(0)
    };
    // Fail-closed preamble: signed two-sided intervals and unestablishable
    // profile sides refuse here, before any selection or feature call.
    let mut refusal: Option<String> = None;
    match &rv.kind {
        RK::TwoSided {
            first_rad,
            second_rad,
        } => {
            if *first_rad <= 0.0 || *second_rad <= 0.0 {
                refusal = Some("a two-sided revolve needs both angles positive: the swept interval would not start at the sketch plane (signed angles are refused, never folded)".to_string());
            } else if let Err(reason) = &sigma {
                refusal = Some(format!(
                    "the revolve direction semantics cannot be established: {reason}"
                ));
            }
        }
        RK::Blind { .. } if !full_turn => {
            if let Err(reason) = &sigma {
                refusal = Some(format!(
                    "the revolve direction semantics cannot be established: {reason}"
                ));
            }
        }
        _ => {}
    }
    let mut locals = String::from(
        "    Dim ok As Boolean\n    Dim feat As SldWorks.Feature\n    Dim dm As SldWorks.Dimension\n    Dim ax As Object\n    Dim objState As String\n",
    );
    let mut body = String::new();
    body.push_str(&format!(
        "    If Not gMapValid({sidx}) Then gMsg = \"frame adaptation: the profile sketch carries no valid adaptation map; refusing to report created\": GoTo ExFail\n"
    ));
    // The evidence-scope gate (the plan review's evidence-scoped
    // restriction): the probes measure one frame and ONE directed axis, so
    // the two candidate sweep laws (absolute side in normal units; the
    // right-hand sense about the drawn axis) are indistinguishable there.
    // Where the configuration chirality gMapSide * sigma equals the
    // probe's, the two readings produce IDENTICAL flags and the sweep side
    // is established under either; outside it the readings diverge and the
    // step refuses with a named reason - the sense constants alone never
    // unlock an unestablished case.
    let scope_gate = |sg: i32| {
        format!(
        "    If gMapSide({sidx}) * {sg} <> TT_REV_PROBE_CHIRALITY Then gMsg = \"the revolve sweep-direction evidence is scoped to the probe's configuration chirality (probe-revolvedir: one directed axis and one profile side); this map orientation and profile side are outside it and the candidate sweep laws disagree there; refusing to report created\": GoTo ExFail\n"
    )
    };
    let select_sketch = format!(
        "    ok = False\n    objState = \"missing\"\n    If Not skFeat{sidx} Is Nothing Then\n    objState = \"name=<\" & skFeat{sidx}.Name & \">\"\n    ok = skFeat{sidx}.Select(False)\n    objState = objState & \" select-result=\" & CStr(ok)\n    End If\n    If ok = False Then ok = swDoc.Extension.SelectByID2(skName{sidx}, \"SKETCH\", 0#, 0#, 0#, False, 0, Nothing, swSelectOption_e.swSelectOptionDefault)\n    If ok = False Then gMsg = \"could not select the profile sketch: \" & objState & \" name=<\" & skName{sidx} & \">; tree scan saw: \" & TreeSummary(): GoTo ExFail\n"
    );
    if let Some(reason) = refusal {
        body.push_str(&format!(
            "    gMsg = \"{reason}; refusing to report created\": GoTo ExFail\n"
        ));
    } else {
        // The direction decision runs before any selection.
        let flag = match &rv.kind {
            RK::Blind { reverse, .. } if !full_turn => {
                let Ok(sg) = &sigma else { unreachable!() };
                let want = if *reverse { -sg } else { *sg };
                locals.push_str("    Dim revFlag As Boolean\n");
                body.push_str(&scope_gate(*sg));
                body.push_str("    If TT_REV_SENSE = 0 Then gMsg = \"the revolve sweep-direction truth table is unmeasured: probe-revolvedir has not established which side ReverseDir=False sweeps toward; refusing to report created\": GoTo ExFail\n");
                body.push_str(&format!("    revFlag = (TT_REV_SENSE <> {want})\n"));
                "revFlag"
            }
            RK::Blind { .. } => "False",
            _ => "False",
        };
        if let RK::TwoSided {
            first_rad,
            second_rad,
        } = &rv.kind
        {
            let Ok(sg) = &sigma else { unreachable!() };
            locals.push_str("    Dim swapR As Boolean\n    Dim aA As Double\n    Dim aB As Double\n    Dim wFirst As Long\n    Dim wSecond As Long\n");
            body.push_str(&scope_gate(*sg));
            body.push_str("    If TT_REV_TWO_FIRST_SIDE = 0 Then gMsg = \"the two-sided revolve angle truth table is unmeasured: probe-revolvedir has not established which side receives the first angle; refusing to report created\": GoTo ExFail\n");
            body.push_str(&format!(
                "    swapR = (TT_REV_TWO_FIRST_SIDE <> {sg})\n    If swapR Then\n    aA = {}\n    aB = {}\n    wFirst = 2\n    wSecond = 1\n    Else\n    aA = {}\n    aB = {}\n    wFirst = 1\n    wSecond = 2\n    End If\n",
                vba_num(*second_rad),
                vba_num(*first_rad),
                vba_num(*first_rad),
                vba_num(*second_rad),
            ));
        }
        // Select the profile sketch, draw/anchor the helper axis when the
        // axis is not already a line of the sketch, then select the axis.
        body.push_str(&select_sketch);
        if let AxisSel::Emitted { .. } = &rv.axis {
            body.push_str(&emit_helper_axis(sidx, &rv.axis, &mut locals));
        }
        body.push_str("    swSketchMgr.InsertSketch True\n");
        body.push_str(&select_sketch);
        match &rv.axis {
            AxisSel::Line(li) => body.push_str(&format!(
                "    If Not sl{sidx}({li}).Select(True) Then gMsg = \"could not select the revolve axis\": GoTo ExFail\n"
            )),
            AxisSel::Emitted { .. } => body.push_str(
                "    If Not ax.Select(True) Then gMsg = \"could not select the helper axis\": GoTo ExFail\n",
            ),
        }
        match &rv.kind {
            RK::Blind { angle_rad, .. } => {
                body.push_str(&format!(
                    "    Set feat = gFeatMgr.FeatureRevolve2(True, True, False, False, {flag}, True, swEndConditions_e.swEndCondBlind, swEndConditions_e.swEndCondBlind, {}, 0#, False, False, 0#, 0#, swThinWallType_e.swThinWallOneDirection, 0#, 0#, True, False, True)\n    If feat Is Nothing Then gMsg = \"the revolve was not created (kind=single-dir)\": GoTo ExFail\n",
                    vba_num(*angle_rad)
                ));
                body.push_str(&dim_check(1, angle_rad.abs(), "revolve angle D1"));
                for eq in rv.equations.iter().filter(|e| e.which == 1) {
                    let eq: &DE = eq;
                    body.push_str(&slot_guards(eq, "ExFail"));
                    body.push_str(&dim_eq(
                        1,
                        eq,
                        "revolve angle D1",
                        "the revolve angle equation did not evaluate",
                    ));
                }
            }
            RK::Symmetric { total_rad } => {
                // Direction-free: the mid-plane form sweeps both ways ("the
                // angle is split either side of the profile, whichever way
                // the axis points"); the angle is the TOTAL (the documented
                // mid-plane semantics, probe-confirmable).
                body.push_str(&format!(
                    "    Set feat = gFeatMgr.FeatureRevolve2(True, True, False, False, False, True, swEndConditions_e.swEndCondMidPlane, swEndConditions_e.swEndCondBlind, {}, 0#, False, False, 0#, 0#, swThinWallType_e.swThinWallOneDirection, 0#, 0#, True, False, True)\n    If feat Is Nothing Then gMsg = \"the revolve was not created (kind=midplane)\": GoTo ExFail\n",
                    vba_num(*total_rad)
                ));
                body.push_str(&dim_check(1, total_rad.abs(), "revolve angle D1"));
                for eq in rv.equations.iter().filter(|e| e.which == 1) {
                    let eq: &DE = eq;
                    body.push_str(&slot_guards(eq, "ExFail"));
                    body.push_str(&dim_eq(
                        1,
                        eq,
                        "revolve angle D1",
                        "the revolve angle equation did not evaluate",
                    ));
                }
            }
            RK::TwoSided {
                first_rad,
                second_rad,
            } => {
                body.push_str("    Set feat = gFeatMgr.FeatureRevolve2(False, True, False, False, False, True, swEndConditions_e.swEndCondBlind, swEndConditions_e.swEndCondBlind, aA, aB, False, False, 0#, 0#, swThinWallType_e.swThinWallOneDirection, 0#, 0#, True, False, True)\n    If feat Is Nothing Then gMsg = \"the revolve was not created (kind=two-sided)\": GoTo ExFail\n");
                body.push_str(&dim_check_dyn(
                    "wFirst",
                    first_rad.abs(),
                    "revolve first angle",
                ));
                body.push_str(&dim_check_dyn(
                    "wSecond",
                    second_rad.abs(),
                    "revolve second angle",
                ));
                for eq in &rv.equations {
                    // The equation binds to the dimension carrying this
                    // source angle - after the swap, never by literal D number.
                    let which = if eq.which == 1 { "wFirst" } else { "wSecond" };
                    body.push_str(&slot_guards(eq, "ExFail"));
                    body.push_str(&dim_eq_dyn(
                        which,
                        eq,
                        "revolve angle",
                        "the revolve angle equation did not evaluate",
                    ));
                }
            }
        }
    }
    let message = match &rv.kind {
        RK::Blind { angle_rad, .. } => format!("revolve {} deg", angle_rad.to_degrees()),
        RK::Symmetric { total_rad } => {
            format!("symmetric revolve {} deg total", total_rad.to_degrees())
        }
        RK::TwoSided {
            first_rad,
            second_rad,
        } => format!(
            "two-sided revolve {} + {} deg",
            first_rad.to_degrees(),
            second_rad.to_degrees()
        ),
    };
    out.push_str(&format!(
        "Function BuildRevolve{index}() As String\n    On Error GoTo HardFail\n{locals}"
    ));
    out.push_str(&body);
    out.push_str(&format!(
        "    Set feat{index} = feat\n    gMsg = {}\n    BuildRevolve{index} = \"created\"\n    Exit Function\n\nExFail:\n    On Error Resume Next\n    CloseSketchIfOpen\n    BuildRevolve{index} = \"failed\"\n    Exit Function\n\nHardFail:\n    ' Capture the error FIRST: executing an On Error statement resets Err.\n    Dim errNum As Long, errMsg As String\n    errNum = Err.Number\n    errMsg = Err.Description\n    On Error Resume Next\n    CloseSketchIfOpen\n    gMsg = \"err \" & Trim(Str(errNum)) & \": \" & errMsg\n    BuildRevolve{index} = \"failed\"\nEnd Function\n\n",
        vb_lit(&format!("{message}; sweep direction and volume are acceptance checks"))
    ));
    out
}

/// The helper construction axis for an AxisSel::Emitted revolve: drawn
/// through the sketch's adaptation map LIKE GEOMETRY - both endpoints go
/// through MapU/MapV, so the drawn direction is the mapped source direction
/// (the sweep sense depends on it). The mapped placement is evidence, not
/// assumption: after the fix and a solve, both endpoints are read back
/// against the mapped coordinates and the z component must vanish.
fn emit_helper_axis(sidx: usize, axis: &AxisSel, locals: &mut String) -> String {
    let AxisSel::Emitted { origin, dir } = axis else {
        return String::new();
    };
    let l = 0.05f64; // metres each way
    let (x1, y1) = (
        origin[0] / 1000.0 - dir[0] * l,
        origin[1] / 1000.0 - dir[1] * l,
    );
    let (x2, y2) = (
        origin[0] / 1000.0 + dir[0] * l,
        origin[1] / 1000.0 + dir[1] * l,
    );
    locals.push_str("    Dim vA As Variant\n    Dim vB As Variant\n");
    let mut out = String::new();
    out.push_str("    swSketchMgr.InsertSketch True\n");
    out.push_str(&format!(
        "    Set ax = swSketchMgr.CreateLine(MapU({sidx}, {}, {}), MapV({sidx}, {}, {}), 0#, MapU({sidx}, {}, {}), MapV({sidx}, {}, {}), 0#)\n",
        vba_num(x1),
        vba_num(y1),
        vba_num(x1),
        vba_num(y1),
        vba_num(x2),
        vba_num(y2),
        vba_num(x2),
        vba_num(y2),
    ));
    out.push_str(
        "    If ax Is Nothing Then gMsg = \"the revolve helper axis did not appear\": GoTo ExFail\n",
    );
    out.push_str("    ax.ConstructionGeometry = True\n");
    out.push_str(
        "    If Not ax.Select(False) Then gMsg = \"the helper axis would not select for fixing\": GoTo ExFail\n",
    );
    out.push_str("    swDoc.SketchAddConstraints \"sgFix\"\n    swDoc.ClearSelection2 True\n");
    out.push_str(&format!(
        "    SolveSketch skName{sidx}, skFeat{sidx}\n    vA = ax.GetStartPoint\n    vB = ax.GetEndPoint\n    If Abs(vA(0) - MapU({sidx}, {}, {})) > 0.000001 Then gMsg = \"the revolve helper axis start does not sit where the map put it\": GoTo ExFail\n    If Abs(vA(1) - MapV({sidx}, {}, {})) > 0.000001 Then gMsg = \"the revolve helper axis start does not sit where the map put it\": GoTo ExFail\n    If Abs(vA(2)) > 0.000001 Then gMsg = \"the revolve helper axis start is off the sketch plane\": GoTo ExFail\n    If Abs(vB(0) - MapU({sidx}, {}, {})) > 0.000001 Then gMsg = \"the revolve helper axis end does not sit where the map put it\": GoTo ExFail\n    If Abs(vB(1) - MapV({sidx}, {}, {})) > 0.000001 Then gMsg = \"the revolve helper axis end does not sit where the map put it\": GoTo ExFail\n    If Abs(vB(2)) > 0.000001 Then gMsg = \"the revolve helper axis end is off the sketch plane\": GoTo ExFail\n",
        vba_num(x1),
        vba_num(y1),
        vba_num(x1),
        vba_num(y1),
        vba_num(x2),
        vba_num(y2),
        vba_num(x2),
        vba_num(y2),
    ));
    out
}

/// Primitive builder: a generated sketch on the plane carrying the XY
/// source frame, drawn MAPPED through MapU/MapV like sketch geometry (the
/// box as four segments in deterministic source order - the u edge from the
/// corner carries the length dimension, the v edge the width; the cylinder
/// as a mapped centre with its radius unchanged, the map being an
/// isometry), sizes in SI metres with their dimension equations under the
/// sign contract, then a blind extrude whose side resolves through the
/// direction truth table (wantSide = +1 * gMapSide: the SolidCraft height
/// extends toward +Z, the XY source normal - source-pinned kernel
/// semantics). Fail-closed before any drawing: the map must be valid and a
/// signed permutation (bounded support), and the orientation cell must be
/// measured (probe-primorient). Everything unestablished fails with a
/// named reason and never reports created.
fn emit_prim_subs(index: usize, pr: &crate::replay::PrimStep) -> String {
    use crate::replay::PrimKind;
    let mut out = String::new();
    out.push_str(&format!("' Primitive step {index}: {}\n", vb_lit(&pr.name)));
    let entity_decls = match pr.kind {
        PrimKind::Box => {
            "    Dim ln0 As Object\n    Dim ln1 As Object\n    Dim ln2 As Object\n    Dim ln3 As Object\n"
        }
        PrimKind::Cylinder => "    Dim ci As Object\n",
    };
    out.push_str(&format!(
        "Function BuildPrim{index}() As String\n    On Error GoTo HardFail\n    Const TOL As Double = 0.000001\n    Dim ok As Boolean\n    Dim dimScale As Double\n    Dim feat As SldWorks.Feature\n    Dim skFeat As SldWorks.Feature\n    Dim primName As String\n    Dim dm As SldWorks.Dimension\n    Dim dd As SldWorks.DisplayDimension\n    Dim created As Double\n    Dim signVal As Double\n    Dim chk As Double\n    Dim v As Variant\n    Dim v2 As Variant\n    Dim wantSide As Long\n    Dim dirFlag As Boolean\n    Dim axClassU As Long\n    Dim axClassV As Long\n    Dim nameU As String\n    Dim nameV As String\n    Dim pA As Object\n    Dim pB As Object\n    Dim uncertain As Boolean\n    Dim cur As Object\n{entity_decls}"
    ));
    out.push_str(&format!("    ok = OpenSketchOnPlane({}, \"XY\")\n", index));
    out.push_str("    If ok = False Then gMsg = \"no origin plane carries the XY frame u=(1,0,0) v=(0,1,0) n=(0,0,1); probed: \" & gProbeLog & \"; tree scan saw: \" & TreeSummary(): GoTo ExFail\n");
    // Fail-closed guards before any drawing: the map validity, the bounded
    // map class (signed permutations, plan scope F5), and the orientation
    // cell (probe-primorient). Nothing downstream of an unestablished
    // orientation is ever claimed as created.
    out.push_str(&format!(
        "    If Not gMapValid({index}) Then gMsg = \"frame adaptation: the primitive sketch carries no valid adaptation map; refusing to report created\": GoTo ExFail\n"
    ));
    out.push_str(&format!(
        "    If Not MapIsPermutation({index}) Then gMsg = \"frame adaptation: the primitive profile requires a signed-permutation sketch map (bounded support, this seat only exhibits permutation frames); refusing to report created\": GoTo ExFail\n"
    ));
    out.push_str("    If TT_PRIM_DIR_TRUE_SIDE = 0 Then gMsg = \"the primitive direction truth table is unmeasured: probe-primorient has not established which physical side the primitive height extends toward on this sketch map; refusing to report created\": GoTo ExFail\n");
    let (ox, oy) = (pr.origin[0] / 1000.0, pr.origin[1] / 1000.0);
    match pr.kind {
        PrimKind::Box => {
            let l = pr.sizes[0].value / 1000.0;
            let w = pr.sizes[1].value / 1000.0;
            // The four corners in source order; each edge is drawn between
            // MAPPED corners exactly like sketch geometry. Edge identity is
            // deterministic by construction (never a host segment order):
            // ln0 is the u edge from the corner (the length dimension), ln1
            // the v edge from its far corner (the width dimension).
            let corners = [(ox, oy), (ox + l, oy), (ox + l, oy + w), (ox, oy + w)];
            let edges = [
                ("ln0", 0, 1, "the box length edge"),
                ("ln1", 1, 2, "the box width edge"),
                ("ln2", 2, 3, "the box length edge"),
                ("ln3", 3, 0, "the box width edge"),
            ];
            for (handle, a, b, what) in edges {
                let (x1, y1) = corners[a];
                let (x2, y2) = corners[b];
                out.push_str(&format!(
                    "    Set {handle} = swSketchMgr.CreateLine(MapU({index}, {}, {}), MapV({index}, {}, {}), 0#, MapU({index}, {}, {}), MapV({index}, {}, {}), 0#)\n    If {handle} Is Nothing Then gMsg = \"{what} did not appear\": GoTo ExFail\n",
                    vba_num(x1),
                    vba_num(y1),
                    vba_num(x1),
                    vba_num(y1),
                    vba_num(x2),
                    vba_num(y2),
                    vba_num(x2),
                    vba_num(y2),
                ));
            }
        }
        PrimKind::Cylinder => {
            let r = pr.sizes[0].value / 1000.0;
            out.push_str(&format!(
                "    Set ci = swSketchMgr.CreateCircleByRadius(MapU({index}, {}, {}), MapV({index}, {}, {}), 0#, {})\n    If ci Is Nothing Then gMsg = \"the cylinder profile circle did not appear\": GoTo ExFail\n",
                vba_num(ox),
                vba_num(oy),
                vba_num(ox),
                vba_num(oy),
                vba_num(r)
            ));
        }
    }
    // The sketch name is captured before any equation rebuild: SolveSketch
    // re-enters the sketch by name after AddDimEquation's EditRebuild3
    // exits sketch edit (seat-measured semantics).
    out.push_str("    Set skFeat = swSketchMgr.ActiveSketch\n    If skFeat Is Nothing Then gMsg = \"the primitive sketch was not created\": GoTo ExFail\n    Set skFeat = LastFeature()\n    If skFeat Is Nothing Then gMsg = \"the primitive sketch is missing from the tree\": GoTo ExFail\n    primName = skFeat.Name\n    Set skFeat = FindFeatureByName(primName, uncertain)\n    If skFeat Is Nothing Then gMsg = \"the primitive sketch feature object could not be captured for selection: name=<\" & primName & \">, tree scan saw: \" & TreeSummary(): GoTo ExFail\n");
    match pr.kind {
        PrimKind::Box => {
            // The shape contract (parameter-edit safety): coincident
            // corners close the contour, classified horizontal/vertical
            // relations pin rectangularity and edge directions, and a fix on
            // the corner point anchors the literal placement. No automatic
            // relation is assumed; the edit-follow behaviour is pinned by
            // probe-primorient's box-edit-follow case (docs/mapping.md,
            // Primitives).
            for (ha, hb) in [
                ("ln0", "ln1"),
                ("ln1", "ln2"),
                ("ln2", "ln3"),
                ("ln3", "ln0"),
            ] {
                out.push_str(&format!(
                    "    Set pA = {ha}.GetEndPoint2\n    Set pB = {hb}.GetStartPoint2\n    If pA Is Nothing Or pB Is Nothing Then gMsg = \"the box corner handles are missing\": GoTo ExFail\n    If Not pA.Select(False) Then gMsg = \"the box corner point would not select\": GoTo ExFail\n    If Not pB.Select(True) Then gMsg = \"the box corner point would not select\": GoTo ExFail\n    swDoc.SketchAddConstraints \"sgCOINCIDENT\"\n    swDoc.ClearSelection2 True\n"
                ));
            }
            out.push_str(&format!(
                "    axClassU = AxisClass({index}, True)\n    axClassV = AxisClass({index}, False)\n    If axClassU < 0 Or axClassV < 0 Then gMsg = \"the primitive profile axes are not axis-aligned in this sketch frame (non-permutation map); refusing to report created\": GoTo ExFail\n    If axClassU = 0 Then nameU = \"sgHORIZONTAL2D\" Else nameU = \"sgVERTICAL2D\"\n    If axClassV = 0 Then nameV = \"sgHORIZONTAL2D\" Else nameV = \"sgVERTICAL2D\"\n"
            ));
            for (handle, name, what) in [
                ("ln0", "nameU", "the box length edge"),
                ("ln2", "nameU", "the box length edge"),
                ("ln1", "nameV", "the box width edge"),
                ("ln3", "nameV", "the box width edge"),
            ] {
                out.push_str(&format!(
                    "    If Not {handle}.Select(False) Then gMsg = \"{what} would not select\": GoTo ExFail\n    swDoc.SketchAddConstraints {name}\n    swDoc.ClearSelection2 True\n"
                ));
            }
            out.push_str("    Set pA = ln0.GetStartPoint2\n    If pA Is Nothing Then gMsg = \"the box corner handle is missing\": GoTo ExFail\n    If Not pA.Select(False) Then gMsg = \"the box corner anchor would not select\": GoTo ExFail\n    swDoc.SketchAddConstraints \"sgFix\"\n    swDoc.ClearSelection2 True\n");
        }
        PrimKind::Cylinder => {
            // The centre anchor: placement is literal, so the fix keeps it
            // under parameter edits (same anchoring vocabulary and posture
            // as the revolve helper axis).
            out.push_str("    Set pA = ci.GetCenterPoint2\n    If pA Is Nothing Then gMsg = \"the cylinder centre handle is missing\": GoTo ExFail\n    If Not pA.Select(False) Then gMsg = \"the cylinder centre anchor would not select\": GoTo ExFail\n    swDoc.SketchAddConstraints \"sgFix\"\n    swDoc.ClearSelection2 True\n");
        }
    }
    match pr.kind {
        PrimKind::Box => {
            out.push_str(&emit_size("ln0", &pr.sizes[0], "the box length", false));
            out.push_str(&emit_size("ln1", &pr.sizes[1], "the box width", false));
        }
        PrimKind::Cylinder => {
            out.push_str(&emit_size("ci", &pr.sizes[0], "the cylinder radius", true));
        }
    }
    // Mapped-placement verification (evidence, not assumption - the revolve
    // helper-axis standard): after the solve the drawn geometry must sit
    // exactly where the map put it. The eight edge endpoints against the
    // four mapped corners also prove the contour stayed closed.
    out.push_str("    SolveSketch primName, skFeat\n");
    match pr.kind {
        PrimKind::Box => {
            let l = pr.sizes[0].value / 1000.0;
            let w = pr.sizes[1].value / 1000.0;
            let corners = [(ox, oy), (ox + l, oy), (ox + l, oy + w), (ox, oy + w)];
            for (handle, a, b) in [("ln0", 0, 1), ("ln1", 1, 2), ("ln2", 2, 3), ("ln3", 3, 0)] {
                for (which, at) in [("Start", a), ("End", b)] {
                    let (x, y) = corners[at];
                    out.push_str(&format!(
                        "    v = {handle}.Get{which}Point\n    If Abs(v(0) - MapU({index}, {}, {})) > TOL Then gMsg = \"the {handle} {which} point does not sit where the map put it\": GoTo ExFail\n    If Abs(v(1) - MapV({index}, {}, {})) > TOL Then gMsg = \"the {handle} {which} point does not sit where the map put it\": GoTo ExFail\n    If Abs(v(2)) > TOL Then gMsg = \"the {handle} {which} point is off the sketch plane\": GoTo ExFail\n",
                        vba_num(x),
                        vba_num(y),
                        vba_num(x),
                        vba_num(y)
                    ));
                }
            }
        }
        PrimKind::Cylinder => {
            let r = pr.sizes[0].value / 1000.0;
            out.push_str(&format!(
                "    Set pA = ci.GetCenterPoint2\n    If pA Is Nothing Then gMsg = \"the cylinder centre handle is missing\": GoTo ExFail\n    If Abs(pA.X - MapU({index}, {}, {})) > TOL Then gMsg = \"the cylinder centre does not sit where the map put it\": GoTo ExFail\n    If Abs(pA.Y - MapV({index}, {}, {})) > TOL Then gMsg = \"the cylinder centre does not sit where the map put it\": GoTo ExFail\n    If Abs(pA.Z) > TOL Then gMsg = \"the cylinder centre is off the sketch plane\": GoTo ExFail\n    chk = ci.GetRadius\n    If Abs(chk - {}) > TOL Then gMsg = \"the cylinder radius does not sit where the draw put it\": GoTo ExFail\n",
                vba_num(ox),
                vba_num(oy),
                vba_num(ox),
                vba_num(oy),
                vba_num(r)
            ));
        }
    }
    out.push_str(
        "    Set cur = swSketchMgr.ActiveSketch\n    If cur Is Nothing Then gMsg = \"the primitive sketch was not open at closing\": GoTo ExFail\n    swDoc.ClearSelection2 True\n    swSketchMgr.InsertSketch True\n",
    );
    // The blind extrude carries the last size (height).
    let height = pr.sizes.last().map(|s| s.value / 1000.0).unwrap_or(0.0);
    out.push_str("    ok = False\n    If Not skFeat Is Nothing Then ok = skFeat.Select(False)\n    If ok = False Then ok = swDoc.Extension.SelectByID2(primName, \"SKETCH\", 0#, 0#, 0#, False, 0, Nothing, swSelectOption_e.swSelectOptionDefault)\n");
    out.push_str(
        "    If ok = False Then gMsg = \"could not select the primitive sketch (object and name selection both failed): name=<\" & primName & \">, tree scan saw: \" & TreeSummary(): GoTo ExFail\n",
    );
    // The operation orientation: the SolidCraft height extends toward +Z
    // (the XY source normal), so its end sits toward +1 * gMapSide in
    // sketch-normal units and the Dir flag resolves through the measured
    // cell - the same composition as the extrude family.
    out.push_str(&format!(
        "    wantSide = 1 * gMapSide({index})\n    dirFlag = (wantSide = TT_PRIM_DIR_TRUE_SIDE)\n    Set feat = gFeatMgr.FeatureExtrusion2(True, False, dirFlag, swEndConditions_e.swEndCondBlind, swEndConditions_e.swEndCondBlind, {}, 0#, False, False, False, False, 0#, 0#, False, False, False, False, True, True, True, 0, 0#, False)\n    If feat Is Nothing Then gMsg = \"the primitive extrude was not created\": GoTo ExFail\n",
        vba_num(height)
    ));
    out.push_str("    Set dm = swDoc.Parameter(\"D1@\" & feat.Name)\n    If dm Is Nothing Then gMsg = \"the primitive height dimension is missing\": GoTo ExFail\n");
    out.push_str(&format!(
        "    If Abs(dm.SystemValue - {}) > TOL Then gMsg = \"the primitive height does not match\": GoTo ExFail\n",
        vba_num(height)
    ));
    if let Some(last) = pr.sizes.last() {
        out.push_str(&emit_size_eq(last, "the primitive height"));
    }
    out.push_str(&format!(
        "    gMsg = \"primitive created\"\n    BuildPrim{index} = \"created\"\n    Exit Function\n\nExFail:\n    On Error Resume Next\n    CloseSketchIfOpen\n    BuildPrim{index} = \"failed\"\n    Exit Function\n\nHardFail:\n    ' Capture the error FIRST: executing an On Error statement resets Err.\n    Dim errNum As Long, errMsg As String\n    errNum = Err.Number\n    errMsg = Err.Description\n    On Error Resume Next\n    CloseSketchIfOpen\n    gMsg = \"err \" & Trim(Str(errNum)) & \": \" & errMsg\n    BuildPrim{index} = \"failed\"\nEnd Function\n\n"
    ));
    out
}

/// One primitive sketch-size dimension on the named entity (the handle the
/// builder holds). The kind is established UNAMBIGUOUSLY before the value
/// is written and the dimension registered: a circle dimension whose
/// radius- and diameter-kind readings coincide within tolerance is refused
/// (parse_prim refuses sizes at that scale too), and a diameter-kind
/// reading is refused outright - the sign contract carries a sign, never a
/// factor of two (docs/mapping.md, Primitives). The sign is resolved from
/// the readback and travels through AddDimEquation's signVal into regSign,
/// so downstream SlotRef references normalize back to source semantics.
/// Every equation is followed by a SolveSketch re-entry: AddDimEquation's
/// EditRebuild3 exits sketch edit (seat-measured).
fn emit_size(entity: &str, size: &crate::replay::PrimSize, what: &str, circle: bool) -> String {
    let m = size.value / 1000.0;
    let mn = vba_num(m);
    let mut out = String::new();
    out.push_str(&format!(
        "    If Not {entity}.Select(False) Then gMsg = \"{what} would not select\": GoTo ExFail\n    Set dd = swDoc.AddDimension2(0#, 0#, 0#)\n    If dd Is Nothing Then gMsg = \"{what} dimension did not appear\": GoTo ExFail\n    Set dm = dd.GetDimension2(0)\n    If dm Is Nothing Then gMsg = \"{what} dimension has no core dimension\": GoTo ExFail\n    created = dm.SystemValue\n"
    ));
    if circle {
        out.push_str(&format!(
            "    dimScale = 2#\n    chk = Abs(created)\n    If Abs(chk - 2# * {mn}) > TOL Then gMsg = \"{what} dimension measures \" & Trim(Str(created)) & \" but twice the source radius is {mn} * 2 (the API circle dimension is diameter-valued)\": GoTo ExFail\n    If Abs(created - 2# * {mn}) <= TOL Then\n    signVal = 1#\n    Else\n    signVal = -1#\n    End If\n"
        ));
    } else {
        out.push_str(&format!(
            "    dimScale = 1#\n    If Abs(created - {mn}) <= TOL Then\n    signVal = 1#\n    ElseIf Abs(created + {mn}) <= TOL Then\n    signVal = -1#\n    Else\n    gMsg = \"{what} dimension measures \" & Trim(Str(created)) & \" but the source value is {mn}\": GoTo ExFail\n    End If\n"
        ));
    }
    out.push_str(&format!("    dm.SystemValue = signVal * dimScale * {mn}\n"));
    // Behavioural kind evidence: the value edit must carry exactly the
    // measured quantity (docs/mapping.md, Evidence rules).
    if circle {
        out.push_str(&format!(
            "    chk = {entity}.GetRadius\n    If Abs(chk - {mn}) > TOL Then gMsg = \"{what} geometry did not follow the value edit (radius \" & Trim(Str(chk)) & \")\": GoTo ExFail\n    swDoc.ClearSelection2 True\n"
        ));
    } else {
        out.push_str(&format!(
            "    v = {entity}.GetStartPoint\n    v2 = {entity}.GetEndPoint\n    chk = Sqr((v2(0) - v(0)) ^ 2 + (v2(1) - v(1)) ^ 2)\n    If Abs(chk - {mn}) > TOL Then gMsg = \"{what} geometry did not follow the value edit (length \" & Trim(Str(chk)) & \")\": GoTo ExFail\n    swDoc.ClearSelection2 True\n"
        ));
    }
    if let Some(eq) = &size.equation {
        out.push_str(&slot_guards(eq, "ExFail"));
        let rhs_expr = if circle {
            format!("\"2*(\" & {} & \")\"", rhs_vba(eq))
        } else {
            rhs_vba(eq)
        };
        out.push_str(&format!(
            "    If Not AddDimEquation(dm, {}, signVal * {} * dimScale, {}, signVal, primName, dimScale) Then gMsg = \"{what} equation did not evaluate\": GoTo ExFail\n    SolveSketch primName, skFeat\n",
            rhs_expr,
            mn,
            eq.slot
        ));
    }
    out
}

fn emit_size_eq(size: &crate::replay::PrimSize, what: &str) -> String {
    match &size.equation {
        Some(eq) => {
            let mut out = slot_guards(eq, "ExFail");
            out.push_str(&format!(
            "    If Not AddDimEquation(dm, {}, {}, {}, {}, feat.Name, 1#) Then gMsg = \"{} equation did not evaluate\": GoTo ExFail\n",
            rhs_vba(eq),
            vba_num(eq.expect_si.abs()),
            eq.slot,
            sign_lit(eq.expect_si),
            what
        ));
            out
        }
        None => String::new(),
    }
}

/// The right-hand side as a VBA string expression: static when the equation
/// has no segments, otherwise a concatenation with runtime dimension names.
fn rhs_vba(eq: &crate::replay::DimEquation) -> String {
    use crate::replay::RhsSeg;
    if eq.segs.is_empty() {
        return vb_lit(&eq.rhs);
    }
    let mut parts: Vec<String> = Vec::new();
    for seg in &eq.segs {
        match seg {
            RhsSeg::Text(t) => parts.push(vb_lit(t)),
            RhsSeg::Slot(slot) => parts.push(format!("SlotRef({slot})")),
        }
    }
    parts.join(" & ")
}

/// Fail closed on dead slots: an equation may only reference a dimension
/// whose slot is live (registered after its own verification).
fn slot_guards(eq: &crate::replay::DimEquation, fail_label: &str) -> String {
    use crate::replay::RhsSeg;
    let mut out = String::new();
    for seg in &eq.segs {
        if let RhsSeg::Slot(slot) = seg {
            out.push_str(&format!(
                "    If regName({slot}) = \"\" Then gMsg = \"the parameter slot {slot} is not bound (its dimension was omitted or failed)\": GoTo {fail_label}\n"
            ));
        }
    }
    out
}

/// Modification features: coordinate-picked edges and faces (fail-closed
/// SelectByID2 with explicit types; recipe coordinates map identically into
/// the rebuilt model per docs/mapping.md) and one feature call each. The
/// five feature-call shapes are checkpoint-pending like every other API
/// member here: the seat loop corrects their exact argument lists.
fn emit_mod_subs(index: usize, md: &crate::replay::ModStep) -> String {
    use crate::replay::ModKind;
    let mut out = String::new();
    out.push_str(&format!(
        "' Modification step {index}: {}\n",
        vb_lit(&md.name)
    ));
    out.push_str(&format!(
        "Function BuildMod{index}() As String\n    On Error GoTo HardFail\n    Dim ok As Boolean\n    Dim feat As SldWorks.Feature\n    Dim dm As SldWorks.Dimension\n    Dim anySel As Boolean\n"
    ));
    // Selections first: every pick must land on the intended entity type.
    let (picks, pick_type) = match &md.kind {
        ModKind::Fillet { edges, .. } | ModKind::Chamfer { edges, .. } => (edges, "EDGE"),
        ModKind::Shell { faces, .. } | ModKind::Draft { faces, .. } => (faces, "FACE"),
        ModKind::Hole { at, .. } => (&vec![*at], "FACE"),
    };
    for (i, p) in picks.iter().enumerate() {
        let append = if i > 0 { "True" } else { "False" };
        out.push_str(&format!(
            "    ok = swDoc.Extension.SelectByID2(\"\", \"{pick_type}\", {}, {}, {}, {append}, 0, Nothing, swSelectOption_e.swSelectOptionDefault)\n",
            vba_num(p[0] / 1000.0),
            vba_num(p[1] / 1000.0),
            vba_num(p[2] / 1000.0)
        ));
        out.push_str(&format!(
            "    If ok = False Then gMsg = \"a {pick_type} pick did not land\": GoTo ExFail\n"
        ));
    }
    out.push_str("    anySel = True\n");
    // One feature call per family (argument lists: checkpoint-pending).
    match &md.kind {
        ModKind::Fillet {
            radius,
            tangent_chain,
            ..
        } => {
            out.push_str(&format!(
                "    ' feature call shape pending checkpoint verification\n    Set feat = gDoc.FeatureFillet3({}, {}, 0, 0, 0)\n",
                vba_num(radius / 1000.0),
                if *tangent_chain { "True" } else { "False" }
            ));
            out.push_str(&dim_check(1, radius / 1000.0, "the fillet radius"));
        }
        ModKind::Chamfer {
            distance,
            tangent_chain,
            ..
        } => {
            out.push_str(&format!(
                "    ' feature call shape pending checkpoint verification\n    Set feat = gDoc.FeatureChamfer3(0#, {}, {}, 0#, 0#, 0#, 0#, 0#, 0#, 0#, 0#)\n",
                vba_num(distance / 1000.0),
                vba_num(distance / 1000.0)
            ));
            let _ = tangent_chain;
            out.push_str(&dim_check(1, distance / 1000.0, "the chamfer distance"));
        }
        ModKind::Shell { thickness, .. } => {
            out.push_str(&format!(
                "    ' feature call shape pending checkpoint verification\n    Set feat = gDoc.FeatureShell({}, False, 0#, False, False, False, 0#, 0#, 0#, 0#, 0#)\n",
                vba_num(thickness / 1000.0)
            ));
            out.push_str(&dim_check(1, thickness / 1000.0, "the shell thickness"));
        }
        ModKind::Draft { angle_rad, .. } => {
            out.push_str(&format!(
                "    ' feature call shape pending checkpoint verification\n    Set feat = gDoc.FeatureDraft({}, False, 0#, 0#, False, False, False, 0#, 0#, 0#, 0#, 0#, 0#)\n",
                vba_num(*angle_rad)
            ));
            out.push_str(&dim_check(1, *angle_rad, "the draft angle"));
        }
        ModKind::Hole {
            diameter, depth, ..
        } => {
            let (endcond, d2) = match depth {
                Some(d) => ("swEndConditions_e.swEndCondBlind", vba_num(d / 1000.0)),
                None => ("swEndConditions_e.swEndCondThroughAll", "0#".to_string()),
            };
            out.push_str(&format!(
                "    ' feature call shape pending checkpoint verification\n    Set feat = gDoc.SimpleHole2(True, False, False, {endcond}, {}, {d2}, 0#, 0#, False, False, False, False, 0#, 0#)\n",
                vba_num(diameter / 1000.0)
            ));
            out.push_str(&dim_check(1, diameter / 1000.0, "the hole diameter"));
        }
    }
    out.push_str("    swDoc.ClearSelection2 True\n");
    for eq in &md.equations {
        out.push_str(&slot_guards(eq, "ExFail"));
        out.push_str(&format!(
            "    Set dm = swDoc.Parameter(\"D{}@\" & feat.Name)\n    If dm Is Nothing Then gMsg = \"the dimension D{} is missing\": GoTo ExFail\n    If Not AddDimEquation(dm, {}, {}, {}, {}, feat.Name, 1#) Then gMsg = \"the equation did not evaluate\": GoTo ExFail\n",
            eq.which,
            eq.which,
            rhs_vba(eq),
            vba_num(eq.expect_si.abs()),
            eq.slot,
            sign_lit(eq.expect_si)
        ));
    }
    let degrade_note = if md.degraded_variants.is_empty() {
        String::new()
    } else {
        format!(
            "; variants not replayed: {}",
            md.degraded_variants.join(", ")
        )
    };
    out.push_str(&format!(
        "    gMsg = {}\n    BuildMod{index} = \"{}\"\n    Exit Function\n\nExFail:\n    BuildMod{index} = \"failed\"\n    Exit Function\n\nHardFail:\n    gMsg = \"err \" & Trim(Str(Err.Number)) & \": \" & Err.Description\n    On Error Resume Next\n    BuildMod{index} = \"failed\"\nEnd Function\n\n",
        vb_lit(&format!("created{degrade_note}")),
        if md.degraded_variants.is_empty() { "created" } else { "degraded" }
    ));
    out
}

/// A validated dimension readback against the expected SI value.
fn dim_check(which: usize, expect: f64, what: &str) -> String {
    format!(
        "    If feat Is Nothing Then gMsg = \"the feature was not created\": GoTo ExFail\n    Set dm = swDoc.Parameter(\"D{which}@\" & feat.Name)\n    If dm Is Nothing Then gMsg = \"the {what} dimension is missing\": GoTo ExFail\n    If Abs(dm.SystemValue - {}) > 0.000001 Then gMsg = \"the {what} does not match\": GoTo ExFail\n",
        vba_num(expect)
    )
}

/// A feature dimension equation bound to D{which} (a literal index), under
/// the sign contract: the dimension carries |expect|, the equation right-hand
/// side keeps source semantics, signVal folds the sign into the stored text
/// and the registered regSign normalizes downstream SlotRef references.
fn dim_eq(which: usize, eq: &crate::replay::DimEquation, what: &str, fail: &str) -> String {
    format!(
        "    Set dm = swDoc.Parameter(\"D{which}@\" & feat.Name)\n    If dm Is Nothing Then gMsg = \"the {what} dimension is missing\": GoTo ExFail\n    If Not AddDimEquation(dm, {}, {}, {}, {}, feat.Name, 1#) Then gMsg = \"{fail}\": GoTo ExFail\n",
        rhs_vba(eq),
        vba_num(eq.expect_si.abs()),
        eq.slot,
        sign_lit(eq.expect_si)
    )
}

/// A dimension readback on a RUNTIME-chosen dimension index (a Long local,
/// concatenated locale-independent through Trim(Str(...))): the swap-aware
/// D1/D2 bindings of the direction truth table.
fn dim_check_dyn(which_local: &str, expect: f64, what: &str) -> String {
    format!(
        "    Set dm = swDoc.Parameter(\"D\" & Trim(Str({which_local})) & \"@\" & feat.Name)\n    If dm Is Nothing Then gMsg = \"the {what} dimension is missing\": GoTo ExFail\n    If Abs(dm.SystemValue - {}) > 0.000001 Then gMsg = \"the {what} does not match\": GoTo ExFail\n",
        vba_num(expect)
    )
}

/// A dimension equation bound to a runtime-chosen dimension index (the
/// swap-aware binding), under the same sign contract as [`dim_eq`].
fn dim_eq_dyn(
    which_local: &str,
    eq: &crate::replay::DimEquation,
    what: &str,
    fail: &str,
) -> String {
    format!(
        "    Set dm = swDoc.Parameter(\"D\" & Trim(Str({which_local})) & \"@\" & feat.Name)\n    If dm Is Nothing Then gMsg = \"the {what} dimension is missing\": GoTo ExFail\n    If Not AddDimEquation(dm, {}, {}, {}, {}, feat.Name, 1#) Then gMsg = \"{fail}\": GoTo ExFail\n",
        rhs_vba(eq),
        vba_num(eq.expect_si.abs()),
        eq.slot,
        sign_lit(eq.expect_si)
    )
}

/// Mirrors about origin planes: the seeds are verified native features
/// (live bindings, fail-closed), the plane is selected like a sketch plane,
/// and the mirror call shape carries the checkpoint-pending mark.
fn emit_pat_subs(index: usize, pt: &crate::replay::PatStep) -> String {
    use crate::replay::{PatKind, RhsSeg};
    let _ = RhsSeg::Text(String::new());
    let mut out = String::new();
    out.push_str(&format!("' Pattern step {index}: {}\n", vb_lit(&pt.name)));
    out.push_str(&format!(
        "Function BuildPat{index}() As String\n    On Error GoTo HardFail\n    Dim ok As Boolean\n    Dim feat As SldWorks.Feature\n    Dim dm As SldWorks.Dimension\n    Dim pf As SldWorks.Feature\n    Dim uncertain As Boolean\n"
    ));
    match &pt.kind {
        PatKind::Mirror { seeds, plane } => {
            // Live seed bindings first: never pattern a body that merely
            // stands in for a feature.
            for seed in seeds {
                out.push_str(&format!(
                    "    If feat{seed} Is Nothing Then gMsg = \"a seed feature did not verify\": GoTo ExFail\n"
                ));
            }
            let code = match plane {
                crate::replay::OriginPlane::Xy => "XY",
                crate::replay::OriginPlane::Xz => "XZ",
                crate::replay::OriginPlane::Yz => "YZ",
            };
            out.push_str(&format!("    gPlane = PlaneNameFor(\"{code}\")\n"));
            out.push_str(&format!(
                "    If gPlane = \"\" Then gMsg = \"no origin plane carries the {code} frame {desc}; probed: \" & gProbeLog & \"; tree scan saw: \" & TreeSummary(): GoTo ExFail\n",
                desc = frame_desc(solidcraft_frame(*plane))
            ));
            out.push_str("    Set pf = FindFeatureByName(gPlane, uncertain)\n    ok = False\n    If Not pf Is Nothing Then ok = pf.Select(False)\n    If ok = False Then ok = swDoc.Extension.SelectByID2(gPlane, \"PLANE\", 0#, 0#, 0#, False, 0, Nothing, swSelectOption_e.swSelectOptionDefault)\n");
            out.push_str("    If ok = False Then gMsg = \"the mirror plane could not be selected: gPlane=<\" & gPlane & \"> object-found=\" & CStr(Not pf Is Nothing) & \"; tree scan saw: \" & TreeSummary(): GoTo ExFail\n");
            for seed in seeds {
                out.push_str(&format!("    feat{seed}.Select True\n"));
            }
            out.push_str("    ' feature call shape pending checkpoint verification\n");
            out.push_str("    Set feat = gFeatMgr.FeatureMirror(True, False, False, 0#, 0#)\n");
            out.push_str(
                "    If feat Is Nothing Then gMsg = \"the mirror was not created\": GoTo ExFail\n",
            );
        }
        PatKind::Rectangular { .. } => {
            out.push_str("    gMsg = \"feature patterns await direction selection (ticket #6 remainder)\": GoTo ExFail\n");
        }
    }
    out.push_str("    swDoc.ClearSelection2 True\n");
    let degrade_note = if pt.degraded_variants.is_empty() {
        String::new()
    } else {
        format!(
            "; variants not replayed: {}",
            pt.degraded_variants.join(", ")
        )
    };
    out.push_str(&format!(
        "    gMsg = {}\n    BuildPat{index} = \"{}\"\n    Exit Function\n\nExFail:\n    BuildPat{index} = \"failed\"\n    Exit Function\n\nHardFail:\n    gMsg = \"err \" & Trim(Str(Err.Number)) & \": \" & Err.Description\n    On Error Resume Next\n    BuildPat{index} = \"failed\"\nEnd Function\n\n",
        vb_lit(&format!("created{degrade_note}")),
        if pt.degraded_variants.is_empty() { "created" } else { "degraded" }
    ));
    out
}

/// The fallback builder for a snapshot-covered step: restore the pre-step
/// state, import the boundary snapshot body, invalidate stale bindings.
fn emit_fallback_subs(index: usize, sidecar: usize) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "' Fallback step {index}: boundary snapshot import\n\
Function Fallback{index}() As String\n\
    On Error GoTo HardFail\n\
    Dim ok As Boolean\n\
    DeleteCreated\n\
    ok = swDoc.Extension.ImportFile(MacroDirFromHost() & \"\\fallback-step-{sidecar:04}.step\", \"\")\n\
    If ok = False Then gMsg = \"the fallback body could not be imported\": Fallback{index} = \"failed\": Exit Function\n"
    ));
    out.push_str("    ClearSlotsAll\n");
    out.push_str(&format!(
        "    gMsg = \"fallback body imported from the boundary snapshot\"\n    Fallback{index} = \"fallback\"\n    Exit Function\n\nHardFail:\n    gMsg = \"err \" & Trim(Str(Err.Number)) & \": \" & Err.Description\n    On Error Resume Next\n    Fallback{index} = \"failed\"\nEnd Function\n\n"
    ));
    out
}

/// Delete every feature and sketch the macro has created so far (restore to
/// the pre-step state) and clear their live bindings.
fn emit_delete_created(steps: &[PlannedStep]) -> String {
    let mut out = String::new();
    out.push_str("' Restore the pre-step state: delete everything created so far.\nSub DeleteCreated()\n    On Error Resume Next\n    Dim sel As Boolean\n    Dim selOk As Boolean\n");
    for step in steps.iter().rev() {
        match &step.work {
            Work::Extrude(_) | Work::Revolve(_) | Work::Primitive(_) | Work::Modifier(_) => {
                out.push_str(&format!(
                    "    If Not feat{idx} Is Nothing Then feat{idx}.Select False: swDoc.EditDelete: Set feat{idx} = Nothing\n",
                    idx = step.index
                ));
            }
            Work::Sketch(_) => {
                out.push_str(&format!(
                    "    If skName{idx} <> \"\" Then\n        selOk = False\n        If Not skFeat{idx} Is Nothing Then selOk = skFeat{idx}.Select(False)\n        If Not selOk Then selOk = swDoc.Extension.SelectByID2(skName{idx}, \"SKETCH\", 0#, 0#, 0#, False, 0, Nothing, swSelectOption_e.swSelectOptionDefault)\n        If selOk Then swDoc.EditDelete\n        Set skFeat{idx} = Nothing\n        skName{idx} = \"\"\n        ClearMap {idx}\n    End If\n",
                    idx = step.index
                ));
            }
            _ => {}
        }
    }
    out.push_str("    swDoc.ClearSelection2 True\nEnd Sub\n\n");
    out
}
