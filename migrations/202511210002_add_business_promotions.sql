-- Promotion related enums
DO $$
BEGIN
    IF NOT EXISTS (SELECT 1 FROM pg_type WHERE typname = 'business_promotion_type') THEN
        CREATE TYPE business_promotion_type AS ENUM (
            'discount',
            'contest',
            'event',
            'challenge'
        );
    END IF;
END
$$;

DO $$
BEGIN
    IF NOT EXISTS (SELECT 1 FROM pg_type WHERE typname = 'business_promotion_scope') THEN
        CREATE TYPE business_promotion_scope AS ENUM (
            'business',
            'location'
        );
    END IF;
END
$$;

DO $$
BEGIN
    IF NOT EXISTS (SELECT 1 FROM pg_type WHERE typname = 'business_promotion_status') THEN
        CREATE TYPE business_promotion_status AS ENUM (
            'draft',
            'scheduled',
            'active',
            'expired',
            'cancelled'
        );
    END IF;
END
$$;

-- Business promotions table
CREATE TABLE IF NOT EXISTS business_promotions (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    registration_id UUID NOT NULL REFERENCES business_registration_requests(id) ON DELETE CASCADE,
    title TEXT NOT NULL,
    subtitle TEXT,
    description TEXT,
    promotion_type business_promotion_type NOT NULL,
    scope business_promotion_scope NOT NULL DEFAULT 'business',
    status business_promotion_status NOT NULL DEFAULT 'draft',
    image_url TEXT,
    prize TEXT,
    reward_points INTEGER NOT NULL DEFAULT 0,
    discount_percent INTEGER,
    max_claims INTEGER,
    per_user_limit INTEGER,
    total_claims INTEGER NOT NULL DEFAULT 0,
    requires_check_in BOOLEAN NOT NULL DEFAULT FALSE,
    requires_purchase BOOLEAN NOT NULL DEFAULT FALSE,
    terms TEXT,
    metadata JSONB NOT NULL DEFAULT '{}'::JSONB,
    starts_at TIMESTAMPTZ NOT NULL,
    ends_at TIMESTAMPTZ NOT NULL,
    published_at TIMESTAMPTZ,
    created_by UUID,
    updated_by UUID,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_business_promotions_registration_id
    ON business_promotions(registration_id);

CREATE INDEX IF NOT EXISTS idx_business_promotions_status
    ON business_promotions(status);

CREATE INDEX IF NOT EXISTS idx_business_promotions_schedule
    ON business_promotions(starts_at, ends_at);

-- Link table for location scoped promotions
CREATE TABLE IF NOT EXISTS business_promotion_locations (
    promotion_id UUID NOT NULL REFERENCES business_promotions(id) ON DELETE CASCADE,
    location_id UUID NOT NULL REFERENCES business_locations(id) ON DELETE CASCADE,
    PRIMARY KEY (promotion_id, location_id)
);

CREATE INDEX IF NOT EXISTS idx_promotion_locations_location
    ON business_promotion_locations(location_id);
