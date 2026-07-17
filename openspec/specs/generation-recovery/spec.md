# generation-recovery Specification

## Purpose

TBD - created by archiving change 'generation-recovery-resume'. Update Purpose after archive.

## Requirements

### Requirement: Recovery planning is pure

generation_core SHALL provide a pure function that, given a media manifest, lists every asset whose generation is in flight (an active generation_status plus a backend_job_id) with the recovery action to take, without performing any IO.

#### Scenario: In-flight job listed

- **WHEN** the manifest holds an asset with generation_status "generating" and a backend_job_id
- **THEN** the plan lists that asset with a resubscribe action

#### Scenario: Completed and never-started assets skipped

- **WHEN** assets have generation_status "none" or lack a backend_job_id
- **THEN** the plan excludes them


<!-- @trace
source: generation-recovery-resume
updated: 2026-07-17
code:
  - crates/app_shell_gpui/src/editor_state_hub.rs
-->

---
### Requirement: Backend outcome application

Applying a backend outcome SHALL transition the asset's generation_status and persist result URLs on success or record failure on error, keeping the manifest consistent with the get_media "poll until none" contract.

#### Scenario: Success writes results

- **WHEN** the backend reports success with result URLs for a recovered job
- **THEN** the asset's result_urls are stored and generation_status transitions to none

#### Scenario: Failure recorded

- **WHEN** the backend reports failure
- **THEN** the asset's generation_status reflects the failure and no result URLs are written

<!-- @trace
source: generation-recovery-resume
updated: 2026-07-17
code:
  - crates/app_shell_gpui/src/editor_state_hub.rs
-->