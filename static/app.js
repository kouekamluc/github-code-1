const CAMEROON_BOUNDS = {
  minLatitude: 1.5,
  maxLatitude: 13.5,
  minLongitude: 8,
  maxLongitude: 16.5,
};

const summaryCards = document.getElementById('summary-cards');
const tableBody = document.getElementById('regions-table-body');
const refreshButton = document.getElementById('refresh-button');
const updateForm = document.getElementById('update-form');
const updateStatus = document.getElementById('update-status');
const dataStatus = document.getElementById('data-status');
const regionFilter = document.getElementById('regionFilter');
const departmentFilter = document.getElementById('departmentFilter');
const communeFilter = document.getElementById('communeFilter');

let map;
let markersLayer;
let allStats = [];

async function fetchJson(url, options = {}) {
  const response = await fetch(url, options);
  const contentType = response.headers.get('content-type') || '';
  const body = contentType.includes('application/json') ? await response.json() : null;

  if (!response.ok) {
    throw new Error(body?.message || `Request failed with status ${response.status}`);
  }

  return body;
}

function escapeHtml(value) {
  return String(value)
    .replaceAll('&', '&amp;')
    .replaceAll('<', '&lt;')
    .replaceAll('>', '&gt;')
    .replaceAll('"', '&quot;')
    .replaceAll("'", '&#039;');
}

function formatCoordinate(value) {
  return Number(value).toFixed(4);
}

function getGpsLabel(stat) {
  return `${formatCoordinate(stat.latitude)}, ${formatCoordinate(stat.longitude)}`;
}

function formatNumber(value) {
  return value === null || value === undefined ? 'Unknown' : Number(value).toLocaleString();
}

function formatRate(value) {
  return value === null || value === undefined ? 'No data' : `${Number(value).toFixed(1)}%`;
}

function isInCameroon(latitude, longitude) {
  return Number.isFinite(latitude)
    && Number.isFinite(longitude)
    && latitude >= CAMEROON_BOUNDS.minLatitude
    && latitude <= CAMEROON_BOUNDS.maxLatitude
    && longitude >= CAMEROON_BOUNDS.minLongitude
    && longitude <= CAMEROON_BOUNDS.maxLongitude;
}

function setDataStatus(message, type = 'info') {
  if (!message) {
    dataStatus.innerHTML = '';
    return;
  }

  dataStatus.innerHTML = `<div class="alert alert-${type} py-2 mb-0">${escapeHtml(message)}</div>`;
}

function setUpdateStatus(message, type = 'info') {
  updateStatus.innerHTML = `<div class="alert alert-${type} py-2">${escapeHtml(message)}</div>`;
}

async function fetchSummary() {
  return fetchJson('/api/summary');
}

async function fetchStats() {
  return fetchJson('/api/stats');
}

function renderSummary(summary) {
  summaryCards.innerHTML = `
    <div class="metric-tile accent-blue">
      <span>Phone owners</span>
      <strong>${formatNumber(summary.total_phone_owners)}</strong>
      <small>${summary.estimated_location_count} matrix-estimated units</small>
    </div>
    <div class="metric-tile accent-green">
      <span>Total population</span>
      <strong>${formatNumber(summary.total_population)}</strong>
      <small>${summary.commune_count} arrondissements</small>
    </div>
    <div class="metric-tile accent-gold">
      <span>Ownership rate</span>
      <strong>${summary.percent_with_phone.toFixed(1)}%</strong>
      <small>${summary.measured_location_count} measured overrides</small>
    </div>
    <div class="metric-tile accent-red">
      <span>Administrative reach</span>
      <strong>${summary.department_count}</strong>
      <small>${summary.region_count} regions mapped</small>
    </div>
  `;
}

function renderRegions(regions) {
  if (!regions.length) {
    tableBody.innerHTML = `
      <tr>
        <td colspan="10" class="text-center text-muted py-4">No locations match the selected filters.</td>
      </tr>
    `;
    return;
  }

  tableBody.innerHTML = regions
    .map(region => {
      const rate = region.phone_rate ?? 0;
      const progressWidth = Math.min(Math.max(rate, 0), 100);
      const rateCell = region.phone_rate === null || region.phone_rate === undefined
        ? '<span class="text-muted">No data</span>'
        : `
            <div class="progress ownership-progress">
              <div class="progress-bar" role="progressbar" style="width: ${progressWidth.toFixed(1)}%;" aria-valuenow="${rate.toFixed(1)}" aria-valuemin="0" aria-valuemax="100">
                ${formatRate(region.phone_rate)}
              </div>
            </div>
          `;
      return `
        <tr>
          <td><code>${escapeHtml(region.pcode || 'Manual')}</code></td>
          <td>${escapeHtml(region.region)}</td>
          <td>${escapeHtml(region.department)}</td>
          <td>${escapeHtml(region.commune)}</td>
          <td>${escapeHtml(region.location)}</td>
          <td><code>${escapeHtml(getGpsLabel(region))}</code></td>
          <td>${formatNumber(region.phone_owners)}</td>
          <td>${formatNumber(region.population)}</td>
          <td>${rateCell}</td>
          <td><span class="confidence-pill">${Math.round(region.confidence * 100)}%</span></td>
        </tr>
      `;
    })
    .join('');
}

function createOptionElement(value, label) {
  const option = document.createElement('option');
  option.value = value;
  option.textContent = label;
  return option;
}

function populateFilter(selectElement, values, selectedValue) {
  selectElement.innerHTML = '';
  selectElement.appendChild(createOptionElement('all', `All ${selectElement.dataset.label}`));

  values.forEach(value => {
    const option = createOptionElement(value, value);
    if (selectedValue && value === selectedValue) {
      option.selected = true;
    }
    selectElement.appendChild(option);
  });
}

function buildFilterOptions() {
  const selectedRegion = regionFilter.value;
  const selectedDepartment = departmentFilter.value;
  const selectedCommune = communeFilter.value;

  const regions = [...new Set(allStats.map(stat => stat.region))].sort();
  const nextRegion = regions.includes(selectedRegion) ? selectedRegion : 'all';
  populateFilter(regionFilter, regions, nextRegion !== 'all' ? nextRegion : null);

  const departments = [...new Set(allStats
    .filter(stat => regionFilter.value === 'all' || stat.region === regionFilter.value)
    .map(stat => stat.department))].sort();
  const nextDepartment = departments.includes(selectedDepartment) ? selectedDepartment : 'all';
  populateFilter(departmentFilter, departments, nextDepartment !== 'all' ? nextDepartment : null);

  const communes = [...new Set(allStats
    .filter(stat => (regionFilter.value === 'all' || stat.region === regionFilter.value)
      && (departmentFilter.value === 'all' || stat.department === departmentFilter.value))
    .map(stat => stat.commune))].sort();
  const nextCommune = communes.includes(selectedCommune) ? selectedCommune : 'all';
  populateFilter(communeFilter, communes, nextCommune !== 'all' ? nextCommune : null);
}

function getFilteredStats() {
  return allStats.filter(item => {
    return (regionFilter.value === 'all' || item.region === regionFilter.value)
      && (departmentFilter.value === 'all' || item.department === departmentFilter.value)
      && (communeFilter.value === 'all' || item.commune === communeFilter.value);
  });
}

function updateMapMarkers(stats) {
  markersLayer.clearLayers();

  const validStats = stats.filter(stat => isInCameroon(Number(stat.latitude), Number(stat.longitude)));
  if (!validStats.length) {
    map.setView([6.5, 12.5], 6);
    return;
  }

  const bounds = [];
  validStats.forEach(stat => {
    const rate = stat.phone_rate ?? 0;
    const marker = L.circleMarker([stat.latitude, stat.longitude], {
      radius: Math.max(7, Math.min(16, rate / 7)),
      fillColor: rate >= 78 ? '#16a34a' : rate >= 64 ? '#2563eb' : '#dc2626',
      color: '#fff',
      weight: 2,
      opacity: 1,
      fillOpacity: 0.85,
    });

    marker.bindPopup(`
      <strong>${escapeHtml(stat.location)}</strong><br />
      ${escapeHtml(stat.commune)}, ${escapeHtml(stat.department)}, ${escapeHtml(stat.region)}<br />
      GPS: <code>${escapeHtml(getGpsLabel(stat))}</code><br />
      P-code: ${escapeHtml(stat.pcode || 'Manual')}<br />
      Area: ${stat.area_sqkm ? `${Number(stat.area_sqkm).toLocaleString()} km²` : 'Unknown'}<br />
      Owners: ${formatNumber(stat.phone_owners)}<br />
      Population: ${formatNumber(stat.population)}<br />
      Rate: ${formatRate(stat.phone_rate)}<br />
      Confidence: ${Math.round(stat.confidence * 100)}%<br />
      Source: ${escapeHtml(stat.metric_source)}
    `);
    marker.addTo(markersLayer);
    bounds.push([stat.latitude, stat.longitude]);
  });

  map.fitBounds(L.latLngBounds(bounds), { padding: [40, 40], maxZoom: 10 });
}

async function refreshData() {
  refreshButton.disabled = true;
  setDataStatus('Loading GPS matrix...', 'info');

  try {
    const [summary, stats] = await Promise.all([fetchSummary(), fetchStats()]);
    allStats = stats;
    renderSummary(summary);
    buildFilterOptions();
    updateView();
    setDataStatus(`${stats.length} arrondissements modeled with GPS, area, and national telecom baselines.`, 'success');
  } catch (error) {
    console.error('Unable to fetch data', error);
    setDataStatus(error.message || 'Unable to fetch data from the local API.', 'danger');
  } finally {
    refreshButton.disabled = false;
  }
}

function updateView() {
  const filtered = getFilteredStats();
  renderRegions(filtered);
  updateMapMarkers(filtered);
}

function initMap() {
  map = L.map('map', { scrollWheelZoom: true }).setView([6.5, 12.5], 6);
  L.tileLayer('https://{s}.tile.openstreetmap.org/{z}/{x}/{y}.png', {
    maxZoom: 19,
    attribution: '&copy; OpenStreetMap contributors',
  }).addTo(map);
  markersLayer = L.layerGroup().addTo(map);
}

function buildFormPayload() {
  return {
    pcode: document.getElementById('pcode').value.trim() || null,
    region: document.getElementById('region').value.trim(),
    department: document.getElementById('department').value.trim(),
    commune: document.getElementById('commune').value.trim(),
    location: document.getElementById('location').value.trim(),
    latitude: Number(document.getElementById('latitude').value),
    longitude: Number(document.getElementById('longitude').value),
    phone_owners: document.getElementById('phoneOwners').value === ''
      ? null
      : Number(document.getElementById('phoneOwners').value),
    population: document.getElementById('population').value === ''
      ? null
      : Number(document.getElementById('population').value),
  };
}

function validatePayload(payload) {
  if (!payload.region || !payload.department || !payload.commune || !payload.location) {
    return 'Region, department, commune, and location are required.';
  }

  if (!Number.isFinite(payload.latitude) || !Number.isFinite(payload.longitude)) {
    return 'Latitude and longitude must be valid numbers.';
  }

  if (!isInCameroon(payload.latitude, payload.longitude)) {
    return 'GPS coordinates must be inside Cameroon.';
  }

  if ((payload.phone_owners === null) !== (payload.population === null)) {
    return 'Phone owners and population must be provided together.';
  }

  if (payload.phone_owners === null && payload.population === null) {
    return null;
  }

  if (!Number.isInteger(payload.phone_owners) || !Number.isInteger(payload.population)) {
    return 'Phone owners and population must be whole numbers.';
  }

  if (payload.phone_owners < 0 || payload.population < 0) {
    return 'Phone owners and population cannot be negative.';
  }

  if (payload.phone_owners > payload.population) {
    return 'Phone owners cannot be greater than population.';
  }

  return null;
}

updateForm.addEventListener('submit', async event => {
  event.preventDefault();
  const payload = buildFormPayload();
  const validationError = validatePayload(payload);

  if (validationError) {
    setUpdateStatus(validationError, 'warning');
    return;
  }

  try {
    updateForm.querySelector('button[type="submit"]').disabled = true;
    allStats = await fetchJson('/api/stats/update', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(payload),
    });

    buildFilterOptions();
    updateView();
    renderSummary(await fetchSummary());
    setDataStatus(`${allStats.length} arrondissements modeled with GPS, area, and national telecom baselines.`, 'success');
    setUpdateStatus('Location data updated successfully.', 'success');
    updateForm.reset();
  } catch (error) {
    console.error(error);
    setUpdateStatus(error.message || 'Unable to save location data.', 'danger');
  } finally {
    updateForm.querySelector('button[type="submit"]').disabled = false;
  }
});

refreshButton.addEventListener('click', refreshData);
regionFilter.addEventListener('change', () => { buildFilterOptions(); updateView(); });
departmentFilter.addEventListener('change', () => { buildFilterOptions(); updateView(); });
communeFilter.addEventListener('change', updateView);
window.addEventListener('load', () => { initMap(); refreshData(); });
