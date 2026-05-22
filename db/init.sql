CREATE TABLE IF NOT EXISTS mobile_phone_stats (
    id SERIAL PRIMARY KEY,
    pcode TEXT UNIQUE,
    region TEXT NOT NULL,
    department TEXT NOT NULL,
    commune TEXT NOT NULL,
    location TEXT NOT NULL,
    latitude DOUBLE PRECISION NOT NULL,
    longitude DOUBLE PRECISION NOT NULL,
    area_sqkm DOUBLE PRECISION,
    phone_owners BIGINT,
    population BIGINT,
    data_source TEXT NOT NULL DEFAULT 'Manual entry',
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT mobile_phone_stats_gps_bounds CHECK (
        latitude BETWEEN 1.5 AND 13.5
        AND longitude BETWEEN 8.0 AND 16.5
    ),
    CONSTRAINT mobile_phone_stats_non_negative_counts CHECK (
        (phone_owners IS NULL OR phone_owners >= 0)
        AND (population IS NULL OR population >= 0)
        AND (
            phone_owners IS NULL
            OR population IS NULL
            OR phone_owners <= population
        )
    ),
    CONSTRAINT mobile_phone_stats_area_bounds CHECK (
        area_sqkm IS NULL OR area_sqkm >= 0
    ),
    UNIQUE(region, department, commune)
);

CREATE TABLE IF NOT EXISTS organizations (
    id BIGSERIAL PRIMARY KEY,
    name TEXT NOT NULL UNIQUE,
    org_type TEXT NOT NULL,
    contact_name TEXT,
    contact_email TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS projects (
    id BIGSERIAL PRIMARY KEY,
    organization_id BIGINT REFERENCES organizations(id) ON DELETE SET NULL,
    name TEXT NOT NULL,
    sector TEXT NOT NULL,
    region TEXT,
    status TEXT NOT NULL DEFAULT 'planning',
    language_mode TEXT NOT NULL DEFAULT 'bilingual',
    channel_strategy TEXT NOT NULL DEFAULT 'field_team_whatsapp_sms',
    target_segment TEXT NOT NULL DEFAULT 'council_ngo_operator',
    start_date DATE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(organization_id, name)
);

CREATE TABLE IF NOT EXISTS site_profiles (
    id BIGSERIAL PRIMARY KEY,
    project_id BIGINT REFERENCES projects(id) ON DELETE SET NULL,
    name TEXT NOT NULL,
    site_type TEXT NOT NULL,
    region TEXT NOT NULL,
    department TEXT NOT NULL,
    commune TEXT NOT NULL,
    latitude DOUBLE PRECISION NOT NULL,
    longitude DOUBLE PRECISION NOT NULL,
    beneficiary_estimate BIGINT,
    trust_signal TEXT NOT NULL DEFAULT 'field_verified',
    access_notes TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(name, commune)
);

CREATE TABLE IF NOT EXISTS survey_campaigns (
    id BIGSERIAL PRIMARY KEY,
    project_id BIGINT REFERENCES projects(id) ON DELETE SET NULL,
    name TEXT NOT NULL,
    form_type TEXT NOT NULL,
    target_region TEXT,
    target_department TEXT,
    target_commune TEXT,
    status TEXT NOT NULL DEFAULT 'draft',
    language_mode TEXT NOT NULL DEFAULT 'bilingual',
    offline_enabled BOOLEAN NOT NULL DEFAULT TRUE,
    starts_on DATE,
    ends_on DATE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(project_id, name)
);

CREATE TABLE IF NOT EXISTS infrastructure_assets (
    id BIGSERIAL PRIMARY KEY,
    project_id BIGINT REFERENCES projects(id) ON DELETE SET NULL,
    site_profile_id BIGINT REFERENCES site_profiles(id) ON DELETE SET NULL,
    asset_type TEXT NOT NULL,
    name TEXT NOT NULL,
    region TEXT NOT NULL,
    department TEXT NOT NULL,
    commune TEXT NOT NULL,
    latitude DOUBLE PRECISION NOT NULL,
    longitude DOUBLE PRECISION NOT NULL,
    status TEXT NOT NULL,
    operator TEXT,
    installed_at DATE,
    last_checked_at TIMESTAMPTZ,
    notes TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(name, commune)
);

CREATE TABLE IF NOT EXISTS field_reports (
    id BIGSERIAL PRIMARY KEY,
    project_id BIGINT REFERENCES projects(id) ON DELETE SET NULL,
    site_profile_id BIGINT REFERENCES site_profiles(id) ON DELETE SET NULL,
    campaign_id BIGINT REFERENCES survey_campaigns(id) ON DELETE SET NULL,
    asset_id BIGINT REFERENCES infrastructure_assets(id) ON DELETE SET NULL,
    report_type TEXT NOT NULL,
    region TEXT NOT NULL,
    department TEXT NOT NULL,
    commune TEXT NOT NULL,
    latitude DOUBLE PRECISION NOT NULL,
    longitude DOUBLE PRECISION NOT NULL,
    status TEXT NOT NULL,
    evidence_quality TEXT NOT NULL DEFAULT 'unverified',
    notes TEXT NOT NULL,
    submitted_by TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS alerts (
    id BIGSERIAL PRIMARY KEY,
    project_id BIGINT REFERENCES projects(id) ON DELETE SET NULL,
    site_profile_id BIGINT REFERENCES site_profiles(id) ON DELETE SET NULL,
    asset_id BIGINT REFERENCES infrastructure_assets(id) ON DELETE SET NULL,
    severity TEXT NOT NULL,
    title TEXT NOT NULL,
    message TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'open',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    resolved_at TIMESTAMPTZ
);

CREATE TABLE IF NOT EXISTS maintenance_tickets (
    id BIGSERIAL PRIMARY KEY,
    project_id BIGINT REFERENCES projects(id) ON DELETE SET NULL,
    site_profile_id BIGINT REFERENCES site_profiles(id) ON DELETE SET NULL,
    asset_id BIGINT REFERENCES infrastructure_assets(id) ON DELETE SET NULL,
    alert_id BIGINT REFERENCES alerts(id) ON DELETE SET NULL,
    title TEXT NOT NULL,
    priority TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'open',
    assigned_to TEXT,
    due_date DATE,
    sla_hours INTEGER,
    resolution_notes TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS iot_readings (
    id BIGSERIAL PRIMARY KEY,
    project_id BIGINT REFERENCES projects(id) ON DELETE SET NULL,
    site_profile_id BIGINT REFERENCES site_profiles(id) ON DELETE SET NULL,
    asset_id BIGINT NOT NULL REFERENCES infrastructure_assets(id) ON DELETE CASCADE,
    reading_type TEXT NOT NULL,
    value DOUBLE PRECISION NOT NULL,
    unit TEXT NOT NULL,
    latitude DOUBLE PRECISION,
    longitude DOUBLE PRECISION,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS decision_snapshots (
    id BIGSERIAL PRIMARY KEY,
    project_id BIGINT REFERENCES projects(id) ON DELETE SET NULL,
    site_profile_id BIGINT REFERENCES site_profiles(id) ON DELETE SET NULL,
    asset_id BIGINT REFERENCES infrastructure_assets(id) ON DELETE SET NULL,
    title TEXT NOT NULL,
    decision_stage TEXT NOT NULL DEFAULT 'draft',
    priority_score DOUBLE PRECISION NOT NULL DEFAULT 0,
    recommended_budget_xaf BIGINT,
    owner_name TEXT,
    risk_level TEXT NOT NULL DEFAULT 'medium',
    evidence_score DOUBLE PRECISION NOT NULL DEFAULT 0,
    approval_notes TEXT,
    execution_status TEXT NOT NULL DEFAULT 'not_started',
    rationale TEXT NOT NULL,
    next_action TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS execution_plans (
    id BIGSERIAL PRIMARY KEY,
    decision_id BIGINT REFERENCES decision_snapshots(id) ON DELETE SET NULL,
    project_id BIGINT REFERENCES projects(id) ON DELETE SET NULL,
    site_profile_id BIGINT REFERENCES site_profiles(id) ON DELETE SET NULL,
    asset_id BIGINT REFERENCES infrastructure_assets(id) ON DELETE SET NULL,
    title TEXT NOT NULL,
    owner_name TEXT,
    status TEXT NOT NULL DEFAULT 'planned',
    budget_xaf BIGINT,
    planned_start DATE,
    planned_end DATE,
    local_focal_point_confirmed BOOLEAN NOT NULL DEFAULT FALSE,
    gps_photo_proof_required BOOLEAN NOT NULL DEFAULT TRUE,
    offline_survey_ready BOOLEAN NOT NULL DEFAULT FALSE,
    bilingual_script_ready BOOLEAN NOT NULL DEFAULT TRUE,
    transport_access_notes TEXT,
    xaf_budget_approved BOOLEAN NOT NULL DEFAULT FALSE,
    blocker TEXT,
    outcome_notes TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
