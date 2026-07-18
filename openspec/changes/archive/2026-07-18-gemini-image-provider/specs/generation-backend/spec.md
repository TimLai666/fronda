## ADDED Requirements

### Requirement: Gateway serves generated media and hosts a real image provider

The gateway SHALL store generated media bytes and serve them at `GET /v1/results/{id}` (bearer, correct content-type), so a succeeded job's resultUrls are fetchable real media (not placeholder schemes). A Gemini image provider SHALL implement the provider trait using the Gemini generateContent REST surface with a bring-your-own key, decoding the inlineData image bytes into the result store; it is registered only when a key is configured.

#### Scenario: Real bytes round-trip through the pipeline (key-free)

- **WHEN** a mock provider returns image bytes and a client submits then polls to success
- **THEN** the resultUrl serves those exact bytes with the correct content-type

#### Scenario: Gemini registers only with a key

- **WHEN** no Gemini API key is configured
- **THEN** the image catalog lists only the stub provider and the gateway still starts
