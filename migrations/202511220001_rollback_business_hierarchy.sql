-- Rollback migration: Remove business_companies and business_units tables
-- This reverts changes from migration 202511210003_add_business_hierarchy.sql

-- Remove unit_id column from business_promotions
ALTER TABLE business_promotions DROP COLUMN IF EXISTS unit_id;

-- Drop business_units table
DROP TABLE IF EXISTS business_units;

-- Drop business_companies table
DROP TABLE IF EXISTS business_companies;

-- Remove indexes if they still exist (in case table drops didn't remove them)
DROP INDEX IF EXISTS idx_business_companies_owner;
DROP INDEX IF EXISTS idx_business_units_company;
DROP INDEX IF EXISTS idx_business_units_primary;
DROP INDEX IF EXISTS idx_business_promotions_unit;

-- Note: This migration preserves the original business_registration_requests table
-- and business_promotions table (minus the unit_id column) as they are part of
-- the core requirements.
