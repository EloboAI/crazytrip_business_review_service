-- Add business_locations table to support multiple locations per registration
CREATE TABLE IF NOT EXISTS business_locations (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    registration_id UUID NOT NULL REFERENCES business_registration_requests(id) ON DELETE CASCADE,
    business_id UUID,
    label TEXT NOT NULL,
    formatted_address TEXT NOT NULL,
    street TEXT,
    city TEXT,
    state_region TEXT,
    postal_code TEXT,
    country TEXT,
    latitude DOUBLE PRECISION,
    longitude DOUBLE PRECISION,
    google_place_id TEXT,
    timezone TEXT,
    phone TEXT,
    is_primary BOOLEAN NOT NULL DEFAULT FALSE,
    notes TEXT,
    metadata JSONB NOT NULL DEFAULT '{}'::JSONB,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_business_locations_registration_id
    ON business_locations(registration_id);
CREATE INDEX IF NOT EXISTS idx_business_locations_business_id
    ON business_locations(business_id) WHERE business_id IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_business_locations_place_id
    ON business_locations(google_place_id) WHERE google_place_id IS NOT NULL;

-- Ensure only one primary location per registration
CREATE UNIQUE INDEX IF NOT EXISTS idx_business_locations_primary_unique
    ON business_locations(registration_id)
    WHERE is_primary;
