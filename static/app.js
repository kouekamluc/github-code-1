const CAMEROON_BOUNDS = { minLatitude: 1.5, maxLatitude: 13.5, minLongitude: 8, maxLongitude: 16.5 };

const summaryCards = document.getElementById('summary-cards');
const tableBody = document.getElementById('regions-table-body');
const refreshButton = document.getElementById('refresh-button');
const dataStatus = document.getElementById('data-status');
const authStatus = document.getElementById('auth-status');
const regionFilter = document.getElementById('regionFilter');
const departmentFilter = document.getElementById('departmentFilter');
const communeFilter = document.getElementById('communeFilter');
const areaProfile = document.getElementById('area-profile');
const matrixSearch = document.getElementById('matrixSearch');
const matrixSort = document.getElementById('matrixSort');
const matrixOwnershipFilter = document.getElementById('matrixOwnershipFilter');
const matrixConfidenceFilter = document.getElementById('matrixConfidenceFilter');
const matrixOpportunityFilter = document.getElementById('matrixOpportunityFilter');
const matrixValidationFilter = document.getElementById('matrixValidationFilter');
const matrixProjectFilter = document.getElementById('matrixProjectFilter');
const matrixMinPopulation = document.getElementById('matrixMinPopulation');
const matrixMinPriority = document.getElementById('matrixMinPriority');
const matrixMaxPriority = document.getElementById('matrixMaxPriority');
const matrixExportButton = document.getElementById('matrix-export-button');
const assetSearch = document.getElementById('assetSearch');
const assetStatusFilter = document.getElementById('assetStatusFilter');
const assetTypeFilter = document.getElementById('assetTypeFilter');
const workspaceSearch = document.getElementById('workspaceSearch');
const workspaceStatusFilter = document.getElementById('workspaceStatusFilter');
const workspaceTypeFilter = document.getElementById('workspaceTypeFilter');

let map;
let markersLayer;
let assetLayer;
let reportLayer;
let nationalSummary = null;
let allStats = [];
let assets = [];
let reports = [];
let alerts = [];
let tickets = [];
let organizations = [];
let projects = [];
let workspaceDashboard = null;
let phoneMatrixDashboard = null;
let overviewIntelligence = null;
let signalProbeDashboard = null;
let selectedAreaDossier = null;
let decisionBoard = null;
let executionBoard = null;
let sites = [];
let campaigns = [];
let decisionSnapshots = [];
let readings = [];
let imeiCompliance = null;
let priorityZones = [];
let selectedArea = null;
let currentMatrixRows = [];
let workspaceTemplates = [];
let authSession = JSON.parse(localStorage.getItem('kkEvoAuth') || 'null');

function authHeaders() {
  if (!authSession?.token) return {};
  return {
    'x-kk-session': authSession.token,
  };
}

async function fetchJson(url, options = {}) {
  const response = await fetch(url, {
    ...options,
    headers: {
      ...authHeaders(),
      ...(options.headers || {}),
    },
  });
  const contentType = response.headers.get('content-type') || '';
  const body = contentType.includes('application/json') ? await response.json() : null;
  if (!response.ok) throw new Error(body?.message || `Request failed with status ${response.status}`);
  return body;
}

function formatBytes(value) {
  const bytes = Number(value || 0);
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

function entityLabel(type) {
  return {
    organization: 'Organization',
    project: 'Project',
    site_profile: 'Site profile',
    survey_campaign: 'Survey campaign',
    infrastructure_asset: 'Asset',
    field_report: 'Field report',
    alert: 'Alert',
    maintenance_ticket: 'Ticket',
    decision_snapshot: 'Decision',
    execution_plan: 'Execution plan',
    operator_imei_event: 'IMEI event',
  }[type] || type.replaceAll('_', ' ');
}

function detailTitle(type, record) {
  return record?.title || record?.name || record?.report_type || record?.operator_name || `${entityLabel(type)} #${record?.id || ''}`;
}

function renderRecordFields(record) {
  const hidden = new Set(['id', 'created_at', 'updated_at', 'resolved_at', 'last_checked_at', 'sha256_hash', 'storage_path']);
  return Object.entries(record || {})
    .filter(([key, value]) => !hidden.has(key) && value !== null && value !== undefined && value !== '')
    .slice(0, 18)
    .map(([key, value]) => `
      <div>
        <span>${escapeHtml(key.replaceAll('_', ' '))}</span>
        <strong>${escapeHtml(typeof value === 'boolean' ? (value ? 'Yes' : 'No') : value)}</strong>
      </div>
    `)
    .join('');
}

function renderEvidenceList(files) {
  if (!files?.length) return '<div class="empty-state">No evidence files attached yet.</div>';
  return files.map(file => `
    <article class="evidence-item">
      <div>
        <strong>${escapeHtml(file.file_name)}</strong>
        <span>${escapeHtml(file.content_type)} &middot; ${formatBytes(file.file_size)} &middot; ${escapeHtml(file.uploaded_by)}</span>
      </div>
      <small>${escapeHtml(file.captured_at || file.created_at)}${file.latitude ? ` &middot; ${Number(file.latitude).toFixed(4)}, ${Number(file.longitude).toFixed(4)}` : ''}</small>
    </article>
  `).join('');
}

function renderAuditList(events) {
  if (!events?.length) return '<div class="empty-state">No audit events for this record yet.</div>';
  return events.map(event => `
    <article class="audit-item">
      <div>
        <strong>${escapeHtml(event.field_name)}</strong>
        <span>${escapeHtml(event.actor)} &middot; ${escapeHtml(event.created_at)}</span>
      </div>
      <p>${escapeHtml(event.note || `${event.old_value || 'empty'} -> ${event.new_value || 'empty'}`)}</p>
    </article>
  `).join('');
}

async function openEntityDetail(entityType, entityId) {
  if (!entityId) return;
  const panel = document.getElementById('entity-detail-panel');
  const title = document.getElementById('entity-detail-title');
  const subtitle = document.getElementById('entity-detail-subtitle');
  const body = document.getElementById('entity-detail-body');
  const evidence = document.getElementById('entity-evidence-list');
  const audit = document.getElementById('entity-audit-list');
  const form = document.getElementById('entity-evidence-form');
  const status = document.getElementById('entity-evidence-status');
  if (!panel || !title || !subtitle || !body || !evidence || !audit || !form) return;

  panel.classList.add('is-open');
  panel.setAttribute('aria-hidden', 'false');
  title.textContent = 'Loading record...';
  subtitle.textContent = `${entityLabel(entityType)} #${entityId}`;
  body.innerHTML = '<div class="empty-state">Loading backend detail.</div>';
  evidence.innerHTML = '';
  audit.innerHTML = '';
  status.innerHTML = '';
  form.dataset.entityType = entityType;
  form.dataset.entityId = entityId;

  try {
    const detail = await fetchJson(`/api/entities/${entityType}/${entityId}`);
    title.textContent = detailTitle(detail.entity_type, detail.record);
    subtitle.textContent = `${entityLabel(detail.entity_type)} #${detail.entity_id}`;
    body.innerHTML = `<div class="entity-field-grid">${renderRecordFields(detail.record)}</div>`;
    evidence.innerHTML = renderEvidenceList(detail.evidence);
    audit.innerHTML = renderAuditList(detail.audit_events);
  } catch (error) {
    title.textContent = 'Detail unavailable';
    body.innerHTML = `<div class="empty-state">${escapeHtml(error.message)}</div>`;
  }
}

function closeEntityDetail() {
  const panel = document.getElementById('entity-detail-panel');
  if (!panel) return;
  panel.classList.remove('is-open');
  panel.setAttribute('aria-hidden', 'true');
}

function wireEntityDetailButtons(scope = document) {
  scope.querySelectorAll('[data-entity-detail]').forEach(button => {
    if (button.dataset.detailWired) return;
    button.dataset.detailWired = 'true';
    button.addEventListener('click', event => {
      event.preventDefault();
      event.stopPropagation();
      openEntityDetail(button.dataset.entityType, button.dataset.entityId);
    });
  });
}

function escapeHtml(value) {
  return String(value ?? '')
    .replaceAll('&', '&amp;')
    .replaceAll('<', '&lt;')
    .replaceAll('>', '&gt;')
    .replaceAll('"', '&quot;')
    .replaceAll("'", '&#039;');
}

function formatNumber(value) {
  return Number(value ?? 0).toLocaleString();
}

function formatCoordinate(value) {
  return Number(value).toFixed(4);
}

function formatRate(value) {
  return `${Number(value ?? 0).toFixed(1)}%`;
}

function confidenceLabel(confidence) {
  if (confidence >= 0.86) return 'High';
  if (confidence >= 0.68) return 'Medium';
  if (confidence > 0) return 'Low';
  return 'Unknown';
}

function formatMoneyXaf(value) {
  return value ? `${formatNumber(value)} XAF` : 'Budget not set';
}

function phoneMatrixRowToStat(row) {
  return {
    ...row,
    phone_owners: row.estimated_phone_owners,
    phone_rate: row.ownership_rate,
    metric_source: row.method,
    confidence: row.confidence,
    urban_signal: row.urban_signal || 0,
    data_source: row.data_source,
    updated_at: row.last_updated,
  };
}

function compactMoneyXaf(value) {
  const amount = Number(value || 0);
  if (amount >= 1_000_000) return `${(amount / 1_000_000).toFixed(1)}M XAF`;
  if (amount >= 1_000) return `${Math.round(amount / 1_000)}K XAF`;
  return `${formatNumber(amount)} XAF`;
}

function labelize(value) {
  return String(value || 'Unspecified').replaceAll('_', ' ').replace(/\b\w/g, letter => letter.toUpperCase());
}

function shortDate(value) {
  if (!value) return 'not set';
  const date = new Date(value);
  return Number.isNaN(date.getTime()) ? String(value).slice(0, 10) : date.toLocaleDateString();
}

function estimateBudgetXaf(area, context = localContextForArea(area)) {
  const base = 450_000;
  const populationComponent = Math.min(1_900_000, Math.round((area.population || 0) * 5.5));
  const validationComponent = area.confidence < 0.68 ? 380_000 : 180_000;
  const alertComponent = context.localAlerts.length * 300_000;
  const travelComponent = area.phone_rate < 65 ? 260_000 : 120_000;
  return base + populationComponent + validationComponent + alertComponent + travelComponent;
}

function estimateReach(area) {
  return Math.round((area.population || 0) * Math.max(0.08, Math.min(0.32, (100 - area.phone_rate) / 180)));
}

function channelRecommendation(area) {
  if (area.phone_rate < 65) return 'Offline forms + SMS follow-up + local focal point';
  if (area.confidence < 0.7) return 'GPS/photo survey + WhatsApp coordination';
  return 'WhatsApp coordination + targeted GPS spot checks';
}

function riskLabel(area, context = localContextForArea(area)) {
  const score = (area.confidence < 0.68 ? 2 : 0)
    + (area.phone_rate < 65 ? 2 : 0)
    + (context.localAlerts.length ? 2 : 0)
    + (!context.localSites.length ? 1 : 0);
  if (score >= 5) return 'High risk';
  if (score >= 3) return 'Medium risk';
  return 'Controlled risk';
}

function setStatus(element, message, type = 'info') {
  if (!element) return;
  element.innerHTML = message ? `<div class="alert alert-${type} py-2 mb-0">${escapeHtml(message)}</div>` : '';
}

function isInCameroon(latitude, longitude) {
  return Number.isFinite(latitude)
    && Number.isFinite(longitude)
    && latitude >= CAMEROON_BOUNDS.minLatitude
    && latitude <= CAMEROON_BOUNDS.maxLatitude
    && longitude >= CAMEROON_BOUNDS.minLongitude
    && longitude <= CAMEROON_BOUNDS.maxLongitude;
}

function gpsLabel(item) {
  return `${formatCoordinate(item.latitude)}, ${formatCoordinate(item.longitude)}`;
}

function activeKey() {
  return `${regionFilter.value}|${departmentFilter.value}|${communeFilter.value}`;
}

function areaKey(item) {
  return `${item.region}|${item.department}|${item.commune}`;
}

function normalizeAreaPart(value) {
  return (value || '').toString().normalize('NFD').replace(/[\u0300-\u036f]/g, '').toLowerCase().trim();
}

function areaMatchesLocation(area, item) {
  return normalizeAreaPart(area.region) === normalizeAreaPart(item.region)
    && normalizeAreaPart(area.department) === normalizeAreaPart(item.department)
    && normalizeAreaPart(area.commune) === normalizeAreaPart(item.commune);
}

function areaFromLocation(item) {
  if (!item) return null;
  const exact = allStats.find(area => areaMatchesLocation(area, item));
  if (exact) return exact;
  const nearby = allStats
    .filter(area => normalizeAreaPart(area.region) === normalizeAreaPart(item.region))
    .map(area => ({
      area,
      distance: Math.abs(Number(area.latitude || 0) - Number(item.latitude || 0))
        + Math.abs(Number(area.longitude || 0) - Number(item.longitude || 0)),
    }))
    .sort((a, b) => a.distance - b.distance)[0];
  if (nearby && nearby.distance <= 0.35) return nearby.area;
  return allStats.find(area => (
    normalizeAreaPart(area.region) === normalizeAreaPart(item.region)
    && normalizeAreaPart(area.department) === normalizeAreaPart(item.department)
  )) || null;
}

function selectArea(area, view = 'profile') {
  selectedArea = area;
  renderAreaProfile();
  loadAreaDossier(area);
  if (view) switchView(view);
}

function dossierMatches(area, dossier = selectedAreaDossier) {
  return Boolean(dossier?.area && areaKey(dossier.area) === areaKey(area));
}

async function loadAreaDossier(area) {
  if (!area) return;
  const params = new URLSearchParams({
    region: area.region,
    department: area.department,
    commune: area.commune,
  });
  try {
    selectedAreaDossier = await fetchJson(`/api/area-dossier?${params.toString()}`);
    if (selectedArea && areaKey(selectedArea) === areaKey(area)) renderAreaProfile();
  } catch (error) {
    console.warn('Area dossier unavailable', error);
  }
}

function priorityForArea(area) {
  return priorityZones.find(zone => areaKey(zone) === areaKey(area));
}

function localContextForArea(area) {
  const key = areaKey(area);
  const localAssets = assets.filter(asset => areaKey(asset) === key);
  const localReports = reports.filter(report => areaKey(report) === key);
  const localSites = sites.filter(site => areaKey(site) === key);
  const localCampaigns = campaigns.filter(campaign => (
    (!campaign.target_region || campaign.target_region === area.region)
    && (!campaign.target_department || campaign.target_department === area.department)
    && (!campaign.target_commune || campaign.target_commune === area.commune)
  ));
  const localAlerts = alerts.filter(alert => {
    const alertAsset = assets.find(asset => asset.id === alert.asset_id);
    return alert.status !== 'resolved'
      && ((alertAsset && areaKey(alertAsset) === key)
        || localSites.some(site => site.id === alert.site_profile_id));
  });
  const localTickets = tickets.filter(ticket => {
    const ticketAsset = assets.find(asset => asset.id === ticket.asset_id);
    return ticket.status !== 'done'
      && ticket.status !== 'cancelled'
      && ((ticketAsset && areaKey(ticketAsset) === key)
        || localSites.some(site => site.id === ticket.site_profile_id));
  });
  return {
    localAssets,
    localReports,
    localSites,
    localCampaigns,
    localAlerts,
    localTickets,
    localPriority: priorityForArea(area),
  };
}

function probeHealthFor(asset) {
  return signalProbeDashboard?.health?.find(item => item.asset_id === asset.id);
}

function assetArea(asset) {
  return areaFromLocation(asset);
}

function assetContext(asset) {
  const assetAlerts = alerts.filter(alert => alert.asset_id === asset.id && alert.status !== 'resolved');
  const assetTickets = tickets.filter(ticket => (
    ticket.asset_id === asset.id
    && ticket.status !== 'done'
    && ticket.status !== 'cancelled'
  ));
  const assetReports = reports.filter(report => report.asset_id === asset.id);
  const assetReadings = readings.filter(reading => reading.asset_id === asset.id);
  return { assetAlerts, assetTickets, assetReports, assetReadings, health: probeHealthFor(asset) };
}

function filteredAssets() {
  const query = (assetSearch?.value || '').trim().toLowerCase();
  const status = assetStatusFilter?.value || 'all';
  const type = assetTypeFilter?.value || 'all';
  return assets.filter(asset => {
    const haystack = [
      asset.name,
      asset.asset_type,
      asset.status,
      asset.operator,
      asset.project_name,
      asset.site_name,
      asset.region,
      asset.department,
      asset.commune,
      asset.notes,
    ].join(' ').toLowerCase();
    if (query && !haystack.includes(query)) return false;
    if (status !== 'all' && asset.status !== status) return false;
    if (type !== 'all' && asset.asset_type !== type) return false;
    return true;
  }).sort((a, b) => {
    const healthA = probeHealthFor(a)?.health_score ?? 100;
    const healthB = probeHealthFor(b)?.health_score ?? 100;
    return healthA - healthB;
  });
}

function areaActionText(area, context) {
  if (context.localAlerts.length) return 'Resolve open alerts and dispatch field validation before new deployment.';
  if (!context.localSites.length) return 'Create a site profile to anchor proof, beneficiaries, and local access notes.';
  if (!context.localCampaigns.length) return 'Launch an offline GPS/photo campaign for phone access and signal proof.';
  if ((context.localPriority?.priority_score || 0) >= 52) return 'Prepare a decision snapshot with budget, rationale, and next field action.';
  return 'Keep in watchlist and refresh when new GPS, survey, or telemetry signals arrive.';
}

function switchView(view) {
  const requested = document.querySelector(`.tab-button[data-view="${view}"]`);
  if (requested?.dataset.navScope === 'auth' && !authSession?.token) {
    view = 'login';
  }
  document.querySelectorAll('.tab-button').forEach(button => {
    button.classList.toggle('active', button.dataset.view === view);
  });
  document.querySelectorAll('.view-section').forEach(section => {
    section.classList.toggle('active', section.id === `view-${view}`);
  });
  if (view === 'overview' && map) setTimeout(() => map.invalidateSize(), 150);
}

function renderAuthState() {
  const authenticated = Boolean(authSession?.token);
  document.querySelectorAll('[data-nav-scope="auth"]').forEach(button => {
    button.style.display = authenticated ? '' : 'none';
  });
  document.querySelectorAll('[data-nav-scope="guest"]').forEach(button => {
    button.style.display = authenticated ? 'none' : '';
  });
  if (authStatus) {
    authStatus.innerHTML = authenticated
      ? `<div class="alert alert-success py-2 mb-0">Signed in as ${escapeHtml(authSession.display_name || authSession.actor)} · ${escapeHtml(authSession.role)} <button class="btn btn-sm btn-outline-secondary" id="logout-button">Sign out</button></div>`
      : '<div class="alert alert-info py-2 mb-0">Public preview mode</div>';
    document.getElementById('logout-button')?.addEventListener('click', () => {
      authSession = null;
      localStorage.removeItem('kkEvoAuth');
      renderAuthState();
      switchView('overview');
    });
  }
  if (!authenticated) {
    const active = document.querySelector('.view-section.active');
    const activeView = active?.id?.replace('view-', '');
    const activeButton = activeView ? document.querySelector(`.tab-button[data-view="${activeView}"]`) : null;
    if (activeButton?.dataset.navScope === 'auth') switchView('overview');
  }
  if (window.lucide) lucide.createIcons();
  wireEntityDetailButtons();
}

function renderSummary(summary, overview = overviewIntelligence) {
  if (overview?.kpis?.length) {
    summaryCards.innerHTML = overview.kpis.map((kpi, index) => `
      <div class="metric-tile clickable-card summary-nav-card accent-${escapeHtml(kpi.tone)} ${index === 0 ? 'featured-metric' : ''}" role="button" tabindex="0" data-view="${index === 0 ? 'priority' : index === 1 ? 'workspaces' : index === 2 ? 'tickets' : 'areas'}">
        <span>${escapeHtml(kpi.label)}</span>
        <strong>${escapeHtml(kpi.value)}</strong>
        <small>${escapeHtml(kpi.detail)}</small>
      </div>
    `).join('');
    bindSummaryNavCards();
    return;
  }

  summaryCards.innerHTML = `
    <div class="metric-tile clickable-card summary-nav-card accent-bronze featured-metric" role="button" tabindex="0" data-view="areas">
      <span>Estimated phone owners</span>
      <strong>${formatNumber(summary.total_phone_owners)}</strong>
      <small>Modeled across ${summary.estimated_location_count} arrondissements</small>
    </div>
    <div class="metric-tile clickable-card summary-nav-card accent-green" role="button" tabindex="0" data-view="areas">
      <span>Population covered</span>
      <strong>${formatNumber(summary.total_population)}</strong>
      <small>${summary.commune_count} arrondissements in the national matrix</small>
    </div>
    <div class="metric-tile clickable-card summary-nav-card accent-gold" role="button" tabindex="0" data-view="areas">
      <span>Estimated ownership rate</span>
      <strong>${formatRate(summary.percent_with_phone)}</strong>
      <small>Blended from population, GPS, and telecom baselines</small>
    </div>
    <div class="metric-tile clickable-card summary-nav-card accent-red" role="button" tabindex="0" data-view="areas">
      <span>Departments covered</span>
      <strong>${summary.department_count}</strong>
      <small>${summary.region_count} regions mapped</small>
    </div>
  `;
  bindSummaryNavCards();
}

function bindSummaryNavCards() {
  document.querySelectorAll('.summary-nav-card').forEach(card => {
    const open = () => switchView(card.dataset.view);
    card.addEventListener('click', event => {
      if (!cardClickGuard(event)) open();
    });
    card.addEventListener('keydown', event => {
      if (event.key === 'Enter' || event.key === ' ') {
        event.preventDefault();
        open();
      }
    });
  });
}

function areaFromOpportunity(opportunity) {
  return allStats.find(area => (
    area.region === opportunity.region
    && area.department === opportunity.department
    && area.commune === opportunity.commune
  ));
}

function areaFromZone(zone) {
  return allStats.find(area => (
    area.region === zone.region
    && area.department === zone.department
    && area.commune === zone.commune
  ));
}

function areaFromAction(action) {
  const titleMatch = action.title?.match(/^Turn (.+) into /);
  if (titleMatch) {
    const commune = titleMatch[1];
    return allStats.find(area => (
      area.commune === commune
      && (!action.area || action.area.includes(area.department))
      && (!action.area || action.area.includes(area.region))
    ));
  }
  return null;
}

function openAreaFollowUp(area, view = 'profile') {
  if (!area) return;
  selectArea(area, view);
}

function areaForAssetId(assetId) {
  const asset = assets.find(item => item.id === Number(assetId));
  return asset && assetArea(asset);
}

function areaForSiteId(siteId) {
  const site = sites.find(item => item.id === Number(siteId));
  return site && areaFromLocation(site);
}

function openSiteFollowUp(siteId) {
  const area = areaForSiteId(siteId);
  if (area) openAreaFollowUp(area);
  else switchView('workspaces');
}

function openCampaignFollowUp(campaignId) {
  const campaign = campaigns.find(item => item.id === Number(campaignId));
  if (!campaign) return switchView('workspaces');
  switchView('reports');
  document.getElementById('reportCampaignId').value = campaign.id;
  document.getElementById('reportType').value = campaign.form_type;
  document.getElementById('reportRegion').value = campaign.target_region || '';
  document.getElementById('reportDepartment').value = campaign.target_department || '';
  document.getElementById('reportCommune').value = campaign.target_commune || '';
  document.getElementById('reportType').focus();
}

function openAlertFollowUp(alertId) {
  const alert = alerts.find(item => item.id === Number(alertId));
  if (!alert) return switchView('alerts');
  const area = alert.asset_id ? areaForAssetId(alert.asset_id) : areaForSiteId(alert.site_profile_id);
  if (area) openAreaFollowUp(area);
  else switchView('alerts');
}

function openTicketFollowUp(ticketId) {
  const ticket = tickets.find(item => item.id === Number(ticketId));
  if (!ticket) return switchView('tickets');
  const area = ticket.asset_id ? areaForAssetId(ticket.asset_id) : areaForSiteId(ticket.site_profile_id);
  if (area) openAreaFollowUp(area);
  else switchView('tickets');
}

function openDecisionFollowUp(item) {
  const area = item?.asset_id ? areaForAssetId(item.asset_id) : areaForSiteId(item?.site_profile_id);
  if (area) openAreaFollowUp(area);
  else switchView('decision');
}

function focusWorkspaceList(value) {
  switchView('workspaces');
  if (workspaceSearch) workspaceSearch.value = value || '';
  renderWorkspaces();
  document.getElementById('project-operating-list')?.scrollIntoView({ behavior: 'smooth', block: 'start' });
}

function cardClickGuard(event) {
  return Boolean(event.target.closest('button, a, input, select, textarea'));
}

function renderOverviewIntelligence() {
  const opportunityTarget = document.getElementById('overview-opportunities');
  const actionTarget = document.getElementById('overview-actions');
  const riskTarget = document.getElementById('overview-risks');
  const readoutTarget = document.getElementById('overview-market-readout');
  if (!overviewIntelligence || !opportunityTarget || !actionTarget || !riskTarget) return;

  opportunityTarget.innerHTML = overviewIntelligence.top_opportunities?.length ? overviewIntelligence.top_opportunities.map((opportunity, index) => `
    <article class="opportunity-card priority-${escapeHtml(opportunity.priority_label.toLowerCase())}">
      <button class="opportunity-main overview-opportunity-action" data-index="${index}">
        <span>${escapeHtml(opportunity.region)} / ${escapeHtml(opportunity.department)}</span>
        <strong>${escapeHtml(opportunity.commune)}</strong>
        <small>${escapeHtml(opportunity.business_case)}</small>
      </button>
      <div class="opportunity-metrics">
        <div><span>Budget</span><strong>${formatMoneyXaf(opportunity.estimated_budget_xaf)}</strong></div>
        <div><span>Reach</span><strong>${formatNumber(opportunity.likely_reach)}</strong></div>
        <div><span>Score</span><strong>${Number(opportunity.priority_score).toFixed(0)}</strong></div>
      </div>
      <p>${escapeHtml(opportunity.recommended_channel)}</p>
      <div class="export-actions">
        <button class="btn btn-outline-secondary btn-sm overview-action" data-action="campaign" data-index="${index}"><i data-lucide="clipboard-plus"></i> Campaign</button>
        <button class="btn btn-outline-secondary btn-sm overview-action" data-action="decision" data-index="${index}"><i data-lucide="file-plus-2"></i> Decision</button>
      </div>
    </article>
  `).join('') : '<div class="empty-state">No overview opportunities are available yet.</div>';

  actionTarget.innerHTML = overviewIntelligence.action_queue?.length ? overviewIntelligence.action_queue.map((action, index) => `
    <article class="compact-card clickable-card overview-next-action priority-${escapeHtml(action.urgency === 'urgent' ? 'high' : action.urgency)}" role="button" tabindex="0" data-index="${index}">
      <div>
        <strong>${escapeHtml(action.title)}</strong>
        <span>${escapeHtml(action.action_type)}${action.area ? ` &middot; ${escapeHtml(action.area)}` : ''}</span>
      </div>
      <span class="priority-badge priority-${escapeHtml(action.urgency === 'urgent' ? 'high' : action.urgency)}">${escapeHtml(action.urgency)}</span>
      <p>${escapeHtml(action.reason)}</p>
    </article>
  `).join('') : '<div class="empty-state">No immediate action queue.</div>';

  riskTarget.innerHTML = overviewIntelligence.trust_risks?.map((risk, index) => `
    <article class="risk-card clickable-card overview-risk-action severity-${escapeHtml(risk.severity)}" role="button" tabindex="0" data-index="${index}">
      <span>${escapeHtml(risk.label)}</span>
      <strong>${escapeHtml(risk.value)}</strong>
      <p>${escapeHtml(risk.mitigation)}</p>
    </article>
  `).join('') || '';

  if (readoutTarget) {
    readoutTarget.innerHTML = (overviewIntelligence.market_readout || []).map(item => (
      `<p>${escapeHtml(item)}</p>`
    )).join('');
  }

  document.querySelectorAll('.overview-opportunity-action').forEach(button => {
    button.addEventListener('click', () => {
      const opportunity = overviewIntelligence.top_opportunities[Number(button.dataset.index)];
      const area = opportunity && areaFromOpportunity(opportunity);
      if (area) selectArea(area);
    });
  });
  document.querySelectorAll('.overview-action').forEach(button => {
    button.addEventListener('click', () => {
      const opportunity = overviewIntelligence.top_opportunities[Number(button.dataset.index)];
      const area = opportunity && areaFromOpportunity(opportunity);
      if (area) prepareAreaAction(button.dataset.action, area);
    });
  });
  document.querySelectorAll('.overview-next-action').forEach(card => {
    const open = () => {
      const action = overviewIntelligence.action_queue[Number(card.dataset.index)];
      const area = action && areaFromAction(action);
      if (area) {
        openAreaFollowUp(area, action.action_type === 'decision' ? 'decision' : 'profile');
        return;
      }
      if (action?.action_type === 'maintenance') switchView('tickets');
      else if (action?.action_type === 'campaign' || action?.action_type === 'site') switchView('workspaces');
      else if (action?.action_type === 'decision') switchView('decision');
    };
    card.addEventListener('click', event => {
      if (!cardClickGuard(event)) open();
    });
    card.addEventListener('keydown', event => {
      if (event.key === 'Enter' || event.key === ' ') {
        event.preventDefault();
        open();
      }
    });
  });
  document.querySelectorAll('.overview-risk-action').forEach(card => {
    const open = () => {
      const risk = overviewIntelligence.trust_risks[Number(card.dataset.index)];
      switchView('areas');
      if (risk.label.toLowerCase().includes('low-confidence')) {
        matrixConfidenceFilter.value = 'low';
      }
      if (risk.label.toLowerCase().includes('weak phone')) {
        matrixOwnershipFilter.value = 'under65';
      }
      if (risk.label.toLowerCase().includes('open alerts')) {
        matrixOpportunityFilter.value = 'high';
      }
      updateView();
    };
    card.addEventListener('click', event => {
      if (!cardClickGuard(event)) open();
    });
    card.addEventListener('keydown', event => {
      if (event.key === 'Enter' || event.key === ' ') {
        event.preventDefault();
        open();
      }
    });
  });
  const topDecisionButton = document.getElementById('overview-top-decision');
  if (topDecisionButton) topDecisionButton.onclick = () => {
    const opportunity = overviewIntelligence.top_opportunities?.[0];
    const area = opportunity && areaFromOpportunity(opportunity);
    if (area) prepareAreaAction('decision', area);
  };
  if (window.lucide) lucide.createIcons();
}

async function refreshOverviewLayer() {
  overviewIntelligence = await fetchJson('/api/overview');
  if (nationalSummary) renderSummary(nationalSummary, overviewIntelligence);
  renderOverviewIntelligence();
}

function actionLabel(action) {
  return {
    probe: 'signal probe',
    campaign: 'survey campaign',
    report: 'validation report task',
    site: 'site profile',
    decision: 'decision snapshot',
    alert: 'coverage alert',
    ticket: 'maintenance ticket',
    full: 'full action bundle',
  }[action] || action;
}

async function refreshAfterBackendAction(message, view = null) {
  await refreshData();
  if (view) switchView(view);
  setStatus(dataStatus, message, 'success');
  if (window.htmx) {
    htmx.trigger('#data-status', 'refresh');
    htmx.trigger('#workspace-activity', 'refresh');
  }
}

async function runAreaBackendAction(action, area, view = null) {
  if (!authSession?.token) {
    switchView('login');
    setStatus(document.getElementById('login-status'), 'Sign in to create operational records from the console.', 'info');
    return;
  }
  setStatus(dataStatus, `Creating ${actionLabel(action)} for ${area.commune}...`, 'info');
  try {
    const result = await fetchJson('/api/actions/area', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({
        action,
        region: area.region,
        department: area.department,
        commune: area.commune,
      }),
    });
    const created = result.created?.length ? ` Ensured: ${result.created.join(', ')}.` : '';
    await refreshAfterBackendAction(`${result.message}${created}`, view);
  } catch (error) {
    setStatus(dataStatus, error.message, 'danger');
  }
}

function renderAreaProfile(area = selectedArea) {
  if (!area) {
    areaProfile.innerHTML = '<div class="empty-state">Select an arrondissement from the map or matrix to inspect its intelligence profile.</div>';
    return;
  }

  const context = localContextForArea(area);
  const dossier = dossierMatches(area) ? selectedAreaDossier : null;
  const economics = dossier?.economics;
  const {
    localAssets,
    localReports,
    localSites,
    localCampaigns,
    localAlerts,
    localTickets,
    localPriority,
  } = context;
  const actionText = areaActionText(area, context);

  areaProfile.innerHTML = `
    <div class="profile-hero">
      <div>
        <p class="eyebrow mb-2">${escapeHtml(area.region)} / ${escapeHtml(area.department)}</p>
        <h3>${escapeHtml(area.commune)}</h3>
        <p>${escapeHtml(area.pcode || 'Manual area')} &middot; ${gpsLabel(area)} &middot; ${area.area_sqkm ? `${formatNumber(Math.round(area.area_sqkm))} km2` : 'Area unknown'}</p>
      </div>
      <span class="priority-badge priority-${escapeHtml((localPriority?.priority_label || 'Watch').toLowerCase())}">${escapeHtml(localPriority?.priority_label || 'Watch')}</span>
    </div>
    <div class="profile-grid-inner">
      <div class="metric-tile accent-bronze"><span>Population</span><strong>${formatNumber(area.population)}</strong><small>Matrix or measured</small></div>
      <div class="metric-tile accent-green"><span>Phone owners</span><strong>${formatNumber(area.phone_owners)}</strong><small>${formatRate(area.phone_rate)} ownership</small></div>
      <div class="metric-tile accent-gold"><span>Confidence</span><strong>${Math.round(area.confidence * 100)}%</strong><small>${escapeHtml(area.metric_source)}</small></div>
      <div class="metric-tile accent-red"><span>Priority score</span><strong>${localPriority ? localPriority.priority_score.toFixed(0) : '0'}</strong><small>${localAlerts.length} alerts / ${localTickets.length} active tickets</small></div>
    </div>

    <div class="business-case-grid">
      <article class="business-card">
        <span>Estimated pilot budget</span>
        <strong>${formatMoneyXaf(economics?.estimated_budget_xaf || estimateBudgetXaf(area, context))}</strong>
        <p>Lean field validation, local coordination, and first response reserve.</p>
      </article>
      <article class="business-card">
        <span>Likely direct reach</span>
        <strong>${formatNumber(economics?.likely_reach || estimateReach(area))}</strong>
        <p>People likely affected by the first survey, repair, or access intervention.</p>
      </article>
      <article class="business-card">
        <span>Channel strategy</span>
        <strong>${escapeHtml(economics?.channel_strategy || channelRecommendation(area))}</strong>
        <p>Designed around uneven connectivity and trust-building field proof.</p>
      </article>
      <article class="business-card">
        <span>Execution risk</span>
        <strong>${escapeHtml(economics?.execution_risk || riskLabel(area, context))}</strong>
        <p>Driven by confidence, ownership, alerts, and whether a trusted site exists.</p>
      </article>
    </div>

    <div class="area-action-panel">
      <div>
        <p class="eyebrow">Recommended field action</p>
        <strong>${escapeHtml(economics?.next_action || actionText)}</strong>
      </div>
      <div class="export-actions">
        <button class="btn btn-outline-secondary btn-sm area-action" data-action="probe"><i data-lucide="radio-tower"></i> Probe</button>
        <button class="btn btn-outline-secondary btn-sm area-action" data-action="campaign"><i data-lucide="clipboard-plus"></i> Survey</button>
        <button class="btn btn-outline-secondary btn-sm area-action" data-action="report"><i data-lucide="clipboard-check"></i> Report</button>
        <button class="btn btn-outline-secondary btn-sm area-action" data-action="site"><i data-lucide="map-pin-plus"></i> Site</button>
        <button class="btn btn-outline-secondary btn-sm area-action" data-action="decision"><i data-lucide="file-plus-2"></i> Decision</button>
      </div>
    </div>

    ${dossier ? `
      <div class="dossier-intelligence">
        <div>
          <p class="eyebrow">Rust dossier intelligence</p>
          <strong>${escapeHtml(dossier.economics.trust_gap)}</strong>
        </div>
        <div class="probe-meta-grid">
          <div><span>Probes</span><strong>${dossier.assets.length}</strong></div>
          <div><span>Sites</span><strong>${dossier.sites.length}</strong></div>
          <div><span>Evidence</span><strong>${dossier.reports.length + dossier.readings.length}</strong></div>
          <div><span>Open work</span><strong>${dossier.alerts.length + dossier.tickets.length}</strong></div>
        </div>
        <div class="market-readout">${dossier.market_notes.map(note => `<p>${escapeHtml(note)}</p>`).join('')}</div>
      </div>
    ` : '<div class="empty-state">Loading Rust area dossier...</div>'}

    <div class="area-dossier-grid">
      <section class="surface nested-surface">
        <div class="surface-header"><div><p class="eyebrow">Proof layer</p><h2>Sites</h2></div><span class="status-pill">${localSites.length}</span></div>
        <div class="list-stack">${localSites.length ? localSites.map(site => `
          <article class="mini-card clickable-card profile-site-card" role="button" tabindex="0" data-site="${site.id}"><strong>${escapeHtml(site.name)}</strong><span>${escapeHtml(site.site_type)} &middot; ${formatNumber(site.beneficiary_estimate || 0)} beneficiaries</span><p>${escapeHtml(site.trust_signal)} &middot; ${escapeHtml(site.access_notes || 'No access notes')}</p></article>
        `).join('') : '<div class="empty-state">No site profile in this arrondissement.</div>'}</div>
      </section>
      <section class="surface nested-surface">
        <div class="surface-header"><div><p class="eyebrow">Monitored assets</p><h2>Assets</h2></div><span class="status-pill">${localAssets.length}</span></div>
        <div class="list-stack">${localAssets.length ? localAssets.map(asset => `
          <article class="mini-card clickable-card profile-asset-card status-${escapeHtml(asset.status)}" role="button" tabindex="0" data-id="${asset.id}"><strong>${escapeHtml(asset.name)}</strong><span>${escapeHtml(asset.asset_type)} &middot; ${escapeHtml(asset.status)} &middot; ${escapeHtml(probeHealthFor(asset)?.health_label || 'Not scored')}</span><p>${escapeHtml(asset.operator || 'No operator')} &middot; ${escapeHtml(asset.notes || 'No notes')}</p><div class="ticket-actions"><button class="btn btn-sm btn-outline-secondary" data-entity-detail data-entity-type="infrastructure_asset" data-entity-id="${asset.id}">Details</button><button class="btn btn-sm btn-outline-secondary asset-action" data-action="telemetry" data-id="${asset.id}">Telemetry</button><button class="btn btn-sm btn-outline-secondary asset-action" data-action="report" data-id="${asset.id}">Report</button><button class="btn btn-sm btn-outline-secondary asset-action" data-action="ticket" data-id="${asset.id}">Ticket</button></div></article>
        `).join('') : '<div class="empty-state">No monitored assets here.</div>'}</div>
      </section>
      <section class="surface nested-surface">
        <div class="surface-header"><div><p class="eyebrow">Ground truth</p><h2>Reports and campaigns</h2></div><span class="status-pill">${localReports.length + localCampaigns.length}</span></div>
        <div class="list-stack">${[
          ...localCampaigns.map(campaign => `<article class="mini-card clickable-card profile-campaign-card" role="button" tabindex="0" data-campaign="${campaign.id}"><strong>${escapeHtml(campaign.name)}</strong><span>${escapeHtml(campaign.form_type)} &middot; ${escapeHtml(campaign.status)}</span><p>${campaign.offline_enabled ? 'Offline-ready' : 'Online only'} &middot; ${escapeHtml(campaign.language_mode)}</p></article>`),
          ...localReports.slice(0, 4).map(report => `<article class="mini-card clickable-card profile-report-card" role="button" tabindex="0" data-key="${escapeHtml(areaKey(report))}"><strong>${escapeHtml(report.report_type)}</strong><span>${escapeHtml(report.status)} &middot; ${escapeHtml(report.evidence_quality)}</span><p>${escapeHtml(report.notes)}</p></article>`),
        ].join('') || '<div class="empty-state">No campaign or field report yet.</div>'}</div>
      </section>
      <section class="surface nested-surface">
        <div class="surface-header"><div><p class="eyebrow">Execution</p><h2>Alerts and tickets</h2></div><span class="status-pill">${localAlerts.length + localTickets.length}</span></div>
        <div class="list-stack">${[
          ...localAlerts.map(alert => `<article class="mini-card clickable-card profile-alert-card severity-${escapeHtml(alert.severity)}" role="button" tabindex="0" data-id="${alert.id}"><strong>${escapeHtml(alert.title)}</strong><span>${escapeHtml(alert.severity)} &middot; ${escapeHtml(alert.status)}</span><p>${escapeHtml(alert.message)}</p></article>`),
          ...localTickets.map(ticket => `<article class="mini-card clickable-card profile-ticket-card priority-${escapeHtml(ticket.priority)}" role="button" tabindex="0" data-id="${ticket.id}"><strong>${escapeHtml(ticket.title)}</strong><span>${escapeHtml(ticket.priority)} &middot; ${escapeHtml(ticket.status)}</span><p>${escapeHtml(ticket.assigned_to || 'Unassigned')} &middot; Due ${escapeHtml(ticket.due_date || 'not set')}</p></article>`),
        ].join('') || '<div class="empty-state">No open execution work.</div>'}</div>
      </section>
    </div>

    <div class="profile-notes">
      <strong>Market interpretation:</strong>
      ${area.phone_rate < 65 ? 'Low estimated ownership suggests stronger offline and SMS-assisted workflows.' : 'Phone access is relatively strong, so digital survey and WhatsApp-style coordination can work if trust proof is visible.'}
      Confidence is ${Math.round(area.confidence * 100)}%, so ${area.confidence < 0.7 ? 'field validation should happen before budget commitment.' : 'the matrix is usable for prioritization while field teams continue to collect proof.'}
    </div>
  `;

  document.querySelectorAll('.area-action').forEach(button => {
    button.addEventListener('click', () => prepareAreaAction(button.dataset.action, area));
  });
  document.querySelectorAll('.asset-action').forEach(button => {
    button.addEventListener('click', () => {
      const asset = assets.find(item => item.id === Number(button.dataset.id));
      if (asset) prepareAssetAction(button.dataset.action, asset);
    });
  });
  document.querySelectorAll('.profile-site-card').forEach(card => {
    const open = () => openSiteFollowUp(card.dataset.site);
    card.addEventListener('click', event => {
      if (!cardClickGuard(event)) open();
    });
    card.addEventListener('keydown', event => {
      if (event.key === 'Enter' || event.key === ' ') {
        event.preventDefault();
        open();
      }
    });
  });
  wireEntityDetailButtons();
  document.querySelectorAll('.profile-asset-card').forEach(card => {
    const open = () => openEntityDetail('infrastructure_asset', card.dataset.id);
    card.addEventListener('click', event => {
      if (!cardClickGuard(event)) open();
    });
    card.addEventListener('keydown', event => {
      if (event.key === 'Enter' || event.key === ' ') {
        event.preventDefault();
        open();
      }
    });
  });
  document.querySelectorAll('.profile-campaign-card').forEach(card => {
    const open = () => openCampaignFollowUp(card.dataset.campaign);
    card.addEventListener('click', event => {
      if (!cardClickGuard(event)) open();
    });
    card.addEventListener('keydown', event => {
      if (event.key === 'Enter' || event.key === ' ') {
        event.preventDefault();
        open();
      }
    });
  });
  document.querySelectorAll('.profile-report-card').forEach(card => {
    const open = () => {
      const reportArea = allStats.find(item => areaKey(item) === card.dataset.key);
      openAreaFollowUp(reportArea);
    };
    card.addEventListener('click', event => {
      if (!cardClickGuard(event)) open();
    });
    card.addEventListener('keydown', event => {
      if (event.key === 'Enter' || event.key === ' ') {
        event.preventDefault();
        open();
      }
    });
  });
  document.querySelectorAll('.profile-alert-card').forEach(card => {
    const open = () => openAlertFollowUp(card.dataset.id);
    card.addEventListener('click', event => {
      if (!cardClickGuard(event)) open();
    });
    card.addEventListener('keydown', event => {
      if (event.key === 'Enter' || event.key === ' ') {
        event.preventDefault();
        open();
      }
    });
  });
  document.querySelectorAll('.profile-ticket-card').forEach(card => {
    const open = () => openTicketFollowUp(card.dataset.id);
    card.addEventListener('click', event => {
      if (!cardClickGuard(event)) open();
    });
    card.addEventListener('keydown', event => {
      if (event.key === 'Enter' || event.key === ' ') {
        event.preventDefault();
        open();
      }
    });
  });
  if (window.lucide) lucide.createIcons();
}

function renderWorkspaces() {
  const orgSelect = document.getElementById('projectOrganization');
  orgSelect.innerHTML = '<option value="">No organization</option>' + organizations.map(organization => (
    `<option value="${organization.id}">${escapeHtml(organization.name)}</option>`
  )).join('');

  ['siteProject', 'campaignProject', 'decisionProject', 'assetProject', 'ticketProjectId'].forEach(id => {
    const select = document.getElementById(id);
    if (!select) return;
    select.innerHTML = '<option value="">No project</option>' + projects.map(project => (
      `<option value="${project.id}">${escapeHtml(project.name)}</option>`
    )).join('');
  });

  const assetSiteSelect = document.getElementById('assetSite');
  if (assetSiteSelect) {
    assetSiteSelect.innerHTML = '<option value="">No site profile</option>' + sites.map(site => (
      `<option value="${site.id}">${escapeHtml(site.name)} - ${escapeHtml(site.commune)}</option>`
    )).join('');
  }
  populateAssetLinkedControls();

  const health = workspaceDashboard?.health;
  const healthTarget = document.getElementById('workspace-health');
  if (healthTarget && health) {
    const cards = workspaceDashboard?.business_cards?.length ? workspaceDashboard.business_cards : [
      { label: 'Projects', value: health.projects, detail: `${health.organizations} client workspaces`, tone: 'bronze' },
      { label: 'Field sites', value: health.sites, detail: 'Physical proof layer', tone: 'green' },
      { label: 'Campaigns', value: health.campaigns, detail: 'Offline survey plans', tone: 'gold' },
      { label: 'Decision records', value: health.decision_snapshots, detail: `${health.active_tickets} active tickets`, tone: 'red' },
    ];
    healthTarget.innerHTML = cards.slice(0, 6).map((card, index) => `
      <div class="metric-tile clickable-card workspace-health-card accent-${card.tone === 'bronze' ? 'bronze' : card.tone === 'green' ? 'green' : card.tone === 'gold' ? 'gold' : 'red'} ${index === 0 ? 'featured-metric' : ''}" role="button" tabindex="0" data-jump="${index === 0 ? 'projects-list' : index === 1 ? 'sites-list' : index === 2 ? 'campaigns-list' : 'decision-snapshots-list'}">
        <span>${escapeHtml(card.label)}</span>
        <strong>${escapeHtml(card.value)}</strong>
        <small>${escapeHtml(card.detail)}</small>
      </div>
    `).join('');
  }

  const workspaceQuery = (workspaceSearch?.value || '').trim().toLowerCase();
  const workspaceStatus = workspaceStatusFilter?.value || 'all';
  const workspaceType = workspaceTypeFilter?.value || 'all';
  const organizationRows = (workspaceDashboard?.organization_intelligence || organizations.map(organization => ({ organization })))
    .filter(item => {
      const organization = item.organization;
      const haystack = [organization.name, organization.org_type, organization.contact_name, organization.contact_email].join(' ').toLowerCase();
      if (workspaceQuery && !haystack.includes(workspaceQuery)) return false;
      if (workspaceType !== 'all' && organization.org_type !== workspaceType) return false;
      return true;
    });
  document.getElementById('organizations-list').innerHTML = organizationRows.length ? organizationRows.map(item => {
    const organization = item.organization;
    return `
    <article class="list-card clickable-card workspace-organization-card status-online" role="button" tabindex="0" data-id="${organization.id}" data-search="${escapeHtml(organization.name)}">
      <div>
        <strong>${escapeHtml(organization.name)}</strong>
        <span>${escapeHtml(labelize(organization.org_type))} &middot; ${escapeHtml(organization.contact_name || 'No contact')}</span>
      </div>
      <span class="status-pill">Org #${organization.id}</span>
      <p>${escapeHtml(organization.contact_email || 'No email recorded')} &middot; ${item.project_count || 0} projects &middot; ${item.linked_site_count || 0} sites &middot; ${item.active_decision_count || 0} active decisions &middot; ${item.open_alert_count || 0} open alerts &middot; Last ${shortDate(item.last_activity || organization.created_at)}</p>
      <div class="ticket-actions">
        <button class="btn btn-sm btn-outline-secondary" data-entity-detail data-entity-type="organization" data-entity-id="${organization.id}">Details</button>
      </div>
    </article>
  `;
  }).join('') : '<div class="empty-state">No organizations yet. Create the first client, partner, or operating organization.</div>';

  const projectRows = (workspaceDashboard?.project_intelligence || projects.map(project => ({ project })))
    .filter(item => {
      const project = item.project;
      const haystack = [project.name, project.organization_name, project.sector, project.region, project.status].join(' ').toLowerCase();
      if (workspaceQuery && !haystack.includes(workspaceQuery)) return false;
      if (workspaceStatus !== 'all' && project.status !== workspaceStatus) return false;
      return true;
    });
  document.getElementById('projects-list').innerHTML = projectRows.length ? projectRows.map(item => {
    const project = item.project;
    return `
    <article class="list-card clickable-card workspace-project-card status-${escapeHtml(project.status)}" role="button" tabindex="0" data-id="${project.id}" data-search="${escapeHtml(project.name)}">
      <div>
        <strong>${escapeHtml(project.name)}</strong>
        <span>${escapeHtml(project.organization_name || 'No organization')} &middot; ${escapeHtml(labelize(project.sector))}</span>
      </div>
      <span class="status-pill">${escapeHtml(project.status)}</span>
      <p>${escapeHtml(project.region || 'National')} &middot; Starts ${shortDate(project.start_date)} &middot; ${item.site_count || 0} sites &middot; ${item.campaign_count || 0} campaigns &middot; ${item.decision_count || 0} decisions &middot; ${item.asset_count || 0} assets &middot; ${item.open_ticket_count || 0} open tickets &middot; ${Number(item.execution_readiness || 0).toFixed(0)}% ready</p>
      <p>${escapeHtml(item.recommended_next_action || 'Add proof and operational actions to improve readiness.')}</p>
      <div class="ticket-actions">
        <button class="btn btn-sm btn-outline-secondary" data-entity-detail data-entity-type="project" data-entity-id="${project.id}">Details</button>
      </div>
    </article>
  `;
  }).join('') : '<div class="empty-state">No projects match the current filters.</div>';

  const projectOpsTarget = document.getElementById('project-operating-list');
  if (projectOpsTarget) {
    projectOpsTarget.innerHTML = projects.length ? projects.map(project => {
      const projectSites = sites.filter(site => site.project_id === project.id);
      const projectAssets = assets.filter(asset => asset.project_id === project.id);
      const projectCampaigns = campaigns.filter(campaign => campaign.project_id === project.id);
      const projectTickets = tickets.filter(ticket => ticket.project_id === project.id && ticket.status !== 'done' && ticket.status !== 'cancelled');
      const projectDecisions = decisionSnapshots.filter(decision => decision.project_id === project.id);
      const readiness = Math.min(100,
        (projectSites.length ? 24 : 0)
        + (projectAssets.length ? 22 : 0)
        + (projectCampaigns.length ? 22 : 0)
        + (projectDecisions.length ? 20 : 0)
        + (projectTickets.length === 0 ? 12 : 6));
      return `
        <article class="list-card clickable-card workspace-project-card" role="button" tabindex="0" data-id="${project.id}" data-search="${escapeHtml(project.name)}">
          <div>
            <strong>${escapeHtml(project.name)}</strong>
            <span>${escapeHtml(project.organization_name || 'No organization')} &middot; ${escapeHtml(project.region || 'National')}</span>
          </div>
          <span class="priority-badge priority-${readiness >= 70 ? 'watch' : readiness >= 45 ? 'medium' : 'high'}">${readiness}% ready</span>
          <div class="workspace-progress"><span style="width:${readiness}%"></span></div>
          <p>${projectSites.length} sites &middot; ${projectAssets.length} assets &middot; ${projectCampaigns.length} campaigns &middot; ${projectTickets.length} active tickets &middot; ${projectDecisions.length} decisions</p>
          <div class="ticket-actions">
            <button class="btn btn-sm btn-outline-secondary" data-entity-detail data-entity-type="project" data-entity-id="${project.id}">Details</button>
            <button class="btn btn-sm btn-outline-secondary project-action" data-action="site" data-project="${project.id}">Site</button>
            <button class="btn btn-sm btn-outline-secondary project-action" data-action="campaign" data-project="${project.id}">Campaign</button>
            <button class="btn btn-sm btn-outline-secondary project-action" data-action="decision" data-project="${project.id}">Decision</button>
          </div>
        </article>
      `;
    }).join('') : '<div class="empty-state">Create a project to see execution readiness.</div>';
  }

  const realitiesTarget = document.getElementById('workspace-market-realities');
  if (realitiesTarget) {
    realitiesTarget.innerHTML = (workspaceDashboard?.market_realities || []).map((item, index) => `
      <article class="insight-card">
        <span>${index + 1}</span>
        <strong>${escapeHtml(item)}</strong>
      </article>
    `).join('');
  }

  const templatesTarget = document.getElementById('workspace-templates');
  if (templatesTarget) {
    templatesTarget.innerHTML = workspaceTemplates.length ? workspaceTemplates.map(template => `
      <button class="template-card workspace-template" data-template="${escapeHtml(template.id)}">
        <span>${escapeHtml(template.org_type.replaceAll('_', ' '))}</span>
        <strong>${escapeHtml(template.title)}</strong>
        <small>${escapeHtml(template.description)}</small>
        <small>${escapeHtml((template.required_evidence || []).join(' / '))}</small>
      </button>
    `).join('') : '<div class="empty-state">No backend templates are active.</div>';
  }

  const siteRows = workspaceDashboard?.site_intelligence || sites.map(site => ({ site }));
  document.getElementById('sites-list').innerHTML = siteRows.length ? siteRows.map(item => {
    const site = item.site;
    return `
    <article class="list-card clickable-card workspace-site-card" role="button" tabindex="0" data-site="${site.id}">
      <div>
        <strong>${escapeHtml(site.name)}</strong>
        <span>${escapeHtml(labelize(site.site_type))} &middot; ${escapeHtml(site.commune)}, ${escapeHtml(site.department)}</span>
      </div>
      <span class="status-pill">${escapeHtml(site.trust_signal)}</span>
      <p>${escapeHtml(site.project_name || 'No project')} &middot; ${formatNumber(site.beneficiary_estimate || 0)} people &middot; ${item.linked_assets || 0} assets &middot; ${item.linked_reports || 0} reports &middot; ${item.linked_alerts || 0} alerts &middot; ${item.linked_tickets || 0} tickets</p>
      <p>${escapeHtml(site.access_notes || 'No access notes')} &middot; GPS ${gpsLabel(site)}</p>
      <div class="ticket-actions">
        <button class="btn btn-sm btn-outline-secondary" data-entity-detail data-entity-type="site_profile" data-entity-id="${site.id}">Details</button>
        <button class="btn btn-sm btn-outline-secondary site-action" data-action="profile" data-site="${site.id}">Area</button>
        <button class="btn btn-sm btn-outline-secondary site-action" data-action="report" data-site="${site.id}">Report</button>
        <button class="btn btn-sm btn-outline-secondary site-action" data-action="probe" data-site="${site.id}">Probe</button>
        <button class="btn btn-sm btn-outline-secondary site-action" data-action="alert" data-site="${site.id}">Alert</button>
        <button class="btn btn-sm btn-outline-secondary site-action" data-action="ticket" data-site="${site.id}">Ticket</button>
        <button class="btn btn-sm btn-outline-secondary site-action" data-action="decision" data-site="${site.id}">Decision</button>
      </div>
    </article>
  `;
  }).join('') : '<div class="empty-state">No site profiles yet.</div>';

  const campaignRows = workspaceDashboard?.campaign_intelligence || campaigns.map(campaign => ({ campaign }));
  document.getElementById('campaigns-list').innerHTML = campaignRows.length ? campaignRows.map(item => {
    const campaign = item.campaign;
    return `
    <article class="list-card clickable-card workspace-campaign-card status-${escapeHtml(campaign.status)}" role="button" tabindex="0" data-campaign="${campaign.id}">
      <div>
        <strong>${escapeHtml(campaign.name)}</strong>
        <span>${escapeHtml(labelize(campaign.form_type))} &middot; ${escapeHtml(campaign.target_commune || campaign.target_region || 'National')}</span>
      </div>
      <span class="status-pill">${campaign.offline_enabled ? 'offline-ready' : 'online-only'}</span>
      <p>${escapeHtml(campaign.project_name || 'No project')} &middot; ${escapeHtml(campaign.language_mode)} &middot; ${shortDate(campaign.starts_on)} to ${shortDate(campaign.ends_on)} &middot; ${item.submitted_reports || 0} reports</p>
      <p>${escapeHtml(item.field_validation_purpose || 'Collect field validation evidence.')}</p>
      <div class="ticket-actions">
        <button class="btn btn-sm btn-outline-secondary" data-entity-detail data-entity-type="survey_campaign" data-entity-id="${campaign.id}">Details</button>
        <button class="btn btn-sm btn-outline-secondary campaign-action" data-action="reports" data-campaign="${campaign.id}">Reports</button>
        <button class="btn btn-sm btn-outline-secondary campaign-action" data-action="decision" data-campaign="${campaign.id}">Decision</button>
        ${campaignStatusActions(campaign)}
        <a class="btn btn-sm btn-outline-secondary" href="/api/export/phone-matrix.csv">Export</a>
      </div>
    </article>
  `;
  }).join('') : '<div class="empty-state">No survey campaigns yet.</div>';

  document.getElementById('decision-snapshots-list').innerHTML = decisionSnapshots.length ? decisionSnapshots.map(decision => `
    <article class="list-card clickable-card workspace-decision-card priority-${Number(decision.priority_score) >= 70 ? 'high' : Number(decision.priority_score) >= 45 ? 'medium' : 'watch'}" role="button" tabindex="0" data-decision="${decision.id}">
      <div>
        <strong>${escapeHtml(decision.title)}</strong>
        <span>${escapeHtml(decision.project_name || 'No project')} &middot; ${escapeHtml(decision.decision_stage)}</span>
      </div>
      <span class="priority-badge priority-${Number(decision.priority_score) >= 70 ? 'high' : Number(decision.priority_score) >= 45 ? 'medium' : 'watch'}">${Number(decision.priority_score).toFixed(0)}</span>
      <p>${escapeHtml(decision.rationale)} Next: ${escapeHtml(decision.next_action)}</p>
      <div class="ticket-actions">
        <button class="btn btn-sm btn-outline-secondary" data-entity-detail data-entity-type="decision_snapshot" data-entity-id="${decision.id}">Details</button>
      </div>
    </article>
  `).join('') : '<div class="empty-state">No decision snapshots yet.</div>';

  const activityTarget = document.getElementById('workspace-activity');
  if (activityTarget) {
    const activity = workspaceDashboard?.activity || [];
    activityTarget.innerHTML = activity.length ? activity.map(item => `
      <article class="compact-card">
        <div>
          <strong>${escapeHtml(item.action)}</strong>
          <span>${escapeHtml(item.related_entity)} &middot; ${escapeHtml(item.source)} &middot; ${shortDate(item.timestamp)}</span>
        </div>
        <span class="status-pill">Activity</span>
        <p>${escapeHtml(item.description)}</p>
      </article>
    `).join('') : '<div class="empty-state">No workspace activity yet.</div>';
  }

  if (window.lucide) lucide.createIcons();

  document.querySelectorAll('.workspace-health-card').forEach(card => {
    const open = () => document.getElementById(card.dataset.jump)?.scrollIntoView({ behavior: 'smooth', block: 'start' });
    card.addEventListener('click', event => {
      if (!cardClickGuard(event)) open();
    });
    card.addEventListener('keydown', event => {
      if (event.key === 'Enter' || event.key === ' ') {
        event.preventDefault();
        open();
      }
    });
  });

  document.querySelectorAll('.workspace-organization-card, .workspace-project-card').forEach(card => {
    const open = () => {
      if (card.classList.contains('workspace-organization-card')) return openEntityDetail('organization', card.dataset.id);
      return openEntityDetail('project', card.dataset.id);
    };
    card.addEventListener('click', event => {
      if (!cardClickGuard(event)) open();
    });
    card.addEventListener('keydown', event => {
      if (event.key === 'Enter' || event.key === ' ') {
        event.preventDefault();
        open();
      }
    });
  });

  document.querySelectorAll('.workspace-site-card').forEach(card => {
    const open = () => openEntityDetail('site_profile', card.dataset.site);
    card.addEventListener('click', event => {
      if (!cardClickGuard(event)) open();
    });
    card.addEventListener('keydown', event => {
      if (event.key === 'Enter' || event.key === ' ') {
        event.preventDefault();
        open();
      }
    });
  });

  document.querySelectorAll('.workspace-campaign-card').forEach(card => {
    const open = () => openEntityDetail('survey_campaign', card.dataset.campaign);
    card.addEventListener('click', event => {
      if (!cardClickGuard(event)) open();
    });
    card.addEventListener('keydown', event => {
      if (event.key === 'Enter' || event.key === ' ') {
        event.preventDefault();
        open();
      }
    });
  });

  document.querySelectorAll('.workspace-decision-card').forEach(card => {
    const open = () => openEntityDetail('decision_snapshot', card.dataset.decision);
    card.addEventListener('click', event => {
      if (!cardClickGuard(event)) open();
    });
    card.addEventListener('keydown', event => {
      if (event.key === 'Enter' || event.key === ' ') {
        event.preventDefault();
        open();
      }
    });
  });

  document.querySelectorAll('.project-action').forEach(button => {
    button.addEventListener('click', () => {
      const projectId = Number(button.dataset.project);
      const project = projects.find(item => item.id === projectId);
      prepareProjectAction(button.dataset.action, project);
    });
  });

  document.querySelectorAll('.site-action').forEach(button => {
    button.addEventListener('click', () => {
      const site = sites.find(item => item.id === Number(button.dataset.site));
      if (!site) return;
      const area = allStats.find(item => areaKey(item) === areaKey(site)) || site;
      if (button.dataset.action === 'profile') return selectArea(area);
      if (button.dataset.action === 'report') return prepareAreaAction('report', area);
      if (button.dataset.action === 'probe') return prepareAreaAction('probe', area);
      if (button.dataset.action === 'decision') return prepareAreaAction('decision', area);
      if (button.dataset.action === 'alert') {
        switchView('alerts');
        document.getElementById('alertSiteId').value = site.id;
        document.getElementById('alertAssetId').value = '';
        document.getElementById('alertTitle').value = `${site.name} validation alert`;
        document.getElementById('alertMessage').value = `${site.commune} site needs field validation. ${site.access_notes || 'Review access and trust proof.'}`;
        document.getElementById('alertTitle').focus();
      }
      if (button.dataset.action === 'ticket') {
        switchView('tickets');
        document.getElementById('ticketProjectId').value = site.project_id || '';
        document.getElementById('ticketSiteId').value = site.id;
        document.getElementById('ticketAssetId').value = '';
        document.getElementById('ticketAlertId').value = '';
        document.getElementById('ticketTitle').value = `${site.name} field follow-up`;
        document.getElementById('ticketPriority').value = 'high';
        document.getElementById('ticketTitle').focus();
      }
    });
  });

  document.querySelectorAll('.campaign-action').forEach(button => {
    button.addEventListener('click', () => {
      const campaign = campaigns.find(item => item.id === Number(button.dataset.campaign));
      if (!campaign) return;
      if (button.dataset.action === 'reports') {
        switchView('reports');
        document.getElementById('reportCampaignId').value = campaign.id;
        document.getElementById('reportType').value = campaign.form_type;
        document.getElementById('reportRegion').value = campaign.target_region || '';
        document.getElementById('reportDepartment').value = campaign.target_department || '';
        document.getElementById('reportCommune').value = campaign.target_commune || '';
        document.getElementById('reportType').focus();
      }
      if (button.dataset.action === 'decision') {
        switchView('workspaces');
        document.getElementById('decisionProject').value = campaign.project_id || '';
        document.getElementById('decisionSite').value = sites.find(site => (
          normalizeAreaPart(site.region) === normalizeAreaPart(campaign.target_region)
          && normalizeAreaPart(site.department) === normalizeAreaPart(campaign.target_department)
          && normalizeAreaPart(site.commune) === normalizeAreaPart(campaign.target_commune)
        ))?.id || '';
        document.getElementById('decisionAsset').value = '';
        document.getElementById('decisionTitle').value = `${campaign.name} evidence decision`;
        document.getElementById('decisionRationale').value = `${campaign.name} is ${campaign.status} for ${campaign.target_commune || campaign.target_region || 'the target area'} and should be converted into an evidence-backed decision.`;
        document.getElementById('decisionNextAction').value = 'Review submitted reports, confirm evidence quality, and approve the next field action.';
        document.getElementById('decisionTitle').focus();
      }
      if (button.dataset.action === 'status') {
        fetchJson(`/api/survey-campaigns/${campaign.id}/status`, {
          method: 'PATCH',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify({ status: button.dataset.status }),
        }).then(async data => {
          campaigns = data;
          workspaceDashboard = await fetchJson('/api/workspaces/dashboard');
          renderWorkspaces();
          setStatus(document.getElementById('campaign-status'), `${campaign.name} moved to ${button.dataset.status}.`, 'success');
        }).catch(error => setStatus(document.getElementById('campaign-status'), error.message, 'danger'));
      }
    });
  });

  document.querySelectorAll('.workspace-template').forEach(button => {
    button.addEventListener('click', () => applyWorkspaceTemplate(button.dataset.template));
  });
}

function renderRegions(regions) {
  currentMatrixRows = applyMatrixControls(regions);
  renderMatrixInsights(currentMatrixRows);
  if (!regions.length) {
    tableBody.innerHTML = '<tr><td colspan="13" class="text-center text-muted py-4">No areas match the selected filters.</td></tr>';
    return;
  }

  if (!currentMatrixRows.length) {
    tableBody.innerHTML = '<tr><td colspan="13" class="text-center text-muted py-4">No areas match the matrix controls.</td></tr>';
    return;
  }

  tableBody.innerHTML = currentMatrixRows.map(area => {
    const width = Math.min(Math.max(area.phone_rate, 0), 100);
    const priority = priorityForArea(area);
    return `
      <tr class="matrix-row" data-key="${escapeHtml(areaKey(area))}">
        <td data-label="Area"><strong>${escapeHtml(area.commune)}</strong><small class="matrix-row-meta">${escapeHtml(area.location || gpsLabel(area))}</small></td>
        <td data-label="P-code"><code>${escapeHtml(area.pcode || 'Manual')}</code></td>
        <td data-label="Region">${escapeHtml(area.region)}</td>
        <td data-label="Department">${escapeHtml(area.department)}</td>
        <td data-label="Population">${formatNumber(area.population)}</td>
        <td data-label="Phone owners">${formatNumber(area.phone_owners)}</td>
        <td data-label="Subscriptions">${formatNumber(area.estimated_mobile_subscriptions || Math.round(area.phone_owners * 1.08))}</td>
        <td data-label="Ownership rate"><div class="progress ownership-progress"><div class="progress-bar" style="width:${width.toFixed(1)}%">${formatRate(area.phone_rate)}</div></div></td>
        <td data-label="Confidence">
          <span class="confidence-pill">${escapeHtml(area.confidence_level || `${Math.round(area.confidence * 100)}%`)}</span>
          <small class="matrix-row-meta">${Math.round(area.confidence * 100)}% confidence</small>
        </td>
        <td data-label="Opportunity"><span class="priority-badge priority-${(area.opportunity_level || 'Medium').toLowerCase()}">${escapeHtml(area.opportunity_level || 'Medium')} ${Number(area.opportunity_score || 0).toFixed(0)}</span></td>
        <td data-label="Priority"><span class="priority-badge priority-${escapeHtml((priority?.priority_label || area.priority_label || 'Watch').toLowerCase())}">${Number(priority?.priority_score || area.priority_score || 0).toFixed(0)}</span></td>
        <td data-label="Recommended action">${escapeHtml(area.recommended_action || areaActionText(area, localContextForArea(area)))}</td>
        <td data-label="Actions">
          <div class="ticket-actions">
            <button class="row-action matrix-profile-action" data-action="profile" data-key="${escapeHtml(areaKey(area))}" title="Open area profile"><i data-lucide="arrow-right"></i></button>
            <button class="row-action matrix-area-action" data-action="campaign" data-key="${escapeHtml(areaKey(area))}" title="Create campaign"><i data-lucide="clipboard-plus"></i></button>
            <button class="row-action matrix-area-action" data-action="decision" data-key="${escapeHtml(areaKey(area))}" title="Create decision"><i data-lucide="file-plus-2"></i></button>
            <button class="row-action matrix-area-action" data-action="report" data-key="${escapeHtml(areaKey(area))}" title="Mark for validation"><i data-lucide="badge-check"></i></button>
          </div>
        </td>
      </tr>
    `;
  }).join('');

  document.querySelectorAll('.matrix-row').forEach(row => {
    row.addEventListener('click', () => {
      const area = allStats.find(item => areaKey(item) === row.dataset.key);
      if (area) {
        renderMatrixDetailPanel(area);
        selectArea(area, null);
      }
    });
  });
  document.querySelectorAll('.matrix-area-action').forEach(button => {
    button.addEventListener('click', event => {
      event.stopPropagation();
      const area = allStats.find(item => areaKey(item) === button.dataset.key);
      if (area) prepareAreaAction(button.dataset.action, area);
    });
  });
  if (currentMatrixRows[0]) renderMatrixDetailPanel(currentMatrixRows[0]);
  if (window.lucide) lucide.createIcons();
}

function applyMatrixControls(regions) {
  const query = (matrixSearch?.value || '').trim().toLowerCase();
  const ownership = matrixOwnershipFilter?.value || 'all';
  const confidence = matrixConfidenceFilter?.value || 'all';
  const opportunity = matrixOpportunityFilter?.value || 'all';
  const validation = matrixValidationFilter?.value || 'all';
  const projectCoverage = matrixProjectFilter?.value || 'all';
  const minPopulation = Number(matrixMinPopulation?.value || 0);
  const minPriority = matrixMinPriority?.value ? Number(matrixMinPriority.value) : null;
  const maxPriority = matrixMaxPriority?.value ? Number(matrixMaxPriority.value) : null;
  const sort = matrixSort?.value || 'priority';

  return regions.filter(area => {
    const haystack = [area.pcode, area.region, area.department, area.commune, gpsLabel(area)]
      .join(' ')
      .toLowerCase();
    if (query && !haystack.includes(query)) return false;
    if (ownership === 'under65' && area.phone_rate >= 65) return false;
    if (ownership === '65to78' && (area.phone_rate < 65 || area.phone_rate > 78)) return false;
    if (ownership === 'over78' && area.phone_rate <= 78) return false;
    if (confidence === 'low' && area.confidence >= 0.68) return false;
    if (confidence === 'medium' && (area.confidence < 0.68 || area.confidence > 0.78)) return false;
    if (confidence === 'high' && area.confidence <= 0.78) return false;
    if (opportunity !== 'all' && String(area.opportunity_level || '').toLowerCase() !== opportunity) return false;
    if (validation === 'needs' && !area.needs_validation) return false;
    if (validation === 'covered' && area.needs_validation) return false;
    if (projectCoverage === 'linked' && !(area.project_count || area.site_count || area.campaign_count)) return false;
    if (projectCoverage === 'unlinked' && (area.project_count || area.site_count || area.campaign_count)) return false;
    if (minPopulation && area.population < minPopulation) return false;
    if (minPriority !== null && Number(area.priority_score || priorityForArea(area)?.priority_score || 0) < minPriority) return false;
    if (maxPriority !== null && Number(area.priority_score || priorityForArea(area)?.priority_score || 0) > maxPriority) return false;
    return true;
  }).sort((a, b) => {
    if (sort === 'ownership-low') return a.phone_rate - b.phone_rate;
    if (sort === 'population-high') return b.population - a.population;
    if (sort === 'confidence-low') return a.confidence - b.confidence;
    if (sort === 'name') return a.commune.localeCompare(b.commune);
    return (b.priority_score || priorityForArea(b)?.priority_score || 0) - (a.priority_score || priorityForArea(a)?.priority_score || 0);
  });
}

function renderMatrixInsights(rows) {
  const target = document.getElementById('matrix-insights');
  if (!target) return;
  const population = rows.reduce((sum, row) => sum + row.population, 0);
  const phoneOwners = rows.reduce((sum, row) => sum + row.phone_owners, 0);
  const subscriptions = rows.reduce((sum, row) => sum + (row.estimated_mobile_subscriptions || Math.round(row.phone_owners * 1.08)), 0);
  const avgOwnership = population ? (phoneOwners / population) * 100 : 0;
  const highOpportunity = rows.filter(row => Number(row.opportunity_score || 0) >= 68).length;
  const lowConfidence = rows.filter(row => row.confidence < 0.68 || row.confidence_level === 'Low').length;
  const validationCount = rows.filter(row => row.needs_validation).length;
  const topRegion = rows.reduce((acc, row) => {
    acc[row.region] = (acc[row.region] || 0) + Number(row.opportunity_score || 0);
    return acc;
  }, {});
  const topRegionName = Object.entries(topRegion).sort((a, b) => b[1] - a[1])[0]?.[0] || 'None';
  target.innerHTML = `
    <div class="metric-tile accent-bronze featured-metric"><span>Total population analyzed</span><strong>${formatNumber(population)}</strong><small>${formatNumber(rows.length)} areas in view</small></div>
    <div class="metric-tile accent-green"><span>Estimated phone owners</span><strong>${formatNumber(phoneOwners)}</strong><small>${formatRate(avgOwnership)} average ownership</small></div>
    <div class="metric-tile accent-gold"><span>Mobile subscriptions</span><strong>${formatNumber(subscriptions)}</strong><small>Modeled active SIM/subscription footprint</small></div>
    <div class="metric-tile accent-red"><span>Needs validation</span><strong>${validationCount}</strong><small>${lowConfidence} low-confidence areas</small></div>
    <div class="metric-tile accent-green"><span>High opportunity</span><strong>${highOpportunity}</strong><small>Best areas for campaigns or decisions</small></div>
    <div class="metric-tile accent-gold"><span>Top region</span><strong>${escapeHtml(topRegionName)}</strong><small>Highest opportunity score in current filter</small></div>
  `;
  renderMatrixActionLab(rows);
}

function renderMatrixActionLab(rows) {
  const target = document.getElementById('matrix-action-lab');
  if (!target) return;
  const topRows = rows.slice(0, 3);
  const totalBudget = topRows.reduce((sum, area) => sum + estimateBudgetXaf(area), 0);
  const totalReach = topRows.reduce((sum, area) => sum + estimateReach(area), 0);
  target.innerHTML = `
    <div>
      <p class="eyebrow">Action lab</p>
      <strong>${topRows.length ? `${topRows.length} best next areas in current filter` : 'No matching areas'}</strong>
      <span>${formatMoneyXaf(totalBudget)} estimated pilot budget &middot; ${formatNumber(totalReach)} likely direct reach</span>
    </div>
    <div class="matrix-action-list">
      ${topRows.map(area => `
        <button class="matrix-chip matrix-chip-action" data-key="${escapeHtml(areaKey(area))}">
          <strong>${escapeHtml(area.commune)}</strong>
          <span>${formatRate(area.phone_rate)} ownership &middot; ${priorityForArea(area)?.priority_score.toFixed(0) || '0'} score</span>
        </button>
      `).join('')}
    </div>
    <div class="export-actions">
      <button class="btn btn-outline-secondary btn-sm" id="matrix-top-campaign"><i data-lucide="clipboard-plus"></i> Campaign from top</button>
      <button class="btn btn-outline-secondary btn-sm" id="matrix-top-decision"><i data-lucide="file-plus-2"></i> Decision from top</button>
    </div>
  `;

  document.querySelectorAll('.matrix-chip-action').forEach(button => {
    button.addEventListener('click', () => {
      const area = allStats.find(item => areaKey(item) === button.dataset.key);
      if (area) selectArea(area);
    });
  });
  document.getElementById('matrix-top-campaign')?.addEventListener('click', () => {
    if (topRows[0]) prepareAreaAction('campaign', topRows[0]);
  });
  document.getElementById('matrix-top-decision')?.addEventListener('click', () => {
    if (topRows[0]) prepareAreaAction('decision', topRows[0]);
  });
  if (window.lucide) lucide.createIcons();
}

function renderMatrixDetailPanel(area) {
  const target = document.getElementById('matrix-detail-panel');
  if (!target || !area) return;
  const assumptions = phoneMatrixDashboard?.assumptions || {
    adult_share: 0.6,
    national_adult_phone_ownership: 0.8,
    mobile_subscriptions_per_person: 1.082,
    assumption_version: 'KK-EVO-CMR-2026.05',
  };
  const regionalFactor = area.region === 'Centre' || area.region === 'Littoral' ? 1.24 : area.region === 'Ouest' ? 1.08 : 1;
  const urbanFactor = Math.max(0.72, Math.min(1.22, 0.78 + Number(area.urban_signal || 0) * 0.44));
  const maxOwners = Math.round(area.population * assumptions.adult_share * 0.95);
  target.innerHTML = `
    <div>
      <p class="eyebrow">Selected area calculation</p>
      <strong>${escapeHtml(area.commune)}, ${escapeHtml(area.department)}</strong>
    </div>
    <div class="probe-meta-grid">
      <div><span>Population</span><strong>${formatNumber(area.population)}</strong></div>
      <div><span>Adult share</span><strong>${Math.round(assumptions.adult_share * 100)}%</strong></div>
      <div><span>Adult ownership</span><strong>${Math.round(assumptions.national_adult_phone_ownership * 100)}%</strong></div>
      <div><span>Regional factor</span><strong>${regionalFactor.toFixed(2)}</strong></div>
      <div><span>Urban/rural factor</span><strong>${urbanFactor.toFixed(2)}</strong></div>
      <div><span>Estimated owners</span><strong>${formatNumber(area.phone_owners)}</strong></div>
      <div><span>Max allowed</span><strong>${formatNumber(maxOwners)}</strong></div>
      <div><span>Subscriptions</span><strong>${formatNumber(area.estimated_mobile_subscriptions || Math.round(area.phone_owners * assumptions.mobile_subscriptions_per_person))}</strong></div>
    </div>
    <div class="market-readout">
      <p>Estimated phone owners = Population x adult share x ownership rate x regional factor x urban/rural factor, capped at Population x adult share x 0.95.</p>
      <p>${escapeHtml(area.confidence_reason || 'These are intelligence estimates, not official measured phone ownership counts.')}</p>
      <p>${escapeHtml(area.validation_reason || area.recommended_action || 'Review confidence and field proof before committing budget.')}</p>
      <p>Assumption version: ${escapeHtml(assumptions.assumption_version)}. Changing assumptions affects future estimates; recalculate affected areas to update records.</p>
    </div>
    <div class="export-actions">
      <button class="btn btn-outline-secondary btn-sm matrix-detail-action" data-action="campaign"><i data-lucide="clipboard-plus"></i> Campaign</button>
      <button class="btn btn-outline-secondary btn-sm matrix-detail-action" data-action="site"><i data-lucide="map-pin-plus"></i> Site</button>
      <button class="btn btn-outline-secondary btn-sm matrix-detail-action" data-action="decision"><i data-lucide="file-plus-2"></i> Decision</button>
      <button class="btn btn-outline-secondary btn-sm matrix-detail-action" data-action="report"><i data-lucide="badge-check"></i> Field validation</button>
      <button class="btn btn-outline-secondary btn-sm" id="matrix-recalculate-selected"><i data-lucide="refresh-cw"></i> Recalculate selected</button>
    </div>
  `;
  document.querySelectorAll('.matrix-detail-action').forEach(button => {
    button.addEventListener('click', () => prepareAreaAction(button.dataset.action, area));
  });
  document.getElementById('matrix-recalculate-selected')?.addEventListener('click', async () => {
    const logs = await fetchJson('/api/phone-matrix/recalculate', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ scope: 'selected', region: area.region, department: area.department, commune: area.commune }),
    });
    setStatus(dataStatus, `${logs.length} recalculation log generated for ${area.commune}.`, 'success');
  });
  if (window.lucide) lucide.createIcons();
}

function createOption(value, label) {
  const option = document.createElement('option');
  option.value = value;
  option.textContent = label;
  return option;
}

function assetOptionLabel(asset) {
  return `#${asset.id} ${asset.name} - ${asset.commune} (${asset.status})`;
}

function siteOptionLabel(site) {
  return `#${site.id} ${site.name} - ${site.commune}`;
}

function campaignOptionLabel(campaign) {
  return `#${campaign.id} ${campaign.name} - ${campaign.target_commune || campaign.target_region || 'National'}`;
}

function alertOptionLabel(alert) {
  return `#${alert.id} ${alert.title} (${alert.status})`;
}

function projectOptionLabel(project) {
  return `#${project.id} ${project.name}`;
}

function setSelectOptions(select, placeholder, rows, labelBuilder, selectedValue = '') {
  if (!select) return;
  const selected = selectedValue || select.value;
  select.innerHTML = `<option value="">${escapeHtml(placeholder)}</option>`
    + rows.map(row => `<option value="${row.id}">${escapeHtml(labelBuilder(row))}</option>`).join('');
  if (selected && rows.some(row => String(row.id) === String(selected))) {
    select.value = selected;
  }
}

function projectIdFromLinkedRecords({ projectId, siteId, assetId, campaignId, alertId }) {
  if (projectId) return Number(projectId);
  const asset = assets.find(item => item.id === Number(assetId));
  if (asset?.project_id) return asset.project_id;
  const site = sites.find(item => item.id === Number(siteId || asset?.site_profile_id));
  if (site?.project_id) return site.project_id;
  const campaign = campaigns.find(item => item.id === Number(campaignId));
  if (campaign?.project_id) return campaign.project_id;
  const alert = alerts.find(item => item.id === Number(alertId));
  if (alert?.project_id) return alert.project_id;
  return null;
}

function populateAssetLinkedControls() {
  const reportSelect = document.getElementById('reportAssetId');
  const reportSiteSelect = document.getElementById('reportSiteId');
  const reportCampaignSelect = document.getElementById('reportCampaignId');
  const alertSelect = document.getElementById('alertAssetId');
  const alertSiteSelect = document.getElementById('alertSiteId');
  const iotSelect = document.getElementById('iotAssetId');
  const decisionSiteSelect = document.getElementById('decisionSite');
  const decisionAssetSelect = document.getElementById('decisionAsset');
  const ticketProjectSelect = document.getElementById('ticketProjectId');
  const ticketSiteSelect = document.getElementById('ticketSiteId');
  const ticketAssetSelect = document.getElementById('ticketAssetId');
  const ticketAlertSelect = document.getElementById('ticketAlertId');

  setSelectOptions(reportSelect, 'No linked asset', assets, assetOptionLabel);
  setSelectOptions(alertSelect, 'General coverage alert', assets, assetOptionLabel);
  setSelectOptions(iotSelect, 'Select monitored probe or asset', assets, assetOptionLabel);
  setSelectOptions(decisionAssetSelect, 'No linked asset', assets, assetOptionLabel);
  setSelectOptions(ticketAssetSelect, 'No linked asset', assets, assetOptionLabel);

  setSelectOptions(reportSiteSelect, 'No linked site profile', sites, siteOptionLabel);
  setSelectOptions(alertSiteSelect, 'No linked site profile', sites, siteOptionLabel);
  setSelectOptions(decisionSiteSelect, 'No linked site profile', sites, siteOptionLabel);
  setSelectOptions(ticketSiteSelect, 'No linked site profile', sites, siteOptionLabel);

  setSelectOptions(reportCampaignSelect, 'No linked campaign', campaigns, campaignOptionLabel);
  setSelectOptions(ticketAlertSelect, 'No linked alert', alerts, alertOptionLabel);
  setSelectOptions(ticketProjectSelect, 'Infer project from linked record', projects, projectOptionLabel);
}

function fillTelemetryCoordinatesFromAsset() {
  const asset = assets.find(item => item.id === Number(document.getElementById('iotAssetId')?.value));
  if (!asset) return;
  document.getElementById('iotLatitude').value = formatCoordinate(asset.latitude);
  document.getElementById('iotLongitude').value = formatCoordinate(asset.longitude);
  if (!document.getElementById('iotValue').value) {
    document.getElementById('iotValue').value = asset.status === 'online' ? 82 : asset.status === 'warning' ? 54 : 22;
  }
}

function populateFilter(select, values, selected) {
  select.innerHTML = '';
  select.appendChild(createOption('all', `All ${select.dataset.label}`));
  values.forEach(value => {
    const option = createOption(value, value);
    option.selected = value === selected;
    select.appendChild(option);
  });
}

function buildFilterOptions() {
  const selectedRegion = regionFilter.value;
  const selectedDepartment = departmentFilter.value;
  const selectedCommune = communeFilter.value;
  const regions = [...new Set(allStats.map(item => item.region))].sort();
  populateFilter(regionFilter, regions, regions.includes(selectedRegion) ? selectedRegion : null);

  const departments = [...new Set(allStats
    .filter(item => regionFilter.value === 'all' || item.region === regionFilter.value)
    .map(item => item.department))].sort();
  populateFilter(departmentFilter, departments, departments.includes(selectedDepartment) ? selectedDepartment : null);

  const communes = [...new Set(allStats
    .filter(item => (regionFilter.value === 'all' || item.region === regionFilter.value)
      && (departmentFilter.value === 'all' || item.department === departmentFilter.value))
    .map(item => item.commune))].sort();
  populateFilter(communeFilter, communes, communes.includes(selectedCommune) ? selectedCommune : null);
}

function filteredStats() {
  return allStats.filter(item => (regionFilter.value === 'all' || item.region === regionFilter.value)
    && (departmentFilter.value === 'all' || item.department === departmentFilter.value)
    && (communeFilter.value === 'all' || item.commune === communeFilter.value));
}

function updateMapMarkers(stats) {
  markersLayer.clearLayers();
  assetLayer.clearLayers();
  reportLayer.clearLayers();

  const bounds = [];
  stats.forEach(area => {
    if (!isInCameroon(area.latitude, area.longitude)) return;
    const marker = L.circleMarker([area.latitude, area.longitude], {
      radius: Math.max(6, Math.min(15, area.phone_rate / 7)),
      fillColor: area.phone_rate >= 78 ? '#1f4a34' : area.phone_rate >= 64 ? '#8a6a2f' : '#8b4a2f',
      color: '#fff',
      weight: 2,
      opacity: 1,
      fillOpacity: 0.82,
    });
    marker.bindPopup(`
      <strong>${escapeHtml(area.commune)}</strong><br />
      ${escapeHtml(area.department)}, ${escapeHtml(area.region)}<br />
      Population: ${formatNumber(area.population)}<br />
      Phone owners: ${formatNumber(area.phone_owners)}<br />
      Ownership: ${formatRate(area.phone_rate)}<br />
      Confidence: ${Math.round(area.confidence * 100)}%
    `);
    marker.on('click', () => {
      selectArea(area, null);
    });
    marker.addTo(markersLayer);
    bounds.push([area.latitude, area.longitude]);
  });

  assets.forEach(asset => {
    if (!isInCameroon(asset.latitude, asset.longitude)) return;
    const marker = L.marker([asset.latitude, asset.longitude]);
    marker.bindPopup(`<strong>${escapeHtml(asset.name)}</strong><br />${escapeHtml(asset.asset_type)} &middot; ${escapeHtml(asset.status)}<br />${escapeHtml(asset.commune)}`);
    marker.addTo(assetLayer);
  });

  reports.slice(0, 50).forEach(report => {
    if (!isInCameroon(report.latitude, report.longitude)) return;
    const marker = L.circleMarker([report.latitude, report.longitude], {
      radius: 5,
      fillColor: '#d59a28',
      color: '#fff',
      weight: 1,
      fillOpacity: 0.9,
    });
    marker.bindPopup(`<strong>${escapeHtml(report.report_type)}</strong><br />${escapeHtml(report.notes)}<br />${escapeHtml(report.commune)}`);
    marker.addTo(reportLayer);
  });

  if (bounds.length) map.fitBounds(L.latLngBounds(bounds), { padding: [36, 36], maxZoom: 10 });
}

function renderAssets() {
  renderSignalProbeHealth();
  const rows = filteredAssets();
  const target = document.getElementById('assets-list');
  target.innerHTML = rows.length ? rows.map(asset => {
    const context = assetContext(asset);
    const health = context.health;
    return `
      <article class="probe-card clickable-card asset-card status-${escapeHtml(asset.status)}" role="button" tabindex="0" data-id="${asset.id}">
        <div>
          <p class="eyebrow">${escapeHtml(asset.asset_type.replaceAll('_', ' '))}</p>
          <strong>${escapeHtml(asset.name)}</strong>
          <span>Asset #${asset.id} &middot; ${escapeHtml(asset.commune)}, ${escapeHtml(asset.department)} &middot; ${escapeHtml(asset.project_name || 'No project')}</span>
        </div>
        <div class="probe-health">
          <span class="status-pill status-${escapeHtml(asset.status)}">${escapeHtml(asset.status)}</span>
          <strong>${health ? health.health_score.toFixed(0) : '0'}</strong>
          <small>${escapeHtml(health?.health_label || 'Not scored')}</small>
        </div>
        <p>${escapeHtml(health?.recommended_action || asset.notes || 'No action recommendation yet.')}</p>
        <div class="probe-meta-grid">
          <div><span>Alerts</span><strong>${context.assetAlerts.length}</strong></div>
          <div><span>Tickets</span><strong>${context.assetTickets.length}</strong></div>
          <div><span>Reports</span><strong>${context.assetReports.length}</strong></div>
          <div><span>Readings</span><strong>${context.assetReadings.length}</strong></div>
        </div>
        <div class="ticket-actions">
          <button class="btn btn-sm btn-outline-secondary asset-action" data-action="profile" data-id="${asset.id}"><i data-lucide="map-pin"></i> Area</button>
          <button class="btn btn-sm btn-outline-secondary" data-entity-detail data-entity-type="infrastructure_asset" data-entity-id="${asset.id}"><i data-lucide="file-search"></i> Details</button>
          <button class="btn btn-sm btn-outline-secondary asset-action" data-action="telemetry" data-id="${asset.id}"><i data-lucide="activity"></i> Telemetry</button>
          <button class="btn btn-sm btn-outline-secondary asset-action" data-action="report" data-id="${asset.id}"><i data-lucide="clipboard-check"></i> Report</button>
          <button class="btn btn-sm btn-outline-secondary asset-action" data-action="alert" data-id="${asset.id}"><i data-lucide="triangle-alert"></i> Alert</button>
          <button class="btn btn-sm btn-outline-secondary asset-action" data-action="ticket" data-id="${asset.id}"><i data-lucide="wrench"></i> Ticket</button>
          <button class="btn btn-sm btn-success asset-status-action" data-status="online" data-id="${asset.id}"><i data-lucide="check"></i> Online</button>
          <button class="btn btn-sm btn-outline-secondary asset-status-action" data-status="warning" data-id="${asset.id}"><i data-lucide="circle-alert"></i> Watch</button>
          <button class="btn btn-sm btn-outline-secondary asset-status-action" data-status="critical" data-id="${asset.id}"><i data-lucide="octagon-alert"></i> Critical</button>
        </div>
      </article>
    `;
  }).join('') : '<div class="empty-state">No signal probes match the current filters.</div>';
  populateAssetLinkedControls();

  document.querySelectorAll('.asset-action').forEach(button => {
    button.addEventListener('click', () => {
      const asset = assets.find(item => item.id === Number(button.dataset.id));
      if (asset) prepareAssetAction(button.dataset.action, asset);
    });
  });
  wireEntityDetailButtons();
  document.querySelectorAll('.asset-card').forEach(card => {
    const open = () => openEntityDetail('infrastructure_asset', card.dataset.id);
    card.addEventListener('click', event => {
      if (!cardClickGuard(event)) open();
    });
    card.addEventListener('keydown', event => {
      if (event.key === 'Enter' || event.key === ' ') {
        event.preventDefault();
        open();
      }
    });
  });
  document.querySelectorAll('.asset-status-action').forEach(button => {
    button.addEventListener('click', async () => {
      const asset = assets.find(item => item.id === Number(button.dataset.id));
      assets = await fetchJson(`/api/assets/${button.dataset.id}/status`, {
        method: 'PATCH',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          status: button.dataset.status,
          notes: asset ? `${asset.name} marked ${button.dataset.status} from Signal Probes console.` : null,
        }),
      });
      signalProbeDashboard = await fetchJson('/api/signal-probes/dashboard');
      renderAssets();
      updateView();
      await refreshOverviewLayer();
    });
  });
  if (window.lucide) lucide.createIcons();
}

function renderSignalProbeHealth() {
  const target = document.getElementById('signal-probe-health');
  if (!target || !signalProbeDashboard) return;
  target.innerHTML = `
    <div class="metric-tile accent-bronze featured-metric"><span>Total probes</span><strong>${signalProbeDashboard.total_probes}</strong><small>${signalProbeDashboard.online_probes} online across monitored sites</small></div>
    <div class="metric-tile accent-green"><span>Healthy online</span><strong>${signalProbeDashboard.online_probes}</strong><small>Ready for continued monitoring</small></div>
    <div class="metric-tile accent-gold"><span>Watch list</span><strong>${signalProbeDashboard.warning_probes}</strong><small>Need field or telemetry follow-up</small></div>
    <div class="metric-tile accent-red"><span>Critical/offline</span><strong>${signalProbeDashboard.critical_probes + signalProbeDashboard.offline_probes}</strong><small>${signalProbeDashboard.open_alerts} open alerts / ${signalProbeDashboard.active_tickets} active tickets</small></div>
  `;
}

function renderReports() {
  document.getElementById('reports-list').innerHTML = reports.map(report => `
    <article class="list-card clickable-card report-card" role="button" tabindex="0" data-id="${report.id}" data-key="${escapeHtml(areaKey(report))}">
      <div><strong>${escapeHtml(report.report_type)}</strong><span>${escapeHtml(report.commune)}, ${escapeHtml(report.department)} &middot; ${escapeHtml(report.submitted_by)}</span></div>
      <span class="status-pill">${escapeHtml(report.status)}</span>
      <p>${escapeHtml(report.notes)}</p>
      <div class="ticket-actions">
        <button class="btn btn-sm btn-outline-secondary" data-entity-detail data-entity-type="field_report" data-entity-id="${report.id}">Details</button>
      </div>
    </article>
  `).join('');

  wireEntityDetailButtons();
  document.querySelectorAll('.report-card').forEach(card => {
    const open = () => openEntityDetail('field_report', card.dataset.id);
    card.addEventListener('click', event => {
      if (!cardClickGuard(event)) open();
    });
    card.addEventListener('keydown', event => {
      if (event.key === 'Enter' || event.key === ' ') {
        event.preventDefault();
        open();
      }
    });
  });
}

function renderAlerts() {
  document.getElementById('alerts-list').innerHTML = alerts.map(alert => `
    <article class="list-card clickable-card alert-card severity-${escapeHtml(alert.severity)}" role="button" tabindex="0" data-id="${alert.id}">
      <div><strong>${escapeHtml(alert.title)}</strong><span>${escapeHtml(alert.severity)} &middot; ${escapeHtml(alert.status)}</span></div>
      <div class="ticket-actions">
        <button class="btn btn-sm btn-outline-secondary" data-entity-detail data-entity-type="alert" data-entity-id="${alert.id}">Details</button>
        ${alert.status === 'open' ? `<button class="btn btn-sm btn-outline-secondary alert-status-action" data-id="${alert.id}" data-status="acknowledged">Acknowledge</button>` : ''}
        <button class="btn btn-sm btn-outline-secondary alert-ticket-action" data-id="${alert.id}">Ticket</button>
        ${alert.status !== 'resolved' ? `<button class="btn btn-sm btn-outline-secondary alert-status-action" data-id="${alert.id}" data-status="resolved">Resolve</button>` : ''}
      </div>
      <p>${escapeHtml(alert.message)}</p>
    </article>
  `).join('');

  document.querySelectorAll('.alert-ticket-action').forEach(button => {
    button.addEventListener('click', () => {
      const alert = alerts.find(item => item.id === Number(button.dataset.id));
      if (!alert) return;
      switchView('tickets');
      document.getElementById('ticketProjectId').value = alert.project_id || '';
      document.getElementById('ticketSiteId').value = alert.site_profile_id || '';
      document.getElementById('ticketAlertId').value = alert.id;
      document.getElementById('ticketAssetId').value = alert.asset_id || '';
      document.getElementById('ticketTitle').value = `${alert.title} follow-up`;
      document.getElementById('ticketPriority').value = alert.severity === 'critical' ? 'urgent' : 'high';
      document.getElementById('ticketAssignedTo').value = 'Coverage response team';
      document.getElementById('ticketTitle').focus();
    });
  });
  wireEntityDetailButtons();

  document.querySelectorAll('.alert-card').forEach(card => {
    const open = () => openEntityDetail('alert', card.dataset.id);
    card.addEventListener('click', event => {
      if (!cardClickGuard(event)) open();
    });
    card.addEventListener('keydown', event => {
      if (event.key === 'Enter' || event.key === ' ') {
        event.preventDefault();
        open();
      }
    });
  });

  document.querySelectorAll('.alert-status-action').forEach(button => {
    button.addEventListener('click', async () => {
      alerts = await fetchJson(`/api/alerts/${button.dataset.id}`, {
        method: 'PATCH',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ status: button.dataset.status }),
      });
      signalProbeDashboard = await fetchJson('/api/signal-probes/dashboard');
      renderAlerts();
      renderAssets();
      priorityZones = await fetchJson('/api/priority-zones');
      renderPriority();
      await refreshOverviewLayer();
    });
  });

  renderOverviewAlerts();
}

function renderPriority() {
  document.getElementById('priority-list').innerHTML = priorityZones.slice(0, 18).map(zone => {
    const tone = priorityZoneTone(zone);
    return `
    <article class="list-card clickable-card priority-zone-card priority-zone-${tone}" role="button" tabindex="0" data-key="${escapeHtml(areaKey(zone))}">
      <div><strong>${escapeHtml(zone.commune)}</strong><span>${escapeHtml(zone.department)}, ${escapeHtml(zone.region)} &middot; ${formatNumber(zone.population)} people</span></div>
      <span class="priority-badge priority-${escapeHtml(tone)}">${tone === 'high' ? 'RED ALERT' : escapeHtml(zone.priority_label)} ${zone.priority_score.toFixed(0)}</span>
      <p class="priority-alert-line">${escapeHtml(priorityZoneAlert(zone))}</p>
      <div class="priority-signal-grid">
        <div><span>Open alerts</span><strong>${zone.open_alert_count}</strong></div>
        <div><span>Assets</span><strong>${zone.asset_count}</strong></div>
        <div><span>Reports</span><strong>${zone.report_count}</strong></div>
        <div><span>Phone rate</span><strong>${formatRate(zone.phone_rate)}</strong></div>
      </div>
    </article>
  `;
  }).join('');

  document.querySelectorAll('.priority-zone-card').forEach(card => {
    const open = () => {
      const area = allStats.find(item => areaKey(item) === card.dataset.key);
      openAreaFollowUp(area);
    };
    card.addEventListener('click', event => {
      if (!cardClickGuard(event)) open();
    });
    card.addEventListener('keydown', event => {
      if (event.key === 'Enter' || event.key === ' ') {
        event.preventDefault();
        open();
      }
    });
  });

  renderOverviewPriority();
}

function renderOverviewAlerts() {
  const target = document.getElementById('overview-alerts');
  if (!target) return;
  const openAlerts = alerts.filter(alert => alert.status !== 'resolved').slice(0, 4);
  target.innerHTML = openAlerts.length ? openAlerts.map(alert => `
    <article class="compact-card clickable-card overview-alert-card severity-${escapeHtml(alert.severity)}" role="button" tabindex="0" data-id="${alert.id}">
      <div>
        <strong>${escapeHtml(alert.title)}</strong>
        <span>${escapeHtml(alert.severity)} validation signal &middot; ${escapeHtml(alert.status)}</span>
      </div>
      <span class="status-pill">${escapeHtml(alert.severity)}</span>
      <p>${escapeHtml(alert.message)}</p>
    </article>
  `).join('') : '<div class="empty-state">No open alerts.</div>';

  document.querySelectorAll('.overview-alert-card').forEach(card => {
    const open = () => openAlertFollowUp(card.dataset.id);
    card.addEventListener('click', event => {
      if (!cardClickGuard(event)) open();
    });
    card.addEventListener('keydown', event => {
      if (event.key === 'Enter' || event.key === ' ') {
        event.preventDefault();
        open();
      }
    });
  });
}

function priorityZoneTone(zone) {
  const label = String(zone.priority_label || 'Watch').toLowerCase();
  if (label === 'high' || Number(zone.priority_score || 0) >= 52) return 'high';
  if (label === 'medium' || Number(zone.priority_score || 0) >= 38) return 'medium';
  return 'watch';
}

function priorityZoneAlert(zone) {
  const tone = priorityZoneTone(zone);
  if (tone === 'high') {
    return `Red alert: ${zone.open_alert_count} open alerts and ${Math.round(zone.confidence * 100)}% confidence require immediate field action.`;
  }
  if (tone === 'medium') {
    return `Watch closely: strengthen evidence before this area becomes a red alert.`;
  }
  return `Monitor: keep telemetry and field reports current.`;
}

function campaignStatusActions(campaign) {
  const next = {
    draft: ['ready', 'Ready'],
    ready: ['in_field', 'Start fieldwork'],
    in_field: ['reviewing', 'Review'],
    active: ['reviewing', 'Review'],
    reviewing: ['completed', 'Complete'],
    paused: ['ready', 'Resume'],
  }[campaign.status];
  return next
    ? `<button class="btn btn-sm btn-success campaign-action" data-action="status" data-status="${next[0]}" data-campaign="${campaign.id}">${next[1]}</button>`
    : '';
}

function ticketStatusActions(ticket) {
  const actions = {
    open: [['in_progress', 'Start', 'outline-secondary'], ['blocked', 'Block', 'outline-secondary']],
    assigned: [['in_progress', 'Start', 'outline-secondary'], ['blocked', 'Block', 'outline-secondary']],
    in_progress: [['blocked', 'Block', 'outline-secondary'], ['done', 'Done', 'success']],
    blocked: [['in_progress', 'Resume', 'outline-secondary'], ['cancelled', 'Cancel', 'outline-secondary']],
  }[ticket.status] || [];
  return actions.map(([status, label, tone]) => (
    `<button class="btn btn-sm btn-${tone} ticket-status-action" data-id="${ticket.id}" data-status="${status}">${label}</button>`
  )).join('');
}

function decisionStageActions(decision) {
  const stageActions = {
    draft: [['recommended', 'Recommend', 'outline-secondary'], ['blocked', 'Block', 'outline-secondary']],
    recommended: [['approved', 'Approve', 'outline-secondary'], ['blocked', 'Block', 'outline-secondary']],
    approved: [['executing', 'Execute', 'outline-secondary'], ['blocked', 'Block', 'outline-secondary']],
    executing: [['completed', 'Complete', 'success'], ['blocked', 'Block', 'outline-secondary']],
    blocked: [['recommended', 'Reopen', 'outline-secondary']],
  }[decision.decision_stage] || [];
  const buttons = stageActions.map(([stage, label, tone]) => (
    `<button class="btn btn-sm btn-${tone} decision-stage-action" data-id="${decision.id}" data-stage="${stage}">${label}</button>`
  ));
  if (decision.decision_stage === 'approved' && Number(decision.evidence_score || 0) >= 60) {
    buttons.push(`<button class="btn btn-sm btn-outline-secondary decision-plan-action" data-id="${decision.id}"><i data-lucide="list-checks"></i> Plan</button>`);
  }
  return buttons.join('');
}

function executionPlanStatusActions(plan) {
  const actions = {
    planned: [['ready', 'Ready', 'outline-secondary'], ['blocked', 'Block', 'outline-secondary']],
    ready: [['in_progress', 'Start', 'outline-secondary'], ['blocked', 'Block', 'outline-secondary']],
    in_progress: [['blocked', 'Block', 'outline-secondary'], ['completed', 'Complete', 'success']],
    blocked: [['ready', 'Ready', 'outline-secondary'], ['in_progress', 'Resume', 'outline-secondary']],
  }[plan.status] || [];
  return actions.map(([status, label, tone]) => (
    `<button class="btn btn-sm btn-${tone} execution-status-action" data-id="${plan.id}" data-status="${status}">${label}</button>`
  )).join('');
}

function renderTickets() {
  document.getElementById('tickets-list').innerHTML = tickets.map(ticket => `
    <article class="list-card clickable-card ticket-card priority-${escapeHtml(ticket.priority)}" role="button" tabindex="0" data-id="${ticket.id}">
      <div>
        <strong>${escapeHtml(ticket.title)}</strong>
        <span>Ticket #${ticket.id} &middot; ${escapeHtml(ticket.status)} &middot; ${escapeHtml(ticket.assigned_to || 'Unassigned')}</span>
      </div>
      <div class="ticket-actions">
        <button class="btn btn-sm btn-outline-secondary" data-entity-detail data-entity-type="maintenance_ticket" data-entity-id="${ticket.id}">Details</button>
        ${ticketStatusActions(ticket)}
      </div>
      <p>Priority ${escapeHtml(ticket.priority)} &middot; Asset ${escapeHtml(ticket.asset_id || 'n/a')} &middot; Alert ${escapeHtml(ticket.alert_id || 'n/a')} &middot; Due ${escapeHtml(ticket.due_date || 'not set')}</p>
    </article>
  `).join('');

  document.querySelectorAll('.ticket-status-action').forEach(button => {
    button.addEventListener('click', async () => {
      tickets = await fetchJson(`/api/tickets/${button.dataset.id}`, {
        method: 'PATCH',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          status: button.dataset.status,
          resolution_notes: button.dataset.status === 'done' ? 'Marked complete from the operations console.' : null,
        }),
      });
      signalProbeDashboard = await fetchJson('/api/signal-probes/dashboard');
      renderTickets();
      renderAssets();
      renderOverviewTickets();
      await refreshOverviewLayer();
    });
  });
  wireEntityDetailButtons();
  document.querySelectorAll('.ticket-card').forEach(card => {
    const open = () => openEntityDetail('maintenance_ticket', card.dataset.id);
    card.addEventListener('click', event => {
      if (!cardClickGuard(event)) open();
    });
    card.addEventListener('keydown', event => {
      if (event.key === 'Enter' || event.key === ' ') {
        event.preventDefault();
        open();
      }
    });
  });

  renderOverviewTickets();
}

function renderOverviewTickets() {
  const target = document.getElementById('overview-tickets');
  if (!target) return;
  const activeTickets = tickets.filter(ticket => ticket.status !== 'done').slice(0, 5);
  target.innerHTML = activeTickets.length ? activeTickets.map(ticket => `
    <article class="compact-card clickable-card overview-ticket-card priority-${escapeHtml(ticket.priority)}" role="button" tabindex="0" data-id="${ticket.id}">
      <div>
        <strong>${escapeHtml(ticket.title)}</strong>
        <span>${escapeHtml(ticket.assigned_to || 'Unassigned')} &middot; ${escapeHtml(ticket.status)}</span>
      </div>
      <span class="priority-badge priority-${escapeHtml(ticket.priority === 'urgent' ? 'high' : ticket.priority)}">${escapeHtml(ticket.priority)}</span>
      <p>Due ${escapeHtml(ticket.due_date || 'not set')} &middot; Asset ${escapeHtml(ticket.asset_id || 'n/a')}</p>
    </article>
  `).join('') : '<div class="empty-state">No active tickets.</div>';

  document.querySelectorAll('.overview-ticket-card').forEach(card => {
    const open = () => openTicketFollowUp(card.dataset.id);
    card.addEventListener('click', event => {
      if (!cardClickGuard(event)) open();
    });
    card.addEventListener('keydown', event => {
      if (event.key === 'Enter' || event.key === ' ') {
        event.preventDefault();
        open();
      }
    });
  });
}

function renderOverviewPriority() {
  const target = document.getElementById('overview-priority');
  if (!target) return;
  target.innerHTML = priorityZones.slice(0, 5).map(zone => {
    const tone = priorityZoneTone(zone);
    return `
    <article class="compact-card clickable-card overview-priority-card priority-zone-${tone}" role="button" tabindex="0" data-key="${escapeHtml(areaKey(zone))}">
      <div>
        <strong>${escapeHtml(zone.commune)}</strong>
        <span>${escapeHtml(zone.department)}, ${escapeHtml(zone.region)}</span>
      </div>
      <span class="priority-badge priority-${escapeHtml(tone)}">${tone === 'high' ? 'ALERT' : zone.priority_score.toFixed(0)}</span>
      <p class="priority-alert-line">${escapeHtml(priorityZoneAlert(zone))}</p>
      <p>${formatNumber(zone.population)} people &middot; ${formatRate(zone.phone_rate)} phone ownership &middot; ${Math.round(zone.confidence * 100)}% confidence</p>
    </article>
  `;
  }).join('');

  document.querySelectorAll('.overview-priority-card').forEach(card => {
    const open = () => {
      const area = allStats.find(item => areaKey(item) === card.dataset.key);
      openAreaFollowUp(area);
    };
    card.addEventListener('click', event => {
      if (!cardClickGuard(event)) open();
    });
    card.addEventListener('keydown', event => {
      if (event.key === 'Enter' || event.key === ' ') {
        event.preventDefault();
        open();
      }
    });
  });
}

function renderIot() {
  document.getElementById('iot-list').innerHTML = readings.map(reading => `
    <article class="list-card clickable-card telemetry-card" role="button" tabindex="0" data-asset="${reading.asset_id}">
      <div><strong>${escapeHtml(reading.reading_type)}</strong><span>Asset #${reading.asset_id} &middot; ${escapeHtml(reading.created_at)}</span></div>
      <span class="status-pill">${Number(reading.value).toLocaleString()} ${escapeHtml(reading.unit)}</span>
    </article>
  `).join('');
  document.querySelectorAll('.telemetry-card').forEach(card => {
    const open = () => {
      const asset = assets.find(item => item.id === Number(card.dataset.asset));
      if (asset) prepareAssetAction('profile', asset);
    };
    card.addEventListener('click', event => {
      if (!cardClickGuard(event)) open();
    });
    card.addEventListener('keydown', event => {
      if (event.key === 'Enter' || event.key === ' ') {
        event.preventDefault();
        open();
      }
    });
  });
  renderImeiCompliance();
}

function renderImeiCompliance() {
  const healthTarget = document.getElementById('imei-compliance-health');
  const listTarget = document.getElementById('imei-events-list');
  if (!healthTarget || !listTarget) return;
  if (!imeiCompliance) {
    healthTarget.innerHTML = '';
    listTarget.innerHTML = '<div class="empty-state">Sign in to load operator IMEI compliance events.</div>';
    return;
  }
  healthTarget.innerHTML = `
    <div class="metric-tile accent-bronze featured-metric"><span>Operator events</span><strong>${imeiCompliance.total_events}</strong><small>${imeiCompliance.distinct_devices} distinct devices in the compliance feed</small></div>
    <div class="metric-tile accent-green"><span>Cleared</span><strong>${imeiCompliance.cleared_events}</strong><small>Allowed or customs-cleared device signals</small></div>
    <div class="metric-tile accent-gold"><span>Pending</span><strong>${imeiCompliance.pending_events}</strong><small>Require customs or operator verification</small></div>
    <div class="metric-tile accent-red"><span>Blocked/unknown</span><strong>${imeiCompliance.blocked_events + imeiCompliance.unknown_events}</strong><small>${(imeiCompliance.operators || []).join(', ') || 'No operator feed yet'}</small></div>
  `;
  listTarget.innerHTML = imeiCompliance.latest_events?.length ? imeiCompliance.latest_events.map(event => `
    <article class="list-card clickable-card imei-event-card priority-${event.compliance_status === 'blocked' ? 'urgent' : event.compliance_status === 'pending' ? 'medium' : 'watch'}" role="button" tabindex="0" data-id="${event.id}">
      <div>
        <strong>${escapeHtml(event.operator_name)} &middot; ${escapeHtml(event.event_type)}</strong>
        <span>IMEI *${escapeHtml(event.imei_last4 || 'hash')} &middot; ${escapeHtml(event.region || 'Cameroon')}${event.commune ? ` / ${escapeHtml(event.commune)}` : ''}</span>
      </div>
      <span class="status-pill">${escapeHtml(event.compliance_status)}</span>
      <p>${escapeHtml(event.source_system)} &middot; ${escapeHtml(event.raw_reference || 'no reference')} &middot; ${escapeHtml(event.created_at)}</p>
      <div class="ticket-actions">
        <button class="btn btn-sm btn-outline-secondary" data-entity-detail data-entity-type="operator_imei_event" data-entity-id="${event.id}">Details</button>
      </div>
    </article>
  `).join('') : `
    <div class="empty-state">${escapeHtml(imeiCompliance.regulatory_note)}</div>
  `;
  wireEntityDetailButtons();
  document.querySelectorAll('.imei-event-card').forEach(card => {
    const open = () => openEntityDetail('operator_imei_event', card.dataset.id);
    card.addEventListener('click', event => {
      if (!cardClickGuard(event)) open();
    });
    card.addEventListener('keydown', event => {
      if (event.key === 'Enter' || event.key === ' ') {
        event.preventDefault();
        open();
      }
    });
  });
}

function renderDecision(report) {
  renderDecisionBoard();
  renderExecutionBoard();
  document.getElementById('decision-report').innerHTML = `
    <div class="profile-grid-inner">
      <div class="metric-tile accent-bronze"><span>Monitored assets</span><strong>${report.monitored_assets}</strong><small>${report.open_alerts} open alerts</small></div>
      <div class="metric-tile accent-green"><span>Field reports</span><strong>${report.field_reports}</strong><small>Ground-truth submissions</small></div>
      <div class="metric-tile accent-gold"><span>Active tickets</span><strong>${report.active_tickets}</strong><small>${report.overdue_tickets} overdue</small></div>
      <div class="metric-tile accent-red"><span>Top zones</span><strong>${report.top_priority_zones.length}</strong><small>Ranked for action</small></div>
    </div>
    <h3 class="mt-4">Recommended execution</h3>
    <ul>${report.recommendations.map(item => `<li>${escapeHtml(item)}</li>`).join('')}</ul>
    <h3 class="mt-4">Top priority zones</h3>
    <div class="list-stack">
      ${report.top_priority_zones.map(zone => `<article class="list-card clickable-card decision-report-zone-card" role="button" tabindex="0" data-key="${escapeHtml(areaKey(zone))}"><strong>${escapeHtml(zone.commune)}</strong><span>${escapeHtml(zone.department)}, ${escapeHtml(zone.region)} &middot; Score ${zone.priority_score.toFixed(0)}</span></article>`).join('')}
    </div>
  `;

  document.querySelectorAll('.decision-report-zone-card').forEach(card => {
    const open = () => {
      const area = allStats.find(item => areaKey(item) === card.dataset.key);
      openAreaFollowUp(area);
    };
    card.addEventListener('click', event => {
      if (!cardClickGuard(event)) open();
    });
    card.addEventListener('keydown', event => {
      if (event.key === 'Enter' || event.key === ' ') {
        event.preventDefault();
        open();
      }
    });
  });
}

function renderDecisionBoard() {
  const healthTarget = document.getElementById('decision-board-health');
  const boardTarget = document.getElementById('decision-board');
  if (!healthTarget || !boardTarget || !decisionBoard) return;
  const totalBudget = decisionBoard.decisions.reduce((sum, decision) => sum + Number(decision.recommended_budget_xaf || 0), 0);
  const averageEvidence = decisionBoard.decisions.length
    ? decisionBoard.decisions.reduce((sum, decision) => sum + Number(decision.evidence_score || 0), 0) / decisionBoard.decisions.length
    : 0;
  const approved = decisionBoard.decisions.filter(decision => ['approved', 'executing', 'completed'].includes(decision.decision_stage)).length;
  const blocked = decisionBoard.decisions.filter(decision => decision.decision_stage === 'blocked').length;

  healthTarget.innerHTML = `
    <div class="metric-tile accent-bronze featured-metric"><span>Pipeline budget</span><strong>${compactMoneyXaf(totalBudget)}</strong><small>${decisionBoard.decisions.length} decisions tracked</small></div>
    <div class="metric-tile accent-green"><span>Approved or executing</span><strong>${approved}</strong><small>Ready to become field work</small></div>
    <div class="metric-tile accent-gold"><span>Evidence score</span><strong>${averageEvidence.toFixed(0)}</strong><small>Average proof readiness</small></div>
    <div class="metric-tile accent-red"><span>Blocked</span><strong>${blocked}</strong><small>Need owner or proof intervention</small></div>
  `;

  const stageLabels = {
    draft: 'Draft',
    recommended: 'Recommended',
    approved: 'Approved',
    blocked: 'Blocked',
    executing: 'Executing',
    completed: 'Completed',
  };
  boardTarget.innerHTML = `
    <div class="decision-recommendations">
      ${(decisionBoard.recommendations || []).map(item => `<p>${escapeHtml(item)}</p>`).join('')}
    </div>
    <div class="decision-stage-grid">
      ${(decisionBoard.stages || []).map(stage => {
        const stageDecisions = decisionBoard.decisions.filter(decision => decision.decision_stage === stage.stage);
        return `
          <section class="decision-stage">
            <div class="surface-header">
              <div>
                <p class="eyebrow">${escapeHtml(stageLabels[stage.stage] || stage.stage)}</p>
                <h2>${stage.count} decisions</h2>
              </div>
              <span class="status-pill">${compactMoneyXaf(stage.total_budget_xaf)}</span>
            </div>
            <div class="list-stack">
              ${stageDecisions.length ? stageDecisions.map(renderDecisionCard).join('') : '<div class="empty-state">No decisions in this stage.</div>'}
            </div>
          </section>
        `;
      }).join('')}
    </div>
  `;

  document.querySelectorAll('.decision-stage-action').forEach(button => {
    button.addEventListener('click', async () => {
      decisionBoard = await fetchJson(`/api/decision-snapshots/${button.dataset.id}/status`, {
        method: 'PATCH',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          decision_stage: button.dataset.stage,
          approval_notes: `Moved to ${button.dataset.stage} from decision board.`,
        }),
      });
      decisionSnapshots = decisionBoard.decisions;
      renderDecisionBoard();
      renderWorkspaces();
      await refreshOverviewLayer();
    });
  });
  document.querySelectorAll('.decision-plan-action').forEach(button => {
    button.addEventListener('click', async () => {
      executionBoard = await fetchJson(`/api/decision-snapshots/${button.dataset.id}/execution-plan`, {
        method: 'POST',
      });
      decisionBoard = await fetchJson('/api/decision-board');
      renderDecisionBoard();
      renderExecutionBoard();
      await refreshOverviewLayer();
    });
  });
  wireEntityDetailButtons();
  document.querySelectorAll('.decision-card-link').forEach(card => {
    const open = () => openEntityDetail('decision_snapshot', card.dataset.id);
    card.addEventListener('click', event => {
      if (!cardClickGuard(event)) open();
    });
    card.addEventListener('keydown', event => {
      if (event.key === 'Enter' || event.key === ' ') {
        event.preventDefault();
        open();
      }
    });
  });
  if (window.lucide) lucide.createIcons();
}

function renderDecisionCard(decision) {
  const evidence = Number(decision.evidence_score || 0);
  const risk = decision.risk_level || 'medium';
  return `
    <article class="decision-card clickable-card decision-card-link priority-${escapeHtml(risk === 'high' ? 'high' : risk === 'low' ? 'watch' : 'medium')}" role="button" tabindex="0" data-id="${decision.id}">
      <div>
        <strong>${escapeHtml(decision.title)}</strong>
        <span>${escapeHtml(decision.project_name || 'No project')} &middot; ${escapeHtml(decision.site_name || decision.asset_name || 'No proof link')}</span>
      </div>
      <span class="priority-badge priority-${escapeHtml(risk === 'high' ? 'high' : risk === 'low' ? 'watch' : 'medium')}">${escapeHtml(risk)}</span>
      <p>${escapeHtml(decision.rationale)} Next: ${escapeHtml(decision.next_action)}</p>
      <div class="workspace-progress"><span style="width:${Math.max(0, Math.min(100, evidence))}%"></span></div>
      <div class="probe-meta-grid">
        <div><span>Evidence</span><strong>${evidence.toFixed(0)}</strong></div>
        <div><span>Priority</span><strong>${Number(decision.priority_score || 0).toFixed(0)}</strong></div>
        <div><span>Budget</span><strong>${compactMoneyXaf(decision.recommended_budget_xaf || 0)}</strong></div>
        <div><span>Owner</span><strong>${escapeHtml(decision.owner_name || 'Unset')}</strong></div>
      </div>
      <div class="ticket-actions">
        <button class="btn btn-sm btn-outline-secondary" data-entity-detail data-entity-type="decision_snapshot" data-entity-id="${decision.id}"><i data-lucide="file-search"></i> Details</button>
        ${decisionStageActions(decision)}
      </div>
    </article>
  `;
}

function renderExecutionBoard() {
  const healthTarget = document.getElementById('execution-board-health');
  const boardTarget = document.getElementById('execution-board');
  if (!healthTarget || !boardTarget || !executionBoard) return;
  const totalBudget = executionBoard.plans.reduce((sum, plan) => sum + Number(plan.budget_xaf || 0), 0);
  const inMotion = executionBoard.plans.filter(plan => ['ready', 'in_progress'].includes(plan.status)).length;
  const blocked = executionBoard.plans.filter(plan => plan.status === 'blocked').length;
  const avgChecklist = executionBoard.plans.length
    ? executionBoard.plans.reduce((sum, plan) => sum + planChecklistCompletion(plan), 0) / executionBoard.plans.length
    : 0;

  healthTarget.innerHTML = `
    <div class="metric-tile accent-bronze featured-metric"><span>Execution budget</span><strong>${compactMoneyXaf(totalBudget)}</strong><small>${executionBoard.plans.length} execution plans</small></div>
    <div class="metric-tile accent-green"><span>Ready/in progress</span><strong>${inMotion}</strong><small>Field work can move</small></div>
    <div class="metric-tile accent-gold"><span>Checklist completion</span><strong>${avgChecklist.toFixed(0)}%</strong><small>Cameroon readiness controls</small></div>
    <div class="metric-tile accent-red"><span>Blocked</span><strong>${blocked}</strong><small>Need owner intervention</small></div>
  `;

  const statusLabels = {
    planned: 'Planned',
    ready: 'Ready',
    in_progress: 'In progress',
    blocked: 'Blocked',
    completed: 'Completed',
  };
  boardTarget.innerHTML = `
    <div class="decision-recommendations">
      ${(executionBoard.recommendations || []).map(item => `<p>${escapeHtml(item)}</p>`).join('')}
    </div>
    <div class="decision-stage-grid execution-stage-grid">
      ${(executionBoard.stages || []).map(stage => {
        const plans = executionBoard.plans.filter(plan => plan.status === stage.status);
        return `
          <section class="decision-stage">
            <div class="surface-header">
              <div>
                <p class="eyebrow">${escapeHtml(statusLabels[stage.status] || stage.status)}</p>
                <h2>${stage.count} plans</h2>
              </div>
              <span class="status-pill">${stage.checklist_completion.toFixed(0)}%</span>
            </div>
            <div class="list-stack">
              ${plans.length ? plans.map(renderExecutionPlanCard).join('') : '<div class="empty-state">No execution plans in this stage.</div>'}
            </div>
          </section>
        `;
      }).join('')}
    </div>
  `;

  document.querySelectorAll('.execution-status-action').forEach(button => {
    button.addEventListener('click', async () => {
      executionBoard = await fetchJson(`/api/execution-plans/${button.dataset.id}/status`, {
        method: 'PATCH',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          status: button.dataset.status,
          local_focal_point_confirmed: ['ready', 'in_progress', 'completed'].includes(button.dataset.status),
          offline_survey_ready: ['ready', 'in_progress', 'completed'].includes(button.dataset.status),
          xaf_budget_approved: ['ready', 'in_progress', 'completed'].includes(button.dataset.status),
          blocker: button.dataset.status === 'blocked' ? 'Blocked from execution board.' : null,
          outcome_notes: button.dataset.status === 'completed' ? 'Marked completed from execution board.' : null,
        }),
      });
      renderExecutionBoard();
    });
  });
  wireEntityDetailButtons();
  document.querySelectorAll('.execution-plan-card-link').forEach(card => {
    const open = () => openEntityDetail('execution_plan', card.dataset.id);
    card.addEventListener('click', event => {
      if (!cardClickGuard(event)) open();
    });
    card.addEventListener('keydown', event => {
      if (event.key === 'Enter' || event.key === ' ') {
        event.preventDefault();
        open();
      }
    });
  });
  if (window.lucide) lucide.createIcons();
}

function planChecklistCompletion(plan) {
  const checks = [
    plan.local_focal_point_confirmed,
    plan.gps_photo_proof_required,
    plan.offline_survey_ready,
    plan.bilingual_script_ready,
    plan.xaf_budget_approved,
  ];
  return (checks.filter(Boolean).length / checks.length) * 100;
}

function renderExecutionPlanCard(plan) {
  const completion = planChecklistCompletion(plan);
  return `
    <article class="decision-card clickable-card execution-plan-card-link priority-${plan.status === 'blocked' ? 'high' : plan.status === 'completed' ? 'watch' : 'medium'}" role="button" tabindex="0" data-id="${plan.id}">
      <div>
        <strong>${escapeHtml(plan.title)}</strong>
        <span>${escapeHtml(plan.project_name || 'No project')} &middot; ${escapeHtml(plan.site_name || plan.asset_name || plan.decision_title || 'No linked proof')}</span>
      </div>
      <span class="priority-badge priority-${plan.status === 'blocked' ? 'high' : plan.status === 'completed' ? 'watch' : 'medium'}">${escapeHtml(plan.status)}</span>
      <p>${escapeHtml(plan.transport_access_notes || plan.blocker || 'Confirm local access and field readiness.')}</p>
      <div class="workspace-progress"><span style="width:${completion}%"></span></div>
      <div class="probe-meta-grid">
        <div><span>Checklist</span><strong>${completion.toFixed(0)}%</strong></div>
        <div><span>Budget</span><strong>${compactMoneyXaf(plan.budget_xaf || 0)}</strong></div>
        <div><span>Owner</span><strong>${escapeHtml(plan.owner_name || 'Unset')}</strong></div>
        <div><span>Dates</span><strong>${escapeHtml(plan.planned_start || 'TBD')}</strong></div>
      </div>
      <div class="ticket-actions">
        <button class="btn btn-sm btn-outline-secondary" data-entity-detail data-entity-type="execution_plan" data-entity-id="${plan.id}"><i data-lucide="file-search"></i> Details</button>
        ${executionPlanStatusActions(plan)}
      </div>
    </article>
  `;
}

function prepareProjectAction(action, project) {
  if (!project) return;
  switchView('workspaces');
  if (action === 'site') {
    document.getElementById('siteProject').value = project.id;
    document.getElementById('siteRegion').value = project.region || '';
    document.getElementById('siteName').focus();
  }
  if (action === 'campaign') {
    document.getElementById('campaignProject').value = project.id;
    document.getElementById('campaignRegion').value = project.region || '';
    document.getElementById('campaignName').focus();
  }
  if (action === 'decision') {
    document.getElementById('decisionProject').value = project.id;
    document.getElementById('decisionTitle').value = `${project.name} decision`;
    document.getElementById('decisionTitle').focus();
  }
}

function applyWorkspaceTemplate(templateId) {
  const template = workspaceTemplates.find(item => item.id === templateId);
  if (!template) return;
  if (!authSession?.token) {
    switchView('login');
    setStatus(document.getElementById('login-status'), 'Sign in to apply workspace templates.', 'info');
    return;
  }
  const focusRegion = regionFilter.value !== 'all' ? regionFilter.value : '';
  const focusArea = selectedArea || currentMatrixRows[0] || allStats[0];

  setStatus(dataStatus, `Applying ${template.title}...`, 'info');
  fetchJson('/api/workspace-templates/apply', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({
      template_id: templateId,
      region: focusArea?.region || null,
      department: focusArea?.department || null,
      commune: focusArea?.commune || null,
    }),
  }).then(async result => {
    await refreshAfterBackendAction(`${result.message} Ensured: ${(result.created || []).join(', ')}.`, 'workspaces');
  }).catch(error => setStatus(dataStatus, error.message, 'danger'));
}

function prepareAreaAction(action, area) {
  if (['probe', 'campaign', 'report', 'site', 'decision', 'alert', 'ticket', 'full'].includes(action)) {
    const view = {
      probe: 'assets',
      campaign: 'workspaces',
      report: 'reports',
      site: 'workspaces',
      decision: 'decision',
      alert: 'alerts',
      ticket: 'tickets',
      full: 'workspaces',
    }[action];
    runAreaBackendAction(action, area, view);
    return;
  }
  const project = projects.find(item => item.region === area.region) || projects[0];
  if (action === 'probe') {
    switchView('assets');
    document.getElementById('assetProject').value = project?.id || '';
    document.getElementById('assetSite').value = sites.find(site => areaKey(site) === areaKey(area))?.id || '';
    document.getElementById('assetName').value = `${area.commune} signal probe`;
    document.getElementById('assetType').value = 'connectivity_probe';
    document.getElementById('assetRegion').value = area.region;
    document.getElementById('assetDepartment').value = area.department;
    document.getElementById('assetCommune').value = area.commune;
    document.getElementById('assetLatitude').value = formatCoordinate(area.latitude);
    document.getElementById('assetLongitude').value = formatCoordinate(area.longitude);
    document.getElementById('assetStatus').value = area.confidence < 0.68 ? 'warning' : 'online';
    document.getElementById('assetOperator').value = project?.organization_name || 'Local field team';
    document.getElementById('assetNotes').value = `Probe planned from area profile: ${area.commune}, ${formatRate(area.phone_rate)} ownership, ${Math.round(area.confidence * 100)}% confidence.`;
    document.getElementById('assetName').focus();
    return;
  }

  switchView('workspaces');
  if (action === 'site') {
    document.getElementById('siteProject').value = project?.id || '';
    document.getElementById('siteName').value = `${area.commune} field site`;
    document.getElementById('siteRegion').value = area.region;
    document.getElementById('siteDepartment').value = area.department;
    document.getElementById('siteCommune').value = area.commune;
    document.getElementById('siteLatitude').value = formatCoordinate(area.latitude);
    document.getElementById('siteLongitude').value = formatCoordinate(area.longitude);
    document.getElementById('siteBeneficiaries').value = area.population;
    document.getElementById('siteAccessNotes').value = `Validate local access, trusted focal point, and GPS/photo proof for ${area.commune}.`;
    document.getElementById('siteName').focus();
  }
  if (action === 'campaign') {
    document.getElementById('campaignProject').value = project?.id || '';
    document.getElementById('campaignName').value = `${area.commune} phone access validation`;
    document.getElementById('campaignRegion').value = area.region;
    document.getElementById('campaignDepartment').value = area.department;
    document.getElementById('campaignCommune').value = area.commune;
    document.getElementById('campaignFormType').value = area.phone_rate < 65 ? 'phone_ownership_baseline' : 'gps_photo_survey';
    document.getElementById('campaignStatus').value = 'draft';
    document.getElementById('campaignOffline').checked = true;
    document.getElementById('campaignName').focus();
  }
  if (action === 'report') {
    switchView('reports');
    const localAsset = assets.find(asset => areaKey(asset) === areaKey(area));
    const localSite = sites.find(site => areaKey(site) === areaKey(area));
    const localCampaign = campaigns.find(campaign => (
      (!campaign.target_region || normalizeAreaPart(campaign.target_region) === normalizeAreaPart(area.region))
      && (!campaign.target_department || normalizeAreaPart(campaign.target_department) === normalizeAreaPart(area.department))
      && (!campaign.target_commune || normalizeAreaPart(campaign.target_commune) === normalizeAreaPart(area.commune))
    ));
    document.getElementById('reportCampaignId').value = localCampaign?.id || '';
    document.getElementById('reportSiteId').value = localSite?.id || localAsset?.site_profile_id || '';
    document.getElementById('reportAssetId').value = localAsset?.id || '';
    document.getElementById('reportType').value = area.confidence < 0.7 ? 'Phone access ground-truth check' : 'GPS/photo validation';
    document.getElementById('reportRegion').value = area.region;
    document.getElementById('reportDepartment').value = area.department;
    document.getElementById('reportCommune').value = area.commune;
    document.getElementById('reportLatitude').value = formatCoordinate(area.latitude);
    document.getElementById('reportLongitude').value = formatCoordinate(area.longitude);
    document.getElementById('reportStatus').value = area.confidence < 0.7 ? 'needs_followup' : 'verified';
    document.getElementById('reportSubmittedBy').value = project?.organization_name || 'Field validation team';
    document.getElementById('reportNotes').value = `Validate ${area.commune}: ${formatRate(area.phone_rate)} phone ownership, ${Math.round(area.confidence * 100)}% model confidence, ${formatNumber(area.population)} people.`;
    document.getElementById('reportType').focus();
  }
  if (action === 'decision') {
    const priority = priorityForArea(area);
    const localAsset = assets.find(asset => areaKey(asset) === areaKey(area));
    const localSite = sites.find(site => areaKey(site) === areaKey(area));
    document.getElementById('decisionProject').value = project?.id || '';
    document.getElementById('decisionSite').value = localSite?.id || localAsset?.site_profile_id || '';
    document.getElementById('decisionAsset').value = localAsset?.id || '';
    document.getElementById('decisionTitle').value = `${area.commune} validation decision`;
    document.getElementById('decisionStage').value = 'recommended';
    document.getElementById('decisionScore').value = priority ? priority.priority_score.toFixed(0) : '';
    document.getElementById('decisionBudget').value = estimateBudgetXaf(area, localContextForArea(area));
    document.getElementById('decisionOwner').value = project?.organization_name || 'Field operations lead';
    document.getElementById('decisionEvidence').value = Math.round((area.confidence * 45) + (localContextForArea(area).localSites.length ? 25 : 0) + (localContextForArea(area).localAssets.length ? 20 : 0));
    document.getElementById('decisionRationale').value = `${area.commune} has ${formatNumber(area.population)} people, ${formatRate(area.phone_rate)} estimated ownership, and ${Math.round(area.confidence * 100)}% confidence.`;
    document.getElementById('decisionNextAction').value = areaActionText(area, localContextForArea(area));
    document.getElementById('decisionTitle').focus();
  }
}

function prepareAssetAction(action, asset) {
  const area = assetArea(asset);
  const context = assetContext(asset);
  if (action === 'profile' && area) {
    selectArea(area);
    return;
  }
  if (action === 'telemetry') {
    switchView('iot');
    document.getElementById('iotAssetId').value = asset.id;
    document.getElementById('iotReadingType').value = asset.asset_type === 'connectivity_probe' ? 'signal_quality' : 'asset_health';
    document.getElementById('iotValue').value = asset.status === 'online' ? 82 : asset.status === 'warning' ? 54 : 18;
    document.getElementById('iotUnit').value = asset.asset_type === 'connectivity_probe' ? 'score' : 'percent';
    document.getElementById('iotLatitude').value = formatCoordinate(asset.latitude);
    document.getElementById('iotLongitude').value = formatCoordinate(asset.longitude);
    document.getElementById('iotValue').focus();
    return;
  }
  if (action === 'alert') {
    fetchJson('/api/alerts', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({
        project_id: asset.project_id,
        site_profile_id: asset.site_profile_id,
        asset_id: asset.id,
        severity: asset.status === 'critical' || asset.status === 'offline' ? 'critical' : 'warning',
        title: `${asset.name} validation required`,
        message: `${asset.commune} probe needs field validation. ${context.health?.recommended_action || 'Review telemetry and local proof.'}`,
      }),
    }).then(async data => {
      alerts = data;
      priorityZones = await fetchJson('/api/priority-zones');
      signalProbeDashboard = await fetchJson('/api/signal-probes/dashboard');
      renderAlerts();
      renderPriority();
      renderAssets();
      updateView();
      await refreshOverviewLayer();
      setStatus(document.getElementById('asset-status'), `Alert created for ${asset.name}.`, 'success');
    }).catch(error => setStatus(document.getElementById('asset-status'), error.message, 'danger'));
    return;
  }
  if (action === 'report') {
    switchView('reports');
    document.getElementById('reportSiteId').value = asset.site_profile_id || '';
    document.getElementById('reportAssetId').value = asset.id;
    document.getElementById('reportType').value = asset.status === 'online' ? 'Routine probe verification' : 'Signal probe exception check';
    document.getElementById('reportRegion').value = asset.region;
    document.getElementById('reportDepartment').value = asset.department;
    document.getElementById('reportCommune').value = asset.commune;
    document.getElementById('reportLatitude').value = formatCoordinate(asset.latitude);
    document.getElementById('reportLongitude').value = formatCoordinate(asset.longitude);
    document.getElementById('reportStatus').value = asset.status === 'online' ? 'verified' : 'needs_followup';
    document.getElementById('reportSubmittedBy').value = asset.operator || 'Field validation team';
    document.getElementById('reportNotes').value = `${asset.name} requires GPS/photo confirmation in ${asset.commune}. Current status: ${asset.status}.`;
    document.getElementById('reportType').focus();
    return;
  }
  if (action === 'ticket') {
    if (!authSession?.token) {
      switchView('login');
      setStatus(document.getElementById('login-status'), 'Sign in to create tickets.', 'info');
      return;
    }
    fetchJson('/api/tickets', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({
        project_id: asset.project_id,
        site_profile_id: asset.site_profile_id,
        asset_id: asset.id,
        title: `${asset.name} field follow-up`,
        priority: asset.status === 'critical' || asset.status === 'offline' ? 'urgent' : 'high',
        assigned_to: asset.operator || 'Local technician',
        due_date: null,
        sla_hours: asset.status === 'critical' || asset.status === 'offline' ? 48 : 120,
      }),
    }).then(async data => {
      tickets = data;
      signalProbeDashboard = await fetchJson('/api/signal-probes/dashboard');
      await refreshAfterBackendAction(`Ticket created for ${asset.name}.`, 'tickets');
    }).catch(error => setStatus(dataStatus, error.message, 'danger'));
  }
}

function exportCurrentMatrixCsv() {
  const rows = currentMatrixRows.length ? currentMatrixRows : filteredStats();
  const header = ['region', 'department', 'arrondissement', 'pcode', 'population', 'estimated_phone_owners', 'estimated_mobile_subscriptions', 'ownership_rate', 'confidence_level', 'opportunity_score', 'priority_score', 'recommended_action', 'needs_validation', 'data_source', 'last_updated'];
  const csvRows = rows.map(area => {
    const priority = priorityForArea(area);
    return [
      area.region,
      area.department,
      area.commune,
      area.pcode || '',
      area.population,
      area.phone_owners,
      area.estimated_mobile_subscriptions || Math.round(area.phone_owners * 1.08),
      area.phone_rate?.toFixed(2),
      area.confidence_level || confidenceLabel(area.confidence),
      Number(area.opportunity_score || 0).toFixed(2),
      Number(priority?.priority_score || area.priority_score || 0).toFixed(2),
      area.recommended_action || '',
      area.needs_validation ? 'yes' : 'no',
      area.data_source || '',
      area.updated_at || area.last_updated || '',
    ].map(value => `"${String(value).replaceAll('"', '""')}"`).join(',');
  });
  const blob = new Blob([[header.join(','), ...csvRows].join('\n')], { type: 'text/csv;charset=utf-8' });
  const url = URL.createObjectURL(blob);
  const link = document.createElement('a');
  link.href = url;
  link.download = 'kk-evo-filtered-phone-matrix.csv';
  link.click();
  URL.revokeObjectURL(url);
}

function updateView() {
  const stats = filteredStats();
  renderRegions(stats);
  updateMapMarkers(stats);
  selectedArea = stats[0] || selectedArea;
  renderAreaProfile();
  if (selectedArea) loadAreaDossier(selectedArea);
}

function initMap() {
  map = L.map('map', { scrollWheelZoom: true }).setView([6.5, 12.5], 6);
  L.tileLayer('https://{s}.tile.openstreetmap.org/{z}/{x}/{y}.png', {
    maxZoom: 19,
    attribution: '&copy; OpenStreetMap contributors',
  }).addTo(map);
  markersLayer = L.layerGroup().addTo(map);
  assetLayer = L.layerGroup().addTo(map);
  reportLayer = L.layerGroup().addTo(map);
}

async function refreshData() {
  refreshButton.disabled = true;
  setStatus(dataStatus, 'Loading KK Evo intelligence layers...', 'info');
  try {
    const [summary, overviewData, phoneMatrixData, workspaceData, templateData, orgData, projectData, siteData, campaignData, snapshotData, decisionBoardData, executionBoardData, assetData, probeData, reportData, alertData, ticketData, readingData, imeiData, priorityData, decisionData] = await Promise.all([
      fetchJson('/api/summary'),
      fetchJson('/api/overview'),
      fetchJson('/api/phone-matrix'),
      fetchJson('/api/workspaces/dashboard'),
      fetchJson('/api/workspace-templates'),
      fetchJson('/api/organizations'),
      fetchJson('/api/projects'),
      fetchJson('/api/site-profiles'),
      fetchJson('/api/survey-campaigns'),
      fetchJson('/api/decision-snapshots'),
      fetchJson('/api/decision-board'),
      fetchJson('/api/execution-board'),
      fetchJson('/api/assets'),
      fetchJson('/api/signal-probes/dashboard'),
      fetchJson('/api/reports'),
      fetchJson('/api/alerts'),
      fetchJson('/api/tickets'),
      fetchJson('/api/iot/readings'),
      authSession?.token ? fetchJson('/api/operator-imei-events') : Promise.resolve(null),
      fetchJson('/api/priority-zones'),
      fetchJson('/api/decision-report'),
    ]);
    phoneMatrixDashboard = phoneMatrixData;
    allStats = phoneMatrixData.rows.map(phoneMatrixRowToStat);
    nationalSummary = summary;
    overviewIntelligence = overviewData;
    workspaceDashboard = workspaceData;
    workspaceTemplates = templateData;
    organizations = orgData;
    projects = projectData;
    sites = siteData;
    campaigns = campaignData;
    decisionSnapshots = snapshotData;
    decisionBoard = decisionBoardData;
    executionBoard = executionBoardData;
    assets = assetData;
    signalProbeDashboard = probeData;
    reports = reportData;
    alerts = alertData;
    tickets = ticketData;
    readings = readingData;
    imeiCompliance = imeiData;
    priorityZones = priorityData;
    renderSummary(summary, overviewData);
    renderOverviewIntelligence();
    buildFilterOptions();
    renderWorkspaces();
    renderAssets();
    renderReports();
    renderAlerts();
    renderTickets();
    renderPriority();
    renderIot();
    renderDecision(decisionData);
    updateView();
    setStatus(dataStatus, `${allStats.length} arrondissements, ${assets.length} assets, ${alerts.filter(a => a.status !== 'resolved').length} open alerts.`, 'success');
  } catch (error) {
    console.error(error);
    setStatus(dataStatus, error.message || 'Unable to load intelligence layers.', 'danger');
  } finally {
    refreshButton.disabled = false;
  }
}

function payloadFrom(prefix) {
  return {
    name: document.getElementById(`${prefix}Name`)?.value.trim(),
    asset_type: document.getElementById(`${prefix}Type`)?.value,
    region: document.getElementById(`${prefix}Region`)?.value.trim(),
    department: document.getElementById(`${prefix}Department`)?.value.trim(),
    commune: document.getElementById(`${prefix}Commune`)?.value.trim(),
    latitude: Number(document.getElementById(`${prefix}Latitude`)?.value),
    longitude: Number(document.getElementById(`${prefix}Longitude`)?.value),
    status: document.getElementById(`${prefix}Status`)?.value,
  };
}

document.getElementById('asset-form').addEventListener('submit', async event => {
  event.preventDefault();
  const payload = payloadFrom('asset');
  payload.project_id = document.getElementById('assetProject').value ? Number(document.getElementById('assetProject').value) : null;
  payload.site_profile_id = document.getElementById('assetSite').value ? Number(document.getElementById('assetSite').value) : null;
  payload.operator = document.getElementById('assetOperator').value.trim() || null;
  payload.installed_at = document.getElementById('assetInstalledAt').value || null;
  payload.notes = document.getElementById('assetNotes').value.trim() || null;
  try {
    assets = await fetchJson('/api/assets', { method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify(payload) });
    signalProbeDashboard = await fetchJson('/api/signal-probes/dashboard');
    workspaceDashboard = await fetchJson('/api/workspaces/dashboard');
    populateAssetLinkedControls();
    renderAssets();
    renderWorkspaces();
    updateView();
    await refreshOverviewLayer();
    setStatus(document.getElementById('asset-status'), 'Asset saved.', 'success');
    event.target.reset();
  } catch (error) {
    setStatus(document.getElementById('asset-status'), error.message, 'danger');
  }
});

document.getElementById('report-form').addEventListener('submit', async event => {
  event.preventDefault();
  const reportAsset = assets.find(asset => asset.id === Number(document.getElementById('reportAssetId').value));
  const reportSite = sites.find(site => site.id === Number(document.getElementById('reportSiteId').value || reportAsset?.site_profile_id));
  const reportCampaign = campaigns.find(campaign => campaign.id === Number(document.getElementById('reportCampaignId').value));
  const payload = {
    project_id: projectIdFromLinkedRecords({
      projectId: null,
      siteId: reportSite?.id,
      assetId: reportAsset?.id,
      campaignId: reportCampaign?.id,
    }),
    site_profile_id: reportSite?.id || reportAsset?.site_profile_id || null,
    campaign_id: reportCampaign?.id || null,
    asset_id: document.getElementById('reportAssetId').value ? Number(document.getElementById('reportAssetId').value) : null,
    report_type: document.getElementById('reportType').value.trim(),
    region: document.getElementById('reportRegion').value.trim(),
    department: document.getElementById('reportDepartment').value.trim(),
    commune: document.getElementById('reportCommune').value.trim(),
    latitude: Number(document.getElementById('reportLatitude').value),
    longitude: Number(document.getElementById('reportLongitude').value),
    status: document.getElementById('reportStatus').value,
    evidence_quality: 'gps_photo_verified',
    notes: document.getElementById('reportNotes').value.trim(),
    submitted_by: document.getElementById('reportSubmittedBy').value.trim(),
  };
  try {
    reports = await fetchJson('/api/reports', { method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify(payload) });
    signalProbeDashboard = await fetchJson('/api/signal-probes/dashboard');
    workspaceDashboard = await fetchJson('/api/workspaces/dashboard');
    renderReports();
    renderAssets();
    renderWorkspaces();
    updateView();
    setStatus(document.getElementById('report-status'), 'Report submitted.', 'success');
    event.target.reset();
  } catch (error) {
    setStatus(document.getElementById('report-status'), error.message, 'danger');
  }
});

document.getElementById('alert-form').addEventListener('submit', async event => {
  event.preventDefault();
  const alertAsset = assets.find(asset => asset.id === Number(document.getElementById('alertAssetId').value));
  const alertSite = sites.find(site => site.id === Number(document.getElementById('alertSiteId').value || alertAsset?.site_profile_id));
  const payload = {
    project_id: projectIdFromLinkedRecords({
      projectId: null,
      siteId: alertSite?.id,
      assetId: alertAsset?.id,
    }),
    site_profile_id: alertSite?.id || alertAsset?.site_profile_id || null,
    asset_id: alertAsset?.id || null,
    severity: document.getElementById('alertSeverity').value,
    title: document.getElementById('alertTitle').value.trim(),
    message: document.getElementById('alertMessage').value.trim(),
  };
  try {
    alerts = await fetchJson('/api/alerts', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(payload),
    });
    signalProbeDashboard = await fetchJson('/api/signal-probes/dashboard');
    priorityZones = await fetchJson('/api/priority-zones');
    renderAlerts();
    renderPriority();
    renderAssets();
    updateView();
    await refreshOverviewLayer();
    setStatus(document.getElementById('alert-status'), 'Alert created and added to the coverage queue.', 'success');
    event.target.reset();
  } catch (error) {
    setStatus(document.getElementById('alert-status'), error.message, 'danger');
  }
});

document.getElementById('organization-form').addEventListener('submit', async event => {
  event.preventDefault();
  const payload = {
    name: document.getElementById('organizationName').value.trim(),
    org_type: document.getElementById('organizationType').value,
    contact_name: document.getElementById('organizationContactName').value.trim() || null,
    contact_email: document.getElementById('organizationContactEmail').value.trim() || null,
  };
  try {
    organizations = await fetchJson('/api/organizations', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(payload),
    });
    workspaceDashboard = await fetchJson('/api/workspaces/dashboard');
    renderWorkspaces();
    setStatus(document.getElementById('organization-status'), 'Organization saved.', 'success');
    event.target.reset();
  } catch (error) {
    setStatus(document.getElementById('organization-status'), error.message, 'danger');
  }
});

document.getElementById('project-form').addEventListener('submit', async event => {
  event.preventDefault();
  const payload = {
    organization_id: document.getElementById('projectOrganization').value ? Number(document.getElementById('projectOrganization').value) : null,
    name: document.getElementById('projectName').value.trim(),
    sector: document.getElementById('projectSector').value,
    region: document.getElementById('projectRegion').value.trim() || null,
    status: document.getElementById('projectStatus').value,
    start_date: document.getElementById('projectStartDate').value || null,
  };
  try {
    projects = await fetchJson('/api/projects', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(payload),
    });
    workspaceDashboard = await fetchJson('/api/workspaces/dashboard');
    renderWorkspaces();
    setStatus(document.getElementById('project-status'), 'Project saved.', 'success');
    event.target.reset();
  } catch (error) {
    setStatus(document.getElementById('project-status'), error.message, 'danger');
  }
});

document.getElementById('site-form').addEventListener('submit', async event => {
  event.preventDefault();
  const payload = {
    project_id: document.getElementById('siteProject').value ? Number(document.getElementById('siteProject').value) : null,
    name: document.getElementById('siteName').value.trim(),
    site_type: document.getElementById('siteType').value,
    region: document.getElementById('siteRegion').value.trim(),
    department: document.getElementById('siteDepartment').value.trim(),
    commune: document.getElementById('siteCommune').value.trim(),
    latitude: Number(document.getElementById('siteLatitude').value),
    longitude: Number(document.getElementById('siteLongitude').value),
    beneficiary_estimate: document.getElementById('siteBeneficiaries').value ? Number(document.getElementById('siteBeneficiaries').value) : null,
    trust_signal: document.getElementById('siteTrustSignal').value,
    access_notes: document.getElementById('siteAccessNotes').value.trim() || null,
  };
  try {
    sites = await fetchJson('/api/site-profiles', { method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify(payload) });
    workspaceDashboard = await fetchJson('/api/workspaces/dashboard');
    await refreshOverviewLayer();
    renderWorkspaces();
    setStatus(document.getElementById('site-status'), 'Site profile saved.', 'success');
    event.target.reset();
  } catch (error) {
    setStatus(document.getElementById('site-status'), error.message, 'danger');
  }
});

document.getElementById('campaign-form').addEventListener('submit', async event => {
  event.preventDefault();
  const payload = {
    project_id: document.getElementById('campaignProject').value ? Number(document.getElementById('campaignProject').value) : null,
    name: document.getElementById('campaignName').value.trim(),
    form_type: document.getElementById('campaignFormType').value,
    target_region: document.getElementById('campaignRegion').value.trim() || null,
    target_department: document.getElementById('campaignDepartment').value.trim() || null,
    target_commune: document.getElementById('campaignCommune').value.trim() || null,
    status: document.getElementById('campaignStatus').value,
    language_mode: document.getElementById('campaignLanguage').value,
    offline_enabled: document.getElementById('campaignOffline').checked,
    starts_on: document.getElementById('campaignStartsOn').value || null,
    ends_on: document.getElementById('campaignEndsOn').value || null,
  };
  try {
    campaigns = await fetchJson('/api/survey-campaigns', { method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify(payload) });
    workspaceDashboard = await fetchJson('/api/workspaces/dashboard');
    await refreshOverviewLayer();
    renderWorkspaces();
    setStatus(document.getElementById('campaign-status'), 'Survey campaign saved.', 'success');
    event.target.reset();
    document.getElementById('campaignOffline').checked = true;
  } catch (error) {
    setStatus(document.getElementById('campaign-status'), error.message, 'danger');
  }
});

document.getElementById('decision-snapshot-form').addEventListener('submit', async event => {
  event.preventDefault();
  const decisionProjectId = document.getElementById('decisionProject').value;
  const decisionSiteId = document.getElementById('decisionSite').value;
  const decisionAssetId = document.getElementById('decisionAsset').value;
  const payload = {
    project_id: projectIdFromLinkedRecords({
      projectId: decisionProjectId,
      siteId: decisionSiteId,
      assetId: decisionAssetId,
    }),
    site_profile_id: decisionSiteId ? Number(decisionSiteId) : null,
    asset_id: decisionAssetId ? Number(decisionAssetId) : null,
    title: document.getElementById('decisionTitle').value.trim(),
    decision_stage: document.getElementById('decisionStage').value,
    priority_score: document.getElementById('decisionScore').value ? Number(document.getElementById('decisionScore').value) : null,
    recommended_budget_xaf: document.getElementById('decisionBudget').value ? Number(document.getElementById('decisionBudget').value) : null,
    owner_name: document.getElementById('decisionOwner').value.trim() || null,
    risk_level: document.getElementById('decisionRisk').value || null,
    evidence_score: document.getElementById('decisionEvidence').value ? Number(document.getElementById('decisionEvidence').value) : null,
    rationale: document.getElementById('decisionRationale').value.trim() || null,
    next_action: document.getElementById('decisionNextAction').value.trim() || null,
  };
  try {
    decisionSnapshots = await fetchJson('/api/decision-snapshots', { method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify(payload) });
    decisionBoard = await fetchJson('/api/decision-board');
    workspaceDashboard = await fetchJson('/api/workspaces/dashboard');
    await refreshOverviewLayer();
    renderWorkspaces();
    renderDecisionBoard();
    setStatus(document.getElementById('decision-snapshot-status'), 'Decision snapshot saved.', 'success');
    event.target.reset();
  } catch (error) {
    setStatus(document.getElementById('decision-snapshot-status'), error.message, 'danger');
  }
});

document.getElementById('ticket-form').addEventListener('submit', async event => {
  event.preventDefault();
  const ticketAsset = assets.find(asset => asset.id === Number(document.getElementById('ticketAssetId').value));
  const ticketAlert = alerts.find(alert => alert.id === Number(document.getElementById('ticketAlertId').value));
  const ticketSite = sites.find(site => site.id === Number(document.getElementById('ticketSiteId').value || ticketAsset?.site_profile_id || ticketAlert?.site_profile_id));
  const payload = {
    project_id: projectIdFromLinkedRecords({
      projectId: document.getElementById('ticketProjectId').value,
      siteId: ticketSite?.id,
      assetId: ticketAsset?.id || ticketAlert?.asset_id,
      alertId: ticketAlert?.id,
    }),
    site_profile_id: ticketSite?.id || ticketAsset?.site_profile_id || ticketAlert?.site_profile_id || null,
    asset_id: ticketAsset?.id || ticketAlert?.asset_id || null,
    alert_id: ticketAlert?.id || null,
    title: document.getElementById('ticketTitle').value.trim(),
    priority: document.getElementById('ticketPriority').value,
    assigned_to: document.getElementById('ticketAssignedTo').value.trim() || null,
    due_date: document.getElementById('ticketDueDate').value || null,
  };
  try {
    tickets = await fetchJson('/api/tickets', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(payload),
    });
    alerts = await fetchJson('/api/alerts');
    signalProbeDashboard = await fetchJson('/api/signal-probes/dashboard');
    renderTickets();
    renderAlerts();
    renderAssets();
    await refreshOverviewLayer();
    setStatus(document.getElementById('ticket-status'), 'Ticket created.', 'success');
    event.target.reset();
  } catch (error) {
    setStatus(document.getElementById('ticket-status'), error.message, 'danger');
  }
});

document.getElementById('iot-form').addEventListener('submit', async event => {
  event.preventDefault();
  const telemetryAsset = assets.find(asset => asset.id === Number(document.getElementById('iotAssetId').value));
  if (!telemetryAsset) {
    setStatus(document.getElementById('iot-status'), 'Select a monitored probe or asset first.', 'danger');
    return;
  }
  const latitude = document.getElementById('iotLatitude').value;
  const longitude = document.getElementById('iotLongitude').value;
  const payload = {
    project_id: telemetryAsset.project_id,
    site_profile_id: telemetryAsset.site_profile_id,
    asset_id: telemetryAsset.id,
    reading_type: document.getElementById('iotReadingType').value,
    value: Number(document.getElementById('iotValue').value),
    unit: document.getElementById('iotUnit').value.trim(),
    latitude: latitude ? Number(latitude) : telemetryAsset.latitude,
    longitude: longitude ? Number(longitude) : telemetryAsset.longitude,
  };
  try {
    readings = await fetchJson('/api/iot/readings', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(payload),
    });
    signalProbeDashboard = await fetchJson('/api/signal-probes/dashboard');
    renderIot();
    renderAssets();
    updateView();
    await refreshOverviewLayer();
    setStatus(document.getElementById('iot-status'), `Telemetry saved for ${telemetryAsset.name}.`, 'success');
    event.target.reset();
  } catch (error) {
    setStatus(document.getElementById('iot-status'), error.message, 'danger');
  }
});

document.getElementById('imei-event-form')?.addEventListener('submit', async event => {
  event.preventDefault();
  const payload = {
    operator_name: document.getElementById('imeiOperator').value.trim(),
    imei: document.getElementById('imeiValue').value.trim() || null,
    imei_hash: document.getElementById('imeiHash').value.trim() || null,
    device_type: document.getElementById('imeiDeviceType').value.trim() || null,
    event_type: document.getElementById('imeiEventType').value,
    compliance_status: document.getElementById('imeiComplianceStatus').value,
    region: document.getElementById('imeiRegion').value.trim() || null,
    department: document.getElementById('imeiDepartment').value.trim() || null,
    commune: document.getElementById('imeiCommune').value.trim() || null,
    source_system: 'operator_api',
    raw_reference: document.getElementById('imeiReference').value.trim() || null,
    network_first_seen_at: document.getElementById('imeiFirstSeen').value || null,
  };
  try {
    imeiCompliance = await fetchJson('/api/operator-imei-events', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(payload),
    });
    renderImeiCompliance();
    setStatus(document.getElementById('imei-status'), 'IMEI compliance event saved.', 'success');
    event.target.reset();
  } catch (error) {
    setStatus(document.getElementById('imei-status'), error.message, 'danger');
  }
});

document.getElementById('login-form')?.addEventListener('submit', async event => {
  event.preventDefault();
  try {
    const response = await fetchJson('/api/auth/login', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({
        login: document.getElementById('loginIdentifier').value.trim(),
        password: document.getElementById('loginPassword').value,
      }),
    });
    authSession = response;
    localStorage.setItem('kkEvoAuth', JSON.stringify(response));
    renderAuthState();
    setStatus(document.getElementById('login-status'), 'Signed in. Operational console unlocked.', 'success');
    event.target.reset();
    await refreshData();
    switchView('workspaces');
  } catch (error) {
    setStatus(document.getElementById('login-status'), error.message, 'danger');
  }
});

function readFileAsBase64(file) {
  return new Promise((resolve, reject) => {
    const reader = new FileReader();
    reader.onload = () => resolve(String(reader.result || '').split(',')[1] || '');
    reader.onerror = () => reject(new Error('Could not read evidence file.'));
    reader.readAsDataURL(file);
  });
}

document.getElementById('entity-detail-close')?.addEventListener('click', closeEntityDetail);
document.getElementById('entity-detail-panel')?.addEventListener('click', event => {
  if (event.target.id === 'entity-detail-panel') closeEntityDetail();
});
document.addEventListener('keydown', event => {
  if (event.key === 'Escape') closeEntityDetail();
});

document.getElementById('entity-evidence-form')?.addEventListener('submit', async event => {
  event.preventDefault();
  const form = event.currentTarget;
  const fileInput = document.getElementById('entityEvidenceFile');
  const status = document.getElementById('entity-evidence-status');
  const file = fileInput?.files?.[0];
  if (!file) {
    setStatus(status, 'Choose an evidence file first.', 'danger');
    return;
  }
  try {
    setStatus(status, 'Uploading evidence...', 'info');
    await fetchJson('/api/evidence', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({
        entity_type: form.dataset.entityType,
        entity_id: Number(form.dataset.entityId),
        file_name: file.name,
        content_type: file.type || 'application/octet-stream',
        content_base64: await readFileAsBase64(file),
        latitude: document.getElementById('entityEvidenceLatitude').value ? Number(document.getElementById('entityEvidenceLatitude').value) : null,
        longitude: document.getElementById('entityEvidenceLongitude').value ? Number(document.getElementById('entityEvidenceLongitude').value) : null,
        captured_at: null,
      }),
    });
    form.reset();
    setStatus(status, 'Evidence attached.', 'success');
    await openEntityDetail(form.dataset.entityType, form.dataset.entityId);
  } catch (error) {
    setStatus(status, error.message, 'danger');
  }
});

document.querySelectorAll('.tab-button').forEach(button => {
  button.addEventListener('click', () => switchView(button.dataset.view));
});

if (window.lucide) lucide.createIcons();
renderAuthState();
refreshButton.addEventListener('click', refreshData);
regionFilter.addEventListener('change', () => { buildFilterOptions(); updateView(); });
departmentFilter.addEventListener('change', () => { buildFilterOptions(); updateView(); });
communeFilter.addEventListener('change', updateView);
[matrixSearch, matrixSort, matrixOwnershipFilter, matrixConfidenceFilter, matrixOpportunityFilter, matrixValidationFilter, matrixProjectFilter, matrixMinPopulation, matrixMinPriority, matrixMaxPriority].forEach(control => {
  control?.addEventListener('input', updateView);
  control?.addEventListener('change', updateView);
});
[workspaceSearch, workspaceStatusFilter, workspaceTypeFilter].forEach(control => {
  control?.addEventListener('input', renderWorkspaces);
  control?.addEventListener('change', renderWorkspaces);
});
document.querySelectorAll('[data-workspace-jump]').forEach(button => {
  button.addEventListener('click', () => document.getElementById(button.dataset.workspaceJump)?.focus());
});
[assetSearch, assetStatusFilter, assetTypeFilter].forEach(control => {
  control?.addEventListener('input', renderAssets);
  control?.addEventListener('change', renderAssets);
});
document.getElementById('iotAssetId')?.addEventListener('change', fillTelemetryCoordinatesFromAsset);
document.getElementById('iotReadingType')?.addEventListener('change', event => {
  const unit = document.getElementById('iotUnit');
  if (!unit) return;
  unit.value = ['uptime', 'battery_level', 'asset_health'].includes(event.target.value) ? 'percent' : 'score';
});
matrixExportButton?.addEventListener('click', exportCurrentMatrixCsv);
window.addEventListener('load', () => { initMap(); renderAuthState(); refreshData(); });
