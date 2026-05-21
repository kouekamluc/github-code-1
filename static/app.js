const CAMEROON_BOUNDS = { minLatitude: 1.5, maxLatitude: 13.5, minLongitude: 8, maxLongitude: 16.5 };

const summaryCards = document.getElementById('summary-cards');
const tableBody = document.getElementById('regions-table-body');
const refreshButton = document.getElementById('refresh-button');
const dataStatus = document.getElementById('data-status');
const regionFilter = document.getElementById('regionFilter');
const departmentFilter = document.getElementById('departmentFilter');
const communeFilter = document.getElementById('communeFilter');
const areaProfile = document.getElementById('area-profile');

let map;
let markersLayer;
let assetLayer;
let reportLayer;
let allStats = [];
let assets = [];
let reports = [];
let alerts = [];
let readings = [];
let priorityZones = [];
let selectedArea = null;

async function fetchJson(url, options = {}) {
  const response = await fetch(url, options);
  const contentType = response.headers.get('content-type') || '';
  const body = contentType.includes('application/json') ? await response.json() : null;
  if (!response.ok) throw new Error(body?.message || `Request failed with status ${response.status}`);
  return body;
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

function switchView(view) {
  document.querySelectorAll('.tab-button').forEach(button => {
    button.classList.toggle('active', button.dataset.view === view);
  });
  document.querySelectorAll('.view-section').forEach(section => {
    section.classList.toggle('active', section.id === `view-${view}`);
  });
  if (view === 'overview' && map) setTimeout(() => map.invalidateSize(), 150);
}

function renderSummary(summary) {
  summaryCards.innerHTML = `
    <div class="metric-tile accent-blue">
      <span>Estimated phone owners</span>
      <strong>${formatNumber(summary.total_phone_owners)}</strong>
      <small>Across ${summary.estimated_location_count} modeled arrondissements</small>
    </div>
    <div class="metric-tile accent-green">
      <span>Population covered</span>
      <strong>${formatNumber(summary.total_population)}</strong>
      <small>${summary.commune_count} arrondissements in the matrix</small>
    </div>
    <div class="metric-tile accent-gold">
      <span>National ownership rate</span>
      <strong>${formatRate(summary.percent_with_phone)}</strong>
      <small>Blended from baseline data and GPS signals</small>
    </div>
    <div class="metric-tile accent-red">
      <span>Departments mapped</span>
      <strong>${summary.department_count}</strong>
      <small>${summary.region_count} regions mapped</small>
    </div>
  `;
}

function renderAreaProfile(area = selectedArea) {
  if (!area) {
    areaProfile.innerHTML = '<div class="empty-state">Select an arrondissement from the map or matrix to inspect its intelligence profile.</div>';
    return;
  }

  const key = areaKey(area);
  const localAssets = assets.filter(asset => areaKey(asset) === key);
  const localReports = reports.filter(report => areaKey(report) === key);
  const localPriority = priorityZones.find(zone => areaKey(zone) === key);
  const localAlerts = alerts.filter(alert => {
    const asset = assets.find(item => item.id === alert.asset_id);
    return asset && areaKey(asset) === key && alert.status !== 'resolved';
  });

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
      <div class="metric-tile accent-blue"><span>Population</span><strong>${formatNumber(area.population)}</strong><small>Matrix or measured</small></div>
      <div class="metric-tile accent-green"><span>Phone owners</span><strong>${formatNumber(area.phone_owners)}</strong><small>${formatRate(area.phone_rate)} ownership</small></div>
      <div class="metric-tile accent-gold"><span>Confidence</span><strong>${Math.round(area.confidence * 100)}%</strong><small>${escapeHtml(area.metric_source)}</small></div>
      <div class="metric-tile accent-red"><span>Ops signal</span><strong>${localAlerts.length}</strong><small>${localAssets.length} assets / ${localReports.length} reports</small></div>
    </div>
    <div class="profile-notes">
      <strong>Recommended action:</strong>
      ${localPriority?.priority_score >= 52 ? 'Immediate field validation and maintenance review.' : localPriority?.priority_score >= 38 ? 'Schedule survey and monitor assets.' : 'Keep in watchlist and enrich with field data.'}
    </div>
  `;
}

function renderRegions(regions) {
  if (!regions.length) {
    tableBody.innerHTML = '<tr><td colspan="9" class="text-center text-muted py-4">No areas match the selected filters.</td></tr>';
    return;
  }

  tableBody.innerHTML = regions.map(area => {
    const width = Math.min(Math.max(area.phone_rate, 0), 100);
    return `
      <tr class="matrix-row" data-key="${escapeHtml(areaKey(area))}">
        <td><code>${escapeHtml(area.pcode || 'Manual')}</code></td>
        <td>${escapeHtml(area.region)}</td>
        <td>${escapeHtml(area.department)}</td>
        <td>${escapeHtml(area.commune)}</td>
        <td><code>${gpsLabel(area)}</code></td>
        <td>${formatNumber(area.phone_owners)}</td>
        <td>${formatNumber(area.population)}</td>
        <td><div class="progress ownership-progress"><div class="progress-bar" style="width:${width.toFixed(1)}%">${formatRate(area.phone_rate)}</div></div></td>
        <td><span class="confidence-pill">${Math.round(area.confidence * 100)}%</span></td>
      </tr>
    `;
  }).join('');

  document.querySelectorAll('.matrix-row').forEach(row => {
    row.addEventListener('click', () => {
      selectedArea = allStats.find(area => areaKey(area) === row.dataset.key);
      renderAreaProfile();
      switchView('profile');
    });
  });
}

function createOption(value, label) {
  const option = document.createElement('option');
  option.value = value;
  option.textContent = label;
  return option;
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
      fillColor: area.phone_rate >= 78 ? '#16a34a' : area.phone_rate >= 64 ? '#2563eb' : '#dc2626',
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
      selectedArea = area;
      renderAreaProfile();
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
      fillColor: '#f97316',
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
  document.getElementById('assets-list').innerHTML = assets.map(asset => `
    <article class="list-card status-${escapeHtml(asset.status)}">
      <div><strong>${escapeHtml(asset.name)}</strong><span>Asset #${asset.id} &middot; ${escapeHtml(asset.asset_type)} &middot; ${escapeHtml(asset.commune)}, ${escapeHtml(asset.department)}</span></div>
      <span class="status-pill status-${escapeHtml(asset.status)}">${escapeHtml(asset.status)}</span>
      <p>${escapeHtml(asset.notes || 'No notes')}</p>
    </article>
  `).join('');
}

function renderReports() {
  document.getElementById('reports-list').innerHTML = reports.map(report => `
    <article class="list-card">
      <div><strong>${escapeHtml(report.report_type)}</strong><span>${escapeHtml(report.commune)}, ${escapeHtml(report.department)} &middot; ${escapeHtml(report.submitted_by)}</span></div>
      <span class="status-pill">${escapeHtml(report.status)}</span>
      <p>${escapeHtml(report.notes)}</p>
    </article>
  `).join('');
}

function renderAlerts() {
  document.getElementById('alerts-list').innerHTML = alerts.map(alert => `
    <article class="list-card severity-${escapeHtml(alert.severity)}">
      <div><strong>${escapeHtml(alert.title)}</strong><span>${escapeHtml(alert.severity)} &middot; ${escapeHtml(alert.status)}</span></div>
      <button class="btn btn-sm btn-outline-secondary resolve-alert" data-id="${alert.id}">Resolve</button>
      <p>${escapeHtml(alert.message)}</p>
    </article>
  `).join('');

  document.querySelectorAll('.resolve-alert').forEach(button => {
    button.addEventListener('click', async () => {
      alerts = await fetchJson(`/api/alerts/${button.dataset.id}`, {
        method: 'PATCH',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ status: 'resolved' }),
      });
      renderAlerts();
      priorityZones = await fetchJson('/api/priority-zones');
      renderPriority();
    });
  });

  renderOverviewAlerts();
}

function renderPriority() {
  document.getElementById('priority-list').innerHTML = priorityZones.slice(0, 18).map(zone => `
    <article class="list-card">
      <div><strong>${escapeHtml(zone.commune)}</strong><span>${escapeHtml(zone.department)}, ${escapeHtml(zone.region)} &middot; ${formatNumber(zone.population)} people</span></div>
      <span class="priority-badge priority-${escapeHtml(zone.priority_label.toLowerCase())}">${escapeHtml(zone.priority_label)} ${zone.priority_score.toFixed(0)}</span>
      <p>${zone.open_alert_count} open alerts &middot; ${zone.asset_count} monitored assets &middot; ${zone.report_count} field reports &middot; ${formatRate(zone.phone_rate)} phone ownership</p>
    </article>
  `).join('');

  renderOverviewPriority();
}

function renderOverviewAlerts() {
  const target = document.getElementById('overview-alerts');
  if (!target) return;
  const openAlerts = alerts.filter(alert => alert.status !== 'resolved').slice(0, 4);
  target.innerHTML = openAlerts.length ? openAlerts.map(alert => `
    <article class="compact-card severity-${escapeHtml(alert.severity)}">
      <div>
        <strong>${escapeHtml(alert.title)}</strong>
        <span>${escapeHtml(alert.severity)} &middot; ${escapeHtml(alert.status)}</span>
      </div>
      <span class="status-pill">${escapeHtml(alert.severity)}</span>
      <p>${escapeHtml(alert.message)}</p>
    </article>
  `).join('') : '<div class="empty-state">No open alerts.</div>';
}

function renderOverviewPriority() {
  const target = document.getElementById('overview-priority');
  if (!target) return;
  target.innerHTML = priorityZones.slice(0, 5).map(zone => `
    <article class="compact-card">
      <div>
        <strong>${escapeHtml(zone.commune)}</strong>
        <span>${escapeHtml(zone.department)}, ${escapeHtml(zone.region)}</span>
      </div>
      <span class="priority-badge priority-${escapeHtml(zone.priority_label.toLowerCase())}">${zone.priority_score.toFixed(0)}</span>
      <p>${formatNumber(zone.population)} people &middot; ${zone.open_alert_count} alerts &middot; ${formatRate(zone.phone_rate)} phone ownership</p>
    </article>
  `).join('');
}

function renderIot() {
  document.getElementById('iot-list').innerHTML = readings.map(reading => `
    <article class="list-card">
      <div><strong>${escapeHtml(reading.reading_type)}</strong><span>Asset #${reading.asset_id} &middot; ${escapeHtml(reading.created_at)}</span></div>
      <span class="status-pill">${Number(reading.value).toLocaleString()} ${escapeHtml(reading.unit)}</span>
    </article>
  `).join('');
}

function renderDecision(report) {
  document.getElementById('decision-report').innerHTML = `
    <div class="profile-grid-inner">
      <div class="metric-tile accent-blue"><span>Monitored assets</span><strong>${report.monitored_assets}</strong><small>${report.open_alerts} open alerts</small></div>
      <div class="metric-tile accent-green"><span>Field reports</span><strong>${report.field_reports}</strong><small>Ground-truth submissions</small></div>
      <div class="metric-tile accent-gold"><span>Top zones</span><strong>${report.top_priority_zones.length}</strong><small>Ranked for action</small></div>
    </div>
    <h3 class="mt-4">Recommended execution</h3>
    <ul>${report.recommendations.map(item => `<li>${escapeHtml(item)}</li>`).join('')}</ul>
    <h3 class="mt-4">Top priority zones</h3>
    <div class="list-stack">
      ${report.top_priority_zones.map(zone => `<article class="list-card"><strong>${escapeHtml(zone.commune)}</strong><span>${escapeHtml(zone.department)}, ${escapeHtml(zone.region)} &middot; Score ${zone.priority_score.toFixed(0)}</span></article>`).join('')}
    </div>
  `;
}

function updateView() {
  const stats = filteredStats();
  renderRegions(stats);
  updateMapMarkers(stats);
  selectedArea = stats[0] || selectedArea;
  renderAreaProfile();
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
  setStatus(dataStatus, 'Loading InfraPulse intelligence layers...', 'info');
  try {
    const [summary, stats, assetData, reportData, alertData, readingData, priorityData, decisionData] = await Promise.all([
      fetchJson('/api/summary'),
      fetchJson('/api/stats'),
      fetchJson('/api/assets'),
      fetchJson('/api/reports'),
      fetchJson('/api/alerts'),
      fetchJson('/api/iot/readings'),
      fetchJson('/api/priority-zones'),
      fetchJson('/api/decision-report'),
    ]);
    allStats = stats;
    assets = assetData;
    reports = reportData;
    alerts = alertData;
    readings = readingData;
    priorityZones = priorityData;
    renderSummary(summary);
    buildFilterOptions();
    renderAssets();
    renderReports();
    renderAlerts();
    renderPriority();
    renderIot();
    renderDecision(decisionData);
    updateView();
    setStatus(dataStatus, `${stats.length} arrondissements, ${assets.length} assets, ${alerts.filter(a => a.status !== 'resolved').length} open alerts.`, 'success');
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
  payload.operator = document.getElementById('assetOperator').value.trim() || null;
  payload.installed_at = document.getElementById('assetInstalledAt').value || null;
  payload.notes = document.getElementById('assetNotes').value.trim() || null;
  try {
    assets = await fetchJson('/api/assets', { method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify(payload) });
    renderAssets();
    updateView();
    setStatus(document.getElementById('asset-status'), 'Asset saved.', 'success');
    event.target.reset();
  } catch (error) {
    setStatus(document.getElementById('asset-status'), error.message, 'danger');
  }
});

document.getElementById('report-form').addEventListener('submit', async event => {
  event.preventDefault();
  const payload = {
    asset_id: document.getElementById('reportAssetId').value ? Number(document.getElementById('reportAssetId').value) : null,
    report_type: document.getElementById('reportType').value.trim(),
    region: document.getElementById('reportRegion').value.trim(),
    department: document.getElementById('reportDepartment').value.trim(),
    commune: document.getElementById('reportCommune').value.trim(),
    latitude: Number(document.getElementById('reportLatitude').value),
    longitude: Number(document.getElementById('reportLongitude').value),
    status: document.getElementById('reportStatus').value,
    notes: document.getElementById('reportNotes').value.trim(),
    submitted_by: document.getElementById('reportSubmittedBy').value.trim(),
  };
  try {
    reports = await fetchJson('/api/reports', { method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify(payload) });
    renderReports();
    updateView();
    setStatus(document.getElementById('report-status'), 'Report submitted.', 'success');
    event.target.reset();
  } catch (error) {
    setStatus(document.getElementById('report-status'), error.message, 'danger');
  }
});

document.querySelectorAll('.tab-button').forEach(button => {
  button.addEventListener('click', () => switchView(button.dataset.view));
});

refreshButton.addEventListener('click', refreshData);
regionFilter.addEventListener('change', () => { buildFilterOptions(); updateView(); });
departmentFilter.addEventListener('change', () => { buildFilterOptions(); updateView(); });
communeFilter.addEventListener('change', updateView);
window.addEventListener('load', () => { initMap(); refreshData(); });
