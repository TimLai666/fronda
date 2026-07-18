# Fronda Generation Protocol v1 (v1.1)

Status: adapter shipped (change `generation-backend-d6`). This is a
Fronda-defined protocol for a **bring-your-own generation endpoint**, not a
mirror of any existing service. It exists so that "wire up media generation"
is a *configuration* change (point Fronda at a compatible endpoint) rather
than a code change, and so anyone can self-host a compatible service.

The concrete client is `app_shell_gpui::http_generation_backend`
(`HttpGenerationBackend`), a blocking reqwest client (rustls TLS, no OpenSSL —
portable across macOS/Windows/Linux). It implements the
`agent_contract::GenerationBackend` seam (`submit` + `resume_job`). When no
endpoint is configured the seam stays unset and the `generate_video`/
`generate_image`/`generate_audio` tools return their existing honest
"requires a remote API" error — zero behavior change for users without an
endpoint.

## Configuration

The backend resolves from two environment variables (both required,
whitespace-trimmed; either missing or blank → no backend installed):

| Variable | Meaning |
|----------|---------|
| `FRONDA_GENERATION_URL` | Base URL of the Protocol v1 service, e.g. `https://gen.example.com`. Trailing slash tolerated. |
| `FRONDA_GENERATION_TOKEN` | Bearer token sent as `Authorization: Bearer <token>` on every request. |

Resolution is `GenerationBackendConfig::from_env()`; the host installs the
backend in `EditorStateHub::install_matte_writer` (alongside the matte/audio
seams), so it is available for both submit and generation-recovery whenever a
project is open.

## Transport

- All requests use `Authorization: Bearer <FRONDA_GENERATION_TOKEN>`.
- Request/response bodies are JSON (`Content-Type: application/json`).
- Client timeout defaults to 120s.
- Two endpoints: submit (`POST /v1/generate`) and poll (`GET /v1/jobs/{id}`).

### `POST {base}/v1/generate`

Submit a generation job. Request body:

```json
{
  "kind": "video",            // "video" | "image" | "audio" (lowercase)
  "model": "veo-3",           // catalog model id
  "prompt": "a cat surfing",
  "durationSeconds": 5.0,      // optional (video/audio)
  "sourceUrl": "https://…",   // optional (image/video reference input)
  "targetLanguage": "en",     // optional (audio, e.g. dubbing/tts language)
  "params": { "aspectRatio": "16:9" }  // optional, model-specific passthrough
}
```

Field notes:
- `kind` is the lowercase model-kind token. `params` is an opaque JSON object
  forwarded verbatim; it is omitted when null. `durationSeconds`, `sourceUrl`,
  and `targetLanguage` are omitted when absent.
- The body is produced by the pure `build_submit_body(&GenerationRequest)`.

Success response (2xx):

```json
{ "jobId": "job-123", "status": "queued" }
```

- `jobId` (required, non-empty string) is stored as the manifest entry's
  `backend_job_id` and drives later polling. `status` is informational at
  submit time.
- A 2xx without a usable `jobId`, or any non-2xx, is an error carrying the
  status code and any `error`/`message` string from the body. Parsing is the
  pure `parse_submit_response(status, &body)`.

### `GET {base}/v1/jobs/{jobId}`

Poll a job's status. Response:

```json
{
  "status": "succeeded",          // queued | running | succeeded | failed
  "resultUrls": ["https://…"],    // present on succeeded
  "error": "content policy"        // present on failed
}
```

Status → outcome mapping (pure `parse_poll_response(status, &body)`):

| `status` | Result | Effect on the manifest entry |
|----------|--------|------------------------------|
| `succeeded` (with ≥1 `resultUrls`) | `GenerationOutcome::Success { result_urls }` | status → `"none"`, `result_urls` stored |
| `succeeded` (no/empty `resultUrls`) | `GenerationOutcome::Failure` — a success delivering nothing would flip the asset to ready-with-no-media (a dangling done-but-empty entry), so it is treated as a failure | status → `"failed"` |
| `failed` | `GenerationOutcome::Failure { reason }` (`error` string, default `"generation failed"`) | status → `"failed"` |
| `queued` / `running` / other | `Err("still <status>")` | entry stays `"generating"`; retried next recovery pass |
| non-2xx / missing `status` / unreachable | `Err(...)` | entry untouched; retried next recovery pass |

The `Err` cases are intentional: an unreachable backend or an in-progress job
is **not a verdict**, so the manifest is left pending and a later recovery
pass retries (matching the #216 `apply_generation_outcome` contract — only a
definitive Success/Failure is written back).

## Job lifecycle / state machine

```
generate_* tool  ──submit──▶  POST /v1/generate ──▶ { jobId }
      │                                                  │
      ▼                                                  ▼
 manifest entry: generation_status = "generating", backend_job_id = jobId
      │
      │  (host recovery tick / project reopen — #211/#216)
      ▼
 resume_job(jobId) ──▶ GET /v1/jobs/{jobId}
      ├─ succeeded ─▶ apply_generation_outcome Success ─▶ status "none" + result_urls
      ├─ failed    ─▶ apply_generation_outcome Failure ─▶ status "failed"
      └─ queued/running/unreachable ─▶ stays "generating" ─▶ retried later
```

The submit path never fabricates success: it registers a genuinely pending
asset and returns a non-error result carrying the `mediaRef`. Completion is
the host's existing generation-recovery responsibility
(`plan_generation_recovery` scans `generation_status == "generating"` entries
with a `backend_job_id`, then `resume_job` + `apply_generation_outcome`).

Caveat: if the host never runs a recovery tick, a submitted asset stays
`"generating"` indefinitely. Driving recovery (on project reopen and on a
periodic tick) is the host's job, per #211/#216 — this protocol only defines
the wire contract.

## Self-hosting a compatible service

A minimal compatible service needs to:
1. Accept `POST /v1/generate` with bearer auth, enqueue a job, and return
   `{ "jobId": "<id>", "status": "queued" }` (2xx).
2. Serve `GET /v1/jobs/{id}` with bearer auth, returning the current
   `status`, plus `resultUrls` on `succeeded` and `error` on `failed`.
3. Host the `resultUrls` so Fronda's media layer can fetch/cache them (the
   same `cachedRemoteURL` path used by #135 offline-asset handling).

The protocol is deliberately small; model-specific inputs ride in the opaque
`params` object so a service can support any catalog model without a schema
change here.

## v1.1 additions — multi-provider (backward compatible)

A gateway may front **multiple providers per kind** (e.g. several
image-generation vendors), each bring-your-own-key. v1.1 adds provider
selection and discovery; a v1 client that sends neither is fully compatible —
the gateway routes to the kind's default provider.

### Provider selection on submit

`POST /v1/generate` accepts an optional `provider` string:

```json
{ "kind": "image", "provider": "gemini", "model": "imagen-4", "prompt": "…" }
```

- Absent → the gateway routes to the configured default provider for that
  `kind`.
- A named provider the gateway does not have (for that kind) → an explicit
  error (4xx), not a silent fallback.
- `model` still selects the specific model **within** the chosen provider.

### `GET {base}/v1/providers`

Bearer-authed discovery so the client can populate a provider/model picker:

```json
{
  "video": [{ "name": "stub", "models": ["stub-video"] }],
  "image": [{ "name": "stub", "models": ["stub-image"] }],
  "audio": [{ "name": "stub", "models": ["stub-audio"] }]
}
```

Each kind lists its registered providers (default first) and each provider's
available model ids.

## Reference gateway (`crates/generation_gateway`)

`fronda-gen-gateway` is the in-repo reference implementation (axum + tokio):
a `GenerationProvider` trait + registry routes `(kind, provider)` to a
provider, each reading its own key from config (bring-your-own-key). Phase 1
ships **stub providers** so the full submit→poll→succeeded loop runs with no
external keys — this is what lets the Fronda `HttpGenerationBackend` be
exercised end to end against a real endpoint. Real providers (Gemini, fal,
Replicate, ElevenLabs, …) are added as further trait implementations without
protocol changes.

## Testing

Request building and response parsing are pure functions
(`build_submit_body`, `parse_submit_response`, `parse_poll_response`,
`resolve_config`) with fixture unit tests (submit 2xx/4xx/bad-body, poll
succeeded/failed/queued/running/bad-body, config missing url/token). The live
HTTP round-trip needs a configured endpoint and network and is **not**
auto-tested — the same trade-off as `anthropic_transport`.
