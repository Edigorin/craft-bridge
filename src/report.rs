//! The plan report: the deterministic *prediction* of a rebuild, rendered
//! from the plan. The runtime report the macro writes is the *truth*. The two
//! are separate artifacts and are never merged.

use serde::Serialize;

use crate::replay::{PlanStatus, PlannedStep};

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PlanReport {
    pub artifact: String,
    pub macro_name: String,
    pub profile: String,
    pub steps: Vec<PlanStepReport>,
    pub counts: PlanCounts,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PlanStepReport {
    pub index: usize,
    /// Source-side identity (a solidcraft/57 feature id or parameter
    /// name). Empty for legacy script steps; omitted there so legacy
    /// report goldens stay byte-identical.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub source_id: String,
    pub command: String,
    pub status: PlanStatus,
    pub detail: String,
    /// Driver notes that are not degrade triggers (zero coordinates,
    /// expression/cache disagreements). Omitted when empty.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Default)]
pub struct PlanCounts {
    pub mapped: usize,
    pub degrade_planned: usize,
    pub fallback_planned: usize,
    pub not_executed: usize,
}

/// Build the plan report from the plan. Deterministic: no time, no machine
/// state, only the plan.
pub fn build_report(steps: &[PlannedStep], artifact: &str, macro_name: &str) -> PlanReport {
    let mut counts = PlanCounts::default();
    for step in steps {
        match step.status {
            PlanStatus::Mapped => counts.mapped += 1,
            PlanStatus::DegradePlanned => counts.degrade_planned += 1,
            PlanStatus::FallbackPlanned => counts.fallback_planned += 1,
            PlanStatus::NotExecuted => counts.not_executed += 1,
        }
    }
    PlanReport {
        artifact: artifact.to_string(),
        macro_name: macro_name.to_string(),
        profile: crate::profile::PROFILE_ID.to_string(),
        steps: steps
            .iter()
            .map(|s| PlanStepReport {
                index: s.index,
                source_id: s.source_id.clone(),
                command: s.command.clone(),
                status: s.status,
                detail: s.detail.clone(),
                notes: match &s.work {
                    crate::replay::Work::Sketch(sk) => sk.driver_notes.clone(),
                    _ => Vec::new(),
                },
            })
            .collect(),
        counts,
    }
}

/// The human rendering, pinned by golden tests.
pub fn render_text(report: &PlanReport) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "plan for {} -> {}\nprofile: {}\n",
        report.artifact, report.macro_name, report.profile
    ));
    out.push_str(&format!(
        "counts: {} mapped, {} degrade-planned, {} fallback-planned, {} not-executed\n\n",
        report.counts.mapped,
        report.counts.degrade_planned,
        report.counts.fallback_planned,
        report.counts.not_executed
    ));
    for step in &report.steps {
        if !step.source_id.is_empty() {
            out.push_str(&format!("       source: {}\n", step.source_id));
        }
        for note in &step.notes {
            out.push_str(&format!("       note: {note}\n"));
        }
        out.push_str(&format!(
            "  {:>3}  {:<24} {:<16} {}\n",
            step.index,
            step.command,
            serde_json::to_value(step.status)
                .ok()
                .and_then(|v| v.as_str().map(str::to_string))
                .unwrap_or_default(),
            step.detail
        ));
    }
    out.push_str("\nThis is a prediction. The runtime report the macro writes is the truth.\n");
    out
}
