-- Add optimized indexes for simple search functionality
-- These indexes improve performance for name-based search queries

-- Index for datasets name search (case-insensitive)
-- Using gin with pg_trgm for better ILIKE performance
CREATE EXTENSION IF NOT EXISTS pg_trgm;

-- GIN index for trigram-based pattern matching on dataset names
CREATE INDEX IF NOT EXISTS datasets_name_trgm_idx 
    ON datasets USING gin (name gin_trgm_ops);

-- GIN index for trigram-based pattern matching on job names  
CREATE INDEX IF NOT EXISTS jobs_name_trgm_idx 
    ON jobs USING gin (name gin_trgm_ops);

-- Composite indexes for optimized sorting and filtering
-- Index for datasets: namespace + updated_at for efficient filtering and sorting
CREATE INDEX IF NOT EXISTS datasets_namespace_updated_at_idx 
    ON datasets (namespace_name, updated_at DESC) 
    WHERE is_deleted = false AND is_hidden = false;

-- Index for jobs: namespace + updated_at for efficient filtering and sorting
CREATE INDEX IF NOT EXISTS jobs_namespace_updated_at_idx 
    ON jobs (namespace_name, updated_at DESC) 
    WHERE is_hidden = false AND symlink_target_uuid IS NULL;

-- Additional indexes for common search patterns
-- Index for dataset name + namespace for exact lookups
CREATE INDEX IF NOT EXISTS datasets_name_namespace_idx 
    ON datasets (name, namespace_name) 
    WHERE is_deleted = false AND is_hidden = false;

-- Index for job name + namespace for exact lookups
CREATE INDEX IF NOT EXISTS jobs_name_namespace_idx 
    ON jobs (name, namespace_name) 
    WHERE is_hidden = false AND symlink_target_uuid IS NULL;