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

## Phase 2 addendum — real provider + picker + capability URLs

Changes `gemini-image-provider` and `generation-provider-picker`.

### Media pipeline delivers real bytes (key-free) — PASS
Gateway integration test `gemini_pipeline_serves_exact_original_png_bytes_key_free`:
a mock Gemini server returns base64 `inlineData` (a real 1×1 PNG); the gateway
decodes it, stores it, and serves it from `/v1/results/{id}`. Submitting
`provider:"gemini"` then polling to success yields a result URL that, **fetched
with no bearer token**, returns the byte-equal PNG with `Content-Type:
image/png`. This proves generation→store→serve→fetch end to end with zero
external key.

### Capability-URL results (download-path safe) — PASS
`/v1/results/{id}` is unauthenticated with unguessable UUID ids (the control
endpoints stay bearer-protected). Fronda's generic media downloader carries no
gateway token, so a bearer-gated results route would 401 every download; the
capability-URL model fixes that. Tests assert a tokenless GET returns the bytes
(200) and an unknown id returns 404 (not 401).

### Gemini image provider — code complete, live needs the user's key
`GeminiImageProvider` calls `generateContent` (`x-goog-api-key`,
`responseModalities:["TEXT","IMAGE"]`, base/model/version configurable) and
decodes `inlineData`. Registered only when a key is set. Real Gemini output is
verified by the gated `live_gemini_returns_real_image_bytes` test —
`FRONDA_GEMINI_API_KEY` set → real call; unset → skip. **The real-image check is
the user's to run with their Google API key** (BYO); the pipeline itself is
proven key-free above.

### Full v1.1 client loop (catalog + provider-tagged submit) — PASS
The gated `live_round_trip_submits_and_polls_to_success` was extended: against a
running stub gateway, Fronda's real `HttpGenerationBackend` calls
`fetch_providers()` (asserts the video catalog lists `stub` — the picker's data
source), then submits with `provider:Some("stub")` and polls to success. Skips
cleanly without the env vars (CI-safe).

### Fronda picker — code complete, interaction is manual
`GenerationRequest.provider` threads panel → tool → request → wire; the panel
fetches the catalog on open and shows provider/model dropdowns. gpui interaction
(live fetch, dropdown selection) is verified by compile + structural tests per
repo convention; a manual pass belongs to a future D9-style interactive check.

## Real AI media end-to-end (key-free) — PASS, verified by maintainer

Change `pollinations-image-provider`: a second image provider using Pollinations
(`GET {base}/prompt/{prompt}` → real AI JPEG, NO auth), always registered
alongside stub and (key-gated) Gemini. Image default stays stub; Pollinations is
opt-in (`provider:"pollinations"`).

Full HTTP-surface run against a live gateway (2026-07-18):
- `GET /v1/providers` → image lists `pollinations` + `stub`.
- `POST /v1/generate {"kind":"image","provider":"pollinations","prompt":"a red
  cube on a white background, studio lighting"}` → `{jobId, queued}`.
- Poll running→succeeded (~18s real Pollinations call).
- Result URL is a capability UUID; fetched WITHOUT a bearer token → a genuine
  **768×768 JPEG, 25,193 bytes** (EXIF manufacturer=sana). The image is a real
  red cube matching the prompt.

This is the "real AI media flows end to end" proof, achieved with **no credential
of any kind** — the gap Gemini (paid, BYO Google key) leaves to the user. The
gated `live_pollinations_returns_real_image_bytes` test also passed against the
real service (2.37s). Everything remains key-free and offline in CI (stub
default + mock provider tests); the live paths are env-gated.
