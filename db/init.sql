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
