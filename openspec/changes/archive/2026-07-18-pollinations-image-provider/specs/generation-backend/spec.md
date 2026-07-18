## ADDED Requirements

### Requirement: No-key image provider (Pollinations)

The gateway SHALL register a keyless Pollinations image provider that fetches a generated image from `{base}/prompt/{url-encoded prompt}` (base configurable) and stores the returned bytes in the result store, so real AI media flows end to end with no credential. It coexists with the stub and the (key-gated) Gemini provider; the image default remains the stub.

#### Scenario: Keyless real-media round-trip (mock)

- **WHEN** a client submits with provider "pollinations" against a mock returning JPEG bytes and polls to success
- **THEN** the resultUrl serves those exact bytes with the returned content-type, with no API key anywhere

#### Scenario: Always registered

- **WHEN** the gateway starts with no keys configured
- **THEN** the image catalog lists both stub and pollinations
