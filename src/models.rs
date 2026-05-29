use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::FromRow;

#[derive(FromRow)]
pub(crate) struct DbLocation {
    pub(crate) pcode: Option<String>,
    pub(crate) region: String,
    pub(crate) department: String,
    pub(crate) commune: String,
    pub(crate) location: String,
    pub(crate) latitude: f64,
    pub(crate) longitude: f64,
    pub(crate) area_sqkm: Option<f64>,
    pub(crate) phone_owners: Option<i64>,
    pub(crate) population: Option<i64>,
    pub(crate) data_source: String,
    pub(crate) updated_at: String,
}

#[derive(Serialize, Clone)]
pub(crate) struct LocationStat {
    pub(crate) pcode: Option<String>,
    pub(crate) region: String,
    pub(crate) department: String,
    pub(crate) commune: String,
    pub(crate) location: String,
    pub(crate) latitude: f64,
    pub(crate) longitude: f64,
    pub(crate) area_sqkm: Option<f64>,
    pub(crate) phone_owners: i64,
    pub(crate) population: i64,
    pub(crate) phone_rate: f64,
    pub(crate) metric_source: String,
    pub(crate) confidence: f64,
    pub(crate) urban_signal: f64,
    pub(crate) data_source: String,
    pub(crate) updated_at: String,
}

#[derive(Serialize)]
pub(crate) struct ApiError {
    pub(crate) message: String,
}

#[derive(Serialize)]
pub(crate) struct UserContext {
    pub(crate) actor: String,
    pub(crate) display_name: Option<String>,
    pub(crate) role: String,
    pub(crate) permissions: Vec<String>,
    pub(crate) authenticated: bool,
}

#[derive(Deserialize)]
pub(crate) struct LoginRequest {
    pub(crate) login: String,
    pub(crate) password: String,
}

#[derive(Serialize)]
pub(crate) struct LoginResponse {
    pub(crate) token: String,
    pub(crate) actor: String,
    pub(crate) display_name: String,
    pub(crate) role: String,
    pub(crate) permissions: Vec<String>,
}

#[derive(FromRow)]
pub(crate) struct AuthUser {
    pub(crate) id: i64,
    pub(crate) email: String,
    pub(crate) display_name: String,
    pub(crate) role: String,
    pub(crate) password_hash: String,
    pub(crate) is_active: bool,
}

#[derive(FromRow)]
pub(crate) struct AuthSessionUser {
    pub(crate) email: String,
    pub(crate) display_name: String,
    pub(crate) role: String,
}

#[derive(Serialize, FromRow)]
pub(crate) struct AuditEvent {
    pub(crate) id: i64,
    pub(crate) entity_type: String,
    pub(crate) entity_id: i64,
    pub(crate) field_name: String,
    pub(crate) old_value: Option<String>,
    pub(crate) new_value: Option<String>,
    pub(crate) actor: String,
    pub(crate) note: Option<String>,
    pub(crate) created_at: String,
}

#[derive(Deserialize)]
pub(crate) struct AuditEventQuery {
    pub(crate) entity_type: Option<String>,
    pub(crate) entity_id: Option<i64>,
    pub(crate) limit: Option<i64>,
}

#[derive(Serialize, FromRow, Clone)]
pub(crate) struct EvidenceFile {
    pub(crate) id: i64,
    pub(crate) entity_type: String,
    pub(crate) entity_id: i64,
    pub(crate) file_name: String,
    pub(crate) content_type: String,
    pub(crate) storage_path: String,
    pub(crate) sha256_hash: String,
    pub(crate) file_size: i64,
    pub(crate) latitude: Option<f64>,
    pub(crate) longitude: Option<f64>,
    pub(crate) captured_at: Option<String>,
    pub(crate) uploaded_by: String,
    pub(crate) created_at: String,
}

#[derive(Deserialize)]
pub(crate) struct EvidenceQuery {
    pub(crate) entity_type: String,
    pub(crate) entity_id: i64,
}

#[derive(Deserialize)]
pub(crate) struct EvidenceUploadRequest {
    pub(crate) entity_type: String,
    pub(crate) entity_id: i64,
    pub(crate) file_name: String,
    pub(crate) content_type: String,
    pub(crate) content_base64: String,
    pub(crate) latitude: Option<f64>,
    pub(crate) longitude: Option<f64>,
    pub(crate) captured_at: Option<String>,
}

#[derive(Serialize)]
pub(crate) struct EntityDetail {
    pub(crate) entity_type: String,
    pub(crate) entity_id: i64,
    pub(crate) record: Value,
    pub(crate) evidence: Vec<EvidenceFile>,
    pub(crate) audit_events: Vec<AuditEvent>,
}

#[derive(Serialize)]
pub(crate) struct Summary {
    pub(crate) total_phone_owners: i64,
    pub(crate) total_population: i64,
    pub(crate) percent_with_phone: f64,
    pub(crate) region_count: i64,
    pub(crate) department_count: i64,
    pub(crate) commune_count: i64,
    pub(crate) measured_location_count: i64,
    pub(crate) estimated_location_count: i64,
}

#[derive(Serialize, FromRow, Clone)]
pub(crate) struct Organization {
    pub(crate) id: i64,
    pub(crate) name: String,
    pub(crate) org_type: String,
    pub(crate) contact_name: Option<String>,
    pub(crate) contact_email: Option<String>,
    pub(crate) created_at: String,
}

#[derive(Deserialize)]
pub(crate) struct OrganizationRequest {
    pub(crate) name: String,
    pub(crate) org_type: String,
    pub(crate) contact_name: Option<String>,
    pub(crate) contact_email: Option<String>,
}

#[derive(Serialize, FromRow, Clone)]
pub(crate) struct Project {
    pub(crate) id: i64,
    pub(crate) organization_id: Option<i64>,
    pub(crate) organization_name: Option<String>,
    pub(crate) name: String,
    pub(crate) sector: String,
    pub(crate) region: Option<String>,
    pub(crate) status: String,
    pub(crate) start_date: Option<String>,
    pub(crate) created_at: String,
}

#[derive(Deserialize)]
pub(crate) struct ProjectRequest {
    pub(crate) organization_id: Option<i64>,
    pub(crate) name: String,
    pub(crate) sector: String,
    pub(crate) region: Option<String>,
    pub(crate) status: String,
    pub(crate) start_date: Option<String>,
}

#[derive(Serialize, FromRow, Clone)]
pub(crate) struct InfrastructureAsset {
    pub(crate) id: i64,
    pub(crate) project_id: Option<i64>,
    pub(crate) site_profile_id: Option<i64>,
    pub(crate) project_name: Option<String>,
    pub(crate) site_name: Option<String>,
    pub(crate) asset_type: String,
    pub(crate) name: String,
    pub(crate) region: String,
    pub(crate) department: String,
    pub(crate) commune: String,
    pub(crate) latitude: f64,
    pub(crate) longitude: f64,
    pub(crate) status: String,
    pub(crate) operator: Option<String>,
    pub(crate) installed_at: Option<String>,
    pub(crate) last_checked_at: Option<String>,
    pub(crate) notes: Option<String>,
}

#[derive(Deserialize)]
pub(crate) struct AssetRequest {
    pub(crate) project_id: Option<i64>,
    pub(crate) site_profile_id: Option<i64>,
    pub(crate) asset_type: String,
    pub(crate) name: String,
    pub(crate) region: String,
    pub(crate) department: String,
    pub(crate) commune: String,
    pub(crate) latitude: f64,
    pub(crate) longitude: f64,
    pub(crate) status: String,
    pub(crate) operator: Option<String>,
    pub(crate) installed_at: Option<String>,
    pub(crate) notes: Option<String>,
}

#[derive(Deserialize)]
pub(crate) struct AssetStatusRequest {
    pub(crate) status: String,
    pub(crate) notes: Option<String>,
}

#[derive(Serialize)]
pub(crate) struct SignalProbeHealth {
    pub(crate) asset_id: i64,
    pub(crate) health_score: f64,
    pub(crate) health_label: String,
    pub(crate) open_alerts: i64,
    pub(crate) active_tickets: i64,
    pub(crate) report_count: i64,
    pub(crate) reading_count: i64,
    pub(crate) latest_reading: Option<String>,
    pub(crate) recommended_action: String,
}

#[derive(Serialize)]
pub(crate) struct SignalProbeDashboard {
    pub(crate) total_probes: i64,
    pub(crate) online_probes: i64,
    pub(crate) warning_probes: i64,
    pub(crate) critical_probes: i64,
    pub(crate) offline_probes: i64,
    pub(crate) open_alerts: i64,
    pub(crate) active_tickets: i64,
    pub(crate) health: Vec<SignalProbeHealth>,
}

#[derive(Serialize, FromRow, Clone)]
pub(crate) struct FieldReport {
    pub(crate) id: i64,
    pub(crate) project_id: Option<i64>,
    pub(crate) site_profile_id: Option<i64>,
    pub(crate) campaign_id: Option<i64>,
    pub(crate) asset_id: Option<i64>,
    pub(crate) project_name: Option<String>,
    pub(crate) site_name: Option<String>,
    pub(crate) campaign_name: Option<String>,
    pub(crate) report_type: String,
    pub(crate) region: String,
    pub(crate) department: String,
    pub(crate) commune: String,
    pub(crate) latitude: f64,
    pub(crate) longitude: f64,
    pub(crate) status: String,
    pub(crate) evidence_quality: String,
    pub(crate) notes: String,
    pub(crate) submitted_by: String,
    pub(crate) created_at: String,
}

#[derive(Deserialize)]
pub(crate) struct FieldReportRequest {
    pub(crate) project_id: Option<i64>,
    pub(crate) site_profile_id: Option<i64>,
    pub(crate) campaign_id: Option<i64>,
    pub(crate) asset_id: Option<i64>,
    pub(crate) report_type: String,
    pub(crate) region: String,
    pub(crate) department: String,
    pub(crate) commune: String,
    pub(crate) latitude: f64,
    pub(crate) longitude: f64,
    pub(crate) status: String,
    pub(crate) evidence_quality: Option<String>,
    pub(crate) notes: String,
    pub(crate) submitted_by: String,
}

#[derive(Serialize, FromRow, Clone)]
pub(crate) struct Alert {
    pub(crate) id: i64,
    pub(crate) project_id: Option<i64>,
    pub(crate) site_profile_id: Option<i64>,
    pub(crate) asset_id: Option<i64>,
    pub(crate) project_name: Option<String>,
    pub(crate) site_name: Option<String>,
    pub(crate) severity: String,
    pub(crate) title: String,
    pub(crate) message: String,
    pub(crate) status: String,
    pub(crate) created_at: String,
    pub(crate) resolved_at: Option<String>,
}

#[derive(Deserialize)]
pub(crate) struct AlertRequest {
    pub(crate) project_id: Option<i64>,
    pub(crate) site_profile_id: Option<i64>,
    pub(crate) asset_id: Option<i64>,
    pub(crate) severity: String,
    pub(crate) title: String,
    pub(crate) message: String,
}

#[derive(Deserialize)]
pub(crate) struct AlertStatusRequest {
    pub(crate) status: String,
}

#[derive(Serialize, FromRow, Clone)]
pub(crate) struct MaintenanceTicket {
    pub(crate) id: i64,
    pub(crate) project_id: Option<i64>,
    pub(crate) site_profile_id: Option<i64>,
    pub(crate) asset_id: Option<i64>,
    pub(crate) alert_id: Option<i64>,
    pub(crate) project_name: Option<String>,
    pub(crate) site_name: Option<String>,
    pub(crate) title: String,
    pub(crate) priority: String,
    pub(crate) status: String,
    pub(crate) assigned_to: Option<String>,
    pub(crate) due_date: Option<String>,
    pub(crate) sla_hours: Option<i32>,
    pub(crate) resolution_notes: Option<String>,
    pub(crate) created_at: String,
    pub(crate) updated_at: String,
}

#[derive(Deserialize)]
pub(crate) struct MaintenanceTicketRequest {
    pub(crate) project_id: Option<i64>,
    pub(crate) site_profile_id: Option<i64>,
    pub(crate) asset_id: Option<i64>,
    pub(crate) alert_id: Option<i64>,
    pub(crate) title: String,
    pub(crate) priority: String,
    pub(crate) assigned_to: Option<String>,
    pub(crate) due_date: Option<String>,
    pub(crate) sla_hours: Option<i32>,
}

#[derive(Deserialize)]
pub(crate) struct MaintenanceTicketStatusRequest {
    pub(crate) status: String,
    pub(crate) resolution_notes: Option<String>,
}

#[derive(Serialize, FromRow, Clone)]
pub(crate) struct IotReading {
    pub(crate) id: i64,
    pub(crate) project_id: Option<i64>,
    pub(crate) site_profile_id: Option<i64>,
    pub(crate) asset_id: i64,
    pub(crate) project_name: Option<String>,
    pub(crate) site_name: Option<String>,
    pub(crate) reading_type: String,
    pub(crate) value: f64,
    pub(crate) unit: String,
    pub(crate) latitude: Option<f64>,
    pub(crate) longitude: Option<f64>,
    pub(crate) created_at: String,
}

#[derive(Deserialize)]
pub(crate) struct IotReadingRequest {
    pub(crate) project_id: Option<i64>,
    pub(crate) site_profile_id: Option<i64>,
    pub(crate) asset_id: i64,
    pub(crate) reading_type: String,
    pub(crate) value: f64,
    pub(crate) unit: String,
    pub(crate) latitude: Option<f64>,
    pub(crate) longitude: Option<f64>,
}

#[derive(Serialize, FromRow, Clone)]
pub(crate) struct OperatorImeiEvent {
    pub(crate) id: i64,
    pub(crate) operator_name: String,
    pub(crate) imei_hash: String,
    pub(crate) imei_last4: Option<String>,
    pub(crate) device_type: Option<String>,
    pub(crate) event_type: String,
    pub(crate) compliance_status: String,
    pub(crate) region: Option<String>,
    pub(crate) department: Option<String>,
    pub(crate) commune: Option<String>,
    pub(crate) source_system: String,
    pub(crate) raw_reference: Option<String>,
    pub(crate) network_first_seen_at: Option<String>,
    pub(crate) created_at: String,
}

#[derive(Deserialize)]
pub(crate) struct OperatorImeiEventRequest {
    pub(crate) operator_name: String,
    pub(crate) imei: Option<String>,
    pub(crate) imei_hash: Option<String>,
    pub(crate) device_type: Option<String>,
    pub(crate) event_type: String,
    pub(crate) compliance_status: String,
    pub(crate) region: Option<String>,
    pub(crate) department: Option<String>,
    pub(crate) commune: Option<String>,
    pub(crate) source_system: Option<String>,
    pub(crate) raw_reference: Option<String>,
    pub(crate) network_first_seen_at: Option<String>,
}

#[derive(Serialize)]
pub(crate) struct ImeiComplianceSummary {
    pub(crate) total_events: i64,
    pub(crate) cleared_events: i64,
    pub(crate) pending_events: i64,
    pub(crate) blocked_events: i64,
    pub(crate) unknown_events: i64,
    pub(crate) distinct_devices: i64,
    pub(crate) operators: Vec<String>,
    pub(crate) latest_events: Vec<OperatorImeiEvent>,
    pub(crate) regulatory_note: String,
}

#[derive(Serialize, Clone)]
pub(crate) struct PriorityZone {
    pub(crate) pcode: Option<String>,
    pub(crate) region: String,
    pub(crate) department: String,
    pub(crate) commune: String,
    pub(crate) latitude: f64,
    pub(crate) longitude: f64,
    pub(crate) population: i64,
    pub(crate) phone_rate: f64,
    pub(crate) confidence: f64,
    pub(crate) asset_count: i64,
    pub(crate) open_alert_count: i64,
    pub(crate) report_count: i64,
    pub(crate) priority_score: f64,
    pub(crate) priority_label: String,
}

#[derive(Serialize)]
pub(crate) struct DecisionReport {
    pub(crate) generated_for: String,
    pub(crate) summary: Summary,
    pub(crate) open_alerts: i64,
    pub(crate) monitored_assets: i64,
    pub(crate) field_reports: i64,
    pub(crate) active_tickets: i64,
    pub(crate) overdue_tickets: i64,
    pub(crate) top_priority_zones: Vec<PriorityZone>,
    pub(crate) recommendations: Vec<String>,
    pub(crate) market_realities: Vec<String>,
    pub(crate) workspace_health: WorkspaceHealth,
}

#[derive(Serialize)]
pub(crate) struct OverviewKpi {
    pub(crate) label: String,
    pub(crate) value: String,
    pub(crate) detail: String,
    pub(crate) tone: String,
}

#[derive(Serialize)]
pub(crate) struct OverviewOpportunity {
    pub(crate) region: String,
    pub(crate) department: String,
    pub(crate) commune: String,
    pub(crate) priority_score: f64,
    pub(crate) priority_label: String,
    pub(crate) population: i64,
    pub(crate) phone_rate: f64,
    pub(crate) confidence: f64,
    pub(crate) estimated_budget_xaf: i64,
    pub(crate) likely_reach: i64,
    pub(crate) recommended_channel: String,
    pub(crate) business_case: String,
    pub(crate) next_action: String,
}

#[derive(Serialize)]
pub(crate) struct OverviewAction {
    pub(crate) title: String,
    pub(crate) area: Option<String>,
    pub(crate) action_type: String,
    pub(crate) urgency: String,
    pub(crate) reason: String,
}

#[derive(Serialize)]
pub(crate) struct OverviewRisk {
    pub(crate) label: String,
    pub(crate) value: String,
    pub(crate) severity: String,
    pub(crate) mitigation: String,
}

#[derive(Serialize)]
pub(crate) struct OverviewIntelligence {
    pub(crate) generated_for: String,
    pub(crate) kpis: Vec<OverviewKpi>,
    pub(crate) top_opportunities: Vec<OverviewOpportunity>,
    pub(crate) action_queue: Vec<OverviewAction>,
    pub(crate) trust_risks: Vec<OverviewRisk>,
    pub(crate) market_readout: Vec<String>,
}

#[derive(Serialize)]
pub(crate) struct AreaEconomics {
    pub(crate) estimated_budget_xaf: i64,
    pub(crate) likely_reach: i64,
    pub(crate) channel_strategy: String,
    pub(crate) execution_risk: String,
    pub(crate) next_action: String,
    pub(crate) trust_gap: String,
}

#[derive(Serialize)]
pub(crate) struct AreaDossier {
    pub(crate) area: LocationStat,
    pub(crate) priority: Option<PriorityZone>,
    pub(crate) economics: AreaEconomics,
    pub(crate) assets: Vec<InfrastructureAsset>,
    pub(crate) sites: Vec<SiteProfile>,
    pub(crate) campaigns: Vec<SurveyCampaign>,
    pub(crate) reports: Vec<FieldReport>,
    pub(crate) alerts: Vec<Alert>,
    pub(crate) tickets: Vec<MaintenanceTicket>,
    pub(crate) readings: Vec<IotReading>,
    pub(crate) market_notes: Vec<String>,
}

#[derive(Deserialize)]
pub(crate) struct AreaDossierQuery {
    pub(crate) region: String,
    pub(crate) department: String,
    pub(crate) commune: String,
}

#[derive(Serialize, FromRow, Clone)]
pub(crate) struct SiteProfile {
    pub(crate) id: i64,
    pub(crate) project_id: Option<i64>,
    pub(crate) project_name: Option<String>,
    pub(crate) name: String,
    pub(crate) site_type: String,
    pub(crate) region: String,
    pub(crate) department: String,
    pub(crate) commune: String,
    pub(crate) latitude: f64,
    pub(crate) longitude: f64,
    pub(crate) beneficiary_estimate: Option<i64>,
    pub(crate) trust_signal: String,
    pub(crate) access_notes: Option<String>,
    pub(crate) created_at: String,
}

#[derive(Deserialize)]
pub(crate) struct SiteProfileRequest {
    pub(crate) project_id: Option<i64>,
    pub(crate) name: String,
    pub(crate) site_type: String,
    pub(crate) region: String,
    pub(crate) department: String,
    pub(crate) commune: String,
    pub(crate) latitude: f64,
    pub(crate) longitude: f64,
    pub(crate) beneficiary_estimate: Option<i64>,
    pub(crate) trust_signal: Option<String>,
    pub(crate) access_notes: Option<String>,
}

#[derive(Serialize, FromRow, Clone)]
pub(crate) struct SurveyCampaign {
    pub(crate) id: i64,
    pub(crate) project_id: Option<i64>,
    pub(crate) project_name: Option<String>,
    pub(crate) name: String,
    pub(crate) form_type: String,
    pub(crate) target_region: Option<String>,
    pub(crate) target_department: Option<String>,
    pub(crate) target_commune: Option<String>,
    pub(crate) status: String,
    pub(crate) language_mode: String,
    pub(crate) offline_enabled: bool,
    pub(crate) starts_on: Option<String>,
    pub(crate) ends_on: Option<String>,
    pub(crate) created_at: String,
}

#[derive(Deserialize)]
pub(crate) struct SurveyCampaignRequest {
    pub(crate) project_id: Option<i64>,
    pub(crate) name: String,
    pub(crate) form_type: String,
    pub(crate) target_region: Option<String>,
    pub(crate) target_department: Option<String>,
    pub(crate) target_commune: Option<String>,
    pub(crate) status: Option<String>,
    pub(crate) language_mode: Option<String>,
    pub(crate) offline_enabled: Option<bool>,
    pub(crate) starts_on: Option<String>,
    pub(crate) ends_on: Option<String>,
}

#[derive(Deserialize)]
pub(crate) struct SurveyCampaignStatusRequest {
    pub(crate) status: String,
}

#[derive(Serialize, FromRow, Clone)]
pub(crate) struct DecisionSnapshot {
    pub(crate) id: i64,
    pub(crate) project_id: Option<i64>,
    pub(crate) site_profile_id: Option<i64>,
    pub(crate) asset_id: Option<i64>,
    pub(crate) project_name: Option<String>,
    pub(crate) site_name: Option<String>,
    pub(crate) asset_name: Option<String>,
    pub(crate) title: String,
    pub(crate) decision_stage: String,
    pub(crate) priority_score: f64,
    pub(crate) recommended_budget_xaf: Option<i64>,
    pub(crate) owner_name: Option<String>,
    pub(crate) risk_level: String,
    pub(crate) evidence_score: f64,
    pub(crate) approval_notes: Option<String>,
    pub(crate) execution_status: String,
    pub(crate) rationale: String,
    pub(crate) next_action: String,
    pub(crate) created_at: String,
}

#[derive(Deserialize)]
pub(crate) struct DecisionSnapshotRequest {
    pub(crate) project_id: Option<i64>,
    pub(crate) site_profile_id: Option<i64>,
    pub(crate) asset_id: Option<i64>,
    pub(crate) title: String,
    pub(crate) decision_stage: Option<String>,
    pub(crate) priority_score: Option<f64>,
    pub(crate) recommended_budget_xaf: Option<i64>,
    pub(crate) owner_name: Option<String>,
    pub(crate) risk_level: Option<String>,
    pub(crate) evidence_score: Option<f64>,
    pub(crate) approval_notes: Option<String>,
    pub(crate) execution_status: Option<String>,
    pub(crate) rationale: Option<String>,
    pub(crate) next_action: Option<String>,
}

#[derive(Deserialize)]
pub(crate) struct DecisionStatusRequest {
    pub(crate) decision_stage: String,
    pub(crate) execution_status: Option<String>,
    pub(crate) approval_notes: Option<String>,
}

#[derive(Serialize)]
pub(crate) struct DecisionStageSummary {
    pub(crate) stage: String,
    pub(crate) count: i64,
    pub(crate) total_budget_xaf: i64,
    pub(crate) average_evidence_score: f64,
}

#[derive(Serialize)]
pub(crate) struct DecisionBoard {
    pub(crate) stages: Vec<DecisionStageSummary>,
    pub(crate) decisions: Vec<DecisionSnapshot>,
    pub(crate) recommendations: Vec<String>,
}

#[derive(Serialize, FromRow)]
pub(crate) struct ExecutionPlan {
    pub(crate) id: i64,
    pub(crate) decision_id: Option<i64>,
    pub(crate) decision_title: Option<String>,
    pub(crate) project_id: Option<i64>,
    pub(crate) site_profile_id: Option<i64>,
    pub(crate) asset_id: Option<i64>,
    pub(crate) project_name: Option<String>,
    pub(crate) site_name: Option<String>,
    pub(crate) asset_name: Option<String>,
    pub(crate) title: String,
    pub(crate) owner_name: Option<String>,
    pub(crate) status: String,
    pub(crate) budget_xaf: Option<i64>,
    pub(crate) planned_start: Option<String>,
    pub(crate) planned_end: Option<String>,
    pub(crate) local_focal_point_confirmed: bool,
    pub(crate) gps_photo_proof_required: bool,
    pub(crate) offline_survey_ready: bool,
    pub(crate) bilingual_script_ready: bool,
    pub(crate) transport_access_notes: Option<String>,
    pub(crate) xaf_budget_approved: bool,
    pub(crate) blocker: Option<String>,
    pub(crate) outcome_notes: Option<String>,
    pub(crate) created_at: String,
    pub(crate) updated_at: String,
}

#[derive(Deserialize)]
pub(crate) struct ExecutionPlanStatusRequest {
    pub(crate) status: String,
    pub(crate) local_focal_point_confirmed: Option<bool>,
    pub(crate) gps_photo_proof_required: Option<bool>,
    pub(crate) offline_survey_ready: Option<bool>,
    pub(crate) bilingual_script_ready: Option<bool>,
    pub(crate) xaf_budget_approved: Option<bool>,
    pub(crate) blocker: Option<String>,
    pub(crate) outcome_notes: Option<String>,
}

#[derive(Serialize)]
pub(crate) struct ExecutionStageSummary {
    pub(crate) status: String,
    pub(crate) count: i64,
    pub(crate) total_budget_xaf: i64,
    pub(crate) checklist_completion: f64,
}

#[derive(Serialize)]
pub(crate) struct ExecutionBoard {
    pub(crate) stages: Vec<ExecutionStageSummary>,
    pub(crate) plans: Vec<ExecutionPlan>,
    pub(crate) recommendations: Vec<String>,
}

#[derive(Serialize, Clone)]
pub(crate) struct WorkspaceHealth {
    pub(crate) organizations: i64,
    pub(crate) projects: i64,
    pub(crate) active_projects: i64,
    pub(crate) sites: i64,
    pub(crate) campaigns: i64,
    pub(crate) monitored_assets: i64,
    pub(crate) linked_iot_readings: i64,
    pub(crate) reports_generated: i64,
    pub(crate) open_alerts: i64,
    pub(crate) active_tickets: i64,
    pub(crate) decision_snapshots: i64,
    pub(crate) priority_opportunities: i64,
}

#[derive(Serialize)]
pub(crate) struct WorkspaceBusinessCard {
    pub(crate) label: String,
    pub(crate) value: String,
    pub(crate) detail: String,
    pub(crate) tone: String,
}

#[derive(Serialize)]
pub(crate) struct OrganizationIntelligence {
    pub(crate) organization: Organization,
    pub(crate) project_count: i64,
    pub(crate) linked_site_count: i64,
    pub(crate) active_decision_count: i64,
    pub(crate) open_alert_count: i64,
    pub(crate) last_activity: String,
}

#[derive(Serialize)]
pub(crate) struct ProjectIntelligence {
    pub(crate) project: Project,
    pub(crate) site_count: i64,
    pub(crate) campaign_count: i64,
    pub(crate) decision_count: i64,
    pub(crate) asset_count: i64,
    pub(crate) open_ticket_count: i64,
    pub(crate) execution_readiness: f64,
    pub(crate) recommended_next_action: String,
    pub(crate) latest_activity: String,
}

#[derive(Serialize)]
pub(crate) struct SiteProfileIntelligence {
    pub(crate) site: SiteProfile,
    pub(crate) linked_assets: i64,
    pub(crate) linked_reports: i64,
    pub(crate) linked_alerts: i64,
    pub(crate) linked_tickets: i64,
}

#[derive(Serialize)]
pub(crate) struct CampaignIntelligence {
    pub(crate) campaign: SurveyCampaign,
    pub(crate) submitted_reports: i64,
    pub(crate) field_validation_purpose: String,
}

#[derive(Serialize)]
pub(crate) struct WorkspaceActivity {
    pub(crate) action: String,
    pub(crate) related_entity: String,
    pub(crate) timestamp: String,
    pub(crate) source: String,
    pub(crate) description: String,
}

#[derive(Serialize)]
pub(crate) struct WorkspaceDashboard {
    pub(crate) health: WorkspaceHealth,
    pub(crate) business_cards: Vec<WorkspaceBusinessCard>,
    pub(crate) organizations: Vec<Organization>,
    pub(crate) organization_intelligence: Vec<OrganizationIntelligence>,
    pub(crate) projects: Vec<Project>,
    pub(crate) project_intelligence: Vec<ProjectIntelligence>,
    pub(crate) sites: Vec<SiteProfile>,
    pub(crate) site_intelligence: Vec<SiteProfileIntelligence>,
    pub(crate) campaigns: Vec<SurveyCampaign>,
    pub(crate) campaign_intelligence: Vec<CampaignIntelligence>,
    pub(crate) recent_decisions: Vec<DecisionSnapshot>,
    pub(crate) activity: Vec<WorkspaceActivity>,
    pub(crate) market_realities: Vec<String>,
}

#[derive(Deserialize)]
pub(crate) struct AreaActionRequest {
    pub(crate) action: String,
    pub(crate) region: String,
    pub(crate) department: String,
    pub(crate) commune: String,
    pub(crate) project_id: Option<i64>,
}

#[derive(Deserialize)]
pub(crate) struct WorkspaceTemplateApplyRequest {
    pub(crate) template_id: String,
    pub(crate) region: Option<String>,
    pub(crate) department: Option<String>,
    pub(crate) commune: Option<String>,
}

#[derive(Serialize, FromRow, Clone)]
pub(crate) struct WorkspaceTemplate {
    pub(crate) id: String,
    pub(crate) title: String,
    pub(crate) description: String,
    pub(crate) org_type: String,
    pub(crate) sector: String,
    pub(crate) site_type: String,
    pub(crate) form_type: String,
    pub(crate) trust_signal: String,
    pub(crate) default_project_status: String,
    pub(crate) language_mode: String,
    pub(crate) offline_enabled: bool,
    pub(crate) channel_strategy: String,
    pub(crate) target_segment: String,
    pub(crate) default_actions: Vec<String>,
    pub(crate) required_evidence: Vec<String>,
    pub(crate) creates_asset: bool,
    pub(crate) creates_report_task: bool,
    pub(crate) creates_alert: bool,
    pub(crate) creates_ticket: bool,
    pub(crate) active: bool,
    pub(crate) sort_order: i32,
}

#[derive(Serialize)]
pub(crate) struct ActionResult {
    pub(crate) message: String,
    pub(crate) created: Vec<String>,
    pub(crate) dashboard: WorkspaceDashboard,
}

#[derive(Serialize, Clone)]
pub(crate) struct PhoneMatrixAssumptions {
    pub(crate) adult_share: f64,
    pub(crate) national_adult_phone_ownership: f64,
    pub(crate) mobile_subscriptions_per_person: f64,
    pub(crate) priority_population_weight: f64,
    pub(crate) priority_gap_weight: f64,
    pub(crate) priority_confidence_weight: f64,
    pub(crate) priority_alert_weight: f64,
    pub(crate) assumption_version: String,
    pub(crate) last_updated: String,
}

#[derive(Serialize, Clone)]
pub(crate) struct PhoneMatrixBreakdown {
    pub(crate) population: i64,
    pub(crate) adult_share: f64,
    pub(crate) adult_ownership_rate: f64,
    pub(crate) regional_factor: f64,
    pub(crate) urban_rural_factor: f64,
    pub(crate) estimated_phone_owners_formula: String,
    pub(crate) maximum_owners_allowed: i64,
    pub(crate) confidence_level: String,
    pub(crate) confidence_reason: String,
    pub(crate) data_source: String,
}

#[derive(Serialize, Clone)]
pub(crate) struct PhoneMatrixRow {
    pub(crate) pcode: Option<String>,
    pub(crate) region: String,
    pub(crate) department: String,
    pub(crate) commune: String,
    pub(crate) location: String,
    pub(crate) latitude: f64,
    pub(crate) longitude: f64,
    pub(crate) area_sqkm: Option<f64>,
    pub(crate) population: i64,
    pub(crate) estimated_phone_owners: i64,
    pub(crate) estimated_mobile_subscriptions: i64,
    pub(crate) ownership_rate: f64,
    pub(crate) confidence: f64,
    pub(crate) confidence_level: String,
    pub(crate) confidence_reason: String,
    pub(crate) opportunity_score: f64,
    pub(crate) opportunity_level: String,
    pub(crate) priority_score: f64,
    pub(crate) priority_label: String,
    pub(crate) recommended_action: String,
    pub(crate) needs_validation: bool,
    pub(crate) validation_reason: String,
    pub(crate) data_source: String,
    pub(crate) method: String,
    pub(crate) last_updated: String,
    pub(crate) project_count: i64,
    pub(crate) site_count: i64,
    pub(crate) campaign_count: i64,
    pub(crate) report_count: i64,
    pub(crate) asset_count: i64,
    pub(crate) open_alert_count: i64,
}

#[derive(Serialize)]
pub(crate) struct PhoneMatrixSummary {
    pub(crate) total_population_analyzed: i64,
    pub(crate) estimated_phone_owners: i64,
    pub(crate) estimated_mobile_subscriptions: i64,
    pub(crate) average_ownership_rate: f64,
    pub(crate) high_opportunity_areas: i64,
    pub(crate) low_confidence_areas: i64,
    pub(crate) areas_needing_validation: i64,
    pub(crate) top_region_by_opportunity: String,
}

#[derive(Serialize)]
pub(crate) struct PhoneMatrixDashboard {
    pub(crate) summary: PhoneMatrixSummary,
    pub(crate) assumptions: PhoneMatrixAssumptions,
    pub(crate) rows: Vec<PhoneMatrixRow>,
}

#[derive(Deserialize)]
pub(crate) struct PhoneMatrixDetailQuery {
    pub(crate) region: String,
    pub(crate) department: String,
    pub(crate) commune: String,
}

#[derive(Serialize)]
pub(crate) struct PhoneMatrixDetail {
    pub(crate) row: PhoneMatrixRow,
    pub(crate) breakdown: PhoneMatrixBreakdown,
    pub(crate) related_projects: Vec<Project>,
    pub(crate) related_sites: Vec<SiteProfile>,
    pub(crate) related_campaigns: Vec<SurveyCampaign>,
    pub(crate) related_reports: Vec<FieldReport>,
    pub(crate) related_assets: Vec<InfrastructureAsset>,
    pub(crate) related_alerts: Vec<Alert>,
    pub(crate) related_tickets: Vec<MaintenanceTicket>,
}

#[derive(Deserialize)]
pub(crate) struct PhoneMatrixRecalculateRequest {
    pub(crate) scope: String,
    pub(crate) region: Option<String>,
    pub(crate) department: Option<String>,
    pub(crate) commune: Option<String>,
    pub(crate) limit: Option<usize>,
}

#[derive(Serialize)]
pub(crate) struct PhoneMatrixRecalculationLog {
    pub(crate) area: String,
    pub(crate) old_estimate: i64,
    pub(crate) new_estimate: i64,
    pub(crate) assumption_version: String,
    pub(crate) timestamp: String,
    pub(crate) triggered_by: String,
}

#[derive(Deserialize)]
pub(crate) struct UpdateLocationRequest {
    pub(crate) pcode: Option<String>,
    pub(crate) region: String,
    pub(crate) department: String,
    pub(crate) commune: String,
    pub(crate) location: String,
    pub(crate) latitude: f64,
    pub(crate) longitude: f64,
    pub(crate) area_sqkm: Option<f64>,
    pub(crate) phone_owners: Option<i64>,
    pub(crate) population: Option<i64>,
}

pub(crate) struct SeedLocation {
    pub(crate) region: String,
    pub(crate) department: String,
    pub(crate) commune: String,
    pub(crate) pcode: String,
    pub(crate) latitude: f64,
    pub(crate) longitude: f64,
    pub(crate) area_sqkm: Option<f64>,
    pub(crate) data_source: String,
}

pub(crate) const CAMEROON_MIN_LATITUDE: f64 = 1.5;
pub(crate) const CAMEROON_MAX_LATITUDE: f64 = 13.5;
pub(crate) const CAMEROON_MIN_LONGITUDE: f64 = 8.0;
pub(crate) const CAMEROON_MAX_LONGITUDE: f64 = 16.5;
pub(crate) const CAMEROON_2025_POPULATION: i64 = 29_879_337;
pub(crate) const CAMEROON_2024_MOBILE_SUBSCRIPTIONS_PER_100: f64 = 108.21313;
pub(crate) const MODEL_SOURCE: &str = "Matrix estimate: OCHA COD-AB GPS/area + UN 2025 population + World Bank 2024 mobile subscriptions";

pub(crate) struct UrbanAnchor {
    pub(crate) latitude: f64,
    pub(crate) longitude: f64,
    pub(crate) influence: f64,
}

pub(crate) const URBAN_ANCHORS: &[UrbanAnchor] = &[
    UrbanAnchor {
        latitude: 4.0511,
        longitude: 9.7679,
        influence: 1.35,
    },
    UrbanAnchor {
        latitude: 3.8480,
        longitude: 11.5021,
        influence: 1.30,
    },
    UrbanAnchor {
        latitude: 9.3014,
        longitude: 13.3977,
        influence: 0.82,
    },
    UrbanAnchor {
        latitude: 5.9631,
        longitude: 10.1594,
        influence: 0.80,
    },
    UrbanAnchor {
        latitude: 5.4839,
        longitude: 10.4170,
        influence: 0.78,
    },
    UrbanAnchor {
        latitude: 10.5950,
        longitude: 14.3247,
        influence: 0.74,
    },
    UrbanAnchor {
        latitude: 7.3277,
        longitude: 13.5847,
        influence: 0.70,
    },
    UrbanAnchor {
        latitude: 4.5759,
        longitude: 13.6846,
        influence: 0.62,
    },
    UrbanAnchor {
        latitude: 4.1575,
        longitude: 9.2407,
        influence: 0.66,
    },
    UrbanAnchor {
        latitude: 2.9167,
        longitude: 11.1500,
        influence: 0.55,
    },
];

impl UpdateLocationRequest {
    pub(crate) fn validate(&self) -> Result<(), String> {
        if self.region.trim().is_empty()
            || self.department.trim().is_empty()
            || self.commune.trim().is_empty()
            || self.location.trim().is_empty()
        {
            return Err("Region, department, commune, and location are required.".into());
        }

        if !self.latitude.is_finite() || !self.longitude.is_finite() {
            return Err("Latitude and longitude must be valid GPS coordinates.".into());
        }

        if !(CAMEROON_MIN_LATITUDE..=CAMEROON_MAX_LATITUDE).contains(&self.latitude)
            || !(CAMEROON_MIN_LONGITUDE..=CAMEROON_MAX_LONGITUDE).contains(&self.longitude)
        {
            return Err("GPS coordinates must be inside Cameroon.".into());
        }

        if matches!(self.area_sqkm, Some(area) if !area.is_finite() || area < 0.0) {
            return Err("Area must be a valid non-negative number.".into());
        }

        if matches!(self.phone_owners, Some(phone_owners) if phone_owners < 0)
            || matches!(self.population, Some(population) if population < 0)
        {
            return Err("Phone owners and population cannot be negative.".into());
        }

        match (self.phone_owners, self.population) {
            (Some(phone_owners), Some(population)) if phone_owners > population => {
                return Err("Phone owners cannot be greater than population.".into());
            }
            (Some(_), None) | (None, Some(_)) => {
                return Err("Phone owners and population must be provided together.".into());
            }
            _ => {}
        }

        Ok(())
    }
}
