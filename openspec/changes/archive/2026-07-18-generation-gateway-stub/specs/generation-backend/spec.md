## ADDED Requirements

### Requirement: Self-hosted generation gateway with pluggable providers

A gateway service SHALL implement Fronda Generation Protocol v1.1 (submit, poll, and a providers catalog) and route each request to a provider selected by kind and optional provider name, defaulting per kind when unspecified. Providers SHALL be registered behind a common trait so multiple providers per kind coexist, each with its own bring-your-own-key configuration. Phase 1 SHALL ship stub providers so the full submit→poll→succeeded loop runs with no external keys.

#### Scenario: Stub loop completes end to end

- **WHEN** a client submits a generate request to the gateway and polls the returned job id
- **THEN** the poll transitions running → succeeded and returns a result URL, with no external provider key required

#### Scenario: Provider selection and defaulting

- **WHEN** a request omits the provider field
- **THEN** the gateway routes to the kind's default provider; an unknown named provider yields an explicit error

#### Scenario: Auth is enforced

- **WHEN** a request omits or mismatches the bearer token
- **THEN** the gateway responds 401 and does not run the job
