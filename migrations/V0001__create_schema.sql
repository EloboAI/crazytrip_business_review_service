-- Initial schema for crazytrip_business_review_service

-- Enums
CREATE TYPE business_verification_status AS ENUM ('pending','under_review','approved','rejected','suspended');

CREATE TYPE business_review_action AS ENUM ('approve','reject','request_more_info','suspend','resume','comment');

CREATE TYPE business_promotion_type AS ENUM ('discount','contest','event','challenge');

CREATE TYPE business_promotion_status AS ENUM ('draft','scheduled','active','expired','cancelled');

CREATE TYPE location_admin_role AS ENUM ('owner','manager','staff');

-- Core tables
CREATE TABLE IF NOT EXISTS businesses (
    id uuid PRIMARY KEY,
    registration_id uuid,
    owner_user_id uuid NOT NULL,
    business_name text NOT NULL,
    category text NOT NULL,
    tax_id text,
    description text,
    website text,
    logo_url text,
    is_active boolean NOT NULL DEFAULT true,
    metadata jsonb NOT NULL DEFAULT '{}'::jsonb,
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now()
);

CREATE TABLE IF NOT EXISTS business_locations (
    id uuid PRIMARY KEY,
    business_id uuid NOT NULL REFERENCES businesses(id) ON DELETE CASCADE,
    location_name text NOT NULL,
    formatted_address text NOT NULL,
    street text,
    city text,
    state_region text,
    postal_code text,
    country text,
    latitude double precision,
    longitude double precision,
    google_place_id text,
    timezone text,
    phone text,
    email text,
    is_active boolean NOT NULL DEFAULT true,
    is_primary boolean NOT NULL DEFAULT false,
    operating_hours jsonb,
    notes text,
    metadata jsonb NOT NULL DEFAULT '{}'::jsonb,
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now()
);

CREATE TABLE IF NOT EXISTS business_promotions (
    id uuid PRIMARY KEY,
    location_id uuid NOT NULL REFERENCES business_locations(id) ON DELETE CASCADE,
    title text NOT NULL,
    subtitle text,
    description text,
    promotion_type business_promotion_type NOT NULL,
    status business_promotion_status NOT NULL,
    image_url text,
    prize text,
    reward_points integer NOT NULL DEFAULT 0,
    discount_percent integer,
    max_claims integer,
    per_user_limit integer,
    total_claims integer NOT NULL DEFAULT 0,
    requires_check_in boolean NOT NULL DEFAULT false,
    requires_purchase boolean NOT NULL DEFAULT false,
    terms text,
    metadata jsonb NOT NULL DEFAULT '{}'::jsonb,
    starts_at timestamptz NOT NULL,
    ends_at timestamptz NOT NULL,
    published_at timestamptz,
    created_by uuid,
    updated_by uuid,
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now()
);

CREATE TABLE IF NOT EXISTS business_registration_requests (
    id uuid PRIMARY KEY,
    user_id uuid NOT NULL,
    business_id uuid,
    name text NOT NULL,
    category text NOT NULL,
    address text NOT NULL,
    description text,
    phone text,
    website text,
    tax_id text,
    document_urls jsonb NOT NULL DEFAULT '[]'::jsonb,
    is_multi_user_team boolean NOT NULL DEFAULT false,
    status business_verification_status NOT NULL,
    owner_email text NOT NULL,
    owner_username text NOT NULL,
    rejection_reason text,
    reviewer_notes text,
    reviewer_id uuid,
    reviewer_name text,
    submitted_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now()
);

CREATE TABLE IF NOT EXISTS business_review_events (
    id uuid PRIMARY KEY,
    registration_id uuid NOT NULL REFERENCES business_registration_requests(id) ON DELETE CASCADE,
    reviewer_id uuid,
    reviewer_name text,
    action business_review_action NOT NULL,
    notes text,
    rejection_reason text,
    created_at timestamptz NOT NULL DEFAULT now()
);

CREATE TABLE IF NOT EXISTS business_location_admins (
    id uuid PRIMARY KEY,
    location_id uuid NOT NULL REFERENCES business_locations(id) ON DELETE CASCADE,
    user_id uuid NOT NULL,
    user_email text NOT NULL,
    user_username text NOT NULL,
    role location_admin_role NOT NULL,
    granted_by uuid,
    granted_by_username text,
    is_active boolean NOT NULL DEFAULT true,
    granted_at timestamptz NOT NULL DEFAULT now(),
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now()
);

-- Optionally create schema_migrations (migrate binary creates it too)
CREATE TABLE IF NOT EXISTS schema_migrations (
    version VARCHAR(50) PRIMARY KEY,
    description TEXT,
    installed_on TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
