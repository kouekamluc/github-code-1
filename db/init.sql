CREATE TABLE IF NOT EXISTS mobile_phone_stats (
    id SERIAL PRIMARY KEY,
    region TEXT NOT NULL,
    department TEXT NOT NULL,
    commune TEXT NOT NULL,
    location TEXT NOT NULL,
    latitude DOUBLE PRECISION NOT NULL,
    longitude DOUBLE PRECISION NOT NULL,
    phone_owners INTEGER NOT NULL,
    population INTEGER NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(region, department, commune)
);
