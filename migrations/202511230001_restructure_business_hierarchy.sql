-- ============================================================================
-- MIGRATION: Restructure business hierarchy
-- Date: 2025-11-23
-- Description: Clean redesign of business structure
-- 
-- New structure:
-- 1. business_registration_requests (verification workflow - KEPT)
-- 2. businesses (approved businesses)
-- 3. business_locations (physical locations/branches)
-- 4. business_promotions (promotions per location)
-- 5. business_location_admins (location administrators)
-- ============================================================================

-- Drop old tables and relationships that are being replaced
DROP TABLE IF EXISTS business_promotion_locations CASCADE;
DROP TABLE IF EXISTS business_units CASCADE;
DROP TABLE IF EXISTS business_companies CASCADE;

-- Remove old columns from promotions if they exist
ALTER TABLE business_promotions 
    DROP COLUMN IF EXISTS unit_id CASCADE,
    DROP COLUMN IF EXISTS registration_id CASCADE,
    DROP COLUMN IF EXISTS scope CASCADE;

-- Drop old indexes on business_locations if they reference removed columns
DROP INDEX IF EXISTS idx_business_locations_registration_id;
DROP INDEX IF EXISTS idx_business_locations_business_id;

-- ============================================================================
-- TABLE: businesses (approved businesses)
-- ============================================================================
CREATE TABLE IF NOT EXISTS businesses (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    registration_id UUID UNIQUE REFERENCES business_registration_requests(id) ON DELETE SET NULL,
    owner_user_id UUID NOT NULL,
    business_name TEXT NOT NULL,
    category TEXT NOT NULL,
    tax_id TEXT,
    description TEXT,
    website TEXT,
    logo_url TEXT,
    is_active BOOLEAN NOT NULL DEFAULT TRUE,
    metadata JSONB NOT NULL DEFAULT '{}'::JSONB,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    
    CONSTRAINT business_name_not_empty CHECK (length(trim(business_name)) > 0),
    CONSTRAINT category_not_empty CHECK (length(trim(category)) > 0)
);

CREATE INDEX idx_businesses_owner ON businesses(owner_user_id);
CREATE INDEX idx_businesses_registration ON businesses(registration_id) WHERE registration_id IS NOT NULL;
CREATE INDEX idx_businesses_active ON businesses(is_active) WHERE is_active = TRUE;

COMMENT ON TABLE businesses IS 'Approved businesses (e.g., McDonald''s Corporation)';
COMMENT ON COLUMN businesses.registration_id IS 'Links to the original registration request';

-- ============================================================================
-- TABLE: business_locations (branches/physical locations)
-- ============================================================================
-- Recreate business_locations with new structure
DROP TABLE IF EXISTS business_locations CASCADE;

CREATE TABLE business_locations (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    business_id UUID NOT NULL REFERENCES businesses(id) ON DELETE CASCADE,
    location_name TEXT NOT NULL,
    formatted_address TEXT NOT NULL,
    street TEXT,
    city TEXT,
    state_region TEXT,
    postal_code TEXT,
    country TEXT,
    latitude DOUBLE PRECISION,
    longitude DOUBLE PRECISION,
    google_place_id TEXT UNIQUE,
    timezone TEXT,
    phone TEXT,
    email TEXT,
    is_active BOOLEAN NOT NULL DEFAULT TRUE,
    is_primary BOOLEAN NOT NULL DEFAULT FALSE,
    operating_hours JSONB,
    notes TEXT,
    metadata JSONB NOT NULL DEFAULT '{}'::JSONB,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    
    CONSTRAINT location_name_not_empty CHECK (length(trim(location_name)) > 0),
    CONSTRAINT formatted_address_not_empty CHECK (length(trim(formatted_address)) > 0)
);

CREATE INDEX idx_business_locations_business ON business_locations(business_id);
CREATE INDEX idx_business_locations_active ON business_locations(is_active) WHERE is_active = TRUE;
CREATE INDEX idx_business_locations_coords ON business_locations(latitude, longitude) 
    WHERE latitude IS NOT NULL AND longitude IS NOT NULL;
CREATE INDEX idx_business_locations_place_id ON business_locations(google_place_id) 
    WHERE google_place_id IS NOT NULL;

-- Ensure only one primary location per business
CREATE UNIQUE INDEX idx_business_locations_primary_unique 
    ON business_locations(business_id) 
    WHERE is_primary = TRUE;

COMMENT ON TABLE business_locations IS 'Physical locations/branches (e.g., McDonald''s Providencia)';
COMMENT ON COLUMN business_locations.location_name IS 'Branch name (e.g., "Providencia", "Las Condes")';

-- ============================================================================
-- TABLE: business_promotions (refactored to link to locations)
-- ============================================================================
-- Add location_id to promotions
ALTER TABLE business_promotions 
    ADD COLUMN IF NOT EXISTS location_id UUID NOT NULL REFERENCES business_locations(id) ON DELETE CASCADE;

CREATE INDEX idx_business_promotions_location ON business_promotions(location_id);

COMMENT ON TABLE business_promotions IS 'Promotions specific to a location';
COMMENT ON COLUMN business_promotions.location_id IS 'Links promotion to specific branch/location';

-- ============================================================================
-- TABLE: business_location_admins (location administrators)
-- ============================================================================
DO $$
BEGIN
    IF NOT EXISTS (SELECT 1 FROM pg_type WHERE typname = 'location_admin_role') THEN
        CREATE TYPE location_admin_role AS ENUM (
            'owner',      -- Business owner (full control)
            'manager',    -- Location manager (can manage location and promotions)
            'staff'       -- Staff member (view only)
        );
    END IF;
END
$$;

CREATE TABLE IF NOT EXISTS business_location_admins (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    location_id UUID NOT NULL REFERENCES business_locations(id) ON DELETE CASCADE,
    user_id UUID NOT NULL,
    user_email TEXT NOT NULL,
    user_username TEXT NOT NULL,
    role location_admin_role NOT NULL DEFAULT 'staff',
    granted_by UUID,
    granted_by_username TEXT,
    is_active BOOLEAN NOT NULL DEFAULT TRUE,
    granted_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    
    CONSTRAINT unique_location_user UNIQUE (location_id, user_id)
);

CREATE INDEX idx_location_admins_location ON business_location_admins(location_id);
CREATE INDEX idx_location_admins_user ON business_location_admins(user_id);
CREATE INDEX idx_location_admins_active ON business_location_admins(is_active) WHERE is_active = TRUE;

COMMENT ON TABLE business_location_admins IS 'Administrators/managers for specific locations';
COMMENT ON COLUMN business_location_admins.role IS 'Admin level: owner (full), manager (location), staff (view)';

-- ============================================================================
-- MIGRATION: Move existing data if any
-- ============================================================================

-- Note: Since we're doing a clean restructure and old tables are dropped,
-- existing data in business_registration_requests remains intact for the 
-- verification workflow. Any approved registrations would need to be 
-- migrated manually or through the application logic to create businesses
-- and locations from approved registrations.

-- ============================================================================
-- HELPER FUNCTIONS
-- ============================================================================

-- Function to auto-promote a location to primary if none exists
CREATE OR REPLACE FUNCTION ensure_primary_location()
RETURNS TRIGGER AS $$
DECLARE
    next_location_id UUID;
BEGIN
    -- If deleting the primary location
    IF OLD.is_primary = TRUE THEN
        -- Find the next location to promote
        SELECT id INTO next_location_id
        FROM business_locations
        WHERE business_id = OLD.business_id
            AND id != OLD.id
            AND is_active = TRUE
        ORDER BY created_at ASC
        LIMIT 1;
        
        -- Set it as primary if found
        IF next_location_id IS NOT NULL THEN
            UPDATE business_locations
            SET is_primary = TRUE
            WHERE id = next_location_id;
        END IF;
    END IF;
    RETURN OLD;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER trg_ensure_primary_location
    BEFORE DELETE ON business_locations
    FOR EACH ROW
    EXECUTE FUNCTION ensure_primary_location();

COMMENT ON FUNCTION ensure_primary_location IS 'Ensures at least one location is primary when deleting';
