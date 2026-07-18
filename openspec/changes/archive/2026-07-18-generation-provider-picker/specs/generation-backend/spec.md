## ADDED Requirements

### Requirement: Fronda selects a generation provider and model

GenerationRequest SHALL carry an optional provider that the submit body includes only when set (absent → gateway default); the generate tools SHALL accept and forward it. The client SHALL fetch the `GET /v1/providers` catalog, and the generation panel SHALL present provider and model pickers per kind so the user's selection rides on the submitted request. Omitting a provider SHALL preserve existing behavior.

#### Scenario: Selected provider rides on the request

- **WHEN** the user picks a provider in the generation panel and submits
- **THEN** the submit body carries that provider name; picking none omits the field and the gateway uses its default

#### Scenario: Picker reflects the catalog

- **WHEN** the panel opens against a configured endpoint
- **THEN** it lists the providers and models the gateway's /v1/providers returns for the active kind
