param(
  [string]$BaseUrl = "http://127.0.0.1:8081"
)

$ErrorActionPreference = "Stop"

$envMap = @{}
if (Test-Path ".env") {
  Get-Content ".env" | ForEach-Object {
    if ($_ -match "^([^#=]+)=(.*)$") {
      $envMap[$matches[1]] = $matches[2]
    }
  }
}

$run = "audit-" + (Get-Date -Format "yyyyMMddHHmmss")
$results = New-Object System.Collections.Generic.List[object]

function Add-Result($Feature, $Step, $Status, $Detail = "") {
  $script:results.Add([pscustomobject]@{
    feature = $Feature
    step = $Step
    status = $Status
    detail = $Detail
  }) | Out-Null
}

function Invoke-FeatureApi($Method, $Path, $Feature, $Step, $Body = $null, $Headers = @{}) {
  try {
    $requestHeaders = @{}
    foreach ($key in $Headers.Keys) { $requestHeaders[$key] = $Headers[$key] }
    $params = @{
      Method = $Method
      Uri = ($BaseUrl + $Path)
      Headers = $requestHeaders
      TimeoutSec = 20
    }
    if ($null -ne $Body) {
      $requestHeaders["Content-Type"] = "application/json"
      $params.Body = ($Body | ConvertTo-Json -Depth 10)
    }
    $response = Invoke-RestMethod @params
    Add-Result $Feature $Step "PASS"
    return $response
  } catch {
    Add-Result $Feature $Step "FAIL" $_.Exception.Message
    return $null
  }
}

function Require-Record($Record, $Name, $Feature, $Step) {
  if (-not $Record -or -not $Record.id) {
    Add-Result $Feature $Step "SKIP" "Missing $Name from prior step"
    return $false
  }
  return $true
}

Invoke-FeatureApi GET "/api/summary" "Public cockpit" "summary loads" | Out-Null
Invoke-FeatureApi GET "/api/overview" "Public cockpit" "overview loads" | Out-Null
Invoke-FeatureApi GET "/api/phone-matrix" "Phone matrix" "matrix loads" | Out-Null
Invoke-FeatureApi GET "/api/phone-matrix/detail?region=Littoral&department=Moungo&commune=Bare-Bakem" "Phone matrix" "area detail loads" | Out-Null
Invoke-FeatureApi GET "/api/phone-matrix/assumptions" "Phone matrix" "assumptions load" | Out-Null
Invoke-FeatureApi GET "/api/priority-zones" "Decision engine" "priority zones load" | Out-Null
Invoke-FeatureApi GET "/api/decision-report" "Decision engine" "decision report loads" | Out-Null
Invoke-FeatureApi GET "/fragments/ops-status" "HTMX fragments" "ops status fragment loads" | Out-Null
Invoke-FeatureApi GET "/fragments/workspace-activity" "HTMX fragments" "workspace activity fragment loads" | Out-Null

$login = Invoke-FeatureApi POST "/api/auth/login" "Authentication" "root login" @{
  login = $envMap["ROOT_EMAIL"]
  password = $envMap["ROOT_PASSWORD"]
}
$authHeaders = @{}
if ($login.token) { $authHeaders["x-kk-session"] = $login.token }
Invoke-FeatureApi GET "/api/auth/context" "Authentication" "session context" $null $authHeaders | Out-Null

$orgs = Invoke-FeatureApi POST "/api/organizations" "Workspaces" "create organization" @{
  name = "Codex audit org $run"
  org_type = "ngo"
  contact_name = "Audit lead"
  contact_email = "audit-$run@example.local"
} $authHeaders
$org = $orgs | Where-Object name -eq "Codex audit org $run" | Select-Object -Last 1

if (Require-Record $org "organization" "Workspaces" "continue after organization") {
  $projects = Invoke-FeatureApi POST "/api/projects" "Workspaces" "create project" @{
    organization_id = $org.id
    name = "Codex audit project $run"
    sector = "connectivity"
    region = "Littoral"
    status = "planning"
    start_date = (Get-Date -Format "yyyy-MM-dd")
  } $authHeaders
  $project = $projects | Where-Object name -eq "Codex audit project $run" | Select-Object -Last 1
}

if (Require-Record $project "project" "Site profiles" "continue after project") {
  $sites = Invoke-FeatureApi POST "/api/site-profiles" "Site profiles" "create linked site" @{
    project_id = $project.id
    name = "Codex audit site $run"
    site_type = "telecom_probe_site"
    region = "Littoral"
    department = "Moungo"
    commune = "Bare-Bakem"
    latitude = 4.9827
    longitude = 10.0167
    beneficiary_estimate = 1200
    trust_signal = "gps_photo_verified"
    access_notes = "E2E audit site proof."
  } $authHeaders
  $site = $sites | Where-Object name -eq "Codex audit site $run" | Select-Object -Last 1
  if (Require-Record $site "site" "Site profiles" "update linked site") {
    Invoke-FeatureApi PATCH "/api/site-profiles/$($site.id)" "Site profiles" "edit linked site" @{
      project_id = $project.id
      name = "Codex audit site $run"
      site_type = "telecom_probe_site"
      region = "Littoral"
      department = "Moungo"
      commune = "Bare-Bakem"
      latitude = 4.9827
      longitude = 10.0167
      beneficiary_estimate = 1250
      trust_signal = "gps_photo_verified"
      access_notes = "E2E audit site proof updated."
    } $authHeaders | Out-Null
    Invoke-FeatureApi GET "/api/entities/site_profile/$($site.id)" "Detail pages" "site detail loads" $null $authHeaders | Out-Null
  }

  $campaigns = Invoke-FeatureApi POST "/api/survey-campaigns" "Survey campaigns" "create campaign" @{
    project_id = $project.id
    name = "Codex audit campaign $run"
    form_type = "signal_check"
    target_region = "Littoral"
    target_department = "Moungo"
    target_commune = "Bare-Bakem"
    status = "draft"
    language_mode = "bilingual"
    offline_enabled = $true
    starts_on = (Get-Date -Format "yyyy-MM-dd")
    ends_on = (Get-Date).AddDays(14).ToString("yyyy-MM-dd")
  } $authHeaders
  $campaign = $campaigns | Where-Object name -eq "Codex audit campaign $run" | Select-Object -Last 1
}

if (Require-Record $campaign "campaign" "Survey campaigns" "continue after campaign") {
  Invoke-FeatureApi PATCH "/api/survey-campaigns/$($campaign.id)/status" "Survey campaigns" "transition draft to ready" @{ status = "ready" } $authHeaders | Out-Null
}

if ((Require-Record $project "project" "Signal probes/assets" "continue after project") -and (Require-Record $site "site" "Signal probes/assets" "continue after site")) {
  $assets = Invoke-FeatureApi POST "/api/assets" "Signal probes/assets" "create linked asset" @{
    project_id = $project.id
    site_profile_id = $site.id
    asset_type = "connectivity_probe"
    name = "Codex audit probe $run"
    region = "Littoral"
    department = "Moungo"
    commune = "Bare-Bakem"
    latitude = 4.9827
    longitude = 10.0167
    status = "online"
    operator = "Codex audit operator"
    installed_at = (Get-Date -Format "yyyy-MM-dd")
    notes = "E2E audit probe."
  } $authHeaders
  $asset = $assets | Where-Object name -eq "Codex audit probe $run" | Select-Object -Last 1
}

if (Require-Record $asset "asset" "Signal probes/assets" "continue after asset") {
  Invoke-FeatureApi PATCH "/api/assets/$($asset.id)" "Signal probes/assets" "edit linked asset" @{
    project_id = $project.id
    site_profile_id = $site.id
    asset_type = "connectivity_probe"
    name = "Codex audit probe $run"
    region = "Littoral"
    department = "Moungo"
    commune = "Bare-Bakem"
    latitude = 4.9827
    longitude = 10.0167
    status = "online"
    operator = "Codex audit operator"
    installed_at = (Get-Date -Format "yyyy-MM-dd")
    notes = "E2E audit probe metadata updated."
  } $authHeaders | Out-Null
  Invoke-FeatureApi GET "/api/entities/infrastructure_asset/$($asset.id)" "Detail pages" "asset detail loads" $null $authHeaders | Out-Null
  Invoke-FeatureApi POST "/api/evidence" "Evidence uploads" "attach asset evidence" @{
    entity_type = "infrastructure_asset"
    entity_id = $asset.id
    file_name = "asset-proof-$run.txt"
    content_type = "text/plain"
    content_base64 = [Convert]::ToBase64String([Text.Encoding]::UTF8.GetBytes("Codex asset evidence $run"))
    latitude = 4.9827
    longitude = 10.0167
    captured_at = (Get-Date).ToString("s")
  } $authHeaders | Out-Null
  Invoke-FeatureApi GET "/api/evidence?entity_type=infrastructure_asset&entity_id=$($asset.id)" "Evidence uploads" "list asset evidence" $null $authHeaders | Out-Null
  Invoke-FeatureApi PATCH "/api/assets/$($asset.id)/status" "Signal probes/assets" "transition asset status" @{ status = "warning"; notes = "E2E warning transition." } $authHeaders | Out-Null
  Invoke-FeatureApi POST "/api/iot/readings" "Telemetry" "create linked reading" @{ project_id = $project.id; site_profile_id = $site.id; asset_id = $asset.id; reading_type = "signal_quality"; value = 44.5; unit = "score"; latitude = 4.9827; longitude = 10.0167 } $authHeaders | Out-Null
  $reports = Invoke-FeatureApi POST "/api/reports" "Field reports" "create linked report" @{ project_id = $project.id; site_profile_id = $site.id; campaign_id = $campaign.id; asset_id = $asset.id; report_type = "signal_check"; region = "Littoral"; department = "Moungo"; commune = "Bare-Bakem"; latitude = 4.9827; longitude = 10.0167; status = "verified"; evidence_quality = "gps_photo_verified"; notes = "E2E report with campaign/site/asset links."; submitted_by = "Codex audit" } $authHeaders
  $report = $reports | Where-Object { $_.asset_id -eq $asset.id -and $_.submitted_by -eq "Codex audit" } | Select-Object -Last 1
  if (Require-Record $report "report" "Field reports" "edit linked report") {
    Invoke-FeatureApi PATCH "/api/reports/$($report.id)" "Field reports" "edit linked report" @{ project_id = $project.id; site_profile_id = $site.id; campaign_id = $campaign.id; asset_id = $asset.id; report_type = "signal_check"; region = "Littoral"; department = "Moungo"; commune = "Bare-Bakem"; latitude = 4.9827; longitude = 10.0167; status = "verified"; evidence_quality = "gps_photo_verified"; notes = "E2E report edited with campaign/site/asset links."; submitted_by = "Codex audit" } $authHeaders | Out-Null
    Invoke-FeatureApi GET "/api/entities/field_report/$($report.id)" "Detail pages" "report detail loads" $null $authHeaders | Out-Null
  }
  $alerts = Invoke-FeatureApi POST "/api/alerts" "Alerts" "create linked alert" @{ project_id = $project.id; site_profile_id = $site.id; asset_id = $asset.id; severity = "warning"; title = "Codex audit alert $run"; message = "E2E alert with linked site and asset." } $authHeaders
  $alert = $alerts | Where-Object title -eq "Codex audit alert $run" | Select-Object -Last 1
}

if (Require-Record $alert "alert" "Alerts" "continue after alert") {
  Invoke-FeatureApi GET "/api/entities/alert/$($alert.id)" "Detail pages" "alert detail loads" $null $authHeaders | Out-Null
  Invoke-FeatureApi PATCH "/api/alerts/$($alert.id)" "Alerts" "acknowledge alert" @{ status = "acknowledged" } $authHeaders | Out-Null
  $tickets = Invoke-FeatureApi POST "/api/tickets" "Maintenance tickets" "create ticket from alert" @{ project_id = $project.id; site_profile_id = $site.id; asset_id = $asset.id; alert_id = $alert.id; title = "Codex audit ticket $run"; priority = "high"; assigned_to = "Codex audit team"; due_date = (Get-Date).AddDays(3).ToString("yyyy-MM-dd"); sla_hours = 72 } $authHeaders
  $ticket = $tickets | Where-Object title -eq "Codex audit ticket $run" | Select-Object -Last 1
}

if (Require-Record $ticket "ticket" "Maintenance tickets" "continue after ticket") {
  Invoke-FeatureApi GET "/api/entities/maintenance_ticket/$($ticket.id)" "Detail pages" "ticket detail loads" $null $authHeaders | Out-Null
  Invoke-FeatureApi PATCH "/api/tickets/$($ticket.id)" "Maintenance tickets" "start ticket" @{ status = "in_progress"; resolution_notes = $null } $authHeaders | Out-Null
  Invoke-FeatureApi PATCH "/api/tickets/$($ticket.id)" "Maintenance tickets" "complete ticket and resolve linked alert" @{ status = "done"; resolution_notes = "E2E ticket completed with proof." } $authHeaders | Out-Null
}

Invoke-FeatureApi POST "/api/operator-imei-events" "Operator IMEI API" "ingest operator event" @{ operator_name = "Codex ISP"; imei = "356938035643809"; device_type = "smartphone"; event_type = "verification"; compliance_status = "pending"; region = "Littoral"; department = "Moungo"; commune = "Bare-Bakem"; source_system = "operator_api"; raw_reference = $run; network_first_seen_at = (Get-Date).ToString("s") } $authHeaders | Out-Null
$imeiSummary = Invoke-FeatureApi GET "/api/operator-imei-events" "Operator IMEI API" "read compliance summary" $null $authHeaders
$imeiEvent = $imeiSummary.latest_events | Where-Object raw_reference -eq $run | Select-Object -Last 1
if (Require-Record $imeiEvent "IMEI event" "Operator IMEI API" "detail IMEI event") {
  Invoke-FeatureApi GET "/api/entities/operator_imei_event/$($imeiEvent.id)" "Operator IMEI API" "IMEI event detail loads" $null $authHeaders | Out-Null
}

if ((Require-Record $project "project" "Decision board" "continue after project") -and (Require-Record $site "site" "Decision board" "continue after site") -and (Require-Record $asset "asset" "Decision board" "continue after asset")) {
  $decisions = Invoke-FeatureApi POST "/api/decision-snapshots" "Decision board" "create linked decision" @{ project_id = $project.id; site_profile_id = $site.id; asset_id = $asset.id; title = "Codex audit decision $run"; decision_stage = "recommended"; priority_score = 72; recommended_budget_xaf = 1800000; owner_name = "Codex audit owner"; risk_level = "medium"; evidence_score = 82; approval_notes = ""; execution_status = "not_started"; rationale = "E2E decision with evidence and budget."; next_action = "Approve and create execution plan." } $authHeaders
  $decision = $decisions | Where-Object title -eq "Codex audit decision $run" | Select-Object -Last 1
}

if (Require-Record $decision "decision" "Decision board" "continue after decision") {
  Invoke-FeatureApi GET "/api/entities/decision_snapshot/$($decision.id)" "Detail pages" "decision detail loads" $null $authHeaders | Out-Null
  Invoke-FeatureApi PATCH "/api/decision-snapshots/$($decision.id)/status" "Decision board" "approve decision" @{ decision_stage = "approved"; execution_status = "ready"; approval_notes = "E2E approval under budget threshold." } $authHeaders | Out-Null
  $plans = Invoke-FeatureApi POST "/api/decision-snapshots/$($decision.id)/execution-plan" "Execution board" "create execution plan from decision" @{} $authHeaders
  $plan = $plans.plans | Where-Object decision_id -eq $decision.id | Select-Object -Last 1
}

if (Require-Record $plan "execution plan" "Execution board" "continue after execution plan") {
  Invoke-FeatureApi GET "/api/entities/execution_plan/$($plan.id)" "Detail pages" "execution detail loads" $null $authHeaders | Out-Null
  Invoke-FeatureApi PATCH "/api/execution-plans/$($plan.id)/status" "Execution board" "mark plan ready" @{ status = "ready"; local_focal_point_confirmed = $true; gps_photo_proof_required = $true; offline_survey_ready = $true; bilingual_script_ready = $true; xaf_budget_approved = $true; blocker = $null; outcome_notes = $null } $authHeaders | Out-Null
}

Invoke-FeatureApi GET "/api/workspaces/dashboard" "Workspaces" "dashboard refresh with created records" | Out-Null
Invoke-FeatureApi GET "/api/area-dossier?region=Littoral&department=Moungo&commune=Bare-Bakem" "Area dossier" "area dossier loads linked records" | Out-Null
Invoke-FeatureApi GET "/api/audit-events?limit=10" "Audit log" "audit events readable" $null $authHeaders | Out-Null
foreach ($export in @("assets", "tickets", "priority-zones", "phone-matrix")) {
  Invoke-FeatureApi GET "/api/export/$export.csv" "CSV exports" "$export export downloads" | Out-Null
}

$results | Format-Table -AutoSize
$failed = @($results | Where-Object status -eq "FAIL")
if ($failed.Count -gt 0) {
  exit 1
}
