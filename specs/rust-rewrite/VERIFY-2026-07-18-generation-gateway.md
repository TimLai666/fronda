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


## Picker UI interactive check — BLOCKED on the locked machine (user-only)

Attempted a live macOS pass of the provider/model picker: wrote the endpoint to
preferences.json pointing at a running keyless gateway (image providers
pollinations+stub confirmed via curl), launched Fronda into the editor. The
display had auto-locked after hours idle; waking it showed the macOS lock screen
(Touch ID / password). Unlocking needs the user's credential, which the agent
must not enter — so the *visual* confirmation that the dropdowns render and
populate could not be completed. It joins the user-only checks (physical
audio-out, real-Gemini-with-key). The picker is otherwise built, compiles,
passes structural tests, and its data path (fetch_providers → catalog → request
provider) is proven by the extended gated live test against the running gateway.
Test processes and the test endpoint in preferences.json were cleaned up.

## Post-delivery hardening sweep (adversarial review) — 2026-07-18

An adversarial review of the delivered generation subsystem confirmed the
security core is solid (bearer boundary, capability-URL unguessability,
prompt/provider injection defenses, no silent fallback, async races all clean)
and surfaced three defects. Two were real and fixed:

- **[security] Network bind without a token now hard-rejects.**
  `GatewayConfig::validate()` returns `Err` when the bind is non-loopback and no
  token is set, and `main.rs` propagates it — the gateway refuses to start
  instead of only warning. Mirrors `mcp_server`'s #122 `validate()` posture; a
  gateway that proxies the operator's paid provider key must not be laxer than
  the MCP server. Loopback stays open (local single-user). 3 tests.
- **[correctness] Real-provider HTTP calls are now timeout-bounded.**
  `provider_http_client()` builds the reqwest client with a 120s overall +
  15s connect timeout (was `reqwest::Client::new()`, unbounded). Without it a
  hung upstream left the spawned task never returning → the job stuck `Running`
  forever → the client's asset permanently "generating". Both Gemini and
  Pollinations use it. The 120s ceiling matches the client-side
  `http_generation_backend` timeout; a hang now becomes an explicit `Failed`.

Deliberately **not** changed (contract decision, not a mechanical fix):

- **Real providers ignore the protocol's top-level `model`.** Gemini uses its
  configured model; Pollinations reads `params.model`. Naively forwarding
  `req.model` would send Fronda-namespace catalog ids (what the agent/tool path
  sends) to providers that expect their *own* model namespace, regressing the
  currently-working "use the configured default" behavior. Each provider
  advertises exactly one model today and the default image provider is the stub,
  so there is **no user-visible bug**. Making `model` meaningful requires
  deciding whose namespace it is (Fronda catalog vs gateway/provider catalog) —
  an API-contract call, left for a deliberate follow-up rather than guessed here.
  The picker sources its model ids from `GET /v1/providers` (provider namespace),
  so a future "honor `req.model` when it names an advertised model" is the safe
  shape once a provider offers more than one.

## Reference-gateway known limitations (follow-ups, not bugs)

- The in-memory job/result store is unbounded — fine for a local single-user
  reference gateway; a long-running/multi-user deployment wants eviction (TTL or
  LRU) and/or on-disk results.
- Provider `submit` uses `tokio::spawn`, so it must be called from within a
  tokio runtime (it always is — the axum handler). A caller invoking `submit`
  outside a runtime would panic; the sync-trait/async-work bridge is deliberate
  for the axum usage.
