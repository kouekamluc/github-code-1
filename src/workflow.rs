#[derive(Clone, Copy)]
pub(crate) enum WorkflowKind {
    SurveyCampaign,
    Alert,
    MaintenanceTicket,
    Decision,
    ExecutionPlan,
    Asset,
}

impl WorkflowKind {
    pub(crate) fn entity_type(self) -> &'static str {
        match self {
            Self::SurveyCampaign => "survey_campaign",
            Self::Alert => "alert",
            Self::MaintenanceTicket => "maintenance_ticket",
            Self::Decision => "decision_snapshot",
            Self::ExecutionPlan => "execution_plan",
            Self::Asset => "infrastructure_asset",
        }
    }

    pub(crate) fn field_name(self) -> &'static str {
        match self {
            Self::Decision => "decision_stage",
            _ => "status",
        }
    }
}

pub(crate) fn validate_transition(
    kind: WorkflowKind,
    current: &str,
    next: &str,
) -> Result<(), String> {
    let current = current.trim();
    let next = next.trim();
    if current == next {
        return Ok(());
    }

    let allowed = match kind {
        WorkflowKind::SurveyCampaign => survey_campaign_transition(current, next),
        WorkflowKind::Alert => alert_transition(current, next),
        WorkflowKind::MaintenanceTicket => ticket_transition(current, next),
        WorkflowKind::Decision => decision_transition(current, next),
        WorkflowKind::ExecutionPlan => execution_plan_transition(current, next),
        WorkflowKind::Asset => asset_transition(current, next),
    };

    if allowed {
        Ok(())
    } else {
        Err(format!(
            "Cannot move {} from '{}' to '{}'.",
            kind.entity_type(),
            current,
            next
        ))
    }
}

pub(crate) fn validate_ticket_completion(
    next: &str,
    resolution_notes: Option<&str>,
) -> Result<(), String> {
    if matches!(next, "done" | "completed")
        && resolution_notes
            .map(|notes| notes.trim().is_empty())
            .unwrap_or(true)
    {
        return Err("Completed tickets require resolution notes.".into());
    }
    Ok(())
}

pub(crate) fn validate_execution_completion(
    next: &str,
    outcome_notes: Option<&str>,
) -> Result<(), String> {
    if next == "completed"
        && outcome_notes
            .map(|notes| notes.trim().is_empty())
            .unwrap_or(true)
    {
        return Err("Completed execution plans require outcome notes.".into());
    }
    Ok(())
}

pub(crate) fn validate_decision_approval(
    next_stage: &str,
    evidence_score: f64,
    recommended_budget_xaf: Option<i64>,
    approval_notes: Option<&str>,
) -> Result<(), String> {
    if next_stage != "approved" {
        return Ok(());
    }

    if evidence_score < 60.0 {
        return Err("Approved decisions require an evidence score of at least 60.".into());
    }

    if recommended_budget_xaf.unwrap_or(0) >= 3_000_000
        && approval_notes
            .map(|notes| notes.trim().is_empty())
            .unwrap_or(true)
    {
        return Err("High-budget approvals require approval notes.".into());
    }

    Ok(())
}

pub(crate) fn validate_execution_plan_creation(
    decision_stage: &str,
    evidence_score: f64,
) -> Result<(), String> {
    if decision_stage != "approved" {
        return Err("Execution plans can only be created from approved decisions.".into());
    }
    if evidence_score < 60.0 {
        return Err("Execution plans require a decision evidence score of at least 60.".into());
    }
    Ok(())
}

fn survey_campaign_transition(current: &str, next: &str) -> bool {
    matches!(
        (current, next),
        ("draft", "ready" | "paused" | "cancelled")
            | ("ready", "draft" | "in_field" | "paused" | "cancelled")
            | ("in_field" | "active", "reviewing" | "paused" | "cancelled")
            | ("reviewing", "in_field" | "completed" | "paused")
            | ("paused", "ready" | "in_field" | "cancelled")
    )
}

fn alert_transition(current: &str, next: &str) -> bool {
    matches!(
        (current, next),
        ("open", "acknowledged" | "ticketed" | "resolved")
            | ("acknowledged", "open" | "ticketed" | "resolved")
            | ("ticketed", "open" | "resolved")
    )
}

fn ticket_transition(current: &str, next: &str) -> bool {
    matches!(
        (current, next),
        ("open", "assigned" | "in_progress" | "blocked" | "cancelled")
            | ("assigned", "in_progress" | "blocked" | "cancelled")
            | (
                "in_progress",
                "blocked" | "done" | "completed" | "cancelled"
            )
            | ("blocked", "assigned" | "in_progress" | "cancelled")
    )
}

fn decision_transition(current: &str, next: &str) -> bool {
    matches!(
        (current, next),
        ("draft", "recommended" | "blocked")
            | ("recommended", "draft" | "approved" | "blocked")
            | ("approved", "executing" | "blocked")
            | ("executing", "completed" | "blocked")
            | ("blocked", "draft" | "recommended")
    )
}

fn execution_plan_transition(current: &str, next: &str) -> bool {
    matches!(
        (current, next),
        ("planned", "ready" | "blocked")
            | ("ready", "in_progress" | "blocked")
            | ("in_progress", "blocked" | "completed")
            | ("blocked", "planned" | "ready" | "in_progress")
    )
}

fn asset_transition(current: &str, next: &str) -> bool {
    matches!(
        (current, next),
        ("online", "warning" | "critical" | "offline")
            | ("warning", "online" | "critical" | "offline")
            | ("critical", "warning" | "offline" | "online")
            | ("offline", "warning" | "critical" | "online")
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn survey_campaign_blocks_skipping_review() {
        let result = validate_transition(WorkflowKind::SurveyCampaign, "in_field", "completed");
        assert!(result.is_err());
    }

    #[test]
    fn survey_campaign_allows_review_completion() {
        let result = validate_transition(WorkflowKind::SurveyCampaign, "reviewing", "completed");
        assert!(result.is_ok());
    }

    #[test]
    fn ticket_completion_requires_resolution_notes() {
        let result = validate_ticket_completion("done", Some(" "));
        assert!(result.is_err());
    }

    #[test]
    fn decision_approval_requires_enough_evidence() {
        let result = validate_decision_approval("approved", 59.0, Some(500_000), Some("Approved"));
        assert!(result.is_err());
    }

    #[test]
    fn high_budget_decision_approval_requires_notes() {
        let result = validate_decision_approval("approved", 80.0, Some(3_500_000), None);
        assert!(result.is_err());
    }

    #[test]
    fn execution_plan_requires_approved_decision() {
        let result = validate_execution_plan_creation("recommended", 80.0);
        assert!(result.is_err());
    }

    #[test]
    fn execution_completion_requires_outcome_notes() {
        let result = validate_execution_completion("completed", None);
        assert!(result.is_err());
    }
}
