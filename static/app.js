const summaryCards = document.getElementById('summary-cards');
const tableBody = document.getElementById('regions-table-body');
const refreshButton = document.getElementById('refresh-button');
const updateForm = document.getElementById('update-form');
const updateStatus = document.getElementById('update-status');
const regionFilter = document.getElementById('regionFilter');
const departmentFilter = document.getElementById('departmentFilter');
const communeFilter = document.getElementById('communeFilter');

let map;
let markersLayer;
let allStats = [];

async function fetchSummary() {
  const response = await fetch('/api/summary');
  return response.json();
}

async function fetchStats() {
  const response = await fetch('/api/stats');
  return response.json();
}

function renderSummary(summary) {
  summaryCards.innerHTML = `
    <div class="col-md-4 mb-3">
      <div class="card border-primary">
        <div class="card-body text-center">
          <h5 class="card-subtitle mb-2 text-muted">Phone owners</h5>
          <p class="display-6 mb-0">${summary.total_phone_owners.toLocaleString()}</p>
        </div>
      </div>
    </div>
    <div class="col-md-4 mb-3">
      <div class="card border-success">
        <div class="card-body text-center">
          <h5 class="card-subtitle mb-2 text-muted">Total population</h5>
          <p class="display-6 mb-0">${summary.total_population.toLocaleString()}</p>
        </div>
      </div>
    </div>
    <div class="col-md-4 mb-3">
      <div class="card border-warning">
        <div class="card-body text-center">
          <h5 class="card-subtitle mb-2 text-muted">Ownership rate</h5>
          <p class="display-6 mb-0">${summary.percent_with_phone.toFixed(1)}%</p>
          <small class="text-muted">${summary.region_count} regions · ${summary.department_count} departments</small>
        </div>
      </div>
    </div>
  `;
}

function renderRegions(regions) {
  tableBody.innerHTML = regions
    .map(region => {
      const rate = region.population > 0 ? region.phone_rate : 0;
      return `
        <tr>
          <td>${region.region}</td>
          <td>${region.department}</td>
          <td>${region.commune}</td>
          <td>${region.location}</td>
          <td>${region.phone_owners.toLocaleString()}</td>
          <td>${region.population.toLocaleString()}</td>
          <td>
            <div class="progress" style="height: 1rem;">
              <div class="progress-bar" role="progressbar" style="width: ${rate.toFixed(1)}%;" aria-valuenow="${rate.toFixed(1)}" aria-valuemin="0" aria-valuemax="100">
                ${rate.toFixed(1)}%
              </div>
            </div>
          </td>
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
  const regionValues = [...new Set(allStats.map(s => s.region))].sort();
  populateFilter(regionFilter, regionValues, regionFilter.value !== 'all' ? regionFilter.value : null);

  const departments = [...new Set(allStats
    .filter(s => regionFilter.value === 'all' || s.region === regionFilter.value)
    .map(s => s.department))].sort();
  populateFilter(departmentFilter, departments, departmentFilter.value !== 'all' ? departmentFilter.value : null);

  const communes = [...new Set(allStats
    .filter(s => (regionFilter.value === 'all' || s.region === regionFilter.value) &&
                 (departmentFilter.value === 'all' || s.department === departmentFilter.value))
    .map(s => s.commune))].sort();
  populateFilter(communeFilter, communes, communeFilter.value !== 'all' ? communeFilter.value : null);
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

  if (!stats.length) {
    return;
  }

  const bounds = [];
  stats.forEach(stat => {
    const marker = L.circleMarker([stat.latitude, stat.longitude], {
      radius: 9,
      fillColor: '#0d6efd',
      color: '#fff',
      weight: 1,
      opacity: 1,
      fillOpacity: 0.8,
    });
    marker.bindPopup(`
      <strong>${stat.location}</strong><br />
      ${stat.commune}, ${stat.department}, ${stat.region}<br />
      Owners: ${stat.phone_owners.toLocaleString()}<br />
      Population: ${stat.population.toLocaleString()}<br />
      Rate: ${stat.phone_rate.toFixed(1)}%
    `);
    marker.addTo(markersLayer);
    bounds.push([stat.latitude, stat.longitude]);
  });

  if (bounds.length) {
    const mapBounds = L.latLngBounds(bounds);
    map.fitBounds(mapBounds, { padding: [40, 40], maxZoom: 10 });
  }
}

async function refreshData() {
  try {
    const [summary, stats] = await Promise.all([fetchSummary(), fetchStats()]);
    allStats = stats;
    renderSummary(summary);
    buildFilterOptions();
    const filtered = getFilteredStats();
    renderRegions(filtered);
    updateMapMarkers(filtered);
  } catch (error) {
    console.error('Unable to fetch data', error);
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

updateForm.addEventListener('submit', async event => {
  event.preventDefault();

  const payload = {
    region: document.getElementById('region').value.trim(),
    department: document.getElementById('department').value.trim(),
    commune: document.getElementById('commune').value.trim(),
    location: document.getElementById('location').value.trim(),
    latitude: Number(document.getElementById('latitude').value),
    longitude: Number(document.getElementById('longitude').value),
    phone_owners: Number(document.getElementById('phoneOwners').value),
    population: Number(document.getElementById('population').value),
  };

  try {
    const response = await fetch('/api/stats/update', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(payload),
    });

    if (!response.ok) {
      throw new Error('Update failed');
    }

    allStats = await response.json();
    buildFilterOptions();
    updateView();
    const summary = await fetchSummary();
    renderSummary(summary);
    updateStatus.innerHTML = '<div class="alert alert-success py-2">Location data updated successfully.</div>';
    updateForm.reset();
  } catch (error) {
    console.error(error);
    updateStatus.innerHTML = '<div class="alert alert-danger py-2">Unable to save location data.</div>';
  }
});

refreshButton.addEventListener('click', refreshData);
regionFilter.addEventListener('change', () => { buildFilterOptions(); updateView(); });
departmentFilter.addEventListener('change', () => { buildFilterOptions(); updateView(); });
communeFilter.addEventListener('change', updateView);
window.addEventListener('load', () => { initMap(); refreshData(); });
