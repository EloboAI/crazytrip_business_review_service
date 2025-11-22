-- Enable required extensions
CREATE EXTENSION IF NOT EXISTS "pgcrypto";

-- Verification and review enums
DO $$
BEGIN
    IF NOT EXISTS (SELECT 1 FROM pg_type WHERE typname = 'business_verification_status') THEN
        CREATE TYPE business_verification_status AS ENUM (
            'pending',
            'under_review',
            'approved',
            'rejected',
            'suspended'
        );
    END IF;
END
$$;

DO $$
BEGIN
    IF NOT EXISTS (SELECT 1 FROM pg_type WHERE typname = 'business_review_action') THEN
        CREATE TYPE business_review_action AS ENUM (
            'approve',
            'reject',
            'request_more_info',
            'suspend',
            'resume',
            'comment'
        );
    END IF;
END
$$;

-- Business registration requests submitted by owners
CREATE TABLE IF NOT EXISTS business_registration_requests (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL,
    business_id UUID,
    name TEXT NOT NULL,
    category TEXT NOT NULL,
    address TEXT NOT NULL,
    description TEXT,
    phone TEXT,
    website TEXT,
    tax_id TEXT,
    document_urls TEXT[] NOT NULL DEFAULT ARRAY[]::TEXT[],
    is_multi_user_team BOOLEAN NOT NULL DEFAULT FALSE,
    status business_verification_status NOT NULL DEFAULT 'pending',
    owner_email TEXT NOT NULL,
    owner_username TEXT NOT NULL,
    rejection_reason TEXT,
    reviewer_notes TEXT,
    reviewer_id UUID,
    reviewer_name TEXT,
    submitted_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_registration_user_id
    ON business_registration_requests(user_id);
CREATE INDEX IF NOT EXISTS idx_registration_status
    ON business_registration_requests(status);
CREATE INDEX IF NOT EXISTS idx_registration_submitted_at
    ON business_registration_requests(submitted_at DESC);

-- Audit trail of reviewer actions for compliance
CREATE TABLE IF NOT EXISTS business_review_events (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    registration_id UUID NOT NULL REFERENCES business_registration_requests(id) ON DELETE CASCADE,
    reviewer_id UUID,
    reviewer_name TEXT,
    action business_review_action NOT NULL,
    notes TEXT,
    rejection_reason TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_review_events_registration_id
    ON business_review_events(registration_id);
CREATE INDEX IF NOT EXISTS idx_review_events_created_at
    ON business_review_events(created_at DESC);
