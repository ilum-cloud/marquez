/* SPDX-License-Identifier: Apache-2.0 */

/* Repair runs whose denormalized job_name kept the raw OpenLineage event name
   instead of the job's canonical name. When the first event of a run already
   carried a ParentRunFacet, the job was stored under its parent-qualified name
   ('parent.child') while the run kept 'child'. Queries that resolve the job by
   its canonical name (runs listing) and queries that match runs.job_name
   directly (run counts) then disagreed: the run was counted but never listed.
   Align runs.job_name with the same COALESCE(symlink.name, job.name) that
   runs_view exposes. */
UPDATE runs r
SET job_name = COALESCE(s.name, j.name)
FROM jobs j
LEFT JOIN jobs s ON s.uuid = j.symlink_target_uuid
WHERE r.job_uuid = j.uuid
  AND r.job_name IS DISTINCT FROM COALESCE(s.name, j.name);
