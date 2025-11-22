-- Companies table: represents the owner/parent entity
CREATE TABLE IF NOT EXISTS business_companies (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    owner_user_id UUID NOT NULL,
    company_name TEXT NOT NULL,
    tax_id TEXT,
    legal_entity_type TEXT,
    is_active BOOLEAN NOT NULL DEFAULT TRUE,
    metadata JSONB NOT NULL DEFAULT '{}'::JSONB,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_business_companies_owner
    ON business_companies(owner_user_id);

-- Business units table: individual businesses under a company
CREATE TABLE IF NOT EXISTS business_units (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    company_id UUID NOT NULL REFERENCES business_companies(id) ON DELETE CASCADE,
    registration_id UUID REFERENCES business_registration_requests(id) ON DELETE SET NULL,
    business_id UUID UNIQUE,
    unit_name TEXT NOT NULL,
    category TEXT NOT NULL,
    is_primary BOOLEAN NOT NULL DEFAULT FALSE,
    is_active BOOLEAN NOT NULL DEFAULT TRUE,
    metadata JSONB NOT NULL DEFAULT '{}'::JSONB,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_business_units_company
    ON business_units(company_id);

CREATE INDEX IF NOT EXISTS idx_business_units_primary
    ON business_units(company_id, is_primary) WHERE is_primary = TRUE;

-- Migrate existing registrations to create implicit companies and units
DO $$
DECLARE
    reg RECORD;
    company_uuid UUID;
    unit_uuid UUID;
BEGIN
    FOR reg IN 
        SELECT DISTINCT ON (user_id) 
            id, user_id, business_id, name, category
        FROM business_registration_requests
        WHERE status = 'approved'
        ORDER BY user_id, submitted_at ASC
    LOOP
        -- Create a company for this user if they have approved registrations
        INSERT INTO business_companies (owner_user_id, company_name, is_active)
        VALUES (reg.user_id, reg.name, TRUE)
        RETURNING id INTO company_uuid;

        -- Create a unit for each registration
        INSERT INTO business_units (
            company_id, 
            registration_id, 
            business_id, 
            unit_name, 
            category, 
            is_primary, 
            is_active
        )
        VALUES (
            company_uuid,
            reg.id,
            reg.business_id,
            reg.name,
            reg.category,
            TRUE,
            TRUE
        );
    END LOOP;
END $$;

-- Update promotions to reference business units instead of just registrations
ALTER TABLE business_promotions 
    ADD COLUMN IF NOT EXISTS unit_id UUID REFERENCES business_units(id) ON DELETE CASCADE;

CREATE INDEX IF NOT EXISTS idx_business_promotions_unit
    ON business_promotions(unit_id);

-- Backfill unit_id for existing promotions
UPDATE business_promotions p
SET unit_id = bu.id
FROM business_units bu
WHERE p.registration_id = bu.registration_id
  AND p.unit_id IS NULL;
