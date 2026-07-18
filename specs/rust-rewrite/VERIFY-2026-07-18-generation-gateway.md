# Generation gateway end-to-end verification — 2026-07-18

Change `generation-gateway-stub` (D6 self-host path, phase 1). The stub
gateway `fronda-gen-gateway` was started and Fronda's real D6 client driven
against it — the first actual HTTP round-trip test of `HttpGenerationBackend`,
which unit tests (pure request/response building) could not cover.

## Setup

Gateway (a server daemon — env/config is conventional for a server):
```
FRONDA_GEN_GATEWAY_ADDR=127.0.0.1:8791 FRONDA_GEN_GATEWAY_TOKEN=e2e-secret \
  ./target/debug/fronda-gen-gateway
```

Fronda side (change `generation-endpoint-settings-ui`): the endpoint URL +
token are GUI-set via Settings → AI/Agent pane, persisted to preferences.json
(`generationEndpointUrl`/`generationEndpointToken`) — NOT env vars (Fronda is a
GUI app). The gated live test below reads env only to inject test parameters.

## Gateway HTTP (via curl) — PASS

- `GET /v1/providers` → `{"video":[{"name":"stub","models":["stub-video"]}],
  "image":[…],"audio":[…]}` (bearer).
- `POST /v1/generate {"kind":"video","model":"stub-video","prompt":"…"}` →
  `{"jobId":"job-1","status":"queued"}`.
- `GET /v1/jobs/job-1` → `{"status":"running"}` then
  `{"status":"succeeded","resultUrls":["stub://video/job-1"]}`.
- No/blank token → `401`, job never created.

## Fronda client → gateway (real reqwest round-trip) — PASS

New gated test `http_generation_backend::live_round_trip_submits_and_polls_to_success`
(runs only when FRONDA_GENERATION_URL/_TOKEN are set — CI skips it, same
pattern as anthropic_transport and the whisper model test):

```
FRONDA_GENERATION_URL=http://127.0.0.1:8791 FRONDA_GENERATION_TOKEN=e2e-secret \
  cargo test -p fronda-app-shell-gpui live_round_trip_submits_and_polls
→ ok. 1 passed
```

The real `HttpGenerationBackend` submitted a `GenerationRequest`, received a
non-empty job id, polled through the `Err`-on-running phase, and resolved to
`GenerationOutcome::Success` carrying the stub URL. Without the env vars the
test skips cleanly (verified). This closes the D6 client's "live round-trip
not auto-tested" gap.

## Not covered (phase 2)

Real provider adapters (Gemini, fal, Replicate, ElevenLabs, …) behind the same
`GenerationProvider` trait, each bring-your-own-key; the Fronda-side
provider/model picker reading `GET /v1/providers`. The stub proves the
plumbing and the multi-provider architecture; real providers return real
media URLs instead of `stub://…`.
