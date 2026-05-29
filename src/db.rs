use sqlx::PgPool;

pub(crate) async fn ensure_schema(pool: &PgPool) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS mobile_phone_stats (
            id SERIAL PRIMARY KEY,
            pcode TEXT,
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
            UNIQUE(region, department, commune),
            UNIQUE(pcode)
        )
        "#,
    )
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        ALTER TABLE mobile_phone_stats
        ADD COLUMN IF NOT EXISTS pcode TEXT,
        ADD COLUMN IF NOT EXISTS area_sqkm DOUBLE PRECISION,
        ADD COLUMN IF NOT EXISTS data_source TEXT NOT NULL DEFAULT 'Manual entry',
        ALTER COLUMN phone_owners TYPE BIGINT,
        ALTER COLUMN population TYPE BIGINT,
        ALTER COLUMN phone_owners DROP NOT NULL,
        ALTER COLUMN population DROP NOT NULL
        "#,
    )
    .execute(pool)
    .await?;

    let operational_schema = [
        r#"
        CREATE TABLE IF NOT EXISTS organizations (
            id BIGSERIAL PRIMARY KEY,
            name TEXT NOT NULL UNIQUE,
            org_type TEXT NOT NULL,
            contact_name TEXT,
            contact_email TEXT,
            created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
        )
        "#,
        r#"
        CREATE TABLE IF NOT EXISTS users (
            id BIGSERIAL PRIMARY KEY,
            username TEXT NOT NULL UNIQUE,
            email TEXT NOT NULL UNIQUE,
            display_name TEXT NOT NULL,
            role TEXT NOT NULL DEFAULT 'operator',
            password_hash TEXT NOT NULL,
            is_active BOOLEAN NOT NULL DEFAULT TRUE,
            created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
        )
        "#,
        r#"
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
        )
        "#,
        r#"
        CREATE TABLE IF NOT EXISTS workspace_memberships (
            id BIGSERIAL PRIMARY KEY,
            user_id BIGINT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
            organization_id BIGINT REFERENCES organizations(id) ON DELETE CASCADE,
            project_id BIGINT REFERENCES projects(id) ON DELETE CASCADE,
            role TEXT NOT NULL,
            created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
            UNIQUE(user_id, organization_id, project_id)
        )
        "#,
        r#"
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
        )
        "#,
        r#"
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
        )
        "#,
        r#"
        CREATE TABLE IF NOT EXISTS workspace_templates (
            id TEXT PRIMARY KEY,
            title TEXT NOT NULL,
            description TEXT NOT NULL,
            org_type TEXT NOT NULL,
            sector TEXT NOT NULL,
            site_type TEXT NOT NULL,
            form_type TEXT NOT NULL,
            trust_signal TEXT NOT NULL,
            default_project_status TEXT NOT NULL DEFAULT 'planning',
            language_mode TEXT NOT NULL DEFAULT 'bilingual',
            offline_enabled BOOLEAN NOT NULL DEFAULT TRUE,
            channel_strategy TEXT NOT NULL DEFAULT 'field_team_whatsapp_sms',
            target_segment TEXT NOT NULL DEFAULT 'council_ngo_operator',
            default_actions TEXT[] NOT NULL DEFAULT ARRAY['site', 'campaign', 'decision']::TEXT[],
            required_evidence TEXT[] NOT NULL DEFAULT ARRAY['gps_photo', 'local_focal_point']::TEXT[],
            creates_asset BOOLEAN NOT NULL DEFAULT FALSE,
            creates_report_task BOOLEAN NOT NULL DEFAULT FALSE,
            creates_alert BOOLEAN NOT NULL DEFAULT FALSE,
            creates_ticket BOOLEAN NOT NULL DEFAULT FALSE,
            active BOOLEAN NOT NULL DEFAULT TRUE,
            sort_order INTEGER NOT NULL DEFAULT 100,
            created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
            updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
        )
        "#,
        r#"
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
        )
        "#,
        r#"
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
        )
        "#,
        r#"
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
        )
        "#,
        r#"
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
        )
        "#,
        r#"
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
        )
        "#,
        r#"
        DO $$
        BEGIN
            IF to_regclass('maintenance_tickets') IS NULL
               AND to_regclass('maintenance_tickets_id_seq') IS NOT NULL THEN
                DROP SEQUENCE maintenance_tickets_id_seq;
            END IF;
        END $$;
        "#,
        r#"
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
        )
        "#,
        r#"
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
        )
        "#,
        r#"
        CREATE TABLE IF NOT EXISTS operator_imei_events (
            id BIGSERIAL PRIMARY KEY,
            operator_name TEXT NOT NULL,
            imei_hash TEXT NOT NULL,
            imei_last4 TEXT,
            device_type TEXT,
            event_type TEXT NOT NULL,
            compliance_status TEXT NOT NULL,
            region TEXT,
            department TEXT,
            commune TEXT,
            source_system TEXT NOT NULL DEFAULT 'operator_api',
            raw_reference TEXT,
            network_first_seen_at TIMESTAMPTZ,
            created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
            CONSTRAINT operator_imei_events_status_check CHECK (
                compliance_status IN ('cleared', 'pending', 'blocked', 'unknown')
            ),
            CONSTRAINT operator_imei_events_type_check CHECK (
                event_type IN ('activation', 'verification', 'blocked', 'allowed', 'customs_cleared', 'customs_pending')
            )
        )
        "#,
        r#"
        CREATE TABLE IF NOT EXISTS audit_events (
            id BIGSERIAL PRIMARY KEY,
            entity_type TEXT NOT NULL,
            entity_id BIGINT NOT NULL,
            field_name TEXT NOT NULL,
            old_value TEXT,
            new_value TEXT,
            actor TEXT NOT NULL DEFAULT 'system',
            note TEXT,
            created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
        )
        "#,
        r#"
        CREATE TABLE IF NOT EXISTS evidence_files (
            id BIGSERIAL PRIMARY KEY,
            entity_type TEXT NOT NULL,
            entity_id BIGINT NOT NULL,
            file_name TEXT NOT NULL,
            content_type TEXT NOT NULL,
            storage_path TEXT NOT NULL,
            sha256_hash TEXT NOT NULL,
            file_size BIGINT NOT NULL,
            latitude DOUBLE PRECISION,
            longitude DOUBLE PRECISION,
            captured_at TIMESTAMPTZ,
            uploaded_by TEXT NOT NULL DEFAULT 'system',
            created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
        )
        "#,
        r#"
        CREATE TABLE IF NOT EXISTS auth_sessions (
            id BIGSERIAL PRIMARY KEY,
            user_id BIGINT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
            token TEXT NOT NULL UNIQUE,
            expires_at TIMESTAMPTZ NOT NULL,
            created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
        )
        "#,
    ];

    for statement in operational_schema {
        sqlx::query(statement).execute(pool).await?;
    }

    let compatibility_schema = [
        r#"
        ALTER TABLE projects
        ADD COLUMN IF NOT EXISTS language_mode TEXT NOT NULL DEFAULT 'bilingual',
        ADD COLUMN IF NOT EXISTS channel_strategy TEXT NOT NULL DEFAULT 'field_team_whatsapp_sms',
        ADD COLUMN IF NOT EXISTS target_segment TEXT NOT NULL DEFAULT 'council_ngo_operator'
        "#,
        r#"
        ALTER TABLE infrastructure_assets
        ADD COLUMN IF NOT EXISTS project_id BIGINT REFERENCES projects(id) ON DELETE SET NULL,
        ADD COLUMN IF NOT EXISTS site_profile_id BIGINT REFERENCES site_profiles(id) ON DELETE SET NULL
        "#,
        r#"
        ALTER TABLE field_reports
        ADD COLUMN IF NOT EXISTS project_id BIGINT REFERENCES projects(id) ON DELETE SET NULL,
        ADD COLUMN IF NOT EXISTS site_profile_id BIGINT REFERENCES site_profiles(id) ON DELETE SET NULL,
        ADD COLUMN IF NOT EXISTS campaign_id BIGINT REFERENCES survey_campaigns(id) ON DELETE SET NULL,
        ADD COLUMN IF NOT EXISTS evidence_quality TEXT NOT NULL DEFAULT 'unverified'
        "#,
        r#"
        ALTER TABLE alerts
        ADD COLUMN IF NOT EXISTS project_id BIGINT REFERENCES projects(id) ON DELETE SET NULL,
        ADD COLUMN IF NOT EXISTS site_profile_id BIGINT REFERENCES site_profiles(id) ON DELETE SET NULL
        "#,
        r#"
        ALTER TABLE maintenance_tickets
        ADD COLUMN IF NOT EXISTS project_id BIGINT REFERENCES projects(id) ON DELETE SET NULL,
        ADD COLUMN IF NOT EXISTS site_profile_id BIGINT REFERENCES site_profiles(id) ON DELETE SET NULL,
        ADD COLUMN IF NOT EXISTS sla_hours INTEGER
        "#,
        r#"
        ALTER TABLE iot_readings
        ADD COLUMN IF NOT EXISTS project_id BIGINT REFERENCES projects(id) ON DELETE SET NULL,
        ADD COLUMN IF NOT EXISTS site_profile_id BIGINT REFERENCES site_profiles(id) ON DELETE SET NULL
        "#,
        r#"
        ALTER TABLE decision_snapshots
        ADD COLUMN IF NOT EXISTS site_profile_id BIGINT REFERENCES site_profiles(id) ON DELETE SET NULL,
        ADD COLUMN IF NOT EXISTS asset_id BIGINT REFERENCES infrastructure_assets(id) ON DELETE SET NULL,
        ADD COLUMN IF NOT EXISTS owner_name TEXT,
        ADD COLUMN IF NOT EXISTS risk_level TEXT NOT NULL DEFAULT 'medium',
        ADD COLUMN IF NOT EXISTS evidence_score DOUBLE PRECISION NOT NULL DEFAULT 0,
        ADD COLUMN IF NOT EXISTS approval_notes TEXT,
        ADD COLUMN IF NOT EXISTS execution_status TEXT NOT NULL DEFAULT 'not_started'
        "#,
        r#"
        ALTER TABLE users
        ADD COLUMN IF NOT EXISTS username TEXT,
        ADD COLUMN IF NOT EXISTS password_hash TEXT,
        ADD COLUMN IF NOT EXISTS is_active BOOLEAN NOT NULL DEFAULT TRUE
        "#,
        r#"
        ALTER TABLE evidence_files
        ADD COLUMN IF NOT EXISTS file_size BIGINT NOT NULL DEFAULT 0,
        ADD COLUMN IF NOT EXISTS latitude DOUBLE PRECISION,
        ADD COLUMN IF NOT EXISTS longitude DOUBLE PRECISION,
        ADD COLUMN IF NOT EXISTS captured_at TIMESTAMPTZ,
        ADD COLUMN IF NOT EXISTS uploaded_by TEXT NOT NULL DEFAULT 'system'
        "#,
        r#"
        ALTER TABLE workspace_templates
        ADD COLUMN IF NOT EXISTS description TEXT NOT NULL DEFAULT '',
        ADD COLUMN IF NOT EXISTS default_project_status TEXT NOT NULL DEFAULT 'planning',
        ADD COLUMN IF NOT EXISTS language_mode TEXT NOT NULL DEFAULT 'bilingual',
        ADD COLUMN IF NOT EXISTS offline_enabled BOOLEAN NOT NULL DEFAULT TRUE,
        ADD COLUMN IF NOT EXISTS channel_strategy TEXT NOT NULL DEFAULT 'field_team_whatsapp_sms',
        ADD COLUMN IF NOT EXISTS target_segment TEXT NOT NULL DEFAULT 'council_ngo_operator',
        ADD COLUMN IF NOT EXISTS default_actions TEXT[] NOT NULL DEFAULT ARRAY['site', 'campaign', 'decision']::TEXT[],
        ADD COLUMN IF NOT EXISTS required_evidence TEXT[] NOT NULL DEFAULT ARRAY['gps_photo', 'local_focal_point']::TEXT[],
        ADD COLUMN IF NOT EXISTS creates_asset BOOLEAN NOT NULL DEFAULT FALSE,
        ADD COLUMN IF NOT EXISTS creates_report_task BOOLEAN NOT NULL DEFAULT FALSE,
        ADD COLUMN IF NOT EXISTS creates_alert BOOLEAN NOT NULL DEFAULT FALSE,
        ADD COLUMN IF NOT EXISTS creates_ticket BOOLEAN NOT NULL DEFAULT FALSE,
        ADD COLUMN IF NOT EXISTS active BOOLEAN NOT NULL DEFAULT TRUE,
        ADD COLUMN IF NOT EXISTS sort_order INTEGER NOT NULL DEFAULT 100,
        ADD COLUMN IF NOT EXISTS updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
        "#,
    ];

    for statement in compatibility_schema {
        sqlx::query(statement).execute(pool).await?;
    }

    sqlx::query(
        r#"
        CREATE UNIQUE INDEX IF NOT EXISTS users_username_unique_idx
        ON users(username)
        WHERE username IS NOT NULL
        "#,
    )
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        CREATE INDEX IF NOT EXISTS evidence_files_entity_idx
        ON evidence_files(entity_type, entity_id, created_at DESC)
        "#,
    )
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        INSERT INTO workspace_templates (
            id, title, description, org_type, sector, site_type, form_type,
            trust_signal, default_project_status, language_mode, offline_enabled,
            channel_strategy, target_segment, default_actions, required_evidence,
            creates_asset, creates_report_task, creates_alert, creates_ticket, sort_order
        ) VALUES
            (
                'council-water',
                'Council water reliability pilot',
                'Municipal water validation with GPS/photo proof, local focal point, first report task, and follow-up ticket.',
                'municipal_council',
                'water',
                'water_cluster',
                'gps_photo_survey',
                'council_agent_verified',
                'planning',
                'bilingual',
                TRUE,
                'field_team_whatsapp_sms',
                'municipal_water_users',
                ARRAY['site', 'campaign', 'probe', 'report', 'alert', 'ticket', 'decision']::TEXT[],
                ARRAY['gps_photo', 'water_point_condition', 'local_focal_point', 'beneficiary_count']::TEXT[],
                TRUE,
                TRUE,
                TRUE,
                TRUE,
                10
            ),
            (
                'ngo-inclusion',
                'NGO digital inclusion baseline',
                'Offline phone ownership and inclusion baseline with bilingual survey, site proof, and decision-ready evidence.',
                'ngo',
                'connectivity',
                'public_asset',
                'phone_ownership_baseline',
                'gps_photo_verified',
                'planning',
                'bilingual',
                TRUE,
                'offline_forms_sms_whatsapp',
                'youth_women_community_groups',
                ARRAY['site', 'campaign', 'report', 'decision']::TEXT[],
                ARRAY['phone_ownership_sample', 'gps_photo', 'gender_inclusion_notes', 'local_focal_point']::TEXT[],
                FALSE,
                TRUE,
                FALSE,
                FALSE,
                20
            ),
            (
                'clinic-solar',
                'Clinic solar uptime monitoring',
                'Clinic energy monitoring workspace with health probe, exception alert, technician follow-up, and uptime evidence.',
                'solar_operator',
                'solar',
                'clinic',
                'asset_condition',
                'clinic_staff_verified',
                'planning',
                'bilingual',
                TRUE,
                'field_team_whatsapp_sms',
                'clinic_staff_patients',
                ARRAY['site', 'campaign', 'probe', 'report', 'alert', 'ticket', 'decision']::TEXT[],
                ARRAY['gps_photo', 'battery_or_inverter_status', 'clinic_staff_confirmation', 'uptime_reading']::TEXT[],
                TRUE,
                TRUE,
                TRUE,
                TRUE,
                30
            ),
            (
                'telecom-probe',
                'Telecom signal probe rollout',
                'Telecom probe rollout with signal asset, telemetry-ready workflow, field report task, and approval decision.',
                'telecom',
                'connectivity',
                'telecom_probe_site',
                'signal_check',
                'gps_photo_verified',
                'planning',
                'bilingual',
                TRUE,
                'operator_api_field_team',
                'operator_network_planning',
                ARRAY['site', 'campaign', 'probe', 'report', 'alert', 'ticket', 'decision']::TEXT[],
                ARRAY['gps_photo', 'signal_reading', 'operator_reference', 'local_access_notes']::TEXT[],
                TRUE,
                TRUE,
                TRUE,
                TRUE,
                40
            )
        ON CONFLICT (id)
        DO UPDATE SET
            title = EXCLUDED.title,
            description = EXCLUDED.description,
            org_type = EXCLUDED.org_type,
            sector = EXCLUDED.sector,
            site_type = EXCLUDED.site_type,
            form_type = EXCLUDED.form_type,
            trust_signal = EXCLUDED.trust_signal,
            default_project_status = EXCLUDED.default_project_status,
            language_mode = EXCLUDED.language_mode,
            offline_enabled = EXCLUDED.offline_enabled,
            channel_strategy = EXCLUDED.channel_strategy,
            target_segment = EXCLUDED.target_segment,
            default_actions = EXCLUDED.default_actions,
            required_evidence = EXCLUDED.required_evidence,
            creates_asset = EXCLUDED.creates_asset,
            creates_report_task = EXCLUDED.creates_report_task,
            creates_alert = EXCLUDED.creates_alert,
            creates_ticket = EXCLUDED.creates_ticket,
            active = TRUE,
            sort_order = EXCLUDED.sort_order,
            updated_at = NOW()
        "#,
    )
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        DO $$
        BEGIN
            IF NOT EXISTS (
                SELECT 1 FROM pg_constraint WHERE conname = 'mobile_phone_stats_pcode_unique'
            ) THEN
                ALTER TABLE mobile_phone_stats
                ADD CONSTRAINT mobile_phone_stats_pcode_unique
                UNIQUE (pcode);
            END IF;

            IF NOT EXISTS (
                SELECT 1 FROM pg_constraint WHERE conname = 'mobile_phone_stats_gps_bounds'
            ) THEN
                ALTER TABLE mobile_phone_stats
                ADD CONSTRAINT mobile_phone_stats_gps_bounds
                CHECK (
                    latitude BETWEEN 1.5 AND 13.5
                    AND longitude BETWEEN 8.0 AND 16.5
                );
            END IF;

            IF NOT EXISTS (
                SELECT 1 FROM pg_constraint WHERE conname = 'mobile_phone_stats_non_negative_counts'
            ) THEN
                ALTER TABLE mobile_phone_stats
                ADD CONSTRAINT mobile_phone_stats_non_negative_counts
                CHECK (
                    (phone_owners IS NULL OR phone_owners >= 0)
                    AND (population IS NULL OR population >= 0)
                    AND (
                        phone_owners IS NULL
                        OR population IS NULL
                        OR phone_owners <= population
                    )
                );
            END IF;

            IF NOT EXISTS (
                SELECT 1 FROM pg_constraint WHERE conname = 'mobile_phone_stats_area_bounds'
            ) THEN
                ALTER TABLE mobile_phone_stats
                ADD CONSTRAINT mobile_phone_stats_area_bounds
                CHECK (area_sqkm IS NULL OR area_sqkm >= 0);
            END IF;
        END $$;
        "#,
    )
    .execute(pool)
    .await?;

    Ok(())
}
