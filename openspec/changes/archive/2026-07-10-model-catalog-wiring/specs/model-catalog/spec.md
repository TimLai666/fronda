## ADDED Requirements

### Requirement: Real catalog behind list_models

The list_models tool SHALL return the real model catalog defined in generation_core (mirroring the upstream Swift ModelConfig lists field-for-field), filtered by the requested kind, with the hardcoded placeholder list removed.

#### Scenario: Video models listed

- **WHEN** the agent calls list_models for video
- **THEN** the response contains exactly the catalog's video entries with their real ids and display names

### Requirement: Free-tier gating

Model availability SHALL follow upstream #249: a model is available when the account is paid or the model is not paid_only. The paid flag SHALL come from an injected account-state seam; with no seam installed the executor SHALL treat the account as free tier.

#### Scenario: Free tier sees paid model as gated

- **WHEN** no account seam is installed and a paid_only model is listed
- **THEN** the entry is marked unavailable/upgrade-required rather than hidden, and generate with that model returns an explicit gating error

#### Scenario: Paid account passes

- **WHEN** the account seam reports paid
- **THEN** paid_only models list as available and generate accepts them
