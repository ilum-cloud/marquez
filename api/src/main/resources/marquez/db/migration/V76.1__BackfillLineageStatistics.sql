WITH lineage_stats AS (
    SELECT
        d.uuid AS dataset_uuid,
        d.current_version_uuid,
        COUNT(DISTINCT CASE WHEN mapping.io_type = 'OUTPUT' THEN mapping.job_uuid END) AS in_edges,
        COUNT(DISTINCT CASE WHEN mapping.io_type = 'INPUT' THEN mapping.job_uuid END) AS out_edges,
        array_agg(DISTINCT CASE WHEN mapping.io_type = 'INPUT' THEN j.namespace_name END) FILTER (WHERE j.namespace_name IS NOT NULL) AS consumingNamespaces,
        array_agg(DISTINCT CASE WHEN mapping.io_type = 'OUTPUT' THEN j.namespace_name END) FILTER (WHERE j.namespace_name IS NOT NULL) AS producingNamespaces
    FROM datasets d
    LEFT JOIN job_versions_io_mapping mapping ON mapping.dataset_uuid = d.uuid AND mapping.is_current_job_version = TRUE
    LEFT JOIN jobs j ON j.uuid = mapping.job_uuid
    WHERE d.current_version_uuid IS NOT NULL
    GROUP BY d.uuid, d.current_version_uuid
)
INSERT INTO dataset_facets (
    created_at, dataset_uuid, dataset_version_uuid, run_uuid,
    lineage_event_time, lineage_event_type, type, name, facet
)
SELECT
    NOW(),
    dataset_uuid,
    current_version_uuid,
    NULL,
    NOW(),
    'BACKFILL_LINEAGE_STATS',
    'DATASET',
    'lineageStatistics',
    json_build_object(
      'lineageStatistics', json_build_object(
        'inEdges', in_edges,
        'outEdges', out_edges,
        'consumingNamespaces', COALESCE(consumingNamespaces, ARRAY[]::varchar[]),
        'producingNamespaces', COALESCE(producingNamespaces, ARRAY[]::varchar[]),
        '_producer', 'https://github.com/ilum-cloud/marquez',
        '_schema', 'https://github.com/ilum-cloud/marquez/spec/facets/lineage-statistics.json'
      )
    )
FROM lineage_stats;