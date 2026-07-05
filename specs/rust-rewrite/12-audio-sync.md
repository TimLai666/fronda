# 12 — Audio sync (upstream PR #119)

Design spec for porting Palmier Pro's multi-track audio sync to Fronda. The
upstream audit (`97-upstream-pr-audit.md`, row #119) marks this **"needs a design
spec before porting"**; this is that spec. Nothing here is implemented yet except
the pure correlation math (see [Done](#already-done)).

## Goal

Align two audio recordings of the same moment — a camera's scratch track and a
separate recorder (dual-system sound), or two cameras in a multi-cam shoot — by
measuring the time offset between them from their audio and shifting one to
match. The user picks a **reference** clip and one or more **target** clips; the
target is moved so its waveform lines up with the reference.

## Already done

`audio_core::audio_sync_correlator` is pure, tested (13 tests), and currently has
**no callers**. Public API:

- `RmsFrame { rms: f64, time_seconds: f64 }`
- `SyncOffset { offset_frames: i64, confidence: f64, peak_lag_frames: i64 }` —
  `offset_frames` is at the **project** fps; positive means the reference starts
  earlier (target is delayed).
- `AudioSyncCorrelator::extract_rms_envelope(samples, sample_rate, frame_size) -> Vec<RmsFrame>`
- `AudioSyncCorrelator::cross_correlate(reference, target) -> Vec<(lag, corr)>`
  (Pearson per lag over the overlap)
- `AudioSyncCorrelator::find_sync_offset(reference_samples, target_samples, sample_rate, frame_size, project_fps) -> Option<SyncOffset>`

This is the whole DSP core. The rest of #119 is wiring: a host PCM seam, an agent
tool, a way to apply the offset, export alignment, undo, and UI.

## Pieces to build

### 1. Applying the offset — two options (DECISION REQUIRED)

**Option A — bake into `start_frame` (recommended for v1).** `sync_audio_clips`
moves the target clip so its waveform lines up with the reference's, and its
sync-linked video partner follows the same delta (reuse `move_clips`, which
already shifts linked partners). No model change, no render/export change, fully
covered by undo via `exec_mut`. The computed offset is not retained as metadata —
re-syncing just recomputes.

**Alignment formula (get this right — it's the whole feature).** `find_sync_offset`
returns the lag between the two decoded sample arrays, each measured from its
own **source sample 0**. A clip's source-sample-0 sits at timeline frame
`anchor = start_frame - trim_start_frame` (at speed 1; the general speed case is
out of v1 scope — refuse or warn on speed ≠ 1). So the move is NOT a bare
`start_frame += offset_frames`; it is:

```
ref_anchor = ref.start_frame - ref.trim_start_frame
tgt_anchor = tgt.start_frame - tgt.trim_start_frame
delta      = ref_anchor - tgt_anchor - offset_frames   // NOTE the MINUS on offset
tgt.start_frame = max(0, tgt.start_frame + delta)       // move_clips carries linked partners
```

**Sign, verified against the correlator's own test.** `find_sync_offset(ref, tgt)`
returns POSITIVE `offset_frames` when the target is *delayed* — its content sits
that many frames later than the reference's (`find_sync_offset_shifted_signals`
feeds `tgt = 5120 silent samples + ref` and asserts `offset_frames == 3`). A
delayed target must move **earlier** to line up, so `offset_frames` is
**subtracted**. Worked example: ref and tgt both at `start_frame = 100`, trim 0;
`offset_frames = 3` → `delta = 100 - 100 - 3 = -3` → tgt moves to 97, and its
content (3 frames into the clip) lands back at frame 100 beside the reference. ✓
`start_frame += offset_frames` (the earlier draft) is wrong twice over — wrong
sign and ignores trim/anchor. **Pin it with the padded-clip mock test** (below);
that test is the oracle since no Swift source is in-repo to diff against.

**Option B — store `sync_offset_frames: Option<i64>` on the clip (per specs
`01`/`04`).** Keep `start_frame` as authored and record the offset as metadata; the
audio mixer/exporter shifts the placement by `sync_offset_frames` at render time
(`render_core::audio_plan::mix_timeline_audio` and both video/audio exporters add
the offset to the audio placement start). Pros: reversible ("clear sync"), the
offset is inspectable. Cons: a **data-model change** (needs the model-change
confirmation gate + serde `skip_serializing_if = "Option::is_none"` for
backward-compat), and every render/export audio path must apply it or drift
silently — a new invariant to hold.

**Recommendation:** ship **Option A** first (no model change, no render risk,
immediately useful), and only add the `sync_offset_frames` field (Option B) if a
"non-destructive sync you can toggle" is explicitly wanted. Specs `01` line 146
and `04` line 114 currently describe Option B; if we go with A, update those two
spec bullets in the same change and mark the decision.

### 2. Host PCM seam — reuse #174's `ClipAudioSource`

The correlator needs each clip's decoded PCM. `agent_contract::ClipAudioSource`
(added for `remove_silence`, #174) already decodes a `MediaSource` to interleaved
f32 at a requested rate/channels, with `ProjectAudioSource` (ffmpeg) as the app
impl. Reuse it verbatim — request **mono** at a fixed rate (44.1 kHz) for both
clips so the envelopes are comparable. No new seam.

### 3. Agent tool — `sync_audio_clips` (NEW tool → tool count 59 → 60)

```
sync_audio_clips:
  referenceClipId: string   // the clip to align others to
  targetClipIds: string[]   // clips to move into sync
  minConfidence?: number    // default 0.5; below this, the target is left put and reported as low-confidence
```

Executor (`cmd_sync_audio_clips`, dispatched via `exec_mut`):
1. Resolve reference + each target clip; require the `ClipAudioSource` seam (else
   the honest "unavailable" message, exactly like `remove_silence`).
2. Decode reference PCM once; for each target, decode its PCM, call
   `find_sync_offset(ref, tgt, 44100.0, 1024, timeline.fps)`.
3. If `confidence >= minConfidence`, apply the offset (Option A: move the target
   + its linked video partner). Otherwise skip and report it.
4. Return `{ synced: [{clipId, offsetFrames, confidence}], skipped: [{clipId, confidence}] }`.

Add the tool def, bump every tool-count assertion (tools.rs header + `all_tools`
len test + `spec_tool_snapshots` + `mcp_server` count + `spec_mcp_contract`), and
add a `SYSTEM_INSTRUCTION` line under "# Editing". Keep the
`every_advertised_tool_is_dispatched` guard green.

### 4. Confidence threshold

`find_sync_offset` returns `confidence ∈ [0,1]`. A wrong-content match (two
unrelated recordings) yields low confidence; applying it would misalign. Default
`minConfidence = 0.5`; never move a clip below it — report it so the agent/user
knows sync couldn't be trusted, rather than silently mis-syncing.

### 5. UI (deferred — gpui, not required for the agent path)

Swift ships a sync menu + toast (~part of the ~600 LoC). Fronda's agent path
(`sync_audio_clips`) delivers the capability headlessly; a right-click "Sync to…"
menu + a result toast are a follow-up once the tool lands. Not blocking.

## Testing plan

1. **Pure (done):** correlator — 13 tests already cover envelope/correlation/peak.
2. **Executor (mock seam):** a `MockAudioSource` returning the SAME tone in both
   clips but the target padded with N leading silent frames → assert
   `sync_audio_clips` moves the target by ≈ the pad (Option A) with high
   confidence; a low-confidence (unrelated noise) case → target unmoved, reported
   in `skipped`. Mirrors the `remove_silence` mock tests.
3. **App seam:** `ProjectAudioSource` already covered by #174's WAV round-trip
   tests; no new decoder test needed.
4. **(Option B only)** serde round-trip of `sync_offset_frames` + an
   `mix_timeline_audio` test that the audio placement shifts by the offset.

## Open decisions (need the user before implementation)

1. **Option A vs B** — bake into `start_frame` (no model change) vs store
   `sync_offset_frames` (data-model change + render/export invariant). Spec
   currently sketches B; recommendation is A. **This is the gating decision.**
2. Adding a **new agent tool** (`sync_audio_clips`) is a small surface expansion;
   it's parity with Swift #119, so in-scope, but confirm the tool name/shape.
3. Default `minConfidence` value (0.5 is a guess; Swift's threshold, if any,
   should be matched).

## Estimated size

~150–250 LoC for Option A (tool + executor + tests), reusing the correlator and
the #174 audio seam wholesale. Option B adds ~80 LoC (model field + render/export
offset + serde tests) and the model-change gate. Far smaller than Swift's ~600
LoC because the DSP core and the PCM seam already exist.
