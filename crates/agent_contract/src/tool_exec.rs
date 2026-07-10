//! Tool execution dispatcher: routes agent tool calls to timeline engine.
//!
//! A ToolExecutor holds the mutable project state (Timeline + UndoStack)
//! and provides a single `execute()` entry point for the MCP server.

use crate::read_tools::{
    format_inspect_media, format_search_results, format_timeline_json, InspectMediaInput,
    SearchHitInfo,
};
use crate::undo::{UndoCommand, UndoStack};
use core_model::{
    AnimPair, Clip, ClipType, Crop, Effect, Interpolation, Keyframe, KeyframeTrack,
    LayoutFit, MatteAspect, MediaManifest, MediaManifestEntry, MediaSource, MulticamMemberKind,
    MulticamSource, MulticamSyncMap, TextRgba, TextStyle, Timeline, Transform, VideoLayout,
};
use generation_core::model_catalog;
use serde_json::{json, Value};
use std::sync::Arc;
use uuid::Uuid;

/// Host seam for `create_matte` (#242): render a solid-colour matte PNG and persist it into the
/// current project, returning where it was written. The pure executor stays FS-free — the app
/// shell provides the implementation (which encodes the PNG and writes it into the `.palmier`
/// package); the MCP/headless path leaves it unset, so `create_matte` reports it's unavailable.
pub trait MatteWriter: Send + Sync {
    fn write_matte(
        &self,
        rgba: [u8; 4],
        width: i64,
        height: i64,
        base_name: &str,
    ) -> Result<MediaSource, String>;
}

/// Host seam for model gating (upstream #249): whether the signed-in account
/// is on a paid plan. The pure executor stays account-free — the app shell
/// provides the implementation backed by its account service; the MCP/headless
/// path leaves it unset, which is treated as free tier (paid-only models are
/// listed as upgrade-required and rejected by the generate tools).
pub trait AccountState: Send + Sync {
    fn is_paid(&self) -> bool;
}

/// Host seam for `remove_silence` (#174): decode a media source's audio to
/// interleaved f32 PCM at the requested `sample_rate`/`channels`. The pure
/// executor stays codec-free — the app shell decodes via ffmpeg; the
/// MCP/headless path leaves it unset, so `remove_silence` reports it's
/// unavailable. Returns `None` when the source has no decodable audio.
pub trait ClipAudioSource: Send + Sync {
    fn decode_source_pcm(
        &self,
        source: &MediaSource,
        sample_rate: u32,
        channels: usize,
    ) -> Option<Vec<f32>>;
}

/// A detected speech span in source seconds.
#[derive(Debug, Clone, PartialEq)]
pub struct SpeechSpan {
    pub start_seconds: f64,
    pub end_seconds: f64,
}

/// Host seam for speech analysis (upstream #261's VAD): return a source's
/// speech spans, or `None` when analysis is unavailable for it — the caller
/// then falls back to the RMS silence path. Unset on the pure/MCP path.
pub trait SpeechAnalyzer: Send + Sync {
    fn analyze(&self, source: &MediaSource, sample_rate: u32) -> Option<Vec<SpeechSpan>>;
}

/// One transcribed word with start/end in SOURCE seconds (pre-placement).
#[derive(Debug, Clone, PartialEq)]
pub struct WordStamp {
    pub word: String,
    pub start_seconds: f64,
    pub end_seconds: f64,
}

/// Host seam for transcription: turn a media source's audio into word-level
/// stamps honoring the requested language (BCP-47; `None` → platform default).
/// The pure executor stays model-free — the app shell provides the
/// implementation (whisper-class or platform STT); the MCP/headless path leaves
/// it unset, so transcription-dependent flows keep the injected-words behavior
/// ("No transcribable speech" when none are set).
pub trait TranscriptionProvider: Send + Sync {
    fn transcribe(
        &self,
        source: &MediaSource,
        language: Option<&str>,
    ) -> Result<Vec<WordStamp>, String>;
}

/// What a [`ToolExecutor::transcribe_timeline`] pass covered.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TranscriptionOutcome {
    pub clips_transcribed: usize,
    pub words: usize,
}

/// Host seam for `export_project`: the pure executor validates the request and
/// the app shell performs the actual render/write (video encode, interchange
/// file, or project package). Unset on the MCP/headless path, where the tool
/// reports it's unavailable.
pub struct ExportRequest {
    /// "video" | "xml" | "fcpxml" | "palmier" (validated by the executor).
    pub mode: String,
    /// Video mode: "H.264" | "H.265" | "ProRes".
    pub codec: String,
    /// Video mode: "720p" | "1080p" | "2K" | "4K" | "Match Timeline".
    pub resolution: String,
    /// Absolute destination; None means "a unique project-named file in Downloads".
    pub output_path: Option<String>,
    pub overwrite: bool,
    /// "resolve" | "fcp".
    pub fcpxml_target: String,
    pub timeline: Timeline,
    pub sibling_timelines: Vec<Timeline>,
    pub manifest: MediaManifest,
}

/// What the host did with an export request.
#[derive(Debug)]
pub enum ExportOutcome {
    /// Video renders in the background; the file lands at `path` when done.
    Started { path: String },
    /// Interchange/package writes finish inline.
    Completed { path: String },
}

pub trait ExportHost: Send + Sync {
    fn export(&self, request: ExportRequest) -> Result<ExportOutcome, String>;
}

/// A project known to the app (recents registry), for `get_projects`.
#[derive(Debug, Clone)]
pub struct KnownProject {
    pub id: String,
    pub name: String,
    pub path: String,
    pub is_open: bool,
    pub is_active: bool,
}

/// Host seam for `get_projects`: the app shell reads its recents registry and
/// reports the active project. Read-only. Unset on the pure/MCP path, where
/// the tool reports it's unavailable.
pub trait ProjectLister: Send + Sync {
    /// (known projects, active (name, path) if a project is open)
    fn list(&self) -> Result<(Vec<KnownProject>, Option<(String, String)>), String>;
}

/// Everything the executor needs to become another project (upstream
/// open_project/new_project). The navigator builds this WITHOUT touching the
/// executor lock (the command runs inside it), and the executor swaps itself.
pub struct OpenedProject {
    pub name: String,
    pub root: String,
    pub timeline: Timeline,
    pub sibling_timelines: Vec<Timeline>,
    pub manifest: MediaManifest,
    pub multicam_groups: Vec<MulticamSource>,
    pub seams: ProjectSeams,
}

/// Project-scoped host seams, rebuilt for the newly active project root.
pub struct ProjectSeams {
    pub matte_writer: Arc<dyn MatteWriter>,
    pub audio_source: Arc<dyn ClipAudioSource>,
    pub export_host: Arc<dyn ExportHost>,
    pub project_lister: Arc<dyn ProjectLister>,
}

/// Host seam for open_project/new_project: resolves/loads (or creates) a
/// project package and records it, returning the full replacement state.
/// Must NOT take the executor lock. Unset on the pure path.
pub trait ProjectNavigator: Send + Sync {
    fn open(&self, id: Option<&str>, path: Option<&str>) -> Result<OpenedProject, String>;
    fn create(&self, name: Option<&str>) -> Result<OpenedProject, String>;
    /// close_project (tool-surface-v2): resolve the target (all-None = the
    /// active project), refuse projects that aren't open, save `active` to the
    /// target's package FIRST (save failure leaves the project open), then
    /// close it. Must NOT take the executor lock.
    fn close(
        &self,
        name: Option<&str>,
        id: Option<&str>,
        path: Option<&str>,
        active: ActiveProjectState,
    ) -> Result<ClosedProject, String>;
}

/// Snapshot of the executor's project state, handed to the navigator so
/// close_project can save unsaved changes before closing.
pub struct ActiveProjectState {
    pub timeline: Timeline,
    pub sibling_timelines: Vec<Timeline>,
    pub manifest: MediaManifest,
    /// Groups worth persisting (referenced by some timeline).
    pub multicam_groups: Vec<MulticamSource>,
}

/// Outcome of `ProjectNavigator::close`.
pub struct ClosedProject {
    /// Display name of the closed project.
    pub name: String,
    /// Open projects remaining after the close.
    pub open_count: usize,
    /// Whether the closed project was the active one.
    pub was_active: bool,
    /// Replacement state when the active project closed and another open
    /// project takes over; None → the executor resets to the no-project state.
    pub next_active: Option<OpenedProject>,
    /// Replacement `get_projects` lister for the no-project state (keeps
    /// project discovery working after the last project closes).
    pub lister: Option<Arc<dyn ProjectLister>>,
}

impl std::fmt::Debug for ClosedProject {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ClosedProject")
            .field("name", &self.name)
            .field("open_count", &self.open_count)
            .field("was_active", &self.was_active)
            .field("next_active", &self.next_active.as_ref().map(|o| &o.name))
            .field("lister", &self.lister.is_some())
            .finish()
    }
}

/// Diagnostics-bearing feedback submission (upstream #152). Built by the executor,
/// delivered by the host.
#[derive(Debug, Clone)]
pub struct FeedbackPayload {
    pub message: String,
    pub app_version: String,
    pub timeline_summary: String,
}

/// Host seam for `send_feedback`: upstream posts via its account-authenticated
/// backend, so delivery is host-gated. Unset on the pure/MCP path, where the
/// tool reports it's unavailable.
pub trait FeedbackSender: Send + Sync {
    fn send(&self, payload: &FeedbackPayload) -> Result<(), String>;
}

/// Host seam for generation recovery (upstream #216): re-subscribe to an
/// in-flight backend generation job and report its definitive outcome. Poll vs
/// callback transport is the host's business — the seam only contracts the
/// result: success carries result URLs, failure carries a reason. `Err` means
/// the backend could not be reached; the manifest stays untouched so a later
/// recovery pass can retry. Unset on the pure/MCP path.
pub trait GenerationBackend: Send + Sync {
    fn resume_job(&self, job_id: &str) -> Result<generation_core::GenerationOutcome, String>;
}

/// One planned recovery job and what happened to it (upstream #216).
#[derive(Debug)]
pub struct GenerationRecoveryRecord {
    pub job: generation_core::RecoverableJob,
    /// `None`: no backend installed — the action is recorded only.
    /// `Some(Ok(()))`: the backend outcome was applied to the manifest.
    /// `Some(Err(_))`: resume failed; the manifest is untouched for this job.
    pub outcome: Option<Result<(), String>>,
}

const DEFAULT_CLIP_DURATION_FRAMES: i64 = 150;

/// Upstream #152: at most this many feedback sends per session.
const FEEDBACK_SESSION_CAP: usize = 8;

/// Product feedback destination: Fronda's GitHub issues (we run no feedback
/// backend). The app menu opens it; `send_feedback` returns it as guidance.
pub const FEEDBACK_ISSUES_URL: &str = "https://github.com/TimLai666/fronda/issues/new";

/// Resolved clip placement geometry from optional agent args + manifest entry.
struct ResolvedPlacement {
    media_type: ClipType,
    duration_frames: i64,
    trim_start_frame: i64,
    trim_end_frame: i64,
    fps_warning: Option<String>,
}

/// Resolve a clip's type, duration, and symmetric trim from the manifest entry
/// and optional `trimStartFrame` / `trimEndFrame` / `durationFrames` args.
///
/// Symmetric trim model (upstream palmier-pro #236): `trimStartFrame` trims the
/// head (in-point), `trimEndFrame` trims the tail (out-point), and
/// `durationFrames` is derivable and mutually exclusive with `trimEndFrame`.
/// Trims and durations are clamped to the source length; synthetic clips
/// (image/text/shape) may run any duration.
///
/// Project fps is authoritative (upstream #233): a divergent source fps only
/// yields a warning, it never changes project fps.
fn resolve_placement(
    entry: Option<&MediaManifestEntry>,
    args: &Value,
    project_fps: i64,
) -> Result<ResolvedPlacement, String> {
    let media_type = entry.map(|e| e.r#type.clone()).unwrap_or(ClipType::Video);
    let synthetic = matches!(
        media_type,
        ClipType::Image | ClipType::Text | ClipType::Shape
    );

    let arg_i64 = |key: &str| args.get(key).and_then(Value::as_i64);
    // v2 `source: [startSeconds, endSeconds]` — source seconds scaled by the
    // authoritative PROJECT fps (never the source's own fps).
    let src_s = args.get("sourceStartSeconds").and_then(Value::as_f64);
    let src_e = args.get("sourceEndSeconds").and_then(Value::as_f64);
    let (trim_start_in, duration_in, trim_end_in) = if let (Some(s), Some(e)) = (src_s, src_e) {
        let ts = ((s * project_fps as f64).round() as i64).max(0);
        let d = (((e - s) * project_fps as f64).round() as i64).max(1);
        (ts, Some(d), None)
    } else {
        (
            arg_i64("trimStartFrame").unwrap_or(0).max(0),
            arg_i64("durationFrames"),
            arg_i64("trimEndFrame"),
        )
    };

    // Upstream #264/#265: ceiling-bound before any start+duration arithmetic.
    crate::mutation::require_frame_in_bounds(trim_start_in, "trimStartFrame")?;
    if let Some(d) = duration_in {
        crate::mutation::require_frame_in_bounds(d, "durationFrames")?;
    }
    if let Some(t) = trim_end_in {
        crate::mutation::require_frame_in_bounds(t, "trimEndFrame")?;
    }

    if duration_in.is_some() && trim_end_in.is_some() {
        return Err("Provide either durationFrames or trimEndFrame, not both".to_string());
    }

    // Source length expressed in project frames. Project fps is authoritative,
    // so seconds-based source duration is scaled by project fps, not source fps.
    let source_total = entry
        .filter(|_| !synthetic)
        .map(|e| (e.duration * project_fps as f64).round() as i64)
        .filter(|&total| total > 0);

    let (trim_start_frame, duration_frames, trim_end_frame) = match source_total {
        Some(total) => {
            let trim_start = trim_start_in.min((total - 1).max(0));
            let remaining = (total - trim_start).max(1);
            match (duration_in, trim_end_in) {
                (Some(d), _) => {
                    let d = d.clamp(1, remaining);
                    (trim_start, d, remaining - d)
                }
                (None, Some(te)) => {
                    let te = te.clamp(0, remaining - 1);
                    (trim_start, remaining - te, te)
                }
                (None, None) => (trim_start, remaining, 0),
            }
        }
        // Synthetic clip or no source metadata: any duration, no source trim.
        None => {
            let d = duration_in.unwrap_or(DEFAULT_CLIP_DURATION_FRAMES).max(1);
            (0, d, 0)
        }
    };

    let fps_warning = entry.and_then(|e| e.source_fps).and_then(|source_fps| {
        if source_fps > 0.0 && (source_fps - project_fps as f64).abs() > 0.01 {
            Some(format!(
                "Source fps {source_fps:.3} differs from project fps {project_fps}; \
                 project fps kept unchanged and the clip conforms to it. \
                 Use set_project_settings to change project fps."
            ))
        } else {
            None
        }
    });

    Ok(ResolvedPlacement {
        media_type,
        duration_frames,
        trim_start_frame,
        trim_end_frame,
        fps_warning,
    })
}

/// Parse an agent text-animation spec into a `TextAnimation`, or `None` when the
/// preset is `off`/absent (upstream #225 `parseTextAnimation`).
fn parse_text_animation(
    preset: Option<&str>,
    highlight_hex: Option<&str>,
) -> Result<Option<core_model::TextAnimation>, String> {
    let Some(raw) = preset else {
        return Ok(None);
    };
    let Some(p) = core_model::TextAnimationPreset::from_agent_str(raw) else {
        return Err(format!(
            "invalid animation '{raw}'. Valid: {}",
            core_model::TextAnimationPreset::agent_values().join(", ")
        ));
    };
    if p == core_model::TextAnimationPreset::None {
        return Ok(None);
    }
    let mut anim = core_model::TextAnimation {
        preset: p,
        ..Default::default()
    };
    if let Some(hex) = highlight_hex {
        anim.highlight = Some(core_model::TextRgba::from_hex(hex).ok_or_else(|| {
            format!("invalid highlightColor '{hex}'. Expected '#RGB', '#RRGGBB', or '#RRGGBBAA'")
        })?);
    }
    Ok(Some(anim))
}

/// Base pixel dimensions for an aspect-ratio preset (upstream #177).
/// Mirrors Swift `AspectPreset.width/height`.
fn aspect_preset_dims(aspect: &str) -> Result<(i64, i64), String> {
    match aspect {
        "16:9" => Ok((1920, 1080)),
        "9:14" => Ok((1080, 1680)),
        "9:16" => Ok((1080, 1920)),
        "1:1" => Ok((1080, 1080)),
        "4:3" => Ok((1440, 1080)),
        "2.4:1" => Ok((2560, 1080)),
        other => Err(format!(
            "Unknown aspectRatio '{other}'. Use one of: 16:9, 9:16, 1:1, 4:3, 2.4:1, 9:14"
        )),
    }
}

/// Scale a resolution to a quality preset's short edge, preserving aspect
/// (upstream #177). Mirrors Swift `QualityPreset.resolution` (truncating, like
/// Swift's `Int(Double)`).
fn quality_resolution(
    quality: &str,
    current_width: i64,
    current_height: i64,
) -> Result<(i64, i64), String> {
    let short_edge: i64 = match quality {
        "720p" => 720,
        "1080p" => 1080,
        "2K" => 1440,
        "4K" => 2160,
        other => {
            return Err(format!(
                "Unknown quality '{other}'. Use one of: 720p, 1080p, 2K, 4K"
            ))
        }
    };
    if current_width <= 0 || current_height <= 0 {
        return Err("Cannot apply quality preset to a non-positive resolution".to_string());
    }
    let (w, h) = if current_width <= current_height {
        (
            short_edge,
            (short_edge as f64 * current_height as f64 / current_width as f64) as i64,
        )
    } else {
        (
            (short_edge as f64 * current_width as f64 / current_height as f64) as i64,
            short_edge,
        )
    };
    Ok((w, h))
}

/// A skill available to the in-app agent (upstream #199). The app scans
/// `~/.palmier/skills` and loads these; `read_skill` returns the body.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentSkill {
    pub id: String,
    pub name: String,
    pub description: String,
    pub body: String,
}

/// Resolve a layout slot entry's crop anchor (upstream #226). A named `anchor`
/// picks a preset; `anchorX`/`anchorY` (0..1) override each axis. Default center.
/// Value-column count and label for a keyframe `property`, or `None` if the
/// property is not keyframable. Shared by the executor and `validate_set_keyframes`.
pub(crate) fn keyframe_property_arity(property: &str) -> Option<(usize, &'static str)> {
    match property {
        "opacity" | "volume" | "rotation" => Some((1, "value")),
        "position" => Some((2, "topLeftX, topLeftY")),
        "scale" => Some((2, "width, height")),
        "crop" => Some((4, "top, right, bottom, left")),
        _ => None,
    }
}

/// Parse `[[frame, v0, v1, ..., interp?], ...]` keyframe rows (Swift set_keyframes
/// format) into `(frame, values, interp)` triples with `arity` value columns. Rows
/// are stable-sorted by frame, then de-duplicated so the last row for any repeated
/// frame wins (matches Swift `sortAndDedupe`).
pub(crate) fn parse_keyframe_rows(
    rows: &[Value],
    arity: usize,
    labels: &str,
) -> Result<Vec<(i64, Vec<f64>, Interpolation)>, String> {
    let min_len = arity + 1;
    let max_len = arity + 2;
    let mut out: Vec<(i64, Vec<f64>, Interpolation)> = Vec::with_capacity(rows.len());
    for (i, raw) in rows.iter().enumerate() {
        let row = raw
            .as_array()
            .ok_or_else(|| format!("keyframes[{i}]: expected array [frame, {labels}, interp?]"))?;
        if row.len() != min_len && row.len() != max_len {
            return Err(format!(
                "keyframes[{i}]: expected [frame, {labels}] or [frame, {labels}, interp] (got {} elements)",
                row.len()
            ));
        }
        let frame = row[0]
            .as_i64()
            .ok_or_else(|| format!("keyframes[{i}][0]: frame must be an integer"))?;
        let mut values = Vec::with_capacity(arity);
        for k in 0..arity {
            let v = row[k + 1]
                .as_f64()
                .ok_or_else(|| format!("keyframes[{i}][{}]: expected a number", k + 1))?;
            if !v.is_finite() {
                return Err(format!("keyframes[{i}][{}]: value must be finite", k + 1));
            }
            values.push(v);
        }
        let interp = if row.len() > min_len {
            match row[min_len].as_str() {
                Some("linear") => Interpolation::Linear,
                Some("hold") => Interpolation::Hold,
                Some("smooth") => Interpolation::Smooth,
                Some(other) => {
                    return Err(format!(
                        "keyframes[{i}]: invalid interp '{other}' (expected linear, hold, or smooth)"
                    ))
                }
                None => return Err(format!("keyframes[{i}]: interp must be a string")),
            }
        } else {
            Interpolation::Smooth
        };
        out.push((frame, values, interp));
    }
    out.sort_by_key(|(f, _, _)| *f);
    let mut deduped: Vec<(i64, Vec<f64>, Interpolation)> = Vec::with_capacity(out.len());
    for row in out {
        if deduped.last().map(|(f, _, _)| *f) == Some(row.0) {
            *deduped.last_mut().unwrap() = row;
        } else {
            deduped.push(row);
        }
    }
    Ok(deduped)
}

/// Parse a caption background/border fill object `{enabled?, color?, padding?,
/// cornerRadius?}` into a [`core_model::TextFill`] (Issue #18). Missing fields
/// keep the default; an invalid colour is an error. Full replacement, not a merge.
fn parse_text_fill(v: &Value, what: &str) -> Result<core_model::TextFill, String> {
    let mut fill = core_model::TextFill::default();
    if let Some(en) = v.get("enabled").and_then(|x| x.as_bool()) {
        fill.enabled = en;
    }
    if let Some(hex) = v.get("color").and_then(|x| x.as_str()) {
        fill.color = core_model::TextRgba::from_hex(hex)
            .ok_or_else(|| format!("invalid {what} color '{hex}'"))?;
    }
    if let Some(p) = v.get("padding").and_then(|x| x.as_f64()) {
        fill.padding = Some(p);
    }
    if let Some(r) = v.get("cornerRadius").and_then(|x| x.as_f64()) {
        fill.corner_radius = Some(r);
    }
    Ok(fill)
}

fn resolve_layout_anchor(entry: &Value) -> Result<(f64, f64), String> {
    const ANCHORS: &[(&str, (f64, f64))] = &[
        ("center", (0.5, 0.5)),
        ("top", (0.5, 0.0)),
        ("bottom", (0.5, 1.0)),
        ("left", (0.0, 0.5)),
        ("right", (1.0, 0.5)),
        ("top_left", (0.0, 0.0)),
        ("top_right", (1.0, 0.0)),
        ("bottom_left", (0.0, 1.0)),
        ("bottom_right", (1.0, 1.0)),
    ];
    let mut anchor = (0.5, 0.5);
    if let Some(name) = entry.get("anchor").and_then(Value::as_str) {
        anchor = ANCHORS
            .iter()
            .find(|(n, _)| *n == name)
            .map(|(_, a)| *a)
            .ok_or_else(|| format!("invalid anchor '{name}'"))?;
    }
    for key in ["anchorX", "anchorY"] {
        if let Some(v) = entry.get(key).and_then(Value::as_f64) {
            if !(0.0..=1.0).contains(&v) {
                return Err(format!("{key} must be between 0 and 1 (got {v})"));
            }
        }
    }
    if let Some(ax) = entry.get("anchorX").and_then(Value::as_f64) {
        anchor.0 = ax;
    }
    if let Some(ay) = entry.get("anchorY").and_then(Value::as_f64) {
        anchor.1 = ay;
    }
    Ok(anchor)
}

/// Runtime state for executing agent timeline tools.
/// A captured per-clip "look" — the reusable visual/audio grade behind the
/// clip-preset tools (Issue #157). Session-scoped for now (held on the
/// executor); persisting these to the project is a follow-up.
#[derive(Debug, Clone)]
struct ClipPreset {
    transform: core_model::Transform,
    crop: Crop,
    opacity: f64,
    volume: f64,
    speed: f64,
    effects: Option<Vec<Effect>>,
    blend_mode: core_model::BlendMode,
    chroma_key: Option<core_model::ChromaKey>,
}

pub struct ToolExecutor {
    timeline: Timeline,
    media_manifest: MediaManifest,
    undo_stack: UndoStack,
    /// Named clip presets captured this session (Issue #157). Not yet persisted.
    clip_presets: std::collections::HashMap<String, ClipPreset>,
    /// READ-026: Status reporting for visual indexing.
    /// Set by the caller (app shell) to reflect search model state.
    search_status: String,
    /// Strictly increases after each successful mutating tool execution.
    revision: u64,
    /// Skills loaded from `~/.palmier/skills`, sorted by id (upstream #199).
    skills: Vec<AgentSkill>,
    /// Transcript words mapped onto the current timeline, in global index order
    /// (upstream #160). The host transcriber supplies these (source audio → words is
    /// a platform concern); `remove_words`/`get_transcript` read them. Empty until set.
    timeline_words: Vec<timeline_core::TimelineWord>,
    /// Host writer for `create_matte` (#242): renders + persists the matte PNG into the project.
    /// `None` on the pure/MCP path, where `create_matte` reports it's unavailable.
    matte_writer: Option<Arc<dyn MatteWriter>>,
    /// Host audio decoder for `remove_silence` (#174). `None` on the pure/MCP path,
    /// where `remove_silence` reports it's unavailable.
    audio_source: Option<Arc<dyn ClipAudioSource>>,
    /// Host speech analyzer (VAD seam). `None` → `remove_silence` uses the RMS path.
    speech_analyzer: Option<Arc<dyn SpeechAnalyzer>>,
    /// Host transcription model. App-scoped (like `project_navigator`); `None` on the
    /// pure/MCP path, where `transcribe_timeline` reports it's unavailable.
    transcription_provider: Option<Arc<dyn TranscriptionProvider>>,
    /// Host exporter for `export_project`. `None` on the pure/MCP path.
    export_host: Option<Arc<dyn ExportHost>>,
    /// Host recents-registry reader for `get_projects`. `None` on the pure/MCP path.
    project_lister: Option<Arc<dyn ProjectLister>>,
    /// Host navigator for open_project/new_project. `None` on the pure path.
    project_navigator: Option<Arc<dyn ProjectNavigator>>,
    /// Host feedback backend for `send_feedback` (#152). `None` on the pure/MCP path,
    /// where the tool reports it's unavailable.
    feedback_sender: Option<Arc<dyn FeedbackSender>>,
    /// Messages already sent this session (#152 dedup).
    feedback_sent_messages: std::collections::HashSet<String>,
    /// Successful sends this session (#152 cap).
    feedback_sent_count: usize,
    /// Host account state for model gating (#249). `None` = free tier.
    account_state: Option<Arc<dyn AccountState>>,
    /// Host backend for generation recovery (#216). `None` on the pure/MCP path,
    /// where recovery still plans but only records the actions.
    generation_backend: Option<Arc<dyn GenerationBackend>>,
    /// The project's OTHER timelines (upstream #255) — nest carriers resolve
    /// their children here by id. The active timeline stays in `timeline`.
    sibling_timelines: Vec<Timeline>,
    /// The user's playhead frame, reported by get_timeline v2 (C-5
    /// `currentFrame`). The host app sets it; 0 on the pure/MCP path.
    current_frame: i64,
    /// detect_beats analyses cache, keyed by mediaRef — the whole file is
    /// analysed once; windowed calls only trim the response.
    beat_cache: std::collections::HashMap<String, audio_core::beat_detector::BeatAnalysis>,
    /// Multicam groups (upstream #283), from `ProjectFile.multicam_groups`.
    /// Unreferenced groups stay in memory (undo can restore their clips) but
    /// are filtered from saves via `saved_multicam_groups`.
    multicam_groups: Vec<MulticamSource>,
}

/// Read-only tools: successful execution does not bump the revision.
const READ_ONLY_TOOLS: &[&str] = &[
    "get_timeline",
    "get_media",
    "search_media",
    "list_models",
    "detect_beats",
    "inspect_media",
    "inspect_timeline",
    "get_transcript",
    "inspect_color",
    "read_skill",
    "list_clip_presets",
    "get_projects",
    "get_multicam",
];

/// A resolved organize_media item (resolution happens before any mutation).
enum OrganizeItem {
    Asset(String),
    Timeline(String),
    /// Folder, resolved from its path to the pre-call folder id.
    Folder(String),
}

/// Hue (0–1, saturation/value 1) to RGB — the key.chroma keyHue convention.
fn hue_to_rgb(hue: f64) -> (f64, f64, f64) {
    let h = (hue.rem_euclid(1.0)) * 6.0;
    let x = 1.0 - (h % 2.0 - 1.0).abs();
    match h as u32 {
        0 => (1.0, x, 0.0),
        1 => (x, 1.0, 0.0),
        2 => (0.0, 1.0, x),
        3 => (0.0, x, 1.0),
        4 => (x, 0.0, 1.0),
        _ => (1.0, 0.0, x),
    }
}

/// Track display label (tool-surface-v2 C-4/C-5): visual tracks are
/// V-numbered bottom-up (V1 = the bottom-most visual track — the XMEML
/// export's V1, NLE convention), audio tracks A-numbered top-down within the
/// audio zone (A1 = the first audio track below the videos).
pub fn track_label(timeline: &Timeline, index: usize) -> String {
    let is_audio = timeline.tracks[index].r#type == ClipType::Audio;
    if is_audio {
        let n = timeline.tracks[..=index]
            .iter()
            .filter(|t| t.r#type == ClipType::Audio)
            .count();
        format!("A{n}")
    } else {
        let n = timeline.tracks[index..]
            .iter()
            .filter(|t| t.r#type != ClipType::Audio)
            .count();
        format!("V{n}")
    }
}

impl ToolExecutor {
    pub fn new(timeline: Timeline, media_manifest: MediaManifest) -> Self {
        Self {
            timeline,
            media_manifest,
            undo_stack: UndoStack::new(),
            clip_presets: std::collections::HashMap::new(),
            search_status: String::new(),
            revision: 0,
            skills: Vec::new(),
            timeline_words: Vec::new(),
            matte_writer: None,
            audio_source: None,
            speech_analyzer: None,
            transcription_provider: None,
            export_host: None,
            project_lister: None,
            project_navigator: None,
            feedback_sender: None,
            feedback_sent_messages: std::collections::HashSet::new(),
            feedback_sent_count: 0,
            account_state: None,
            generation_backend: None,
            sibling_timelines: Vec::new(),
            current_frame: 0,
            beat_cache: std::collections::HashMap::new(),
            multicam_groups: Vec::new(),
        }
    }

    /// Report the user's playhead position (get_timeline v2 `currentFrame`).
    pub fn set_current_frame(&mut self, frame: i64) {
        self.current_frame = frame.max(0);
    }

    /// Install the host writer for `create_matte` (#242). The app shell provides an implementation
    /// that encodes the solid-colour PNG and writes it into the open project package.
    pub fn set_matte_writer(&mut self, writer: Arc<dyn MatteWriter>) {
        self.matte_writer = Some(writer);
    }

    /// Install the host audio decoder for `remove_silence` (#174). The app shell
    /// provides an ffmpeg-backed implementation; unset means the tool reports it's
    /// unavailable.
    pub fn set_audio_source(&mut self, source: Arc<dyn ClipAudioSource>) {
        self.audio_source = Some(source);
    }

    /// Install the host speech analyzer. Optional — when unset (or when it
    /// returns None for a source) `remove_silence` uses the RMS adaptive path.
    pub fn set_speech_analyzer(&mut self, analyzer: Arc<dyn SpeechAnalyzer>) {
        self.speech_analyzer = Some(analyzer);
    }

    /// Install the host exporter for `export_project`.
    pub fn set_export_host(&mut self, host: Arc<dyn ExportHost>) {
        self.export_host = Some(host);
    }

    /// Install the host recents-registry reader for `get_projects`.
    pub fn set_project_lister(&mut self, lister: Arc<dyn ProjectLister>) {
        self.project_lister = Some(lister);
    }

    /// Install the host navigator for open_project/new_project.
    pub fn set_project_navigator(&mut self, nav: Arc<dyn ProjectNavigator>) {
        self.project_navigator = Some(nav);
    }

    /// Install the host feedback backend for `send_feedback` (#152).
    pub fn set_feedback_sender(&mut self, sender: Arc<dyn FeedbackSender>) {
        self.feedback_sender = Some(sender);
    }

    /// Install the host account state for model gating (#249). Unset = free tier.
    pub fn set_account_state(&mut self, account: Arc<dyn AccountState>) {
        self.account_state = Some(account);
    }

    /// Whether the account can use paid-only models. Free tier when no seam is set.
    pub fn is_paid_account(&self) -> bool {
        self.account_state.as_ref().is_some_and(|a| a.is_paid())
    }

    /// Install the host generation backend for recovery (#216). Account-scoped
    /// like the navigator — it survives project swaps.
    pub fn set_generation_backend(&mut self, backend: Arc<dyn GenerationBackend>) {
        self.generation_backend = Some(backend);
    }

    /// Whether a generation backend is connected. UI surfaces query this to
    /// gate generation up front ("Coming soon") instead of letting the user
    /// submit and hit an unavailable result.
    pub fn is_generation_available(&self) -> bool {
        self.generation_backend.is_some()
    }

    /// Whether a transcription provider is connected. The captions/transcript
    /// UI gates on this so it reads as "coming soon" rather than surfacing an
    /// empty "No transcribable speech" boundary.
    pub fn is_transcription_available(&self) -> bool {
        self.transcription_provider.is_some()
    }

    /// Plan and run generation recovery over the current manifest (#216):
    /// resume each in-flight job via the host backend and apply its outcome.
    /// Without a backend the plan is still returned with nothing applied (no
    /// error), keeping recovery decoupled from boot wiring. Bumps the revision
    /// when at least one outcome lands so observers see the manifest change.
    pub fn recover_generations(&mut self) -> Vec<GenerationRecoveryRecord> {
        let backend = self.generation_backend.clone();
        let mut records = Vec::new();
        let mut applied = false;
        for job in generation_core::plan_generation_recovery(&self.media_manifest) {
            let outcome = backend.as_ref().map(|b| {
                b.resume_job(&job.backend_job_id).and_then(|o| {
                    generation_core::apply_generation_outcome(
                        &mut self.media_manifest,
                        &job.asset_id,
                        o,
                    )
                })
            });
            applied |= matches!(outcome, Some(Ok(())));
            records.push(GenerationRecoveryRecord { job, outcome });
        }
        if applied {
            self.revision += 1;
        }
        records
    }

    /// Replace the project's multicam groups (upstream #283). The app shell
    /// supplies these from the opened `ProjectFile.multicam_groups`.
    pub fn set_multicam_groups(&mut self, groups: Vec<MulticamSource>) {
        self.multicam_groups = groups;
    }

    pub fn multicam_groups(&self) -> &[MulticamSource] {
        &self.multicam_groups
    }

    /// Groups worth persisting (Swift `savedMulticamGroups`): referenced by
    /// some timeline; `None` when empty so the JSON key is omitted.
    pub fn saved_multicam_groups(&self) -> Option<Vec<MulticamSource>> {
        let live = timeline_core::live_groups(
            &self.multicam_groups,
            std::iter::once(&self.timeline).chain(self.sibling_timelines.iter()),
        );
        (!live.is_empty()).then_some(live)
    }

    /// Replace the project's sibling timelines (upstream #255). The app shell
    /// supplies these from the opened ProjectFile.
    pub fn set_sibling_timelines(&mut self, timelines: Vec<Timeline>) {
        self.sibling_timelines = timelines;
    }

    pub fn sibling_timelines(&self) -> &[Timeline] {
        &self.sibling_timelines
    }

    /// id → timeline map over the siblings, for render/export resolvers.
    pub fn sibling_timeline_map(&self) -> std::collections::HashMap<String, Timeline> {
        timeline_core::timeline_resolver(&self.sibling_timelines)
    }

    /// Supply the timeline-mapped transcript words (upstream #160). The host runs
    /// on-device/cloud transcription and maps each word onto its clip; `remove_words`
    /// and transcript-driven tools read this. Empty means no transcription is connected.
    pub fn set_timeline_words(&mut self, words: Vec<timeline_core::TimelineWord>) {
        self.timeline_words = words;
    }

    pub fn timeline_words(&self) -> &[timeline_core::TimelineWord] {
        &self.timeline_words
    }

    /// Install the host transcription model. The app shell provides a
    /// whisper-class or platform STT implementation; unset means
    /// `transcribe_timeline` reports it's unavailable.
    pub fn set_transcription_provider(&mut self, provider: Arc<dyn TranscriptionProvider>) {
        self.transcription_provider = Some(provider);
    }

    /// Transcribe the timeline's audio-bearing clips (host UI trigger; upstream has
    /// no standalone transcribe tool — get_transcript/remove_words read the result).
    /// Each unique source is transcribed once with `Timeline.transcription_language`,
    /// word stamps are mapped onto every clip via the silence-detector placement
    /// convention (`timeline_core::map_word_stamps`), and the merged timeline-ordered
    /// list replaces the `set_timeline_words` storage. Fails atomically: a provider
    /// error leaves previously stored words untouched.
    pub fn transcribe_timeline(&mut self) -> Result<TranscriptionOutcome, String> {
        let provider = self.transcription_provider.clone().ok_or_else(|| {
            "transcription is unavailable: no transcription provider is connected (run it from the app)."
                .to_string()
        })?;
        let language = self.timeline.transcription_language.clone();
        let fps = self.timeline.fps;

        let mut located: Vec<(usize, usize)> = Vec::new();
        for (ti, track) in self.timeline.tracks.iter().enumerate() {
            for (ci, clip) in track.clips.iter().enumerate() {
                let entry = self.media_manifest.entry_for(&clip.media_ref);
                let audio_bearing = match entry.map(|e| e.r#type) {
                    Some(ClipType::Audio) => true,
                    Some(ClipType::Video) => entry.and_then(|e| e.has_audio).unwrap_or(false),
                    _ => false,
                };
                if audio_bearing {
                    located.push((ti, ci));
                }
            }
        }
        if located.is_empty() {
            return Err("The timeline has no audio-bearing clips to transcribe.".to_string());
        }
        located.sort_by_key(|&(ti, ci)| (self.timeline.tracks[ti].clips[ci].start_frame, ti));

        let mut stamps_by_media: std::collections::HashMap<String, Vec<WordStamp>> =
            Default::default();
        let mut all: Vec<timeline_core::TimelineWord> = Vec::new();
        for (ti, ci) in located.iter().copied() {
            let clip = &self.timeline.tracks[ti].clips[ci];
            if !stamps_by_media.contains_key(&clip.media_ref) {
                let source = self
                    .media_manifest
                    .entry_for(&clip.media_ref)
                    .map(|e| e.source.clone())
                    .ok_or_else(|| format!("Media '{}' is not in the library.", clip.media_ref))?;
                let stamps = provider.transcribe(&source, language.as_deref())?;
                stamps_by_media.insert(clip.media_ref.clone(), stamps);
            }
            let stamps = &stamps_by_media[&clip.media_ref];
            let rows: Vec<(&str, f64, f64)> = stamps
                .iter()
                .map(|w| (w.word.as_str(), w.start_seconds, w.end_seconds))
                .collect();
            all.extend(timeline_core::map_word_stamps(&rows, clip, ti, all.len(), fps));
        }

        let outcome = TranscriptionOutcome {
            clips_transcribed: located.len(),
            words: all.len(),
        };
        self.set_timeline_words(all);
        Ok(outcome)
    }

    /// Load the in-app agent's skills (upstream #199). The app scans
    /// `~/.palmier/skills` and passes the parsed skills here.
    pub fn set_skills(&mut self, skills: Vec<AgentSkill>) {
        self.skills = skills;
    }

    pub fn skills(&self) -> &[AgentSkill] {
        &self.skills
    }

    /// Change counter for UI invalidation: bumps on successful mutations.
    pub fn revision(&self) -> u64 {
        self.revision
    }

    /// Replace project state in place (project open). Clears the undo
    /// stack and bumps the revision; a running MCP server serves the new
    /// state on its next request.
    pub fn load_project(&mut self, timeline: Timeline, media_manifest: MediaManifest) {
        self.timeline = timeline;
        self.media_manifest = media_manifest;
        self.undo_stack = UndoStack::new();
        self.revision += 1;
    }

    /// Adopt an externally-produced timeline (e.g. an XMEML/FCPXML import) as
    /// the new active timeline; the previously active one becomes a sibling.
    /// Assigns a fresh id when the incoming timeline has none. Mirrors the
    /// create_timeline switch (clears undo, bumps the revision). Returns the id.
    pub fn adopt_timeline(&mut self, mut timeline: Timeline) -> String {
        if timeline.id.trim().is_empty() {
            timeline.id = Uuid::new_v4().to_string();
        }
        let id = timeline.id.clone();
        let prev = std::mem::replace(&mut self.timeline, timeline);
        self.sibling_timelines.push(prev);
        self.undo_stack.clear();
        self.revision += 1;
        id
    }

    pub fn media_manifest(&self) -> &MediaManifest {
        &self.media_manifest
    }

    pub fn media_manifest_mut(&mut self) -> &mut MediaManifest {
        &mut self.media_manifest
    }

    pub fn timeline(&self) -> &Timeline {
        &self.timeline
    }

    pub fn timeline_mut(&mut self) -> &mut Timeline {
        &mut self.timeline
    }

    pub fn undo_stack(&self) -> &UndoStack {
        &self.undo_stack
    }

    pub fn undo_stack_mut(&mut self) -> &mut UndoStack {
        &mut self.undo_stack
    }

    /// READ-026: Get the current search indexing status.
    pub fn search_status(&self) -> &str {
        &self.search_status
    }

    /// READ-026: Set the search indexing status (by app shell).
    pub fn set_search_status(&mut self, status: &str) {
        self.search_status = status.to_string();
    }

    /// Returns IDs of media entries that are offline (missing local file, no fresh
    /// cached URL). `now` gates cache-expiry (see `MediaManifestEntry::cache_is_fresh`).
    pub fn media_offline_ids(
        &self,
        now: chrono::DateTime<chrono::Utc>,
        is_missing: impl Fn(&MediaManifestEntry) -> bool,
    ) -> Vec<String> {
        self.media_manifest.missing_entry_ids(now, is_missing)
    }

    /// Returns true if the given media ref is offline.
    pub fn is_media_offline(
        &self,
        media_ref: &str,
        now: chrono::DateTime<chrono::Utc>,
        is_missing: impl Fn(&MediaManifestEntry) -> bool,
    ) -> bool {
        let offline_ids = self.media_offline_ids(now, is_missing);
        offline_ids.iter().any(|id| id == media_ref)
    }

    /// Returns true if the given media ref is unprocessable (present but failed to decode).
    ///
    /// Uses the `is_missing` callback to exclude entries whose files are simply missing
    /// (those are "offline", not "unprocessable"); an entry with a fresh cached copy is
    /// likewise not unprocessable.
    pub fn is_media_unprocessable(
        &self,
        media_ref: &str,
        now: chrono::DateTime<chrono::Utc>,
        is_missing: impl Fn(&MediaManifestEntry) -> bool,
        is_unprocessable: impl Fn(&MediaManifestEntry) -> bool,
    ) -> bool {
        self.media_manifest.entries.iter().any(|e| {
            e.id == media_ref
                && !e.cache_is_fresh(now)
                && !is_missing(e)
                && is_unprocessable(e)
        })
    }

    /// The full id universe for the short-id contract (C-3): timelines,
    /// clips, linkGroupIds, captionGroupIds (active + siblings), and media
    /// assets.
    fn id_universe(&self) -> Vec<String> {
        let mut ids: Vec<String> = Vec::new();
        for t in std::iter::once(&self.timeline).chain(self.sibling_timelines.iter()) {
            ids.push(t.id.clone());
            for clip in t.tracks.iter().flat_map(|tr| &tr.clips) {
                ids.push(clip.id.clone());
                if let Some(lg) = &clip.link_group_id {
                    ids.push(lg.clone());
                }
                if let Some(cg) = &clip.caption_group_id {
                    ids.push(cg.clone());
                }
            }
        }
        for e in &self.media_manifest.entries {
            ids.push(e.id.clone());
        }
        for g in &self.multicam_groups {
            ids.push(g.id.clone());
            for m in &g.members {
                ids.push(m.id.clone());
            }
        }
        ids
    }

    /// Execute a tool by name with validated JSON arguments.
    ///
    /// Returns the JSON result that should become the MCP `content` array.
    /// For mutation tools, automatically snapshots before/after for undo.
    ///
    /// Short-id contract (C-3): known id keys in the input expand from
    /// >= 8-char prefixes; every known full id in the output is shortened to
    /// its unique prefix over the pre ∪ post id universe.
    pub fn execute(&mut self, tool_name: &str, args: &Value) -> Result<Value, String> {
        let pre_universe = self.id_universe();
        let expanded = crate::id_short::expand_input_ids(args, &pre_universe)?;
        let result = self.execute_inner(tool_name, &expanded);
        if result.is_ok() && !READ_ONLY_TOOLS.contains(&tool_name) {
            self.revision += 1;
        }
        result.map(|value| {
            let mut universe = pre_universe;
            universe.extend(self.id_universe());
            let map = crate::id_short::shorten_ids(&universe);
            crate::id_short::shorten_output_ids(&value, &map)
        })
    }

    /// Pre-dispatch gate: run the matching `mutation` validator so the pure
    /// shape/range contract (#144 ranges, #264 frame ceiling, MUT-020) guards
    /// the live path. Validators whose input shape diverges from the live
    /// executor (split_clip, ripple_delete_ranges, import_folder) stay
    /// unwired; set_keyframes already shares its parser with the executor.
    fn validate_args(&self, tool_name: &str, args: &Value) -> Result<(), String> {
        use crate::mutation as m;
        fn gate<T>(r: m::ValidationResult<T>) -> Result<(), String> {
            match r {
                m::ValidationResult::Ok(_) => Ok(()),
                m::ValidationResult::Error(e) => Err(e),
            }
        }
        match tool_name {
            // MUT-010 (text-only field rejection) needs clip types; it stays
            // validator-only (None) pending a Swift-parity decision — the Rust
            // executor deliberately styles non-text clips today.
            "set_clip_properties" => gate(m::validate_set_clip_properties(args, None)),
            "remove_clips" => gate(m::validate_remove_clips(args)),
            "move_clips" | "move_clips_linked" => gate(m::validate_move_clips(args)),
            "duplicate_clips" => gate(m::validate_duplicate_clips(args)),
            "add_clips" => gate(m::validate_add_clips(args)),
            "insert_clips" => gate(m::validate_insert_clips(args)),
            "manage_tracks" => gate(m::validate_manage_tracks(args)),
            "add_texts" => {
                // Mirror cmd_add_texts targeting: an in-range explicit
                // trackIndex targets that track; anything else auto-picks.
                let track_type = args
                    .get("trackIndex")
                    .and_then(|v| v.as_i64())
                    .and_then(|i| usize::try_from(i).ok())
                    .and_then(|i| self.timeline.tracks.get(i))
                    .map(|t| t.r#type.name().to_string());
                gate(m::validate_add_texts(args, track_type))
            }
            "add_captions" => gate(m::validate_add_captions(args)),
            "add_shapes" => gate(m::validate_add_shapes(args)),
            "apply_animation" => gate(m::validate_apply_animation(args)),
            "apply_color" => gate(m::validate_apply_color(args)),
            "apply_effect" => gate(m::validate_apply_effect(args)),
            "organize_media" => gate(m::validate_organize_media(args)),
            "close_project" => gate(m::validate_close_project(args)),
            "import_media" => gate(m::validate_import_media(args)),
            "manage_multicam" => gate(m::validate_manage_multicam(args)),
            "change_cam" => gate(m::validate_change_cam(args)),
            _ => Ok(()),
        }
    }

    fn execute_inner(&mut self, tool_name: &str, args: &Value) -> Result<Value, String> {
        self.validate_args(tool_name, args)?;
        match tool_name {
            // ── Read-only tools ──────────────────────────────────────────
            "get_timeline" => self.cmd_get_timeline(args),

            // ── Mutation tools (undo-tracked, mutation envelope C-4) ─────
            "split_clips" => self.exec_enveloped(tool_name, ToolExecutor::cmd_split_clips, args),
            "remove_clips" => self.exec_enveloped(tool_name, ToolExecutor::cmd_remove_clips, args),
            "move_clips" => self.exec_enveloped(tool_name, ToolExecutor::cmd_move_clips, args),
            "move_clips_linked" => {
                self.exec_enveloped(tool_name, ToolExecutor::cmd_move_clips_linked, args)
            }
            "duplicate_clips" => {
                self.exec_enveloped(tool_name, ToolExecutor::cmd_duplicate_clips, args)
            }
            "set_clip_properties" => {
                self.exec_enveloped(tool_name, ToolExecutor::cmd_set_clip_properties, args)
            }
            "set_keyframes" => {
                self.exec_enveloped(tool_name, ToolExecutor::cmd_set_keyframes, args)
            }
            "ripple_delete_ranges" => {
                self.exec_enveloped(tool_name, ToolExecutor::cmd_ripple_delete_ranges, args)
            }
            "remove_words" => self.exec_enveloped(tool_name, ToolExecutor::cmd_remove_words, args),
            "manage_tracks" => {
                self.exec_enveloped(tool_name, ToolExecutor::cmd_manage_tracks, args)
            }
            "manage_multicam" => {
                self.exec_enveloped(tool_name, ToolExecutor::cmd_manage_multicam, args)
            }
            "change_cam" => self.exec_enveloped(tool_name, ToolExecutor::cmd_change_cam, args),
            "get_multicam" => self.cmd_get_multicam(args),
            "add_clips" => self.exec_enveloped(tool_name, ToolExecutor::cmd_add_clips, args),
            "insert_clips" => self.exec_enveloped(tool_name, ToolExecutor::cmd_insert_clips, args),
            "apply_layout" => self.exec_enveloped(tool_name, ToolExecutor::cmd_apply_layout, args),
            "add_texts" => self.exec_enveloped(tool_name, ToolExecutor::cmd_add_texts, args),
            "add_shapes" => self.exec_mut(tool_name, ToolExecutor::cmd_add_shapes, args),
            "apply_color" => self.exec_enveloped(tool_name, ToolExecutor::cmd_apply_color, args),
            "apply_effect" => self.exec_enveloped(tool_name, ToolExecutor::cmd_apply_effect, args),
            "set_project_settings" => {
                self.exec_mut(tool_name, ToolExecutor::cmd_set_project_settings, args)
            }
            "undo" => self.cmd_undo(),
            "redo" => self.cmd_redo(),

            // ── Media mutation tools (no undo yet) ───────────────────────
            "organize_media" => self.exec_organize_enveloped(args),
            "import_media" => self.cmd_import_media(args),
            "duplicate_project" => self.cmd_duplicate_project(),
            // #238 (half-ported): these tools are advertised but their full behaviour switches the
            // whole app's active project, which needs an app-navigation seam (and delete_project is
            // destructive). Until that lands, report the limitation honestly instead of the
            // misleading "Unknown tool" a bare fallthrough would give.
            "open_project" => self.cmd_open_project(args),
            "new_project" => self.cmd_new_project(args),
            "close_project" => self.cmd_close_project(args),
            // Advertised-but-not-yet-implemented tools (schemas landed ahead of the executor logic
            // in Issues #154/#155/#157/#158/#165/#174). Report the limitation honestly rather than
            // the misleading "Unknown tool" a fallthrough gives; each needs its own port (some are
            // host-gated: audio DSP, on-device silence analysis, XML parsing, a preset store).
            "create_compound_clip" => {
                self.exec_mut(tool_name, ToolExecutor::cmd_create_compound_clip, args)
            }
            "export_project" => self.cmd_export_project(args),
            "get_projects" => self.cmd_get_projects(),
            "send_feedback" => self.cmd_send_feedback(args),
            "update_text" => self.exec_enveloped(tool_name, ToolExecutor::cmd_update_text, args),
            "create_timeline" => self.cmd_create_timeline(args),
            "set_active_timeline" => self.cmd_set_active_timeline(args),
            "dissolve_compound_clip" => {
                self.exec_mut(tool_name, ToolExecutor::cmd_dissolve_compound_clip, args)
            }
            "save_clip_preset" => self.cmd_save_clip_preset(args),
            "apply_clip_preset" => {
                self.exec_mut(tool_name, ToolExecutor::cmd_apply_clip_preset, args)
            }
            "list_clip_presets" => self.cmd_list_clip_presets(),
            "remove_silence" => {
                self.exec_enveloped(tool_name, ToolExecutor::cmd_remove_silence, args)
            }
            "sync_clips" => self.exec_enveloped(tool_name, ToolExecutor::cmd_sync_clips, args),
            "detect_beats" => self.cmd_detect_beats(args),
            "denoise_audio" => {
                self.exec_enveloped(tool_name, ToolExecutor::cmd_denoise_audio, args)
            }

            // ── Read-only tools ──────────────────────────────────────────
            "get_media" => self.cmd_get_media(args),
            "search_media" => self.cmd_search_media(args),
            "list_models" => self.cmd_list_models(args),
            "inspect_media" => self.cmd_inspect_media(args),
            "inspect_timeline" => self.cmd_inspect_timeline(),
            "get_transcript" => self.cmd_get_transcript(args),
            "read_skill" => self.cmd_read_skill(args),

            // ── Generation tools (stub — need remote API) ────────────────
            "generate_video" => self.cmd_generate_video(args),
            "generate_image" => self.cmd_generate_image(args),
            "generate_audio" => self.cmd_generate_audio(args),
            "upscale_media" => self.cmd_upscale_media(args),

            // ── Read-only color inspect (no mutation) ────────────────────
            "inspect_color" => self.cmd_inspect_color(args),

            // ── Captions (stub — needs transcription engine) ─────────────
            "add_captions" => self.exec_enveloped(tool_name, ToolExecutor::cmd_add_captions, args),
            "apply_animation" => self.cmd_apply_animation(args),

            _ => Err(format!("Unknown tool: {tool_name}")),
        }
    }

    // ── Undo-wrapper for mutation tools ──────────────────────────────────

    fn exec_mut(
        &mut self,
        tool_name: &str,
        f: fn(&mut ToolExecutor, &Value) -> Result<Value, String>,
        args: &Value,
    ) -> Result<Value, String> {
        let before = self.timeline.clone();
        let result = f(self, args)?;
        let after = self.timeline.clone();

        if before != after {
            let cmd = UndoCommand::new(
                Uuid::new_v4().to_string(),
                tool_name.to_string(),
                before,
                after,
            );
            self.undo_stack.push_command(cmd);
        }

        Ok(result)
    }

    /// Wrap a JSON payload as MCP text content.
    fn json_content(payload: &Value) -> Value {
        json!({
            "content": [{
                "type": "text",
                "text": serde_json::to_string_pretty(payload).unwrap_or_else(|_| "{}".into()),
            }]
        })
    }

    /// Undo-wrapper + mutation envelope (tool-surface-v2 C-4): `f` returns its
    /// tool-specific extra keys as a plain JSON object; the envelope diff of
    /// the timelines around the call merges them in.
    fn exec_enveloped(
        &mut self,
        tool_name: &str,
        f: fn(&mut ToolExecutor, &Value) -> Result<Value, String>,
        args: &Value,
    ) -> Result<Value, String> {
        let before = self.timeline.clone();
        let extras = f(self, args)?;
        let after = self.timeline.clone();

        if before != after {
            let cmd = UndoCommand::new(
                Uuid::new_v4().to_string(),
                tool_name.to_string(),
                before.clone(),
                after.clone(),
            );
            self.undo_stack.push_command(cmd);
        }

        let env = crate::envelope::build_envelope(&before, &after, extras);
        Ok(Self::json_content(&env))
    }

    /// organize_media's envelope (C-4 exception): when the call deleted the
    /// active timeline and switched to another, the before/after diff is
    /// meaningless — return the plain payload (whose notes already carry the
    /// re-read reminder) instead of an envelope.
    fn exec_organize_enveloped(&mut self, args: &Value) -> Result<Value, String> {
        let before = self.timeline.clone();
        let extras = self.cmd_organize_media(args)?;
        if self.timeline.id != before.id {
            return Ok(Self::json_content(&extras));
        }
        let after = self.timeline.clone();
        let env = crate::envelope::build_envelope(&before, &after, extras);
        Ok(Self::json_content(&env))
    }

    // ── Tool implementations ─────────────────────────────────────────────

    /// GET_TIMELINE v2 (tool-surface-v2 C-5): relationship-first view —
    /// frames [start, end), default-stripping, per-track gaps, A/V link-fold,
    /// caption-group summaries, optional window + captionDetail.
    fn cmd_get_timeline(&self, args: &Value) -> Result<Value, String> {
        use crate::timeline_v2 as v2;
        use timeline_core::TimelineMathExt;

        let start_frame = args.get("startFrame").and_then(|v| v.as_i64());
        let end_frame = args.get("endFrame").and_then(|v| v.as_i64());
        let caption_detail = args
            .get("captionDetail")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        let total_frames = self.timeline.total_frames();
        let window = match (start_frame, end_frame) {
            (None, None) => None,
            (s, e) => Some((s.unwrap_or(0), e.unwrap_or(i64::MAX).min(total_frames))),
        };
        let in_window = |start: i64, end: i64| -> bool {
            match window {
                Some((ws, we)) => start < we && end > ws,
                None => true,
            }
        };

        let fold = v2::folded_audio_partners(&self.timeline);
        let folded_audio_ids: std::collections::HashSet<&str> =
            fold.values().map(|(_, id)| id.as_str()).collect();

        let mut tracks_json: Vec<Value> = Vec::new();
        for (ti, track) in self.timeline.tracks.iter().enumerate() {
            let mut tj = serde_json::Map::new();
            tj.insert("index".into(), json!(ti));
            tj.insert("label".into(), json!(track_label(&self.timeline, ti)));
            tj.insert("type".into(), json!(track.r#type.name()));
            if track.muted {
                tj.insert("muted".into(), json!(true));
            }
            if track.hidden {
                tj.insert("hidden".into(), json!(true));
            }
            if !track.sync_locked {
                tj.insert("syncLocked".into(), json!(false));
            }
            let folded_here = track
                .clips
                .iter()
                .filter(|c| folded_audio_ids.contains(c.id.as_str()))
                .count();
            if folded_here > 0 {
                tj.insert("linkedClips".into(), json!(folded_here));
            }

            // Gaps span the WHOLE track (non-caption clips; folded partners
            // included — they occupy the track even when folded).
            let gap_clips: Vec<&Clip> = track
                .clips
                .iter()
                .filter(|c| c.caption_group_id.is_none())
                .collect();
            let gaps = v2::track_gaps(&gap_clips);
            if !gaps.is_empty() {
                tj.insert("gaps".into(), json!(gaps));
            }

            // Caption groups (windowed members).
            let caption_clips: Vec<&Clip> = track
                .clips
                .iter()
                .filter(|c| c.caption_group_id.is_some())
                .filter(|c| in_window(c.start_frame, c.start_frame + c.duration_frames))
                .collect();
            let groups = v2::caption_groups_v2(&caption_clips, caption_detail);
            let deviant_ids: std::collections::HashSet<String> = groups
                .iter()
                .flat_map(|(g, _)| g.deviant_ids.iter().cloned())
                .collect();
            if !groups.is_empty() {
                tj.insert(
                    "captionGroups".into(),
                    json!(groups.into_iter().map(|(g, _)| g.summary).collect::<Vec<_>>()),
                );
            }

            // Clips: non-caption (deviant caption clips appear individually),
            // folded audio partners omitted, window applied.
            let listable = |c: &&Clip| -> bool {
                !folded_audio_ids.contains(c.id.as_str())
                    && (c.caption_group_id.is_none() || deviant_ids.contains(&c.id))
            };
            let total_listable = track.clips.iter().filter(listable).count();
            let mut clip_rows: Vec<(i64, Value)> = Vec::new();
            for clip in track.clips.iter().filter(listable) {
                if !in_window(clip.start_frame, clip.start_frame + clip.duration_frames) {
                    continue;
                }
                let row = match fold.get(&clip.id) {
                    Some((ati, audio_id)) => {
                        let audio = self.timeline.tracks[*ati]
                            .clips
                            .iter()
                            .find(|c| &c.id == audio_id)
                            .expect("fold map points at a live clip");
                        v2::clip_v2_folded(clip, *ati, audio)
                    }
                    None => v2::clip_v2(clip),
                };
                clip_rows.push((clip.start_frame, Value::Object(row)));
            }
            clip_rows.sort_by_key(|(st, _)| *st);
            if window.is_some() && clip_rows.len() < total_listable {
                tj.insert("totalClips".into(), json!(total_listable));
            }
            if !clip_rows.is_empty() {
                tj.insert(
                    "clips".into(),
                    json!(clip_rows.into_iter().map(|(_, v)| v).collect::<Vec<_>>()),
                );
            }
            tracks_json.push(Value::Object(tj));
        }

        let mut out = serde_json::Map::new();
        out.insert("id".into(), json!(self.timeline.id));
        out.insert("name".into(), json!(self.timeline.name));
        out.insert("fps".into(), json!(self.timeline.fps));
        out.insert("width".into(), json!(self.timeline.width));
        out.insert("height".into(), json!(self.timeline.height));
        if let Some(lang) = &self.timeline.transcription_language {
            out.insert("transcriptionLanguage".into(), json!(lang));
        }
        out.insert("totalFrames".into(), json!(total_frames));
        out.insert(
            "durationSeconds".into(),
            json!(v2::round3(total_frames as f64 / self.timeline.fps.max(1) as f64)),
        );
        out.insert("currentFrame".into(), json!(self.current_frame));
        out.insert("canGenerate".into(), json!(self.is_paid_account()));
        if let Some((ws, we)) = window {
            out.insert("window".into(), json!([ws, we]));
        }
        out.insert("tracks".into(), json!(tracks_json));
        // Multicam groups referenced by this timeline (C-5, upstream #283):
        // {groupId, name, angles: [label], mics: [label]}.
        let referenced = timeline_core::referenced_group_ids([&self.timeline]);
        let group_rows: Vec<Value> = self
            .multicam_groups
            .iter()
            .filter(|g| referenced.contains(&g.id))
            .map(|g| {
                json!({
                    "groupId": g.id,
                    "name": g.name,
                    "angles": g.angles().iter().map(|m| m.angle_label.clone()).collect::<Vec<_>>(),
                    "mics": g.mics().iter().map(|m| m.angle_label.clone()).collect::<Vec<_>>(),
                })
            })
            .collect();
        if !group_rows.is_empty() {
            out.insert("multicamGroups".into(), json!(group_rows));
        }
        // With >1 timeline, list them (Swift #255): {timelineId, name, active?}.
        if !self.sibling_timelines.is_empty() {
            let mut list = vec![json!({
                "timelineId": self.timeline.id, "name": self.timeline.name, "active": true
            })];
            for t in &self.sibling_timelines {
                list.push(json!({"timelineId": t.id, "name": t.name}));
            }
            out.insert("timelines".into(), json!(list));
        }
        Ok(Self::json_content(&Value::Object(out)))
    }

    /// READ_SKILL: return a loaded skill's full SKILL.md body by id (upstream #199).
    fn cmd_read_skill(&self, args: &Value) -> Result<Value, String> {
        let id = args
            .get("id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| "Missing id".to_string())?;
        let skill = self
            .skills
            .iter()
            .find(|s| s.id == id)
            .ok_or_else(|| format!("Skill '{id}' not found"))?;
        Ok(json!({
            "content": [{
                "type": "text",
                "text": skill.body.clone(),
            }]
        }))
    }

    fn cmd_split_clips(&mut self, args: &Value) -> Result<Value, String> {
        use timeline_core::ClipMathExt;

        // Resolve every cut to (track_index, frame), validating up-front so one
        // bad point rejects the whole call with no partial state (upstream #186).
        let mut cuts: Vec<(usize, i64)> = Vec::new();

        let splits = args.get("splits").and_then(|v| v.as_array());
        let has_splits = splits.map(|a| !a.is_empty()).unwrap_or(false);

        if has_splits {
            for (i, item) in splits.unwrap().iter().enumerate() {
                let clip_id = item
                    .get("clipId")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| format!("splits[{i}]: missing clipId"))?;
                let at_frame = item
                    .get("atFrame")
                    .and_then(|v| v.as_i64())
                    .ok_or_else(|| format!("splits[{i}]: missing atFrame"))?;
                crate::mutation::require_frame_in_bounds(at_frame, "atFrame")
                    .map_err(|e| format!("splits[{i}]: {e}"))?;
                let located = self.timeline.tracks.iter().enumerate().find_map(|(ti, t)| {
                    t.clips
                        .iter()
                        .find(|c| c.id == clip_id)
                        .map(|c| (ti, c.start_frame, c.end_frame()))
                });
                let (ti, start, end) =
                    located.ok_or_else(|| format!("splits[{i}]: clip '{clip_id}' not found"))?;
                if at_frame <= start || at_frame >= end {
                    return Err(format!(
                        "splits[{i}]: atFrame {at_frame} must be strictly inside clip '{clip_id}' [{start}, {end})"
                    ));
                }
                cuts.push((ti, at_frame));
            }
        } else {
            let track_index = args
                .get("trackIndex")
                .and_then(|v| v.as_i64())
                .ok_or_else(|| "Provide either 'splits' or 'trackIndex' + 'frames'".to_string())?
                as usize;
            let frames = args
                .get("frames")
                .and_then(|v| v.as_array())
                .filter(|a| !a.is_empty())
                .ok_or_else(|| "'frames' array required with trackIndex".to_string())?;
            if track_index >= self.timeline.tracks.len() {
                return Err(format!("Track index {track_index} out of bounds"));
            }
            for f in frames {
                let frame = f
                    .as_i64()
                    .ok_or_else(|| "frames must be integers".to_string())?;
                crate::mutation::require_frame_in_bounds(frame, "frame")?;
                let inside = self.timeline.tracks[track_index]
                    .clips
                    .iter()
                    .any(|c| frame > c.start_frame && frame < c.end_frame());
                if !inside {
                    return Err(format!(
                        "frame {frame} is not strictly inside any clip on track {track_index}"
                    ));
                }
                cuts.push((track_index, frame));
            }
        }

        cuts.sort_unstable();
        cuts.dedup();

        // Apply — resolve each cut against the CURRENT sub-clips so repeated cuts
        // on the same original clip land on the right piece.
        let mut new_ids: Vec<String> = Vec::new();
        for (ti, frame) in &cuts {
            let clip_id = self.timeline.tracks[*ti]
                .clips
                .iter()
                .find(|c| *frame > c.start_frame && *frame < c.end_frame())
                .map(|c| c.id.clone());
            if let Some(cid) = clip_id {
                new_ids.extend(timeline_core::split_clip(&mut self.timeline, &cid, *frame));
            }
        }

        let _ = new_ids;
        Ok(json!({}))
    }

    fn cmd_remove_clips(&mut self, args: &Value) -> Result<Value, String> {
        let clip_ids: Vec<String> = args
            .get("clipIds")
            .and_then(|v| v.as_array())
            .ok_or_else(|| "Missing clipIds".to_string())?
            .iter()
            .filter_map(|v| v.as_str().map(String::from))
            .collect();

        // v2: a clip in a link group takes its whole group with it (linked
        // delete, matching the UI).
        let id_set: std::collections::BTreeSet<String> = clip_ids.into_iter().collect();
        let expanded = timeline_core::expand_to_link_group(&self.timeline, &id_set);
        timeline_core::remove_clips(&mut self.timeline, expanded, false);
        Ok(json!({}))
    }

    fn cmd_move_clips(&mut self, args: &Value) -> Result<Value, String> {
        // v2 shape: moves: [{clipId, toTrack?, toFrame?}]. The legacy
        // clipIds/toTrack/toFrame shape still parses for older callers.
        let mut moves: Vec<(String, Option<usize>, Option<i64>)> = Vec::new();
        if let Some(arr) = args.get("moves").and_then(|v| v.as_array()) {
            for (i, m) in arr.iter().enumerate() {
                let clip_id = m
                    .get("clipId")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| format!("moves[{i}]: missing clipId"))?;
                let to_track = m.get("toTrack").and_then(|v| v.as_i64());
                let to_frame = m.get("toFrame").and_then(|v| v.as_i64());
                if to_track.is_none() && to_frame.is_none() {
                    return Err(format!(
                        "moves[{i}]: at least one of toTrack or toFrame is required"
                    ));
                }
                let to_track = match to_track {
                    Some(t) => Some(usize::try_from(t).map_err(|_| {
                        format!("moves[{i}]: toTrack must be a non-negative track index")
                    })?),
                    None => None,
                };
                moves.push((clip_id.to_string(), to_track, to_frame));
            }
        } else {
            let clip_ids: Vec<String> = args
                .get("clipIds")
                .and_then(|v| v.as_array())
                .ok_or_else(|| "Missing moves array".to_string())?
                .iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect();
            let to_track = args
                .get("toTrack")
                .and_then(|v| v.as_i64())
                .ok_or_else(|| "Missing toTrack".to_string())? as usize;
            let to_frame = args
                .get("toFrame")
                .and_then(|v| v.as_i64())
                .ok_or_else(|| "Missing toFrame".to_string())?;
            if to_track >= self.timeline.tracks.len() {
                return Err(format!(
                    "Track index {to_track} out of bounds ({} tracks)",
                    self.timeline.tracks.len()
                ));
            }
            let resolved: Vec<(String, usize, i64)> = clip_ids
                .iter()
                .map(|id| (id.clone(), to_track, to_frame))
                .collect();
            if let Some(reason) = timeline_core::multicam_move_violation(
                &self.timeline,
                &self.multicam_groups,
                &resolved,
            ) {
                return Err(reason);
            }
            let placed =
                timeline_core::move_clips(&mut self.timeline, &clip_ids, to_track, to_frame);
            let _ = placed;
            return Ok(json!({}));
        }
        if moves.is_empty() {
            return Err("moves must be non-empty".to_string());
        }

        // Validate every move up front so one bad entry rejects the whole call.
        for (i, (clip_id, to_track, _)) in moves.iter().enumerate() {
            let loc = timeline_core::find_clip(&self.timeline, clip_id)
                .ok_or_else(|| format!("moves[{i}]: clip '{clip_id}' not found"))?;
            if let Some(t) = to_track {
                let track = self
                    .timeline
                    .tracks
                    .get(*t)
                    .ok_or_else(|| format!("moves[{i}]: toTrack {t} out of bounds"))?;
                let mt = self.timeline.tracks[loc.track_index].clips[loc.clip_index].media_type;
                if !track.is_compatible_with(mt) {
                    return Err(format!(
                        "moves[{i}]: track {t} ({}) is incompatible with a {} clip",
                        track.r#type.name(),
                        mt.name()
                    ));
                }
            }
        }

        // Multicam guard (upstream #283): a group moves whole or not at all,
        // and camera clips never change lanes.
        let resolved: Vec<(String, usize, i64)> = moves
            .iter()
            .filter_map(|(clip_id, to_track, to_frame)| {
                let loc = timeline_core::find_clip(&self.timeline, clip_id)?;
                let clip = &self.timeline.tracks[loc.track_index].clips[loc.clip_index];
                Some((
                    clip_id.clone(),
                    to_track.unwrap_or(loc.track_index),
                    to_frame.unwrap_or(clip.start_frame),
                ))
            })
            .collect();
        if let Some(reason) = timeline_core::multicam_move_violation(
            &self.timeline,
            &self.multicam_groups,
            &resolved,
        ) {
            return Err(reason);
        }

        // Apply sequentially; linked partners follow via the core move
        // (startFrame propagates as a delta, tracks stay put).
        for (clip_id, to_track, to_frame) in &moves {
            let Some(loc) = timeline_core::find_clip(&self.timeline, clip_id) else {
                continue;
            };
            let clip = &self.timeline.tracks[loc.track_index].clips[loc.clip_index];
            let dest_track = to_track.unwrap_or(loc.track_index);
            let dest_frame = to_frame.unwrap_or(clip.start_frame);
            timeline_core::move_clips(
                &mut self.timeline,
                &[clip_id.clone()],
                dest_track,
                dest_frame,
            );
        }
        Ok(json!({}))
    }

    fn cmd_move_clips_linked(&mut self, args: &Value) -> Result<Value, String> {
        self.cmd_move_clips(args)
    }

    /// duplicate_clips (upstream #176): full-fidelity copies at new positions.
    /// Each entry names a lead clip + toFrame (+ optional toTrack); linked
    /// partners are duplicated alongside it (relative offset preserved), the
    /// duplicated set keeps its intra-group links via a fresh link group, and
    /// each copy overwrites its destination region. Ported from Swift
    /// `ToolExecutor.duplicateClips` + `duplicateClipsToPositions`.
    fn cmd_duplicate_clips(&mut self, args: &Value) -> Result<Value, String> {
        let entries = args
            .get("entries")
            .and_then(|v| v.as_array())
            .ok_or_else(|| "Missing or empty 'entries' array".to_string())?;
        if entries.is_empty() {
            return Err("Missing or empty 'entries' array".to_string());
        }

        // Pre-seed with every named lead so a partner that is itself a named
        // entry is never duplicated twice (Swift `seen`).
        let mut seen: std::collections::BTreeSet<String> = entries
            .iter()
            .filter_map(|e| e.get("clipId").and_then(|v| v.as_str()).map(String::from))
            .collect();

        // (source clip id, dest track, dest frame) — leads first, then partners.
        let mut moves: Vec<(String, usize, i64)> = Vec::new();
        for (idx, entry) in entries.iter().enumerate() {
            let path = format!("entries[{idx}]");
            let clip_id = entry
                .get("clipId")
                .and_then(|v| v.as_str())
                .ok_or_else(|| format!("{path}: missing 'clipId'"))?;
            let to_frame = entry
                .get("toFrame")
                .and_then(|v| v.as_i64())
                .ok_or_else(|| format!("{path}: missing 'toFrame'"))?;

            let loc = timeline_core::find_clip(&self.timeline, clip_id)
                .ok_or_else(|| format!("{path}: clip not found: {clip_id}"))?;
            if to_frame < 0 {
                return Err(format!("{path}: toFrame must be >= 0 (got {to_frame})"));
            }
            let src_type = self.timeline.tracks[loc.track_index].r#type;
            let media_type = self.timeline.tracks[loc.track_index].clips[loc.clip_index].media_type;
            let to_track = match entry.get("toTrack").and_then(|v| v.as_i64()) {
                Some(ti) => {
                    let last = self.timeline.tracks.len().saturating_sub(1);
                    let ti = usize::try_from(ti)
                        .ok()
                        .filter(|&i| i < self.timeline.tracks.len())
                        .ok_or_else(|| format!("{path}: toTrack {ti} out of range (0..{last})"))?;
                    let dest_type = self.timeline.tracks[ti].r#type;
                    if !self.timeline.tracks[ti].is_compatible_with(media_type) {
                        return Err(format!(
                            "{path}: toTrack {ti} ({}) is incompatible with clip's {} source track",
                            dest_type.name(),
                            src_type.name()
                        ));
                    }
                    ti
                }
                None => loc.track_index,
            };
            moves.push((clip_id.to_string(), to_track, to_frame));

            for pm in timeline_core::partner_moves_for_move_of(&self.timeline, clip_id, to_frame) {
                if seen.contains(&pm.clip_id) {
                    continue;
                }
                moves.push((pm.clip_id.clone(), pm.track_index, pm.to_frame.max(0)));
                seen.insert(pm.clip_id);
            }
        }

        // Snapshot every source clip up front so a clone that overwrites another
        // source doesn't corrupt a later read.
        let sources: Vec<Clip> = moves
            .iter()
            .filter_map(|(id, _, _)| {
                timeline_core::find_clip(&self.timeline, id).map(|loc| {
                    self.timeline.tracks[loc.track_index].clips[loc.clip_index].clone()
                })
            })
            .collect();
        if sources.len() != moves.len() {
            return Err("duplicate_clips: a source clip vanished before duplication".into());
        }

        // Remap link groups: a group with >= 2 duplicated members keeps a fresh
        // shared link (A/V pairs stay in sync); a lone duplicated member unlinks
        // (mirrors the clipboard clone contract / Swift duplication).
        let mut group_counts: std::collections::BTreeMap<String, usize> =
            std::collections::BTreeMap::new();
        for c in &sources {
            if let Some(g) = &c.link_group_id {
                *group_counts.entry(g.clone()).or_default() += 1;
            }
        }
        let mut remap: std::collections::BTreeMap<String, String> =
            std::collections::BTreeMap::new();
        for (g, n) in &group_counts {
            if *n > 1 {
                remap.insert(g.clone(), Uuid::new_v4().to_string());
            }
        }

        let mut created: Vec<String> = Vec::with_capacity(moves.len());
        for (src, (_, dest_track, dest_frame)) in sources.iter().zip(moves.iter()) {
            let mut clone = src.clone();
            clone.id = Uuid::new_v4().to_string();
            clone.start_frame = *dest_frame;
            clone.multicam_group_id = None;
            clone.link_group_id = match &src.link_group_id {
                Some(g) => remap.get(g).cloned(),
                None => None,
            };
            let end = dest_frame + clone.duration_frames.max(0);
            timeline_core::clear_region(&mut self.timeline, *dest_track, *dest_frame, end, false);
            let track = &mut self.timeline.tracks[*dest_track];
            track.clips.push(clone.clone());
            track.clips.sort_by_key(|c| c.start_frame);
            created.push(clone.id);
        }

        let linked = created.len().saturating_sub(entries.len());
        let linked_note = if linked > 0 {
            format!(" (+{linked} linked)")
        } else {
            String::new()
        };
        Ok(json!({
            "duplicatedClipIds": created,
            "note": format!(
                "Duplicated {} clip{}{}.",
                entries.len(),
                if entries.len() == 1 { "" } else { "s" },
                linked_note
            ),
        }))
    }

    fn cmd_set_clip_properties(&mut self, args: &Value) -> Result<Value, String> {
        let clip_ids: Vec<String> = args
            .get("clipIds")
            .and_then(|v| v.as_array())
            .ok_or_else(|| "Missing clipIds".to_string())?
            .iter()
            .filter_map(|v| v.as_str().map(String::from))
            .collect();

        // v2: property fields are top-level (the legacy nested 'properties'
        // object still parses for older callers).
        let properties = args.get("properties").unwrap_or(args);

        // Range/ceiling checks (#144 speed/volume/opacity/trim, #264 frame
        // bounds) run in validate_args before dispatch.
        let duration = properties.get("durationFrames").and_then(|v| v.as_i64());
        let trim_start = properties.get("trimStartFrame").and_then(|v| v.as_i64());
        let trim_end = properties.get("trimEndFrame").and_then(|v| v.as_i64());
        let speed = properties.get("speed").and_then(|v| v.as_f64());

        // Multicam guard (upstream #283): timing fields would slip a stamped
        // clip off its group clock; property fields stay editable.
        let has_timing = duration.is_some()
            || trim_start.is_some()
            || trim_end.is_some()
            || speed.is_some();
        if has_timing
            && clip_ids.iter().any(|id| {
                timeline_core::find_clip(&self.timeline, id).is_some_and(|loc| {
                    self.timeline.tracks[loc.track_index].clips[loc.clip_index]
                        .multicam_group_id
                        .is_some()
                })
            })
        {
            return Err(
                "Timing fields would slip a multicam clip out of sync — switch angles with change_cam; split/delete and property fields (volume, opacity, transform) stay editable."
                    .to_string(),
            );
        }
        let volume = properties.get("volume").and_then(|v| v.as_f64());
        let opacity = properties.get("opacity").and_then(|v| v.as_f64());
        let content = properties.get("content").and_then(|v| v.as_str());
        let font_name = properties.get("fontName").and_then(|v| v.as_str());
        let font_size = properties.get("fontSize").and_then(|v| v.as_f64());
        let font_weight = properties.get("fontWeight").and_then(|v| v.as_f64());
        let color = match properties.get("color").and_then(|v| v.as_str()) {
            Some(hex) => Some(core_model::TextRgba::from_hex(hex).ok_or_else(|| {
                format!("invalid color '{hex}'. Expected '#RGB', '#RRGGBB', or '#RRGGBBAA'")
            })?),
            None => None,
        };
        let alignment = match properties.get("alignment").and_then(|v| v.as_str()) {
            Some(a) => Some(core_model::TextAlignment::from_name(a).ok_or_else(|| {
                format!("invalid alignment '{a}'. Expected 'left', 'center', or 'right'")
            })?),
            None => None,
        };
        let background = match properties.get("background") {
            Some(v) => Some(parse_text_fill(v, "background")?),
            None => None,
        };
        let border = match properties.get("border") {
            Some(v) => Some(parse_text_fill(v, "border")?),
            None => None,
        };

        let transform = properties
            .get("transform")
            .map(|t| timeline_core::PartialTransform {
                center_x: t.get("centerX").and_then(|v| v.as_f64()),
                center_y: t.get("centerY").and_then(|v| v.as_f64()),
                width: t.get("width").and_then(|v| v.as_f64()),
                height: t.get("height").and_then(|v| v.as_f64()),
                rotation: t.get("rotation").and_then(|v| v.as_f64()),
                flip_horizontal: t.get("flipHorizontal").and_then(|v| v.as_bool()),
                flip_vertical: t.get("flipVertical").and_then(|v| v.as_bool()),
            });

        let update = timeline_core::ClipPropertyUpdate {
            duration_frames: duration,
            trim_start_frame: trim_start,
            trim_end_frame: trim_end,
            speed,
            volume,
            opacity,
            transform: transform.as_ref(),
            content,
            font_name,
            font_size,
            font_weight,
            color,
            alignment,
            background,
            border,
        };

        // v2: blendMode (absorbs set_blend_mode). 'normal' clears; rejected on
        // text/audio clips per the schema.
        let blend_mode: Option<core_model::BlendMode> = match properties.get("blendMode").and_then(|v| v.as_str())
        {
            Some(name) => Some(
                serde_json::from_value::<core_model::BlendMode>(json!(name)).map_err(|_| {
                    let valid: Vec<String> = core_model::BlendMode::all()
                        .iter()
                        .map(|m| crate::timeline_v2::blend_mode_name(*m))
                        .collect();
                    format!("invalid blendMode '{name}'. Valid: {}", valid.join(", "))
                })?,
            ),
            None => None,
        };
        if blend_mode.is_some() {
            for clip_id in &clip_ids {
                if let Some(loc) = timeline_core::find_clip(&self.timeline, clip_id) {
                    let mt = self.timeline.tracks[loc.track_index].clips[loc.clip_index].media_type;
                    if matches!(mt, ClipType::Text | ClipType::Audio) {
                        return Err(format!(
                            "blendMode applies to video/image clips only; clip '{clip_id}' is {}.",
                            mt.name()
                        ));
                    }
                }
            }
        }

        let mut changed_count = 0usize;
        let mut changed_fields: Vec<String> = Vec::new();
        for clip_id in &clip_ids {
            let Some(loc) = timeline_core::find_clip(&self.timeline, clip_id) else {
                continue;
            };
            let clip = &mut self.timeline.tracks[loc.track_index].clips[loc.clip_index];
            let changes = timeline_core::set_clip_properties(clip, &update);
            // v2: static volume/opacity replace any keyframe track on that property.
            if volume.is_some() {
                clip.volume_track = None;
            }
            if opacity.is_some() {
                clip.opacity_track = None;
            }
            if let Some(bm) = blend_mode {
                clip.blend_mode = bm;
            }
            changed_count += 1;
            if changed_fields.is_empty() {
                changed_fields = changes.changed;
            }
        }

        // v2: timing changes carry to linked partners so A/V stays in sync;
        // trim and speed are skipped for text partners.
        let has_timing =
            duration.is_some() || trim_start.is_some() || trim_end.is_some() || speed.is_some();
        if has_timing {
            let named: std::collections::BTreeSet<String> = clip_ids.iter().cloned().collect();
            let mut partner_ids: Vec<String> = Vec::new();
            for clip_id in &clip_ids {
                for pid in timeline_core::linked_partner_ids(&self.timeline, clip_id) {
                    if !named.contains(&pid) && !partner_ids.contains(&pid) {
                        partner_ids.push(pid);
                    }
                }
            }
            for pid in partner_ids {
                let Some(loc) = timeline_core::find_clip(&self.timeline, &pid) else {
                    continue;
                };
                let clip = &mut self.timeline.tracks[loc.track_index].clips[loc.clip_index];
                let is_text = clip.media_type == ClipType::Text;
                let partner_update = timeline_core::ClipPropertyUpdate {
                    duration_frames: duration,
                    trim_start_frame: if is_text { None } else { trim_start },
                    trim_end_frame: if is_text { None } else { trim_end },
                    speed: if is_text { None } else { speed },
                    transform: None,
                    content: None,
                    font_name: None,
                    font_size: None,
                    font_weight: None,
                    color: None,
                    alignment: None,
                    background: None,
                    border: None,
                    volume: None,
                    opacity: None,
                };
                let _ = timeline_core::set_clip_properties(clip, &partner_update);
            }
        }

        let _ = (changed_count, changed_fields);
        Ok(json!({}))
    }

    fn cmd_set_keyframes(&mut self, args: &Value) -> Result<Value, String> {
        let clip_id = args
            .get("clipId")
            .and_then(|v| v.as_str())
            .ok_or_else(|| "Missing clipId".to_string())?;
        let property = args
            .get("property")
            .and_then(|v| v.as_str())
            .ok_or_else(|| "Missing property".to_string())?;
        let kf_array = args
            .get("keyframes")
            .and_then(|v| v.as_array())
            .ok_or_else(|| "Missing keyframes array".to_string())?;

        // Rows are `[frame, ...values, interp?]` (Swift set_keyframes format);
        // validate the property up front so we know how many value columns to expect.
        let (arity, labels) = keyframe_property_arity(property).ok_or_else(|| {
            format!(
                "Unknown keyframe property '{property}'. Expected one of: opacity, volume, rotation, position, scale, crop"
            )
        })?;

        let Some(loc) = timeline_core::find_clip(&self.timeline, clip_id) else {
            return Err(format!("Clip '{clip_id}' not found"));
        };
        let rows = parse_keyframe_rows(kf_array, arity, labels)?;
        let clip = &mut self.timeline.tracks[loc.track_index].clips[loc.clip_index];

        let kf_len = rows.len();
        // An empty array clears the track.
        let scalar = |col: usize| {
            if rows.is_empty() {
                None
            } else {
                Some(KeyframeTrack {
                    keyframes: rows
                        .iter()
                        .map(|(f, v, i)| Keyframe {
                            frame: *f,
                            value: v[col],
                            interpolation_out: *i,
                        })
                        .collect(),
                })
            }
        };
        match property {
            "opacity" => clip.opacity_track = scalar(0),
            "volume" => clip.volume_track = scalar(0),
            "rotation" => clip.rotation_track = scalar(0),
            "position" | "scale" => {
                let track = if rows.is_empty() {
                    None
                } else {
                    Some(KeyframeTrack {
                        keyframes: rows
                            .iter()
                            .map(|(f, v, i)| Keyframe {
                                frame: *f,
                                value: AnimPair { a: v[0], b: v[1] },
                                interpolation_out: *i,
                            })
                            .collect(),
                    })
                };
                if property == "position" {
                    clip.position_track = track;
                } else {
                    clip.scale_track = track;
                }
            }
            "crop" => {
                clip.crop_track = if rows.is_empty() {
                    None
                } else {
                    Some(KeyframeTrack {
                        keyframes: rows
                            .iter()
                            .map(|(f, v, i)| Keyframe {
                                // Input order is [top, right, bottom, left].
                                frame: *f,
                                value: Crop {
                                    top: v[0],
                                    right: v[1],
                                    bottom: v[2],
                                    left: v[3],
                                },
                                interpolation_out: *i,
                            })
                            .collect(),
                    })
                };
            }
            _ => unreachable!("property validated above"),
        }

        let _ = kf_len;
        Ok(json!({}))
    }

    fn cmd_ripple_delete_ranges(&mut self, args: &Value) -> Result<Value, String> {
        let clip_id = args.get("clipId").and_then(|v| v.as_str());
        let track_index_arg = args.get("trackIndex").and_then(|v| v.as_i64());
        if clip_id.is_some() == track_index_arg.is_some() {
            return Err("Pass exactly one of 'trackIndex' or 'clipId'.".to_string());
        }
        let units = args
            .get("units")
            .and_then(|v| v.as_str())
            .unwrap_or("frames");
        if !matches!(units, "frames" | "seconds") {
            return Err(format!("Invalid units '{units}'. Use 'frames' or 'seconds'."));
        }
        if units == "seconds" && clip_id.is_none() {
            return Err("units 'seconds' requires clipId mode (source-media seconds).".to_string());
        }

        let ranges_val = args
            .get("ranges")
            .and_then(|v| v.as_array())
            .ok_or_else(|| "Missing ranges array".to_string())?;
        // v2 rows are [start, end] pairs; legacy {start, end} objects still parse.
        let raw_ranges: Vec<(f64, f64)> = ranges_val
            .iter()
            .filter_map(|r| {
                if let Some(pair) = r.as_array() {
                    let s = pair.first().and_then(|v| v.as_f64())?;
                    let e = pair.get(1).and_then(|v| v.as_f64())?;
                    (e > s).then_some((s, e))
                } else {
                    let s = r.get("start").and_then(|v| v.as_f64())?;
                    let e = r.get("end").and_then(|v| v.as_f64())?;
                    (e > s).then_some((s, e))
                }
            })
            .collect();
        if raw_ranges.is_empty() {
            return Err("No valid ranges".to_string());
        }

        // Resolve to (track_index, project-frame ranges).
        let (track_index, ranges) = if let Some(cid) = clip_id {
            let loc = timeline_core::find_clip(&self.timeline, cid)
                .ok_or_else(|| format!("Clip '{cid}' not found"))?;
            let clip = &self.timeline.tracks[loc.track_index].clips[loc.clip_index];
            let (start, end) = (clip.start_frame, clip.start_frame + clip.duration_frames);
            let fps = self.timeline.fps.max(1) as f64;
            let (trim, speed) = (clip.trim_start_frame as f64, clip.speed.max(0.0001));
            let to_project = |v: f64| -> i64 {
                if units == "seconds" {
                    // Source seconds → project frames through the clip mapping.
                    (start as f64 + (v * fps - trim) / speed).round() as i64
                } else {
                    v.round() as i64
                }
            };
            let clamped: Vec<timeline_core::FrameRange> = raw_ranges
                .iter()
                .map(|(s, e)| timeline_core::FrameRange {
                    start: to_project(*s).clamp(start, end),
                    end: to_project(*e).clamp(start, end),
                })
                .filter(|r| r.end > r.start)
                .collect();
            if clamped.is_empty() {
                return Err(format!(
                    "No range overlaps clip '{cid}' [{start}, {end})."
                ));
            }
            (loc.track_index, clamped)
        } else {
            let ti = track_index_arg.unwrap();
            let ti = usize::try_from(ti)
                .ok()
                .filter(|i| *i < self.timeline.tracks.len())
                .ok_or_else(|| format!("Track index {ti} out of bounds"))?;
            let ranges = raw_ranges
                .iter()
                .map(|(s, e)| timeline_core::FrameRange {
                    start: s.round() as i64,
                    end: e.round() as i64,
                })
                .collect();
            (ti, ranges)
        };

        // #207 (v2 name): tracks the caller wants treated as UNLOCKED for this
        // call — a sync-locked track listed here is left in place.
        let ignore_sync_lock_track_indices: std::collections::BTreeSet<usize> = args
            .get("ignoreSyncLockedTracks")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_i64())
                    .filter(|&i| i >= 0)
                    .map(|i| i as usize)
                    .collect()
            })
            .unwrap_or_default();

        self.apply_ripple_delete_on_track(track_index, ranges, ignore_sync_lock_track_indices)
            .map(|(_removed_frames, _cleared)| json!({}))
    }

    /// Apply a ripple delete on one track and return `(removed_frames, cleared_track_count)`.
    /// Fragment-cuts each range on every cleared track (anchor + linked partners + non-ignored
    /// sync-locked followers per #227), then closes the gaps by shifting later clips left on
    /// exactly the cleared tracks (#207-ignored sync-locked tracks are absent → left in place).
    /// `Err` carries the refuse message. Shared by ripple_delete_ranges and remove_words.
    fn apply_ripple_delete_on_track(
        &mut self,
        track_index: usize,
        ranges: Vec<timeline_core::FrameRange>,
        ignore_sync_lock_track_indices: std::collections::BTreeSet<usize>,
    ) -> Result<(i64, usize), String> {
        let range_list = ranges.clone();
        let ignored_for_guard = ignore_sync_lock_track_indices.clone();
        let config = timeline_core::RippleDeleteConfig {
            anchor_track_index: track_index,
            ignore_sync_lock_track_indices,
            ranges,
        };
        match timeline_core::compute_ripple_delete(&self.timeline, config) {
            timeline_core::RippleDeleteOutcome::Ok(report) => {
                let merged = timeline_core::merge_ranges(&range_list);
                let cleared: std::collections::HashSet<usize> =
                    report.cleared_track_indices.iter().copied().collect();

                // Multicam atomicity (upstream #283): a ripple that would
                // shift only SOME of a group's tracks desyncs the rest.
                let mut shifting = cleared.clone();
                for (ti, track) in self.timeline.tracks.iter().enumerate() {
                    if track.sync_locked && !ignored_for_guard.contains(&ti) {
                        shifting.insert(ti);
                    }
                }
                if let Some(reason) = timeline_core::multicam_atomicity_violation(
                    &self.timeline,
                    &self.multicam_groups,
                    &shifting,
                ) {
                    return Err(format!("Ripple delete refused: {reason}"));
                }

                // RPL-004: fragment-cut each range on every cleared track — a clip fully
                // inside a range is removed, a partial overlap is trimmed/split so only the
                // non-overlapping fragments survive.
                for ti in &report.cleared_track_indices {
                    for r in &merged {
                        timeline_core::clear_region(&mut self.timeline, *ti, r.start, r.end, false);
                    }
                }
                // Close the gaps: shift later clips left on every cleared track.
                for (ti, track) in self.timeline.tracks.iter_mut().enumerate() {
                    if !cleared.contains(&ti) {
                        continue;
                    }
                    let shifts =
                        timeline_core::compute_ripple_shifts_for_ranges(&track.clips, &merged);
                    for shift in shifts {
                        if let Some(clip) = track.clips.iter_mut().find(|c| c.id == shift.clip_id) {
                            clip.start_frame = shift.new_start_frame;
                        }
                    }
                }
                Ok((report.removed_frames, report.cleared_track_indices.len()))
            }
            timeline_core::RippleDeleteOutcome::Refused(msg) => {
                Err(format!("Ripple delete refused: {msg}"))
            }
        }
    }

    /// remove_words (upstream #160, #245): cut speech by the word. Resolve the selected
    /// get_transcript indices (or exact `matches` tokens) to frames, plan the cut + kept-gap
    /// ranges, and ripple-delete the primary track (linked A/V partners follow). Requires the
    /// host to have supplied timeline words via `set_timeline_words`; empty → refuse.
    fn cmd_remove_words(&mut self, args: &Value) -> Result<Value, String> {
        let raw_words = args.get("words").and_then(|v| v.as_array());
        let raw_matches = args.get("matches").and_then(|v| v.as_array());
        if raw_words.map(|a| a.is_empty()).unwrap_or(false)
            || raw_matches.map(|a| a.is_empty()).unwrap_or(false)
        {
            return Err("remove_words: words or matches must not be empty.".to_string());
        }
        if raw_words.is_none() && raw_matches.is_none() {
            return Err("Missing 'words' or 'matches'. Pass word indices from get_transcript, e.g. [5, [12, 18]], or exact words like [\"um\", \"uh\"].".to_string());
        }
        if raw_words.is_some() && raw_matches.is_some() {
            return Err("remove_words: pass either words or matches, not both.".to_string());
        }

        let aggressiveness = match args.get("cutAggressiveness").and_then(|v| v.as_str()) {
            Some(raw) => timeline_core::CutAggressiveness::from_raw(raw).ok_or_else(|| {
                format!(
                    "cutAggressiveness must be one of: {}.",
                    timeline_core::CutAggressiveness::ALL
                        .iter()
                        .map(|a| a.as_str())
                        .collect::<Vec<_>>()
                        .join(", ")
                )
            })?,
            None => timeline_core::CutAggressiveness::Balanced,
        };

        let all_words = self.timeline_words.clone();
        if all_words.is_empty() {
            return Err("No transcribable speech on the timeline.".to_string());
        }
        let max_index = (all_words.len() - 1) as i64;

        let mut selected: std::collections::BTreeSet<usize> = Default::default();
        let mut ignored: Vec<i64> = Vec::new();
        if let Some(raw) = raw_words {
            for (a, b) in Self::parse_word_spans(raw)? {
                let lo = a.min(b);
                let hi = a.max(b);
                // Clamp to the valid range so an out-of-range span can't iterate wildly.
                if hi < 0 || lo > max_index {
                    ignored.push(lo);
                    continue;
                }
                if lo < 0 {
                    ignored.push(lo);
                }
                if hi > max_index {
                    ignored.push(hi);
                }
                for idx in lo.max(0)..=hi.min(max_index) {
                    selected.insert(idx as usize);
                }
            }
            if selected.is_empty() {
                return Err(format!(
                    "None of the requested word indices are in range 0...{max_index}. Re-read get_transcript."
                ));
            }
        } else if let Some(raw) = raw_matches {
            let matches = Self::parse_word_matches(raw)?;
            for w in &all_words {
                if matches.contains(&Self::normalized_word_match(&w.text)) {
                    selected.insert(w.index);
                }
            }
            if selected.is_empty() {
                let joined = matches
                    .iter()
                    .map(|s| s.as_str())
                    .collect::<Vec<_>>()
                    .join(", ");
                return Err(format!(
                    "No transcript words matched: {joined}. Re-read get_transcript or pass exact word indices."
                ));
            }
        }

        let keep_gap_frames =
            timeline_core::ms_to_frames(aggressiveness.kept_gap_ms(), self.timeline.fps);
        let plan =
            timeline_core::plan_word_removal(&self.timeline, &all_words, &selected, keep_gap_frames)?;
        let removed_words = plan.removed_texts.len();
        let removed_texts = plan.removed_texts.clone();
        let (removed_frames, tracks_edited) =
            self.apply_ripple_delete_on_track(plan.primary_track, plan.ranges, Default::default())?;

        let mut payload = json!({
            "removedWords": removed_words,
            "removedFrames": removed_frames,
            "tracksEdited": tracks_edited,
            "cutAggressiveness": aggressiveness.as_str(),
            "notes": ["Removed and closed the gaps. Re-read get_transcript before another remove_words."],
        });
        let preview: String = removed_texts
            .iter()
            .take(24)
            .cloned()
            .collect::<Vec<_>>()
            .join(" ");
        if !preview.is_empty() {
            payload["removedText"] = json!(if removed_texts.len() > 24 {
                format!("{preview} …")
            } else {
                preview
            });
        }
        if !ignored.is_empty() {
            ignored.sort();
            payload["indicesIgnored"] = json!(ignored);
        }

        Ok(payload)
    }

    /// Parse the `words` arg: each element is a single integer index or an inclusive
    /// `[start, end]` pair. Mirrors Swift `parseWordSpans`.
    fn parse_word_spans(raw: &[Value]) -> Result<Vec<(i64, i64)>, String> {
        raw.iter()
            .enumerate()
            .map(|(i, element)| {
                if let Some(n) = Self::int_from_value(element) {
                    return Ok((n, n));
                }
                if let Some(pair) = element.as_array() {
                    if pair.len() == 2 {
                        if let (Some(a), Some(b)) =
                            (Self::int_from_value(&pair[0]), Self::int_from_value(&pair[1]))
                        {
                            return Ok((a, b));
                        }
                    }
                }
                Err(format!(
                    "words[{i}]: expected an integer index or an [start, end] pair."
                ))
            })
            .collect()
    }

    /// Parse the `matches` arg into a set of normalized single-word tokens.
    /// Mirrors Swift `parseWordMatches`.
    fn parse_word_matches(raw: &[Value]) -> Result<std::collections::BTreeSet<String>, String> {
        let mut set = std::collections::BTreeSet::new();
        for (i, element) in raw.iter().enumerate() {
            let text = element
                .as_str()
                .ok_or_else(|| format!("matches[{i}]: expected a string."))?;
            let normalized = Self::normalized_word_match(text);
            if normalized.is_empty() {
                return Err(format!("matches[{i}]: expected a non-empty word."));
            }
            set.insert(normalized);
        }
        Ok(set)
    }

    /// Normalize a match token: strip leading/trailing whitespace and punctuation, lowercase.
    /// Mirrors Swift `normalizedWordMatch` (trim whitespace ∪ Unicode punctuation, lowercase).
    fn normalized_word_match(text: &str) -> String {
        text.trim_matches(|c: char| c.is_whitespace() || Self::is_boundary_punctuation(c))
            .to_lowercase()
    }

    /// Approximates Swift's `CharacterSet.punctuationCharacters` (Unicode general category P):
    /// ASCII punctuation plus the common Unicode punctuation blocks (smart quotes, dashes,
    /// ellipsis, inverted marks, CJK and fullwidth punctuation). Not a full category-P table,
    /// but covers the tokens a transcriber or model realistically wraps a word in.
    fn is_boundary_punctuation(c: char) -> bool {
        c.is_ascii_punctuation()
            || matches!(c,
                '\u{00A1}' | '\u{00A7}' | '\u{00B6}' | '\u{00B7}' | '\u{00BF}'
                | '\u{2010}'..='\u{2027}'   // dashes, hyphens, quotes, ellipsis, bullets
                | '\u{2030}'..='\u{205E}'   // general punctuation block
                | '\u{3000}'..='\u{303F}'   // CJK symbols and punctuation
                | '\u{FF01}'..='\u{FF0F}'
                | '\u{FF1A}'..='\u{FF20}'
                | '\u{FF3B}'..='\u{FF40}'
                | '\u{FF5B}'..='\u{FF65}'   // fullwidth/halfwidth punctuation
            )
    }

    fn int_from_value(v: &Value) -> Option<i64> {
        if let Some(i) = v.as_i64() {
            return Some(i);
        }
        if let Some(f) = v.as_f64() {
            // Whole number within i64 range only — an out-of-range float is not an index
            // (Swift `Int(exactly:)` returns nil, failing the parse rather than saturating).
            if f.fract() == 0.0 && f >= -9_223_372_036_854_775_808.0 && f < 9_223_372_036_854_775_808.0
            {
                return Some(f as i64);
            }
        }
        None
    }

    /// MANAGE_TRACKS (tool-surface-v2, replaces remove_tracks): reorder → set
    /// → remove in one undoable action. Every index is resolved to a track id
    /// against the CALL-TIME order up front, so indexes never drift within a
    /// call; reorder entries then apply live, one after another, with the
    /// destination clamped to the track's type zone (video moves stay in the
    /// video zone, audio in audio).
    fn cmd_manage_tracks(&mut self, args: &Value) -> Result<Value, String> {
        let parsed = match crate::mutation::validate_manage_tracks(args) {
            crate::mutation::ValidationResult::Ok(p) => p,
            crate::mutation::ValidationResult::Error(e) => return Err(e),
        };
        let resolve = |index: i64, what: &str| -> Result<String, String> {
            usize::try_from(index)
                .ok()
                .and_then(|i| self.timeline.tracks.get(i))
                .map(|t| t.id.clone())
                .ok_or_else(|| {
                    format!(
                        "manage_tracks: {what} index {index} is out of range (the timeline has {} tracks).",
                        self.timeline.tracks.len()
                    )
                })
        };
        let reorders: Vec<(String, i64)> = parsed
            .reorder
            .iter()
            .map(|(index, to)| resolve(*index, "reorder").map(|id| (id, *to)))
            .collect::<Result<_, _>>()?;
        let sets: Vec<(String, &crate::mutation::ManageTrackSetInput)> = parsed
            .set
            .iter()
            .map(|s| resolve(s.index, "set").map(|id| (id, s)))
            .collect::<Result<_, _>>()?;
        let mut remove_ids: Vec<String> = parsed
            .remove
            .iter()
            .map(|i| resolve(*i, "remove"))
            .collect::<Result<_, _>>()?;
        remove_ids.dedup();

        // Multicam guard (upstream #283): a group's tracks can't be removed
        // or sync-unlocked (mute/hide stay free).
        let multicam_track_ids: std::collections::HashSet<&str> = self
            .timeline
            .tracks
            .iter()
            .filter(|t| t.clips.iter().any(|c| c.multicam_group_id.is_some()))
            .map(|t| t.id.as_str())
            .collect();
        if remove_ids
            .iter()
            .any(|id| multicam_track_ids.contains(id.as_str()))
        {
            return Err(
                "A multicam group's track can't be removed — delete the group's clips first (remove_clips) and the empty track prunes itself."
                    .to_string(),
            );
        }
        if sets.iter().any(|(id, s)| {
            multicam_track_ids.contains(id.as_str()) && s.sync_locked == Some(false)
        }) {
            return Err(
                "Sync lock stays on for a multicam group's tracks — unlocking would let ripples shift the group's members apart."
                    .to_string(),
            );
        }

        for (id, to) in &reorders {
            let pos = self
                .timeline
                .tracks
                .iter()
                .position(|t| &t.id == id)
                .expect("resolved up front");
            let is_audio = self.timeline.tracks[pos].r#type == ClipType::Audio;
            let zone: Vec<usize> = self
                .timeline
                .tracks
                .iter()
                .enumerate()
                .filter(|(_, t)| (t.r#type == ClipType::Audio) == is_audio)
                .map(|(i, _)| i)
                .collect();
            let (lo, hi) = (*zone.first().expect("own zone"), *zone.last().expect("own zone"));
            let dest = usize::try_from(*to).unwrap_or(0).clamp(lo, hi);
            let track = self.timeline.tracks.remove(pos);
            self.timeline.tracks.insert(dest.min(self.timeline.tracks.len()), track);
        }
        for (id, s) in &sets {
            let track = self
                .timeline
                .tracks
                .iter_mut()
                .find(|t| &t.id == id)
                .expect("resolved up front");
            if let Some(m) = s.muted {
                track.muted = m;
            }
            if let Some(h) = s.hidden {
                track.hidden = h;
            }
            if let Some(l) = s.sync_locked {
                track.sync_locked = l;
            }
        }
        for id in &remove_ids {
            if let Some(pos) = self.timeline.tracks.iter().position(|t| &t.id == id) {
                timeline_core::remove_track(&mut self.timeline, pos);
            }
        }

        let tracks: Vec<Value> = self
            .timeline
            .tracks
            .iter()
            .enumerate()
            .map(|(i, t)| {
                let mut o = json!({
                    "index": i,
                    "label": track_label(&self.timeline, i),
                    "type": t.r#type.name(),
                });
                if t.muted {
                    o["muted"] = json!(true);
                }
                if t.hidden {
                    o["hidden"] = json!(true);
                }
                if !t.sync_locked {
                    o["syncLocked"] = json!(false);
                }
                o
            })
            .collect();
        let mut out = json!({ "tracks": tracks });
        if !reorders.is_empty() || !remove_ids.is_empty() {
            out["notes"] = json!([
                "Track indices changed — 'tracks' is the new order; index 0 renders on top."
            ]);
        }
        Ok(out)
    }

    // ── Multicam (upstream #283) ─────────────────────────────────────────

    /// Asset facts the multicam engine needs, from the media manifest.
    fn multicam_assets(&self) -> std::collections::HashMap<String, timeline_core::MulticamAsset> {
        self.media_manifest
            .entries
            .iter()
            .map(|e| {
                (
                    e.id.clone(),
                    timeline_core::MulticamAsset {
                        name: e.name.clone(),
                        clip_type: e.r#type,
                        duration: e.duration,
                        has_audio: e.has_audio.unwrap_or(false),
                        source_width: e.source_width,
                        source_height: e.source_height,
                    },
                )
            })
            .collect()
    }

    fn multicam_group(&self, id: &str) -> Option<&MulticamSource> {
        self.multicam_groups.iter().find(|g| g.id == id)
    }

    fn require_multicam_group(&self, group_id: &str) -> Result<String, String> {
        if self.multicam_group(group_id).is_none() {
            return Err(format!(
                "No multicam group '{group_id}'. get_timeline lists multicamGroups."
            ));
        }
        Ok(group_id.to_string())
    }

    /// Swift `resolveGroupId`: groupId directly, or any group clip's id.
    fn resolve_multicam_group_id(&self, args: &Value) -> Result<String, String> {
        if let Some(group_id) = args.get("groupId").and_then(|v| v.as_str()) {
            return self.require_multicam_group(group_id);
        }
        if let Some(clip_id) = args.get("clipId").and_then(|v| v.as_str()) {
            let group = timeline_core::find_clip(&self.timeline, clip_id)
                .and_then(|loc| {
                    self.timeline.tracks[loc.track_index].clips[loc.clip_index]
                        .multicam_group_id
                        .clone()
                })
                .and_then(|gid| self.multicam_group(&gid));
            return match group {
                Some(g) => Ok(g.id.clone()),
                None => Err(format!(
                    "Clip '{clip_id}' is not part of a multicam group."
                )),
            };
        }
        Err("Pass groupId or clipId.".to_string())
    }

    fn multicam_member_rows(group: &MulticamSource) -> Vec<Value> {
        use crate::timeline_v2::round3;
        group
            .members
            .iter()
            .map(|m| {
                let mut row = json!({
                    "angleLabel": m.angle_label,
                    "kind": m.kind.as_str(),
                    "mediaRef": m.media_ref,
                    "offsetSeconds": round3(m.sync.offset_seconds),
                    "confidence": round3(m.sync.confidence),
                });
                if m.id == group.master_member_id {
                    row["master"] = json!(true);
                }
                if m.sync.locked {
                    row["pinned"] = json!(true);
                }
                if !m.usable() {
                    row["unsynced"] = json!(true);
                }
                row
            })
            .collect()
    }

    fn cmd_manage_multicam(&mut self, args: &Value) -> Result<Value, String> {
        let parsed = match crate::mutation::validate_manage_multicam(args) {
            crate::mutation::ValidationResult::Ok(p) => p,
            crate::mutation::ValidationResult::Error(e) => return Err(e),
        };
        if let Some(create) = parsed.create {
            let created = self.multicam_create(&create)?;
            return Ok(json!({ "created": created }));
        }
        let group_id = self.require_multicam_group(
            parsed
                .ungroup_group_id
                .as_deref()
                .expect("validator guarantees a section"),
        )?;
        timeline_core::strip_group_stamps(&mut self.timeline, &group_id);
        let still_referenced = timeline_core::referenced_group_ids(
            std::iter::once(&self.timeline).chain(self.sibling_timelines.iter()),
        )
        .contains(&group_id);
        if !still_referenced {
            self.multicam_groups.retain(|g| g.id != group_id);
        }
        Ok(json!({ "ungrouped": group_id }))
    }

    /// manage_multicam.create (Swift `createSection`). Sync maps come from
    /// pinned offsets, the master (0), or shared source timecode; audio
    /// correlation is deferred (sync_clips has the correlator) — members it
    /// would have resolved land in `needsAttention` instead.
    fn multicam_create(
        &mut self,
        create: &crate::mutation::ManageMulticamCreate,
    ) -> Result<Value, String> {
        let assets = self.multicam_assets();
        for (i, m) in create.members.iter().enumerate() {
            let path = format!("create.members[{i}]");
            let Some(asset) = assets.get(&m.media_ref) else {
                return Err(format!(
                    "{path}: no media asset '{}'. get_media lists assets.",
                    m.media_ref
                ));
            };
            match m.kind {
                MulticamMemberKind::Angle => {
                    if asset.clip_type != ClipType::Video {
                        return Err(format!("{path}: angle members must be video."));
                    }
                }
                MulticamMemberKind::Mic => {
                    if !(asset.clip_type == ClipType::Audio
                        || (asset.clip_type == ClipType::Video && asset.has_audio))
                    {
                        return Err(format!("{path}: mic members need audio."));
                    }
                }
                MulticamMemberKind::Both => {
                    if !(asset.clip_type == ClipType::Video && asset.has_audio) {
                        return Err(format!("{path}: 'both' members must be video with audio."));
                    }
                }
            }
        }

        let master_ref = self.resolve_multicam_master(create)?;

        // Sync maps (Swift `syncMulticamMembers`' no-audio path): pinned →
        // locked; master → 0; else shared embedded timecode; else unresolved.
        let tc_seconds = |media_ref: &str| -> Option<f64> {
            let e = self.media_manifest.entry_for(media_ref)?;
            let frame = e.source_timecode_frame? as f64;
            let quanta = e.source_timecode_quanta.filter(|q| *q > 0)? as f64;
            Some(frame / quanta)
        };
        let mut maps: std::collections::HashMap<String, MulticamSyncMap> =
            std::collections::HashMap::new();
        let mut failures: Vec<(String, String)> = Vec::new();
        for m in &create.members {
            if let Some(pinned) = m.offset_seconds {
                maps.insert(
                    m.media_ref.clone(),
                    MulticamSyncMap {
                        offset_seconds: pinned,
                        confidence: 1.0,
                        locked: true,
                    },
                );
            } else if m.media_ref == master_ref {
                maps.insert(
                    m.media_ref.clone(),
                    MulticamSyncMap {
                        offset_seconds: 0.0,
                        confidence: 1.0,
                        locked: false,
                    },
                );
            }
        }
        let master_offset = maps.get(&master_ref).map(|m| m.offset_seconds).unwrap_or(0.0);
        let master_tc = tc_seconds(&master_ref);
        for m in &create.members {
            if maps.contains_key(&m.media_ref) {
                continue;
            }
            match (master_tc, tc_seconds(&m.media_ref)) {
                (Some(master), Some(tc)) => {
                    maps.insert(
                        m.media_ref.clone(),
                        MulticamSyncMap {
                            offset_seconds: master_offset + tc - master,
                            confidence: 1.0,
                            locked: false,
                        },
                    );
                }
                _ => {
                    maps.insert(m.media_ref.clone(), MulticamSyncMap::default());
                    failures.push((
                        m.media_ref.clone(),
                        "No audio to sync with and no shared timecode.".to_string(),
                    ));
                }
            }
        }
        // Rebase so the earliest synced member sits at the group zero.
        let base = maps
            .values()
            .filter(|m| m.confidence > 0.0 || m.locked)
            .map(|m| m.offset_seconds)
            .fold(f64::INFINITY, f64::min);
        if base.is_finite() && base != 0.0 {
            for map in maps.values_mut() {
                if map.confidence > 0.0 || map.locked {
                    map.offset_seconds -= base;
                }
            }
        }

        let specs: Vec<timeline_core::MulticamMemberSpec> = create
            .members
            .iter()
            .map(|m| timeline_core::MulticamMemberSpec {
                media_ref: m.media_ref.clone(),
                kind: m.kind,
                angle_label: m.angle_label.clone(),
                pinned_offset_seconds: m.offset_seconds,
            })
            .collect();
        let existing_names: Vec<String> =
            self.multicam_groups.iter().map(|g| g.name.clone()).collect();
        let (group, clip_ids) = timeline_core::create_group(
            &mut self.timeline,
            &specs,
            &maps,
            &master_ref,
            create.name.as_deref(),
            &existing_names,
            &assets,
            create.start_frame,
        )?;

        let mut created = json!({
            "groupId": group.id,
            "members": Self::multicam_member_rows(&group),
            "clipIds": clip_ids,
        });
        if !failures.is_empty() {
            created["needsAttention"] = json!(failures
                .iter()
                .map(|(media_ref, reason)| json!({"mediaRef": media_ref, "reason": reason}))
                .collect::<Vec<_>>());
        }
        self.multicam_groups.push(group);
        Ok(created)
    }

    /// Swift `resolveMasterRef`: explicit master by angleLabel or mediaRef
    /// (short-id expanded), else the first mic/both member.
    fn resolve_multicam_master(
        &self,
        create: &crate::mutation::ManageMulticamCreate,
    ) -> Result<String, String> {
        if let Some(master) = &create.master {
            let expanded = crate::id_short::expand_input_ids(
                &json!({ "mediaRef": master }),
                &self.id_universe(),
            )
            .ok()
            .and_then(|v| v.get("mediaRef").and_then(|m| m.as_str()).map(String::from))
            .unwrap_or_else(|| master.clone());
            let wanted = master.to_lowercase();
            let Some(spec) = create.members.iter().find(|m| {
                m.media_ref == expanded
                    || m.angle_label
                        .as_deref()
                        .is_some_and(|l| l.to_lowercase() == wanted)
            }) else {
                return Err(format!(
                    "master '{master}' doesn't match a member's angleLabel or mediaRef."
                ));
            };
            if spec.kind == MulticamMemberKind::Angle {
                return Err(format!(
                    "master '{master}' is an angle — its audio is scratch. Pick a mic/both member."
                ));
            }
            return Ok(spec.media_ref.clone());
        }
        let pick = create
            .members
            .iter()
            .find(|m| m.kind == MulticamMemberKind::Mic)
            .or_else(|| {
                create
                    .members
                    .iter()
                    .find(|m| m.kind == MulticamMemberKind::Both)
            })
            .unwrap_or(&create.members[0]);
        if pick.kind == MulticamMemberKind::Angle {
            return Err(
                "No mic member to sync against — mark one member as mic/both, or pin offsets explicitly."
                    .to_string(),
            );
        }
        Ok(pick.media_ref.clone())
    }

    fn cmd_change_cam(&mut self, args: &Value) -> Result<Value, String> {
        let parsed = match crate::mutation::validate_change_cam(args) {
            crate::mutation::ValidationResult::Ok(p) => p,
            crate::mutation::ValidationResult::Error(e) => return Err(e),
        };
        let group_id = self.resolve_multicam_group_id(args)?;
        let group = self
            .multicam_group(&group_id)
            .cloned()
            .expect("resolved above");

        let requests: Vec<timeline_core::AngleSwitchRequest> = parsed
            .entries
            .iter()
            .map(|e| match (&e.layout, &e.angle) {
                (Some(layout), _) => timeline_core::AngleSwitchRequest::with_layout(
                    e.range.0..e.range.1,
                    *layout,
                    e.angles.clone(),
                ),
                (None, Some(angle)) => {
                    timeline_core::AngleSwitchRequest::full(e.range.0..e.range.1, angle)
                }
                (None, None) => unreachable!("validator requires angle XOR layout"),
            })
            .collect();

        let assets = self.multicam_assets();
        let outcome =
            timeline_core::switch_angles(&mut self.timeline, &group, &requests, &assets)?;

        let mut extra = json!({ "groupId": group_id, "switched": outcome.switched });
        if outcome.merged > 0 {
            extra["cutsMerged"] = json!(outcome.merged);
        }
        if !outcome.overlay_clip_ids.is_empty() {
            extra["overlayClipIds"] = json!(outcome.overlay_clip_ids);
        }
        let lo = outcome.applied.iter().map(|r| r.start).min();
        let hi = outcome.applied.iter().map(|r| r.end).max();
        if let (Some(lo), Some(hi)) = (lo, hi) {
            let rows = timeline_core::multicam_program_rows(&self.timeline, &group, Some(lo..hi));
            extra["program"] = json!(rows
                .into_iter()
                .map(|(label, s, e)| json!([label, s, e]))
                .collect::<Vec<_>>());
        }
        if !outcome.clamped.is_empty() {
            extra["clamped"] = json!(outcome
                .clamped
                .iter()
                .map(|c| json!({
                    "requested": [c.requested.start, c.requested.end],
                    "applied": [c.applied.start, c.applied.end],
                    "culprit": c.culprit,
                }))
                .collect::<Vec<_>>());
        }
        if !outcome.skipped.is_empty() {
            extra["skipped"] = json!(outcome
                .skipped
                .iter()
                .map(|(r, reason)| json!({"range": [r.start, r.end], "reason": reason}))
                .collect::<Vec<_>>());
        }
        Ok(extra)
    }

    fn cmd_get_multicam(&self, args: &Value) -> Result<Value, String> {
        let group_id = self.resolve_multicam_group_id(args)?;
        let group = self
            .multicam_group(&group_id)
            .expect("resolved above");
        let start = args.get("startFrame").and_then(|v| v.as_i64());
        let end = args.get("endFrame").and_then(|v| v.as_i64());
        let window = match (start, end) {
            (None, None) => None,
            (s, e) => Some(s.unwrap_or(0)..e.unwrap_or(i64::MAX)),
        };
        let rows = timeline_core::multicam_program_rows(&self.timeline, group, window);
        let payload = json!({
            "groupId": group_id,
            "name": group.name,
            "members": Self::multicam_member_rows(group),
            "program": rows
                .into_iter()
                .map(|(label, s, e)| json!([label, s, e]))
                .collect::<Vec<Value>>(),
            "trackIndexes": timeline_core::multicam_track_indexes(&self.timeline, &group_id)
                .into_iter()
                .collect::<Vec<usize>>(),
        });
        Ok(Self::json_content(&payload))
    }

    fn cmd_set_project_settings(&mut self, args: &Value) -> Result<Value, String> {
        let fps_in = args.get("fps").and_then(Value::as_i64);
        let width_in = args.get("width").and_then(Value::as_i64);
        let height_in = args.get("height").and_then(Value::as_i64);
        let aspect_in = args.get("aspectRatio").and_then(Value::as_str);
        let quality_in = args.get("quality").and_then(Value::as_str);

        if fps_in.is_none()
            && width_in.is_none()
            && height_in.is_none()
            && aspect_in.is_none()
            && quality_in.is_none()
        {
            return Err(
                "Provide at least one of: fps, width, height, aspectRatio, quality".to_string(),
            );
        }
        if aspect_in.is_some() && (width_in.is_some() || height_in.is_some()) {
            return Err(
                "'aspectRatio' and explicit 'width'/'height' are mutually exclusive".to_string(),
            );
        }
        if let Some(fps) = fps_in {
            if !(1..=120).contains(&fps) {
                return Err(format!("fps must be between 1 and 120 (got {fps})"));
            }
        }

        let new_fps = fps_in.unwrap_or(self.timeline.fps);

        let (new_width, new_height) = if let Some(aspect) = aspect_in {
            let (base_w, base_h) = aspect_preset_dims(aspect)?;
            match quality_in {
                Some(q) => quality_resolution(q, base_w, base_h)?,
                None => (base_w, base_h),
            }
        } else if let Some(q) = quality_in {
            quality_resolution(q, self.timeline.width, self.timeline.height)?
        } else {
            (
                width_in.unwrap_or(self.timeline.width),
                height_in.unwrap_or(self.timeline.height),
            )
        };

        if new_width <= 0 || new_height <= 0 {
            return Err("Resolution must have positive width and height".to_string());
        }

        let prev_fps = self.timeline.fps;
        let prev_width = self.timeline.width;
        let prev_height = self.timeline.height;

        timeline_core::apply_settings(&mut self.timeline, new_fps, new_width, new_height, |c| {
            c.transform == Transform::default()
        });

        let mut changes: Vec<String> = Vec::new();
        if new_fps != prev_fps {
            changes.push(format!("fps {prev_fps} -> {new_fps}"));
        }
        if new_width != prev_width || new_height != prev_height {
            changes.push(format!(
                "resolution {prev_width}x{prev_height} -> {new_width}x{new_height}"
            ));
        }

        let text = if changes.is_empty() {
            format!("No change - settings already match: {new_width}x{new_height} @ {new_fps}fps")
        } else {
            format!(
                "Updated: {}. Now {new_width}x{new_height} @ {new_fps}fps.",
                changes.join(", ")
            )
        };
        Ok(json!({ "content": [{ "type": "text", "text": text }] }))
    }

    fn cmd_apply_layout(&mut self, args: &Value) -> Result<Value, String> {
        let layout_name = args
            .get("layout")
            .and_then(Value::as_str)
            .ok_or_else(|| "Missing layout".to_string())?;
        let layout = VideoLayout::from_str(layout_name).ok_or_else(|| {
            let valid: Vec<&str> = VideoLayout::ALL.iter().map(|l| l.as_str()).collect();
            format!("unknown layout '{layout_name}'. Valid: {}", valid.join(", "))
        })?;
        let fit_name = args.get("fit").and_then(Value::as_str).unwrap_or("fill");
        let fit = LayoutFit::from_str(fit_name)
            .ok_or_else(|| format!("invalid fit '{fit_name}'. Valid: fill, fit"))?;

        let slots_val = args
            .get("slots")
            .and_then(Value::as_array)
            .filter(|a| !a.is_empty())
            .ok_or_else(|| "apply_layout needs a non-empty 'slots' array".to_string())?;

        let layout_slots = layout.slots();

        // Parse + validate every slot entry before mutating anything.
        let mut seen_slots: std::collections::HashSet<String> = std::collections::HashSet::new();
        let mut seen_clips: std::collections::HashSet<String> = std::collections::HashSet::new();
        let mut uses_media = false;
        // (slot, clip_ids, media_ref, anchor). clip_ids drives re-layout mode;
        // media_ref drives place-new mode (mutually exclusive across all slots).
        let mut entries: Vec<(core_model::LayoutSlot, Vec<String>, Option<String>, (f64, f64))> =
            Vec::new();
        for (i, e) in slots_val.iter().enumerate() {
            let slot_name = e
                .get("slot")
                .and_then(Value::as_str)
                .ok_or_else(|| format!("slots[{i}]: missing 'slot'"))?;
            let slot = layout_slots
                .iter()
                .find(|s| s.id == slot_name)
                .ok_or_else(|| {
                    let ids: Vec<&str> = layout_slots.iter().map(|s| s.id).collect();
                    format!(
                        "slots[{i}]: '{slot_name}' is not a slot of '{layout_name}'. Slots: {}",
                        ids.join(", ")
                    )
                })?;
            if !seen_slots.insert(slot_name.to_string()) {
                return Err(format!("slots[{i}]: duplicate slot '{slot_name}'"));
            }
            let media_ref = e.get("mediaRef").and_then(Value::as_str);
            // Clip assignment: 'clipIds' array (batch) preferred; 'clipId' singular accepted.
            let clip_ids: Option<Vec<String>> =
                if let Some(arr) = e.get("clipIds").and_then(Value::as_array) {
                    Some(
                        arr.iter()
                            .filter_map(Value::as_str)
                            .map(str::to_string)
                            .collect(),
                    )
                } else {
                    e.get("clipId")
                        .and_then(Value::as_str)
                        .map(|c| vec![c.to_string()])
                };
            if media_ref.is_some() == clip_ids.is_some() {
                return Err(format!(
                    "slots[{i}]: provide exactly one of 'mediaRef' or 'clipIds'"
                ));
            }
            if let Some(cids) = &clip_ids {
                if cids.is_empty() {
                    return Err(format!("slots[{i}]: 'clipIds' must not be empty"));
                }
                for cid in cids {
                    if !seen_clips.insert(cid.clone()) {
                        return Err(format!(
                            "slots[{i}]: clip '{cid}' is assigned to more than one slot; each clip can fill only one."
                        ));
                    }
                }
            }
            uses_media = uses_media || media_ref.is_some();
            let anchor = resolve_layout_anchor(e).map_err(|m| format!("slots[{i}]: {m}"))?;
            entries.push((
                slot.clone(),
                clip_ids.unwrap_or_default(),
                media_ref.map(str::to_string),
                anchor,
            ));
        }

        let missing: Vec<&str> = layout_slots
            .iter()
            .map(|s| s.id)
            .filter(|id| !seen_slots.contains(*id))
            .collect();
        if !missing.is_empty() {
            return Err(format!(
                "layout '{layout_name}' needs every slot filled. Missing: {}",
                missing.join(", ")
            ));
        }
        if uses_media {
            return self.apply_layout_place_new(&entries, layout_name, fit, args);
        }

        // Re-layout co-visibility validation (before any mutation):
        //   (a) two clips in DIFFERENT slots on the SAME track must not overlap in
        //       time — only the first would render; (b) with more than one slot,
        //       some frame must have every slot playing, else no frame shows all
        //       regions. Also validates every clip exists and is video/image.
        let mut ranges_by_track: std::collections::HashMap<String, Vec<(String, i64, i64)>> =
            std::collections::HashMap::new();
        let mut intervals_by_slot: std::collections::HashMap<String, Vec<(i64, i64)>> =
            std::collections::HashMap::new();
        for (slot, clip_ids, _media, _) in &entries {
            for cid in clip_ids {
                let loc = timeline_core::find_clip(&self.timeline, cid)
                    .ok_or_else(|| format!("slot '{}': clip not found: {cid}", slot.id))?;
                let track = &self.timeline.tracks[loc.track_index];
                let clip = &track.clips[loc.clip_index];
                if !matches!(clip.media_type, ClipType::Video | ClipType::Image) {
                    return Err(format!(
                        "slot '{}': clip {cid} is {:?}; layout applies to video/image clips",
                        slot.id, clip.media_type
                    ));
                }
                let (start, end) = (clip.start_frame, clip.start_frame + clip.duration_frames);
                let track_id = track.id.clone();
                if let Some(existing) = ranges_by_track.get(&track_id) {
                    for (other_slot, o_start, o_end) in existing {
                        if other_slot != &slot.id && start < *o_end && *o_start < end {
                            return Err(format!(
                                "clips in slots '{other_slot}' and '{}' are on the same track \
                                 and their times overlap; only the first would render. Move them \
                                 to separate tracks so every region shows.",
                                slot.id
                            ));
                        }
                    }
                }
                ranges_by_track
                    .entry(track_id)
                    .or_default()
                    .push((slot.id.to_string(), start, end));
                intervals_by_slot
                    .entry(slot.id.to_string())
                    .or_default()
                    .push((start, end));
            }
        }
        if entries.len() > 1 {
            let candidates: Vec<i64> = intervals_by_slot
                .values()
                .flat_map(|ivs| ivs.iter().map(|(s, _)| *s))
                .collect();
            let coincides = candidates.iter().any(|&f| {
                intervals_by_slot
                    .values()
                    .all(|ivs| ivs.iter().any(|&(s, e)| s <= f && f < e))
            });
            if !coincides {
                return Err(
                    "the selected clips never play at the same time, so no single frame shows \
                     every region. Overlap their timeline ranges before laying them out."
                        .to_string(),
                );
            }
        }

        // Re-layout mode: set each clip's transform + crop from its slot.
        let canvas_w = self.timeline.width;
        let canvas_h = self.timeline.height;
        let mut applied: Vec<String> = Vec::new();
        for (slot, clip_ids, _media, anchor) in &entries {
            for clip_id in clip_ids {
                let loc = timeline_core::find_clip(&self.timeline, clip_id)
                    .ok_or_else(|| format!("slot '{}': clip not found: {clip_id}", slot.id))?;
                let (ti, ci) = (loc.track_index, loc.clip_index);
                let media_ref = self.timeline.tracks[ti].clips[ci].media_ref.clone();
                let entry = self.media_manifest.entry_for(&media_ref);
                let sw = entry.and_then(|e| e.source_width).unwrap_or(0);
                let sh = entry.and_then(|e| e.source_height).unwrap_or(0);
                let (transform, crop) = core_model::video_layout::layout_placement(
                    slot.rect, fit, sw, sh, canvas_w, canvas_h, anchor.0, anchor.1,
                );
                let clip = &mut self.timeline.tracks[ti].clips[ci];
                clip.transform = transform;
                clip.crop = crop;
                clip.position_track = None;
                clip.scale_track = None;
                clip.rotation_track = None;
                clip.crop_track = None;
            }
            applied.push(format!("{} -> {}", slot.id, clip_ids.join(", ")));
        }

        let _ = applied;
        Ok(json!({
            "layout": layout_name,
            "fit": fit.as_str(),
            "notes": ["Stacking follows current track order; reorder tracks if a PIP inset isn't on top."],
        }))
    }

    /// Place-new mode (#226): create a stacked video track per slot (highest z on
    /// top, since tracks[0] is the TOP layer) and place one new clip from each
    /// slot's mediaRef, auto-detecting project settings from the first video, with
    /// the layout transform/crop baked into each clip. New clips at a common
    /// start_frame/duration are inherently co-visible, so no overlap checks apply.
    fn apply_layout_place_new(
        &mut self,
        entries: &[(core_model::LayoutSlot, Vec<String>, Option<String>, (f64, f64))],
        layout_name: &str,
        fit: LayoutFit,
        args: &Value,
    ) -> Result<Value, String> {
        let start_frame = args.get("startFrame").and_then(Value::as_i64).unwrap_or(0);
        let duration_frames = args
            .get("durationFrames")
            .and_then(Value::as_i64)
            .unwrap_or(0);
        if start_frame < 0 {
            return Err(format!("startFrame must be >= 0 (got {start_frame})"));
        }
        if duration_frames < 1 {
            return Err("apply_layout placing new clips requires durationFrames >= 1.".to_string());
        }
        crate::mutation::require_frame_in_bounds(start_frame, "startFrame")?;
        crate::mutation::require_frame_in_bounds(duration_frames, "durationFrames")?;
        // resolve_placement (called per slot below) errors when BOTH durationFrames
        // and trimEndFrame are present. Reject that here, up front, so the failure
        // can't fire mid-loop after tracks are already created (exec_mut does not
        // roll back a partial mutation) — validate everything before mutating.
        if args.get("trimEndFrame").is_some() {
            return Err(
                "apply_layout place-new sizes clips with durationFrames; remove trimEndFrame."
                    .to_string(),
            );
        }

        // Validate every slot's asset exists and is video/image BEFORE mutating.
        for (slot, _clips, media_ref, _anchor) in entries {
            let mref = media_ref.as_deref().unwrap_or_default();
            match self.media_manifest.entry_for(mref).map(|e| e.r#type.clone()) {
                Some(ClipType::Video) | Some(ClipType::Image) => {}
                Some(other) => {
                    return Err(format!(
                        "slot '{}': asset {mref} is {other:?}; layout slots take video or image.",
                        slot.id
                    ))
                }
                None => return Err(format!("slot '{}': asset not found: {mref}", slot.id)),
            }
        }

        // Auto-detect project settings from the first video if not yet configured
        // (same rule as add_clips / Swift applySettingsIfNeeded).
        let mut settings_note: Option<String> = None;
        if !self.timeline.settings_configured {
            let detected = entries.iter().find_map(|(_s, _c, mref, _a)| {
                self.media_manifest
                    .entry_for(mref.as_deref().unwrap_or_default())
                    .filter(|e| e.r#type == ClipType::Video)
                    .map(|e| (e.source_fps, e.source_width, e.source_height))
            });
            if let Some((sfps, sw, sh)) = detected {
                let fps = sfps
                    .map(|f| f.round() as i64)
                    .filter(|f| (1..=120).contains(f))
                    .unwrap_or(self.timeline.fps);
                // Guard degenerate (0/negative) source dims: fall back to the current
                // canvas so the applied settings match the reported note (apply_settings
                // itself ignores a non-positive canvas).
                let width = sw.filter(|&w| w > 0).unwrap_or(self.timeline.width);
                let height = sh.filter(|&h| h > 0).unwrap_or(self.timeline.height);
                let (pf, pw, ph) = (self.timeline.fps, self.timeline.width, self.timeline.height);
                timeline_core::apply_settings(&mut self.timeline, fps, width, height, |c| {
                    c.transform == Transform::default()
                });
                if fps != pf || width != pw || height != ph {
                    settings_note = Some(format!(
                        "Set project to {width}x{height} @ {fps}fps from the first clip."
                    ));
                }
            }
        }
        let project_fps = self.timeline.fps;

        // Create a video track per slot, inserting each at index 0 in ascending z
        // order so the highest-z slot ends up on top (tracks[0] is the TOP layer).
        let mut sorted: Vec<&(core_model::LayoutSlot, Vec<String>, Option<String>, (f64, f64))> =
            entries.iter().collect();
        sorted.sort_by_key(|(s, _, _, _)| s.z);
        let mut track_id_by_slot: std::collections::HashMap<String, String> =
            std::collections::HashMap::new();
        let tracks_before = self.timeline.tracks.len();
        for (slot, _, _, _) in &sorted {
            let idx = timeline_core::insert_track_at(&mut self.timeline, 0, ClipType::Video)
                .map_err(|_| "Failed to create video track".to_string())?;
            track_id_by_slot.insert(slot.id.to_string(), self.timeline.tracks[idx].id.clone());
        }

        let canvas_w = self.timeline.width;
        let canvas_h = self.timeline.height;
        let mut applied: Vec<String> = Vec::new();
        for (slot, _clips, media_ref, anchor) in entries {
            let mref = media_ref.as_deref().unwrap_or_default();
            let entry = self.media_manifest.entry_for(mref);
            let placement = resolve_placement(entry, args, project_fps)?;
            let sw = entry.and_then(|e| e.source_width).unwrap_or(0);
            let sh = entry.and_then(|e| e.source_height).unwrap_or(0);
            let (transform, crop) = core_model::video_layout::layout_placement(
                slot.rect, fit, sw, sh, canvas_w, canvas_h, anchor.0, anchor.1,
            );
            let clip = Clip {
                id: Uuid::new_v4().to_string(),
                media_ref: mref.to_string(),
                media_type: placement.media_type.clone(),
                source_clip_type: placement.media_type,
                start_frame,
                duration_frames: placement.duration_frames,
                trim_start_frame: placement.trim_start_frame,
                trim_end_frame: placement.trim_end_frame,
                speed: 1.0,
                volume: 1.0,
                fade_in_frames: 0,
                fade_out_frames: 0,
                fade_in_interpolation: Interpolation::Linear,
                fade_out_interpolation: Interpolation::Linear,
                opacity: 1.0,
                transform,
                crop,
                link_group_id: None,
                caption_group_id: None,
                text_content: None,
                text_style: None,
                opacity_track: None,
                position_track: None,
                scale_track: None,
                rotation_track: None,
                crop_track: None,
                volume_track: None,
                effects: None,
                shape_style: None,
                stroke_progress_track: None,
                compound_timeline_id: None,
                blend_mode: Default::default(),
                chroma_key: None,
                multicam_group_id: None,
                text_animation: None,
                word_timings: None,
            };
            let Some(tid) = track_id_by_slot.get(slot.id) else {
                continue;
            };
            let Some(tidx) = self.timeline.tracks.iter().position(|t| &t.id == tid) else {
                continue;
            };
            let placed = timeline_core::place_clips(&mut self.timeline, tidx, start_frame, &[clip]);
            if let Some(pid) = placed.first() {
                applied.push(format!("{} -> {pid}", slot.id));
            }
        }

        if applied.is_empty() {
            return Err("apply_layout created no clips.".to_string());
        }
        let created = self.timeline.tracks.len() - tracks_before;
        let _ = (created, applied);
        let mut extras = json!({ "layout": layout_name, "fit": fit.as_str() });
        if let Some(n) = settings_note {
            extras["notes"] = json!([n]);
        }
        Ok(extras)
    }

    /// CLP-007/008: create a linked audio clip for each placed video-with-audio
    /// clip, on the first audio track FREE over the span the audio will occupy
    /// (so existing audio is never clobbered), or a newly created track. Returns
    /// the number of linked audio clips created.
    fn auto_link_placed_audio(&mut self, video_with_audio: &[String]) -> Result<usize, String> {
        if video_with_audio.is_empty() {
            return Ok(0);
        }
        let (mut span_start, mut span_end) = (i64::MAX, i64::MIN);
        for pid in video_with_audio {
            if let Some(loc) = timeline_core::find_clip(&self.timeline, pid) {
                let c = &self.timeline.tracks[loc.track_index].clips[loc.clip_index];
                span_start = span_start.min(c.start_frame);
                span_end = span_end.max(c.start_frame + c.duration_frames);
            }
        }
        let free = self.timeline.tracks.iter().position(|t| {
            t.r#type == ClipType::Audio
                && !t.clips.iter().any(|c| {
                    c.start_frame < span_end && c.start_frame + c.duration_frames > span_start
                })
        });
        let audio_ti = match free {
            Some(ti) => ti,
            None => {
                let at = self.timeline.tracks.len();
                timeline_core::insert_track_at(&mut self.timeline, at, ClipType::Audio)
                    .map_err(|_| "Failed to create audio track for linked audio".to_string())?
            }
        };
        Ok(timeline_core::link_audio_for_placed_clips(&mut self.timeline, video_with_audio, audio_ti).len())
    }

    fn cmd_add_clips(&mut self, args: &Value) -> Result<Value, String> {
        // v2 shape: entries: [{mediaRef, trackIndex?, startFrame, endFrame? |
        // source?}]. Every entry is validated up front; one bad entry rejects
        // the whole call with no partial state.
        let entries_val = args
            .get("entries")
            .and_then(|v| v.as_array())
            .filter(|a| !a.is_empty())
            .ok_or_else(|| "Missing entries array".to_string())?;

        struct AddEntry {
            media_ref: String,
            track_index: Option<usize>,
            start_frame: i64,
            placement_args: Value,
        }
        let mut entries: Vec<AddEntry> = Vec::with_capacity(entries_val.len());
        for (i, e) in entries_val.iter().enumerate() {
            let media_ref = e
                .get("mediaRef")
                .and_then(|v| v.as_str())
                .ok_or_else(|| format!("entries[{i}]: missing mediaRef"))?
                .to_string();
            let start_frame = e
                .get("startFrame")
                .and_then(|v| v.as_i64())
                .ok_or_else(|| format!("entries[{i}]: missing startFrame"))?;
            crate::mutation::require_frame_in_bounds(start_frame, "startFrame")
                .map_err(|err| format!("entries[{i}]: {err}"))?;
            if start_frame < 0 {
                return Err(format!("entries[{i}]: startFrame must be >= 0"));
            }
            let track_index = match e.get("trackIndex").and_then(|v| v.as_i64()) {
                Some(t) => Some(usize::try_from(t).map_err(|_| {
                    format!("entries[{i}]: trackIndex must be a non-negative track index")
                })?),
                None => None,
            };
            let end_frame = e.get("endFrame").and_then(|v| v.as_i64());
            let source = e.get("source").and_then(|v| v.as_array());
            if end_frame.is_some() && source.is_some() {
                return Err(format!(
                    "entries[{i}]: endFrame and source are mutually exclusive"
                ));
            }
            // The trim/duration resolution below reuses resolve_placement's
            // #236 symmetric-trim shape (project fps is fixed after settings
            // auto-detect, so seconds convert there, not here).
            let placement_args = if let Some(end) = end_frame {
                if end <= start_frame {
                    return Err(format!(
                        "entries[{i}]: endFrame {end} must be > startFrame {start_frame}"
                    ));
                }
                json!({ "durationFrames": end - start_frame })
            } else if let Some(pair) = source {
                let s = pair.first().and_then(|v| v.as_f64());
                let e2 = pair.get(1).and_then(|v| v.as_f64());
                let (Some(s), Some(e2)) = (s, e2) else {
                    return Err(format!(
                        "entries[{i}]: source must be [startSeconds, endSeconds]"
                    ));
                };
                if e2 <= s || s < 0.0 {
                    return Err(format!(
                        "entries[{i}]: source must satisfy 0 <= startSeconds < endSeconds"
                    ));
                }
                json!({ "sourceStartSeconds": s, "sourceEndSeconds": e2 })
            } else {
                // Frame-exact trims (the #236 symmetric-trim shape) still
                // resolve when passed per entry.
                let mut pa = serde_json::Map::new();
                for key in ["trimStartFrame", "trimEndFrame", "durationFrames"] {
                    if let Some(v) = e.get(key) {
                        pa.insert(key.into(), v.clone());
                    }
                }
                Value::Object(pa)
            };
            entries.push(AddEntry {
                media_ref,
                track_index,
                start_frame,
                placement_args,
            });
        }

        // v2: trackIndex must be set on every entry or on none.
        let with_track = entries.iter().filter(|e| e.track_index.is_some()).count();
        if with_track != 0 && with_track != entries.len() {
            return Err(
                "Set trackIndex on every entry or on none — mixing is rejected; split into two calls."
                    .to_string(),
            );
        }
        let auto_tracks = with_track == 0;

        // Auto-detect project settings from the first video the FIRST time clips are
        // added (Swift `checkProjectSettings`): silently adopt its fps/size and mark
        // the project configured. Later adds see it configured, keep settings fixed,
        // and only warn on a source-fps mismatch (#233). Runs before `project_fps` so
        // the new clips are placed on the detected timebase.
        let mut settings_note: Option<String> = None;
        if !self.timeline.settings_configured {
            let detected = entries.iter().find_map(|e| {
                self.media_manifest
                    .entry_for(&e.media_ref)
                    .filter(|e| e.r#type == ClipType::Video)
                    .map(|e| (e.source_fps, e.source_width, e.source_height))
            });
            if let Some((sfps, sw, sh)) = detected {
                let fps = sfps
                    .map(|f| f.round() as i64)
                    .filter(|f| (1..=120).contains(f))
                    .unwrap_or(self.timeline.fps);
                // Guard degenerate (0/negative) source dims: fall back to the current
                // canvas so the applied settings match the reported note (apply_settings
                // itself ignores a non-positive canvas).
                let width = sw.filter(|&w| w > 0).unwrap_or(self.timeline.width);
                let height = sh.filter(|&h| h > 0).unwrap_or(self.timeline.height);
                let (pf, pw, ph) = (self.timeline.fps, self.timeline.width, self.timeline.height);
                timeline_core::apply_settings(&mut self.timeline, fps, width, height, |c| {
                    c.transform == Transform::default()
                });
                if fps != pf || width != pw || height != ph {
                    settings_note =
                        Some(format!("Set project to {width}x{height} @ {fps}fps from the first clip."));
                }
            }
        }

        let project_fps = self.timeline.fps;
        let mut warnings: Vec<String> = Vec::new();
        // (clip(s) to place, explicit target track) — nest carriers can expand
        // one entry into a video + linked audio pair.
        let mut placements: Vec<(Vec<Clip>, Option<usize>)> = Vec::with_capacity(entries.len());
        for entry_in in &entries {
            let media_id = &entry_in.media_ref;
            // NESTING (#255): a mediaRef that is a sibling timeline's id places a
            // live nested clip (sequence carrier) + a linked audio carrier when
            // the child has audio. Cycles and empty timelines are rejected.
            if self.media_manifest.entry_for(media_id).is_none() {
                if let Some(child) = self
                    .sibling_timelines
                    .iter()
                    .find(|t| t.id == *media_id)
                    .cloned()
                {
                    // source-seconds windows convert to child frames here
                    // (nest_window reads trimStartFrame/durationFrames).
                    let src_s = entry_in.placement_args.get("sourceStartSeconds").and_then(Value::as_f64);
                    let src_e = entry_in.placement_args.get("sourceEndSeconds").and_then(Value::as_f64);
                    let nest_args = if let (Some(s), Some(e)) = (src_s, src_e) {
                        json!({
                            "trimStartFrame": ((s * project_fps as f64).round() as i64).max(0),
                            "durationFrames": (((e - s) * project_fps as f64).round() as i64).max(1),
                        })
                    } else {
                        entry_in.placement_args.clone()
                    };
                    let mut carriers = self.nest_carrier_clips(&child, &nest_args)?;
                    for c in &mut carriers {
                        c.start_frame = entry_in.start_frame;
                    }
                    placements.push((carriers, entry_in.track_index));
                    continue;
                }
            }
            let entry = self.media_manifest.entry_for(media_id);
            let placement = resolve_placement(entry, &entry_in.placement_args, project_fps)?;
            if let Some(warning) = placement.fps_warning {
                if !warnings.contains(&warning) {
                    warnings.push(warning);
                }
            }
            placements.push((vec![Clip {
                id: Uuid::new_v4().to_string(),
                media_ref: media_id.clone(),
                media_type: placement.media_type.clone(),
                source_clip_type: placement.media_type,
                start_frame: entry_in.start_frame,
                duration_frames: placement.duration_frames,
                trim_start_frame: placement.trim_start_frame,
                trim_end_frame: placement.trim_end_frame,
                speed: 1.0,
                volume: 1.0,
                fade_in_frames: 0,
                fade_out_frames: 0,
                fade_in_interpolation: Interpolation::Linear,
                fade_out_interpolation: Interpolation::Linear,
                opacity: 1.0,
                transform: Transform::default(),
                crop: core_model::Crop::default(),
                link_group_id: None,
                caption_group_id: None,
                text_content: None,
                text_style: None,
                opacity_track: None,
                position_track: None,
                scale_track: None,
                rotation_track: None,
                crop_track: None,
                volume_track: None,
                effects: None,
                shape_style: None,
                stroke_progress_track: None,
                compound_timeline_id: None,
                blend_mode: Default::default(),
                chroma_key: None,
                multicam_group_id: None,
                text_animation: None,
                word_timings: None,
            }], entry_in.track_index));
        }

        // Validate every explicit target track before mutating anything.
        // Only the entry's PRIMARY clip must match its track — a nest's
        // implicit audio carrier routes to an audio track automatically.
        if !auto_tracks {
            for (clips, track_index) in &placements {
                let ti = track_index.expect("with_track == entries.len()");
                let track = self
                    .timeline
                    .tracks
                    .get(ti)
                    .ok_or_else(|| format!("Track index {ti} out of bounds"))?;
                if let Some(clip) = clips.first() {
                    if !track.is_compatible_with(clip.media_type) {
                        return Err(format!(
                            "media type {:?} is not compatible with track {ti} ({:?})",
                            clip.media_type, track.r#type
                        ));
                    }
                }
            }
        }

        // Place each clip at its own startFrame; same-track overlap is
        // resolved by place_clips (trim/split/remove — the UI's drag-onto-track
        // overwrite, #124). Auto mode shares one video track for visual
        // entries and one audio track for audio entries (created if absent).
        let mut placed_visual_ids: Vec<String> = Vec::new();
        let mut auto_video_track: Option<usize> = None;
        let mut auto_audio_track: Option<usize> = None;
        for (clips, track_index) in placements {
            for clip in clips {
                // An explicit track only takes type-compatible clips; a nest's
                // audio carrier falls through to the shared audio track.
                let explicit = track_index.filter(|ti| {
                    self.timeline
                        .tracks
                        .get(*ti)
                        .is_some_and(|t| t.is_compatible_with(clip.media_type))
                });
                let ti = match explicit {
                    Some(ti) => ti,
                    None if clip.media_type.is_visual() => match auto_video_track {
                        Some(ti) => ti,
                        None => {
                            let ti = match self
                                .timeline
                                .tracks
                                .iter()
                                .position(|t| t.r#type != ClipType::Audio)
                            {
                                Some(ti) => ti,
                                None => {
                                    let at = self.timeline.tracks.len();
                                    timeline_core::insert_track_at(
                                        &mut self.timeline,
                                        at,
                                        ClipType::Video,
                                    )
                                    .map_err(|_| "Failed to create video track".to_string())?
                                }
                            };
                            auto_video_track = Some(ti);
                            ti
                        }
                    },
                    None => match auto_audio_track {
                        Some(ti) => ti,
                        None => {
                            let ti = match self
                                .timeline
                                .tracks
                                .iter()
                                .position(|t| t.r#type == ClipType::Audio)
                            {
                                Some(ti) => ti,
                                None => {
                                    let at = self.timeline.tracks.len();
                                    timeline_core::insert_track_at(
                                        &mut self.timeline,
                                        at,
                                        ClipType::Audio,
                                    )
                                    .map_err(|_| "Failed to create audio track".to_string())?
                                }
                            };
                            auto_audio_track = Some(ti);
                            ti
                        }
                    },
                };
                let is_visual = clip.media_type.is_visual();
                let start = clip.start_frame;
                let placed =
                    timeline_core::place_clips(&mut self.timeline, ti, start, &[clip]);
                if is_visual {
                    placed_visual_ids.extend(placed);
                }
            }
        }

        // CLP-007/008: auto-link video-with-audio. Detect from each placed visual
        // clip's own media_ref (works whichever placement path ran).
        let video_with_audio: Vec<String> = placed_visual_ids
            .iter()
            .filter(|pid| {
                let Some(loc) = timeline_core::find_clip(&self.timeline, pid) else {
                    return false;
                };
                let c = &self.timeline.tracks[loc.track_index].clips[loc.clip_index];
                c.media_type == ClipType::Video
                    && self
                        .media_manifest
                        .entry_for(&c.media_ref)
                        .and_then(|e| e.has_audio)
                        == Some(true)
            })
            .cloned()
            .collect();
        let _linked_audio_count = self.auto_link_placed_audio(&video_with_audio)?;

        let mut notes: Vec<String> = Vec::new();
        if let Some(note) = settings_note {
            notes.push(note);
        }
        notes.extend(warnings);
        let mut extras = json!({});
        if !notes.is_empty() {
            extras["notes"] = json!(notes);
        }
        Ok(extras)
    }

    fn cmd_insert_clips(&mut self, args: &Value) -> Result<Value, String> {
        // v2 shape: trackIndex + atFrame + entries: [{mediaRef, source? |
        // durationFrames?}], laid end-to-end from atFrame.
        let entries_val = args
            .get("entries")
            .and_then(|v| v.as_array())
            .filter(|a| !a.is_empty())
            .ok_or_else(|| "Missing entries array".to_string())?;
        let mut media_ids: Vec<String> = Vec::with_capacity(entries_val.len());
        let mut entry_args: Vec<Value> = Vec::with_capacity(entries_val.len());
        for (i, e) in entries_val.iter().enumerate() {
            let media_ref = e
                .get("mediaRef")
                .and_then(|v| v.as_str())
                .ok_or_else(|| format!("entries[{i}]: missing mediaRef"))?;
            let duration = e.get("durationFrames").and_then(|v| v.as_i64());
            let source = e.get("source").and_then(|v| v.as_array());
            if duration.is_some() && source.is_some() {
                return Err(format!(
                    "entries[{i}]: durationFrames and source are mutually exclusive"
                ));
            }
            let pa = if let Some(pair) = source {
                let s = pair.first().and_then(|v| v.as_f64());
                let e2 = pair.get(1).and_then(|v| v.as_f64());
                let (Some(s), Some(e2)) = (s, e2) else {
                    return Err(format!(
                        "entries[{i}]: source must be [startSeconds, endSeconds]"
                    ));
                };
                if e2 <= s || s < 0.0 {
                    return Err(format!(
                        "entries[{i}]: source must satisfy 0 <= startSeconds < endSeconds"
                    ));
                }
                json!({ "sourceStartSeconds": s, "sourceEndSeconds": e2 })
            } else {
                if let Some(d) = duration {
                    if d < 1 {
                        return Err(format!("entries[{i}]: durationFrames must be >= 1"));
                    }
                }
                // Frame-exact trims (the #236 symmetric-trim shape) still
                // resolve when passed per entry.
                let mut pa = serde_json::Map::new();
                for key in ["trimStartFrame", "trimEndFrame", "durationFrames"] {
                    if let Some(v) = e.get(key) {
                        pa.insert(key.into(), v.clone());
                    }
                }
                Value::Object(pa)
            };
            media_ids.push(media_ref.to_string());
            entry_args.push(pa);
        }

        let track_index = args
            .get("trackIndex")
            .and_then(|v| v.as_i64())
            .ok_or_else(|| "Missing trackIndex".to_string())? as usize;

        let frame = args
            .get("atFrame")
            .and_then(|v| v.as_i64())
            .ok_or_else(|| "Missing atFrame".to_string())?;

        if track_index >= self.timeline.tracks.len() {
            return Err(format!("Track index {track_index} out of bounds"));
        }

        let project_fps = self.timeline.fps;
        let mut warnings: Vec<String> = Vec::new();
        let mut clip_specs: Vec<timeline_core::RippleInsertClipSpec> =
            Vec::with_capacity(media_ids.len());
        for (media_id, e_args) in media_ids.iter().zip(entry_args.iter()) {
            // NESTING (#255): as in add_clips, mediaRef may be a sibling
            // timeline's id — splice it in as a sequence carrier.
            if self.media_manifest.entry_for(media_id).is_none() {
                if let Some(child) = self
                    .sibling_timelines
                    .iter()
                    .find(|t| t.id == *media_id)
                    .cloned()
                {
                    let src_s = e_args.get("sourceStartSeconds").and_then(Value::as_f64);
                    let src_e = e_args.get("sourceEndSeconds").and_then(Value::as_f64);
                    let nest_args = if let (Some(s), Some(e2)) = (src_s, src_e) {
                        json!({
                            "trimStartFrame": ((s * project_fps as f64).round() as i64).max(0),
                            "durationFrames": (((e2 - s) * project_fps as f64).round() as i64).max(1),
                        })
                    } else {
                        e_args.clone()
                    };
                    let (trim_start, duration) = self.nest_window(&child, &nest_args)?;
                    clip_specs.push(timeline_core::RippleInsertClipSpec {
                        asset_id: media_id.clone(),
                        duration_frames: duration,
                        trim_start_frame: Some(trim_start),
                        trim_end_frame: Some(0),
                    });
                    continue;
                }
            }
            let entry = self.media_manifest.entry_for(media_id);
            let placement = resolve_placement(entry, e_args, project_fps)?;
            if let Some(warning) = placement.fps_warning {
                if !warnings.contains(&warning) {
                    warnings.push(warning);
                }
            }
            clip_specs.push(timeline_core::RippleInsertClipSpec {
                asset_id: media_id.clone(),
                duration_frames: placement.duration_frames,
                trim_start_frame: Some(placement.trim_start_frame),
                trim_end_frame: Some(placement.trim_end_frame),
            });
        }

        // Reject type-incompatible placement before mutating anything.
        {
            let track = &self.timeline.tracks[track_index];
            for media_id in &media_ids {
                let media_type = self
                    .media_manifest
                    .entry_for(media_id)
                    .map(|e| e.r#type)
                    .unwrap_or_else(|| {
                        if self.sibling_timelines.iter().any(|t| t.id == *media_id) {
                            ClipType::Sequence
                        } else {
                            ClipType::Video
                        }
                    });
                if !track.is_compatible_with(media_type) {
                    return Err(format!(
                        "media type {media_type:?} is not compatible with track {track_index} ({:?})",
                        track.r#type
                    ));
                }
            }
        }

        // CLP-007/008/RPL-010: a video-with-audio inserted on a video track gets a
        // linked audio clip. The audio track is pushed with the video (so room opens
        // at the insert frame) via linked_audio_track_index. When no audio track
        // exists we target the future end index — compute skips pushing an
        // out-of-range track (a new empty one needs no push) — and create it only on
        // success, so a refusal leaves no orphan track.
        let has_linked_audio: Vec<bool> = media_ids
            .iter()
            .map(|mid| {
                if let Some(entry) = self.media_manifest.entry_for(mid) {
                    return entry.r#type == ClipType::Video && entry.has_audio == Some(true);
                }
                // A nested timeline with audio clips gets a linked audio carrier.
                self.sibling_timelines
                    .iter()
                    .find(|t| t.id == *mid)
                    .map(|t| {
                        t.tracks
                            .iter()
                            .any(|tr| tr.r#type == ClipType::Audio && !tr.clips.is_empty())
                    })
                    .unwrap_or(false)
            })
            .collect();
        let existing_audio_ti = self
            .timeline
            .tracks
            .iter()
            .position(|t| t.r#type == ClipType::Audio);
        let linked_audio_ti = if has_linked_audio.iter().any(|&b| b) {
            Some(existing_audio_ti.unwrap_or(self.timeline.tracks.len()))
        } else {
            None
        };

        let config = timeline_core::RippleInsertConfig {
            track_index,
            insert_frame: frame,
            clips: clip_specs,
            linked_audio_track_index: linked_audio_ti,
        };

        let outcome = timeline_core::compute_ripple_insert_with_split(&self.timeline, config);

        match outcome {
            timeline_core::RippleInsertWithSplitOutcome::Ok(plan) => {
                // Apply split actions before shifting.
                for (_, clip_id, split_at) in &plan.split_actions {
                    timeline_core::split_clip(&mut self.timeline, clip_id, *split_at);
                }
                // Shift POSITIONALLY, not by clip id: splitting a straddling clip
                // creates a fresh right-half id the pre-computed shift list cannot
                // reference, so a by-id apply left it un-shifted and place_clips then
                // trimmed (destroyed) its tail. Push every clip at/after the insert
                // frame by total_push on each pushed track (matches the library
                // apply_ripple_insert_with_split).
                let push_tracks: std::collections::BTreeSet<usize> =
                    plan.insert.push_track_indices.iter().copied().collect();
                let insert_frame = plan.insert.insert_frame;
                let total_push = plan.insert.total_push;
                for ti in 0..self.timeline.tracks.len() {
                    if !push_tracks.contains(&ti) {
                        continue;
                    }
                    for clip in &mut self.timeline.tracks[ti].clips {
                        if clip.start_frame >= insert_frame {
                            clip.start_frame += total_push;
                        }
                    }
                    timeline_core::sort_clips_on_track(&mut self.timeline, ti);
                }
                // Place new clips
                let new_clips: Vec<Clip> = plan
                    .insert
                    .clips
                    .iter()
                    .map(|spec| {
                        let media_type = self
                            .media_manifest
                            .entry_for(&spec.asset_id)
                            .map(|e| e.r#type.clone())
                            .unwrap_or_else(|| {
                                if self.sibling_timelines.iter().any(|t| t.id == spec.asset_id) {
                                    ClipType::Sequence
                                } else {
                                    ClipType::Video
                                }
                            });
                        Clip {
                        id: Uuid::new_v4().to_string(),
                        media_ref: spec.asset_id.clone(),
                        media_type: media_type.clone(),
                        source_clip_type: media_type,
                        start_frame: plan.insert.insert_frame,
                        duration_frames: spec.duration_frames,
                        trim_start_frame: spec.trim_start_frame.unwrap_or(0),
                        trim_end_frame: spec.trim_end_frame.unwrap_or(0),
                        speed: 1.0,
                        volume: 1.0,
                        fade_in_frames: 0,
                        fade_out_frames: 0,
                        fade_in_interpolation: Interpolation::Linear,
                        fade_out_interpolation: Interpolation::Linear,
                        opacity: 1.0,
                        transform: Transform::default(),
                        crop: core_model::Crop::default(),
                        link_group_id: None,
                        caption_group_id: None,
                        text_content: None,
                        text_style: None,
                        opacity_track: None,
                        position_track: None,
                        scale_track: None,
                        rotation_track: None,
                        crop_track: None,
                        volume_track: None,
                        effects: None,
                        shape_style: None,
                        stroke_progress_track: None,
                        compound_timeline_id: None,
                        blend_mode: Default::default(),
                        chroma_key: None,
                        multicam_group_id: None,
                        text_animation: None,
                        word_timings: None,
                        }
                    })
                    .collect();

                let placed = timeline_core::place_clips(
                    &mut self.timeline,
                    plan.insert.track_index,
                    plan.insert.insert_frame,
                    &new_clips,
                );

                // Link audio for the placed video-with-audio clips. The audio track
                // has already had room opened by the ripple push (or is a freshly
                // created empty track), so link_audio_for_placed_clips places into a
                // clear region. `placed` is 1:1 with media_ids/has_linked_audio.
                let mut linked_audio_count = 0usize;
                if linked_audio_ti.is_some() {
                    let video_with_audio_ids: Vec<String> = has_linked_audio
                        .iter()
                        .zip(placed.iter())
                        .filter_map(|(&b, pid)| b.then(|| pid.clone()))
                        .collect();
                    if !video_with_audio_ids.is_empty() {
                        let audio_ti = match existing_audio_ti {
                            Some(ti) => ti,
                            None => {
                                let at = self.timeline.tracks.len();
                                timeline_core::insert_track_at(
                                    &mut self.timeline,
                                    at,
                                    ClipType::Audio,
                                )
                                .map_err(|_| {
                                    "Failed to create audio track for linked audio".to_string()
                                })?
                            }
                        };
                        linked_audio_count = timeline_core::link_audio_for_placed_clips(
                            &mut self.timeline,
                            &video_with_audio_ids,
                            audio_ti,
                        )
                        .len();
                    }
                }

                let _ = (placed, linked_audio_count);
                let mut extras = json!({});
                if !warnings.is_empty() {
                    extras["notes"] = json!(warnings);
                }
                Ok(extras)
            }
            timeline_core::RippleInsertWithSplitOutcome::Refused(msg) => {
                Err(format!("Insert refused: {msg}"))
            }
        }
    }

    fn cmd_undo(&mut self) -> Result<Value, String> {
        match self.undo_stack.undo() {
            Ok(timeline) => {
                self.timeline = timeline;
                Ok(json!({
                    "content": [{
                        "type": "text",
                        "text": "Undo successful".to_string()
                    }]
                }))
            }
            Err(_) => Ok(json!({
                "content": [{
                    "type": "text",
                    "text": "Nothing to undo".to_string()
                }],
                "isError": true,
            })),
        }
    }

    fn cmd_redo(&mut self) -> Result<Value, String> {
        match self.undo_stack.redo() {
            Ok(timeline) => {
                self.timeline = timeline;
                Ok(json!({
                    "content": [{
                        "type": "text",
                        "text": "Redo successful".to_string()
                    }]
                }))
            }
            Err(_) => Ok(json!({
                "content": [{
                    "type": "text",
                    "text": "Nothing to redo".to_string()
                }],
                "isError": true,
            })),
        }
    }

    // ── Media read-only tools ──────────────────────────────────────────────

    /// GET_MEDIA v2 (absorbs list_folders): the library inventory — assets
    /// with content hints, plus folders (as paths) and timelines on
    /// unfiltered reads. Filters: ids (placeholder polling), folder (path,
    /// includes subfolders), pending (unresolved generations only).
    fn cmd_get_media(&self, args: &Value) -> Result<Value, String> {
        let ids_filter: Option<Vec<String>> = args.get("ids").and_then(|v| v.as_array()).map(|a| {
            a.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        });
        let folder_filter = args.get("folder").and_then(|v| v.as_str());
        let pending = args
            .get("pending")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        let filtered = ids_filter.is_some() || folder_filter.is_some() || pending;

        // Folder path filter: the named folder plus every descendant.
        let folder_scope: Option<std::collections::HashSet<String>> = match folder_filter {
            Some(path) => {
                let folders = &self.media_manifest.folders;
                let root = crate::organize::resolve_folder(folders, path)
                    .map_err(|e| format!("get_media: {e}"))?
                    .ok_or_else(|| format!("get_media: folder '{path}' not found"))?;
                let mut scope: std::collections::HashSet<String> = Default::default();
                scope.insert(root.clone());
                let mut grew = true;
                while grew {
                    grew = false;
                    for f in folders {
                        if scope.contains(&f.id) {
                            continue;
                        }
                        if f
                            .parent_folder_id
                            .as_ref()
                            .is_some_and(|pid| scope.contains(pid))
                        {
                            scope.insert(f.id.clone());
                            grew = true;
                        }
                    }
                }
                Some(scope)
            }
            None => None,
        };

        let assets: Vec<Value> = self
            .media_manifest
            .entries
            .iter()
            .filter(|e| {
                ids_filter
                    .as_ref()
                    .is_none_or(|ids| ids.iter().any(|id| id == &e.id))
            })
            .filter(|e| {
                folder_scope
                    .as_ref()
                    .is_none_or(|scope| e.folder_id.as_ref().is_some_and(|f| scope.contains(f)))
            })
            .filter(|e| {
                !pending
                    || e.generation_status
                        .as_deref()
                        .is_some_and(|st| !st.is_empty() && st != "none")
            })
            .map(|e| {
                let mut a = serde_json::Map::new();
                a.insert("id".into(), json!(e.id));
                a.insert("name".into(), json!(e.name));
                a.insert("type".into(), json!(e.r#type.name()));
                if e.duration > 0.0 {
                    a.insert(
                        "durationSeconds".into(),
                        json!(crate::timeline_v2::round3(e.duration)),
                    );
                }
                if let Some(w) = e.source_width {
                    a.insert("width".into(), json!(w));
                }
                if let Some(h) = e.source_height {
                    a.insert("height".into(), json!(h));
                }
                if let Some(fps) = e.source_fps {
                    a.insert("fps".into(), json!(crate::timeline_v2::round3(fps)));
                }
                if e.has_audio == Some(true) {
                    a.insert("hasAudio".into(), json!(true));
                }
                if let Some(fid) = &e.folder_id {
                    a.insert(
                        "folder".into(),
                        json!(crate::organize::folder_path(&self.media_manifest.folders, fid)),
                    );
                }
                if let Some(gi) = &e.generation_input {
                    a.insert("prompt".into(), json!(gi.prompt));
                }
                if let Some(st) = e
                    .generation_status
                    .as_deref()
                    .filter(|st| !st.is_empty() && *st != "none")
                {
                    a.insert("generationStatus".into(), json!(st));
                }
                Value::Object(a)
            })
            .collect();

        let mut out = serde_json::Map::new();
        out.insert("assets".into(), json!(assets));
        if !filtered {
            let folder_paths: Vec<String> = self
                .media_manifest
                .folders
                .iter()
                .map(|f| crate::organize::folder_path(&self.media_manifest.folders, &f.id))
                .collect();
            if !folder_paths.is_empty() {
                out.insert("folders".into(), json!(folder_paths));
            }
            let mut timelines = vec![json!({
                "timelineId": self.timeline.id, "name": self.timeline.name, "active": true
            })];
            for t in &self.sibling_timelines {
                timelines.push(json!({"timelineId": t.id, "name": t.name}));
            }
            out.insert("timelines".into(), json!(timelines));
        }
        Ok(Self::json_content(&Value::Object(out)))
    }

    fn cmd_search_media(&self, args: &Value) -> Result<Value, String> {
        let query = args.get("query").and_then(|v| v.as_str()).unwrap_or("");
        let r#type = args.get("type").and_then(|v| v.as_str());
        let limit = args
            .get("limit")
            .and_then(|v| v.as_u64())
            .map(|v| v as usize)
            .unwrap_or(50);

        let results: Vec<&MediaManifestEntry> = self
            .media_manifest
            .entries
            .iter()
            .filter(|e| {
                let name_match =
                    query.is_empty() || e.name.to_lowercase().contains(&query.to_lowercase());
                let type_match = r#type.map_or(true, |t| {
                    let t_lower = t.to_lowercase();
                    let type_str = format!("{:?}", e.r#type).to_lowercase();
                    type_str == t_lower
                });
                name_match && type_match
            })
            .collect();

        // Convert to SearchHitInfo for the files group (name-based search).
        let files: Vec<SearchHitInfo> = results
            .iter()
            .map(|e| SearchHitInfo {
                media_id: e.id.clone(),
                frame: 0,
                score: 1.0,
                kind: "File".to_string(),
            })
            .collect();

        // READ-026: Include search indexing status in output.
        let status = if results.is_empty() && self.search_status.is_empty() {
            "ok".to_string()
        } else if !self.search_status.is_empty() {
            if results.is_empty() {
                self.search_status.clone()
            } else {
                format!("Found {} media; {}", results.len(), self.search_status)
            }
        } else {
            format!("Found {} media", results.len())
        };

        let output = format_search_results(Vec::new(), Vec::new(), files, status, limit);
        let output_json = serde_json::to_string_pretty(&output).unwrap_or_default();

        Ok(json!({
            "content": [{
                "type": "text",
                "text": output_json
            }]
        }))
    }

    fn cmd_list_models(&self, args: &Value) -> Result<Value, String> {
        let filter = args.get("type").and_then(|v| v.as_str());
        let is_paid = self.is_paid_account();
        let models: Vec<Value> = model_catalog::catalog()
            .iter()
            .filter(|m| filter.is_none() || filter == Some(m.kind_str()))
            .map(|m| Self::model_entry_json(m, is_paid))
            .collect();
        let body = json!({ "models": models, "loaded": true });
        Ok(json!({
            "content": [{
                "type": "text",
                "text": serde_json::to_string_pretty(&body).unwrap_or_default()
            }]
        }))
    }

    /// Human-readable labels parallel to a model's raw aspect-ratio ids
    /// (upstream #284): "landscape_16_9" → "Landscape 16:9". Colon-form ids
    /// pass through unchanged.
    fn aspect_ratio_labels(ids: &[&str]) -> Vec<String> {
        ids.iter()
            .map(|id| model_catalog::aspect_ratio_display_label(id))
            .collect()
    }

    /// One list_models entry (mirrors Swift's videoModelInfo/imageModelInfo/
    /// audioModelInfo fields), plus #249 gating: paid-only models on a free plan
    /// are marked unavailable with an upgrade hint rather than hidden.
    fn model_entry_json(m: &model_catalog::ModelConfig, is_paid: bool) -> Value {
        let mut info = json!({
            "id": m.id,
            "displayName": m.display_name,
            "type": m.kind_str(),
        });
        let obj = info.as_object_mut().unwrap();
        match &m.caps {
            model_catalog::ModelCaps::Video(c) => {
                obj.insert("durations".into(), json!(c.durations));
                obj.insert("aspectRatios".into(), json!(c.aspect_ratios));
                obj.insert(
                    "aspectRatioLabels".into(),
                    json!(Self::aspect_ratio_labels(&c.aspect_ratios)),
                );
                obj.insert("supportsFirstFrame".into(), json!(c.supports_first_frame));
                obj.insert("supportsLastFrame".into(), json!(c.supports_last_frame));
                obj.insert("supportsReferences".into(), json!(c.supports_references()));
                if let Some(r) = &c.resolutions {
                    obj.insert("resolutions".into(), json!(r));
                }
                if c.supports_references() {
                    if c.max_reference_images > 0 {
                        obj.insert("maxReferenceImages".into(), json!(c.max_reference_images));
                    }
                    if c.max_reference_videos > 0 {
                        obj.insert("maxReferenceVideos".into(), json!(c.max_reference_videos));
                    }
                    if c.max_reference_audios > 0 {
                        obj.insert("maxReferenceAudios".into(), json!(c.max_reference_audios));
                    }
                    if let Some(total) = c.max_total_references {
                        obj.insert("maxTotalReferences".into(), json!(total));
                    }
                    if let Some(s) = c.max_combined_video_ref_seconds {
                        obj.insert("maxCombinedVideoRefSeconds".into(), json!(s as i64));
                    }
                    if let Some(s) = c.max_combined_audio_ref_seconds {
                        obj.insert("maxCombinedAudioRefSeconds".into(), json!(s as i64));
                    }
                    if c.frames_and_references_exclusive {
                        obj.insert("framesAndReferencesExclusive".into(), json!(true));
                    }
                    obj.insert("referenceTagNoun".into(), json!(c.reference_tag_noun));
                }
                if c.requires_source_video {
                    obj.insert("requiresSourceVideo".into(), json!(true));
                }
            }
            model_catalog::ModelCaps::Image(c) => {
                obj.insert("aspectRatios".into(), json!(c.aspect_ratios));
                obj.insert(
                    "aspectRatioLabels".into(),
                    json!(Self::aspect_ratio_labels(&c.aspect_ratios)),
                );
                obj.insert(
                    "supportsImageReference".into(),
                    json!(c.supports_image_reference),
                );
                if let Some(r) = &c.resolutions {
                    obj.insert("resolutions".into(), json!(r));
                }
                if let Some(q) = &c.qualities {
                    obj.insert("qualities".into(), json!(q));
                }
            }
            model_catalog::ModelCaps::Audio(c) => {
                obj.insert("category".into(), json!(c.category.as_str()));
                obj.insert("minPromptLength".into(), json!(c.min_prompt_length));
                obj.insert("supportsLyrics".into(), json!(c.supports_lyrics));
                obj.insert(
                    "supportsInstrumental".into(),
                    json!(c.supports_instrumental),
                );
                obj.insert(
                    "supportsStyleInstructions".into(),
                    json!(c.supports_style_instructions),
                );
                if let Some(voices) = &c.voices {
                    let sample: Vec<&str> = voices.iter().take(3).copied().collect();
                    obj.insert("voicesSample".into(), json!(sample));
                    obj.insert("voiceCount".into(), json!(voices.len()));
                }
                if let Some(v) = c.default_voice {
                    obj.insert("defaultVoice".into(), json!(v));
                }
                if let Some(d) = &c.durations {
                    obj.insert("durations".into(), json!(d));
                }
            }
        }
        let available = model_catalog::model_available(is_paid, m.paid_only);
        obj.insert("available".into(), json!(available));
        if m.paid_only {
            obj.insert("paidOnly".into(), json!(true));
        }
        if !available {
            obj.insert(
                "upgrade".into(),
                json!("Requires a paid plan. Tell the user to subscribe."),
            );
        }
        info
    }

    /// Resolve a generate-tool model: named id (must exist in `kind`) or the
    /// first plan-available model, then apply #249 gating.
    fn resolve_generation_model(
        &self,
        kind: generation_core::ModelKind,
        requested: Option<&str>,
    ) -> Result<&'static model_catalog::ModelConfig, String> {
        let model = match requested {
            Some(id) => {
                let same_kind = || {
                    model_catalog::catalog()
                        .iter()
                        .filter(|m| m.kind() == kind)
                        .map(|m| m.id)
                        .collect::<Vec<_>>()
                        .join(", ")
                };
                model_catalog::model_by_id(id)
                    .filter(|m| m.kind() == kind)
                    .ok_or_else(|| format!("Unknown model '{}'. Available: {}", id, same_kind()))?
            }
            None => model_catalog::default_model(kind, self.is_paid_account())?,
        };
        self.gate_model(model)?;
        Ok(model)
    }

    /// #249: reject paid-only models on a free plan with the require-plan message.
    fn gate_model(&self, m: &model_catalog::ModelConfig) -> Result<(), String> {
        if model_catalog::model_available(self.is_paid_account(), m.paid_only) {
            Ok(())
        } else {
            Err(model_catalog::require_plan_message(m.id))
        }
    }

    fn cmd_inspect_media(&self, args: &Value) -> Result<Value, String> {
        // v2 key is mediaRef (short-id expansion applies); legacy mediaId
        // still parses for older callers.
        let media_id = args
            .get("mediaRef")
            .or_else(|| args.get("mediaId"))
            .and_then(|v| v.as_str())
            .ok_or_else(|| "Missing mediaRef".to_string())?;

        // Issue #39: resolve language — per-call arg → project setting → None.
        let _language = args
            .get("language")
            .and_then(|v| v.as_str())
            .map(String::from)
            .or_else(|| self.timeline.transcription_language.clone());

        let entry = self
            .media_manifest
            .entries
            .iter()
            .find(|e| e.id == media_id)
            .ok_or_else(|| format!("Media '{}' not found", media_id))?;

        // READ-013: Text clip rejection
        if entry.r#type == core_model::ClipType::Text {
            return Ok(json!({
                "content": [{
                    "type": "text",
                    "text": "Cannot inspect a text clip with inspect_media. Use get_timeline to view text clips."
                }],
                "isError": true,
            }));
        }

        // READ-014: clipId → mediaRef cross-validation
        if let Some(clip_id) = args.get("clipId").and_then(|v| v.as_str()) {
            let all_clips: Vec<&Clip> =
                self.timeline.tracks.iter().flat_map(|t| &t.clips).collect();
            let clip = all_clips
                .iter()
                .find(|c| c.id == clip_id)
                .ok_or_else(|| format!("Clip '{}' not found on timeline", clip_id))?;
            if clip.media_ref != entry.id {
                return Err(format!(
                    "Clip '{}' references media '{}', not '{}'",
                    clip_id, clip.media_ref, media_id
                ));
            }
        }

        // READ-015: maxFrames default 6, clamped to 1..12
        let max_frames: usize = args
            .get("maxFrames")
            .and_then(|v| v.as_str())
            .and_then(|s| s.parse::<usize>().ok())
            .unwrap_or(6)
            .clamp(1, 12);

        // Find matching clip on timeline (if any)
        let clip = self
            .timeline
            .tracks
            .iter()
            .flat_map(|t| &t.clips)
            .find(|c| c.media_ref == media_id)
            .cloned();

        // Build the InspectMediaInput
        let inspect_input = InspectMediaInput {
            entry: entry.clone(),
            clip,
            timeline_fps: self.timeline.fps,
            max_frames,
            inline_image_data: None,         // caller supplies via callbacks
            inline_video_frames: Vec::new(), // caller supplies via callbacks
            transcription_words: Vec::new(), // caller supplies via callbacks
        };

        let details = format_inspect_media(&inspect_input)
            .map_err(|e| format!("inspect_media error: {}", e))?;

        Ok(json!({
            "content": [{
                "type": "text",
                "text": serde_json::to_string_pretty(&details).unwrap_or_default()
            }]
        }))
    }

    fn cmd_inspect_timeline(&self) -> Result<Value, String> {
        let formatted = format_timeline_json(&self.timeline);
        Ok(json!({
            "content": [{
                "type": "text",
                "text": serde_json::to_string_pretty(&formatted).unwrap_or_default()
            }]
        }))
    }

    /// GET_TRANSCRIPT v2 (tool-surface-v2 C-6): the timeline's spoken words
    /// in project frames — global stable 0-based indices, per-clip rows,
    /// optional segments granularity, 10000-word paging. The words come from
    /// the host transcriber (`transcribe_timeline` / `set_timeline_words`).
    fn cmd_get_transcript(&mut self, args: &Value) -> Result<Value, String> {
        // Executor tolerates the undocumented wordTimestamps key (upstream
        // allowed-keys residue).
        let _word_timestamps = args.get("wordTimestamps");
        let start_frame = args.get("startFrame").and_then(|v| v.as_i64());
        let end_frame = args.get("endFrame").and_then(|v| v.as_i64());
        let clip_scope = args
            .get("clipId")
            .and_then(|v| v.as_str())
            .map(String::from);
        let granularity = args
            .get("granularity")
            .and_then(|v| v.as_str())
            .unwrap_or("words");
        if !matches!(granularity, "words" | "segments") {
            return Err(format!(
                "Invalid granularity '{granularity}'. Use 'words' or 'segments'."
            ));
        }
        if let Some(cid) = &clip_scope {
            if timeline_core::find_clip(&self.timeline, cid).is_none() {
                return Err(format!("Clip '{cid}' not found"));
            }
        }

        // Populate words through the host transcriber when none are stored yet.
        if self.timeline_words.is_empty() && self.transcription_provider.is_some() {
            let _ = self.transcribe_timeline();
        }
        let has_audio_content = self.timeline.tracks.iter().flat_map(|t| &t.clips).any(|c| {
            matches!(c.media_type, ClipType::Audio | ClipType::Video)
        });
        if self.timeline_words.is_empty() {
            if has_audio_content && self.transcription_provider.is_none() {
                return Err(
                    "get_transcript is unavailable: no transcription provider is connected (run it from the app)."
                        .to_string(),
                );
            }
        }

        let fps = self.timeline.fps.max(1);
        const WORD_CAP: usize = 10_000;

        // Windowed, optionally clip-scoped selection. Indices stay GLOBAL.
        let selected: Vec<&timeline_core::TimelineWord> = self
            .timeline_words
            .iter()
            .filter(|w| clip_scope.as_deref().is_none_or(|cid| w.clip_id == cid))
            .filter(|w| start_frame.is_none_or(|sf| w.end_frame > sf))
            .filter(|w| end_frame.is_none_or(|ef| w.start_frame < ef))
            .collect();
        let total_words = selected.len();
        let capped = &selected[..total_words.min(WORD_CAP)];
        let next_start_frame = (total_words > WORD_CAP).then(|| selected[WORD_CAP].start_frame);

        // Group consecutive words by clip (timeline order == index order).
        let mut clip_rows: Vec<Value> = Vec::new();
        let mut i = 0usize;
        while i < capped.len() {
            let clip_id = &capped[i].clip_id;
            let mut j = i;
            while j < capped.len() && &capped[j].clip_id == clip_id {
                j += 1;
            }
            let group = &capped[i..j];
            let first = group[0];
            // A word runs to the next word's start; the last to its clip end.
            let effective_end = |k: usize| -> i64 {
                group
                    .get(k + 1)
                    .map(|w| w.start_frame)
                    .unwrap_or(first.clip_end_frame)
            };
            let mut row = serde_json::Map::new();
            row.insert("clipId".into(), json!(first.clip_id));
            row.insert("trackIndex".into(), json!(first.track_index));
            row.insert("startFrame".into(), json!(first.clip_start_frame));
            row.insert("endFrame".into(), json!(first.clip_end_frame));
            if granularity == "words" {
                let words: Vec<Value> = group
                    .iter()
                    .map(|w| json!([w.index, w.text, w.start_frame]))
                    .collect();
                row.insert("words".into(), json!(words));
            } else {
                // Segment flush rules (C-6): gap > 1s, run >= 48 words, or
                // sentence-final punctuation. (Speaker changes would flush
                // too; the host transcriber reports no speakers yet.)
                let mut segments: Vec<Value> = Vec::new();
                let mut seg_start = 0usize;
                for (k, w) in group.iter().enumerate() {
                    let run = k - seg_start + 1;
                    let punct = w
                        .text
                        .trim_end()
                        .ends_with(['.', '!', '?']);
                    let gap_next = group
                        .get(k + 1)
                        .map(|next| next.start_frame - w.end_frame > fps)
                        .unwrap_or(false);
                    if punct || gap_next || run >= 48 || k + 1 == group.len() {
                        let text: Vec<&str> =
                            group[seg_start..=k].iter().map(|w| w.text.as_str()).collect();
                        segments.push(json!([
                            group[seg_start].index,
                            text.join(" "),
                            group[seg_start].start_frame,
                            effective_end(k),
                        ]));
                        seg_start = k + 1;
                    }
                }
                row.insert("segments".into(), json!(segments));
            }
            clip_rows.push(Value::Object(row));
            i = j;
        }

        let mut out = serde_json::Map::new();
        out.insert("fps".into(), json!(fps));
        out.insert("timing".into(), json!("projectFrames"));
        out.insert("transcriptionSource".into(), json!("local"));
        out.insert("clips".into(), json!(clip_rows));
        if granularity == "words" {
            out.insert("wordFormat".into(), json!(["index", "text", "start"]));
        } else {
            out.insert(
                "segmentFormat".into(),
                json!(["firstWordIndex", "text", "start", "end"]),
            );
        }
        if let Some(next) = next_start_frame {
            out.insert("totalWords".into(), json!(total_words));
            out.insert("nextStartFrame".into(), json!(next));
            out.insert(
                "wordsNote".into(),
                json!(format!(
                    "First {WORD_CAP} of {total_words} words. Continue with startFrame = nextStartFrame."
                )),
            );
        }
        Ok(Self::json_content(&Value::Object(out)))
    }

    // ── Media mutation tools ─────────────────────────────────────────────

    /// Pre-call item resolution for organize_media: asset id → timeline id →
    /// folder path, in that order (tool-surface-v2 C-7).
    fn resolve_organize_item(&self, raw: &str) -> Result<OrganizeItem, String> {
        if self.media_manifest.entries.iter().any(|e| e.id == raw) {
            return Ok(OrganizeItem::Asset(raw.to_string()));
        }
        if self.timeline.id == raw || self.sibling_timelines.iter().any(|t| t.id == raw) {
            return Ok(OrganizeItem::Timeline(raw.to_string()));
        }
        match crate::organize::resolve_folder(&self.media_manifest.folders, raw)
            .map_err(|e| format!("organize_media: {e}"))?
        {
            Some(id) => Ok(OrganizeItem::Folder(id)),
            None => Err(format!(
                "organize_media: '{raw}' is not a media asset id, a timeline id, or an existing folder path."
            )),
        }
    }

    /// ORGANIZE_MEDIA (tool-surface-v2, replaces create_folder, rename_folder,
    /// delete_folder, move_to_folder, rename_media, delete_media): one call
    /// for create/move/rename/delete over the library. Arrays apply
    /// createFolders → moves → renames → deletes, but every item reference is
    /// resolved against the library as it was BEFORE the call — only 'into'
    /// destinations may name folders the same call creates. Media-library
    /// mutations are not undo-tracked yet (same as the tools this replaces).
    fn cmd_organize_media(&mut self, args: &Value) -> Result<Value, String> {
        let parsed = match crate::mutation::validate_organize_media(args) {
            crate::mutation::ValidationResult::Ok(p) => p,
            crate::mutation::ValidationResult::Error(e) => return Err(e),
        };

        // ── parse: resolve every item reference against the pre-call library ──
        let moves: Vec<(Vec<OrganizeItem>, Option<String>)> = parsed
            .moves
            .iter()
            .map(|mv| {
                mv.items
                    .iter()
                    .map(|raw| self.resolve_organize_item(raw))
                    .collect::<Result<Vec<_>, _>>()
                    .map(|items| (items, mv.into.clone()))
            })
            .collect::<Result<_, _>>()?;
        let renames: Vec<(OrganizeItem, String)> = parsed
            .renames
            .iter()
            .map(|r| self.resolve_organize_item(&r.item).map(|i| (i, r.name.clone())))
            .collect::<Result<_, _>>()?;
        let deletes: Vec<OrganizeItem> = parsed
            .deletes
            .iter()
            .map(|raw| self.resolve_organize_item(raw))
            .collect::<Result<_, _>>()?;

        // A folder's new name is a name, not a path (renaming never moves).
        for (item, name) in &renames {
            if matches!(item, OrganizeItem::Folder(_)) && name.contains('/') {
                return Err(format!(
                    "organize_media: '{name}' is a path — a rename takes a plain name and never moves the folder."
                ));
            }
        }
        // Cycle guard (parse-time, pre-call paths): a folder can't move into
        // itself or its own subtree — including subtrees this call creates.
        for (items, into) in &moves {
            let Some(into) = into else { continue };
            let dest = crate::organize::normalize_path(into).to_lowercase();
            for item in items {
                if let OrganizeItem::Folder(fid) = item {
                    let own_path =
                        crate::organize::folder_path(&self.media_manifest.folders, fid);
                    let own = own_path.to_lowercase();
                    if dest == own || dest.starts_with(&format!("{own}/")) {
                        return Err(format!(
                            "organize_media: can't move folder '{own_path}' into itself or its own subfolder."
                        ));
                    }
                }
            }
        }
        // Last-timeline guard: the delete list may not cover every timeline.
        let timeline_delete_ids: std::collections::HashSet<String> = deletes
            .iter()
            .filter_map(|i| match i {
                OrganizeItem::Timeline(id) => Some(id.clone()),
                _ => None,
            })
            .collect();
        if !timeline_delete_ids.is_empty()
            && timeline_delete_ids.len() >= 1 + self.sibling_timelines.len()
        {
            return Err("Can't delete every timeline — at least one must remain.".to_string());
        }

        // ── apply: createFolders ──
        let mut created: Vec<String> = Vec::new();
        for path in &parsed.create_folders {
            let (_, c) =
                crate::organize::resolve_or_create_folder(&mut self.media_manifest.folders, path)
                    .map_err(|e| format!("organize_media: {e}"))?;
            created.extend(c);
        }

        // ── apply: moves (destinations may name folders created above) ──
        let mut moved = 0usize;
        for (items, into) in &moves {
            let dest: Option<String> = match into {
                Some(p) => {
                    let (id, c) = crate::organize::resolve_or_create_folder(
                        &mut self.media_manifest.folders,
                        p,
                    )
                    .map_err(|e| format!("organize_media: {e}"))?;
                    created.extend(c);
                    Some(id)
                }
                None => None,
            };
            for item in items {
                match item {
                    OrganizeItem::Asset(id) => {
                        if let Some(entry) =
                            self.media_manifest.entries.iter_mut().find(|e| &e.id == id)
                        {
                            entry.folder_id = dest.clone();
                        }
                    }
                    OrganizeItem::Timeline(id) => {
                        if self.timeline.id == *id {
                            self.timeline.folder_id = dest.clone();
                        } else if let Some(t) =
                            self.sibling_timelines.iter_mut().find(|t| &t.id == id)
                        {
                            t.folder_id = dest.clone();
                        }
                    }
                    OrganizeItem::Folder(id) => {
                        if dest.as_deref() == Some(id.as_str()) {
                            return Err(
                                "organize_media: can't move a folder into itself.".to_string()
                            );
                        }
                        if let Some(f) =
                            self.media_manifest.folders.iter_mut().find(|f| &f.id == id)
                        {
                            f.parent_folder_id = dest.clone();
                        }
                    }
                }
                moved += 1;
            }
        }

        // ── apply: renames (a name, not a path) ──
        let mut renamed = 0usize;
        for (item, name) in &renames {
            match item {
                OrganizeItem::Asset(id) => {
                    if let Some(entry) =
                        self.media_manifest.entries.iter_mut().find(|e| &e.id == id)
                    {
                        entry.name = name.clone();
                    }
                }
                OrganizeItem::Timeline(id) => {
                    if self.timeline.id == *id {
                        self.timeline.name = name.clone();
                    } else if let Some(t) =
                        self.sibling_timelines.iter_mut().find(|t| &t.id == id)
                    {
                        t.name = name.clone();
                    }
                }
                OrganizeItem::Folder(id) => {
                    if let Some(f) = self.media_manifest.folders.iter_mut().find(|f| &f.id == id)
                    {
                        f.name = name.clone();
                    }
                }
            }
            renamed += 1;
        }

        // ── apply: deletes — assets + folders first, timelines last ──
        let mut warnings: Vec<String> = Vec::new();
        let mut asset_delete_ids: std::collections::HashSet<String> = deletes
            .iter()
            .filter_map(|i| match i {
                OrganizeItem::Asset(id) => Some(id.clone()),
                _ => None,
            })
            .collect();
        let folder_delete_roots: Vec<String> = deletes
            .iter()
            .filter_map(|i| match i {
                OrganizeItem::Folder(id) => Some(id.clone()),
                _ => None,
            })
            .collect();

        // Folder cascade: the folder, its subtree, and every asset inside are
        // deleted; timelines inside move to the root instead.
        let mut folders_to_delete: std::collections::HashSet<String> =
            std::collections::HashSet::new();
        for root in &folder_delete_roots {
            let mut queue = vec![root.clone()];
            while let Some(id) = queue.pop() {
                if !folders_to_delete.insert(id.clone()) {
                    continue;
                }
                queue.extend(
                    self.media_manifest
                        .folders
                        .iter()
                        .filter(|f| f.parent_folder_id.as_deref() == Some(id.as_str()))
                        .map(|f| f.id.clone()),
                );
            }
        }
        for entry in &self.media_manifest.entries {
            if entry
                .folder_id
                .as_ref()
                .is_some_and(|f| folders_to_delete.contains(f))
            {
                asset_delete_ids.insert(entry.id.clone());
            }
        }
        if self
            .timeline
            .folder_id
            .as_ref()
            .is_some_and(|f| folders_to_delete.contains(f))
        {
            self.timeline.folder_id = None;
        }
        for t in &mut self.sibling_timelines {
            if t.folder_id
                .as_ref()
                .is_some_and(|f| folders_to_delete.contains(f))
            {
                t.folder_id = None;
            }
        }
        let deleted_folders = folders_to_delete.len();
        self.media_manifest
            .folders
            .retain(|f| !folders_to_delete.contains(&f.id));

        // Assets: drop the entries, then every clip referencing them on every
        // SURVIVING timeline (clips inside deleted timelines don't count).
        let deleted_assets = asset_delete_ids.len();
        self.media_manifest
            .entries
            .retain(|e| !asset_delete_ids.contains(&e.id));
        let mut clips_removed = 0usize;
        if !asset_delete_ids.is_empty() {
            let mut sweep = |timeline: &mut Timeline| {
                for track in &mut timeline.tracks {
                    let before = track.clips.len();
                    track
                        .clips
                        .retain(|c| !asset_delete_ids.contains(&c.media_ref));
                    clips_removed += before - track.clips.len();
                }
            };
            if !timeline_delete_ids.contains(&self.timeline.id) {
                sweep(&mut self.timeline);
            }
            for t in &mut self.sibling_timelines {
                if !timeline_delete_ids.contains(&t.id) {
                    sweep(t);
                }
            }
        }

        // Timelines last: delete siblings, then switch away from a deleted
        // active timeline (the guard above ensures a survivor exists).
        let mut deleted_timeline_names: Vec<(String, String)> = Vec::new();
        let mut active_switched = false;
        self.sibling_timelines.retain(|t| {
            if timeline_delete_ids.contains(&t.id) {
                deleted_timeline_names.push((t.id.clone(), t.name.clone()));
                false
            } else {
                true
            }
        });
        if timeline_delete_ids.contains(&self.timeline.id) {
            let replacement = self
                .sibling_timelines
                .remove(0);
            let old = std::mem::replace(&mut self.timeline, replacement);
            deleted_timeline_names.push((old.id, old.name));
            self.undo_stack.clear();
            active_switched = true;
        }
        let deleted_timelines = deleted_timeline_names.len();
        for (tid, tname) in &deleted_timeline_names {
            let refs = std::iter::once(&self.timeline)
                .chain(self.sibling_timelines.iter())
                .flat_map(|t| t.tracks.iter())
                .flat_map(|t| t.clips.iter())
                .filter(|c| c.source_clip_type == ClipType::Sequence && &c.media_ref == tid)
                .count();
            if refs > 0 {
                warnings.push(format!(
                    "{refs} nested clip(s) still reference deleted timeline '{tname}' — they will render black."
                ));
            }
        }

        // ── result: only what actually happened ──
        let mut out = serde_json::Map::new();
        if !created.is_empty() {
            out.insert("createdFolders".into(), json!(created));
        }
        if moved > 0 {
            out.insert("moved".into(), json!(moved));
        }
        if renamed > 0 {
            out.insert("renamed".into(), json!(renamed));
        }
        if deleted_assets + deleted_folders + deleted_timelines > 0 {
            let mut d = serde_json::Map::new();
            if deleted_assets > 0 {
                d.insert("assets".into(), json!(deleted_assets));
            }
            if deleted_folders > 0 {
                d.insert("folders".into(), json!(deleted_folders));
            }
            if deleted_timelines > 0 {
                d.insert("timelines".into(), json!(deleted_timelines));
            }
            out.insert("deleted".into(), Value::Object(d));
        }
        if clips_removed > 0 {
            out.insert("clipsRemoved".into(), json!(clips_removed));
        }
        if !warnings.is_empty() {
            out.insert("warnings".into(), json!(warnings));
        }
        if active_switched {
            out.insert(
                "notes".into(),
                json!(["Active timeline changed — re-read get_timeline."]),
            );
        }
        Ok(Value::Object(out))
    }

    /// IMPORT_MEDIA (tool-surface-v2): 'source' sets exactly one of url /
    /// path / bytes / matte. path may be a directory (recursive import with
    /// subfolders mirrored as media folders — absorbs import_folder); matte
    /// renders a solid-colour PNG via the MatteWriter host seam (absorbs
    /// create_matte, #242). url/bytes need host download/decode services that
    /// aren't connected on this path and report so honestly.
    fn cmd_import_media(&mut self, args: &Value) -> Result<Value, String> {
        let parsed = match crate::mutation::validate_import_media(args) {
            crate::mutation::ValidationResult::Ok(p) => p,
            crate::mutation::ValidationResult::Error(e) => return Err(e),
        };
        let folder_id: Option<String> = match &parsed.folder {
            Some(p) => Some(
                crate::organize::resolve_or_create_folder(&mut self.media_manifest.folders, p)
                    .map_err(|e| format!("import_media: {e}"))?
                    .0,
            ),
            None => None,
        };
        if let Some(hex) = &parsed.matte_hex {
            return self.import_matte(
                hex,
                parsed.matte_aspect.as_deref(),
                parsed.name.as_deref(),
                folder_id,
            );
        }
        if let Some(path) = &parsed.path {
            let p = std::path::Path::new(path);
            if std::fs::metadata(p).map(|m| m.is_dir()).unwrap_or(false) {
                return self.import_directory(p, folder_id);
            }
            let payload = self.register_import_file(p, parsed.name.as_deref(), folder_id)?;
            return Ok(json!({
                "content": [{
                    "type": "text",
                    "text": serde_json::to_string(&payload).unwrap_or_default()
                }]
            }));
        }
        if parsed.url.is_some() {
            return Err(
                "import_media: url imports need the app's background download service, which                  isn't connected in this context. Download the file and pass source.path instead."
                    .to_string(),
            );
        }
        Err(
            "import_media: bytes imports aren't supported in this context — write the data to a              file and pass source.path."
                .to_string(),
        )
    }

    /// Register one local file as an external asset. Pure registration — the
    /// executor references the absolute path (no copy) and can't probe media
    /// duration, so stills get the 5s default and A/V a 10s placeholder.
    fn register_import_file(
        &mut self,
        path: &std::path::Path,
        name: Option<&str>,
        folder_id: Option<String>,
    ) -> Result<Value, String> {
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
        let kind = ClipType::from_extension(ext).ok_or_else(|| {
            format!(
                "import_media: unsupported file type '.{ext}' — supported: video (mov, mp4, m4v),                  audio (mp3, wav, aac, m4a, aiff, aifc, flac), image (png, jpg, jpeg, tiff, heic)."
            )
        })?;
        let file_name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("Imported asset");
        let display = name
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .unwrap_or(file_name)
            .to_string();
        let duration = if kind == ClipType::Image { 5.0 } else { 10.0 };
        let entry = MediaManifestEntry {
            id: Uuid::new_v4().to_string(),
            name: display.clone(),
            r#type: kind,
            source: MediaSource::External {
                absolute_path: path.to_string_lossy().into_owned(),
            },
            duration,
            generation_input: None,
            source_width: None,
            source_height: None,
            source_fps: None,
            has_audio: None,
            folder_id,
            cached_remote_url: None,
            cached_remote_url_expires_at: None,
            source_timecode_frame: None,
            source_timecode_quanta: None,
            source_timecode_drop_frame: None,
            ai_tags: None,
            ai_description: None,
            ai_label_status: None,
            generation_status: None,
        };
        let id = entry.id.clone();
        self.media_manifest.entries.push(entry);
        Ok(json!({
            "mediaRef": id,
            "status": "ready",
            "name": display,
            "type": kind.name(),
        }))
    }

    /// Directory import (absorbs import_folder): the directory becomes a
    /// media folder; every supported file inside registers recursively with
    /// the subfolder tree mirrored. Finishes inline.
    fn import_directory(
        &mut self,
        dir: &std::path::Path,
        parent: Option<String>,
    ) -> Result<Value, String> {
        let top_name = dir
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("Imported folder")
            .to_string();
        let top = core_model::MediaFolder {
            id: Uuid::new_v4().to_string(),
            name: top_name,
            parent_folder_id: parent,
        };
        let top_id = top.id.clone();
        self.media_manifest.folders.push(top);
        let mut imported = 0usize;
        let mut skipped = 0usize;
        let mut folders_created = 1usize;
        self.import_dir_recursive(dir, &top_id, 0, &mut imported, &mut skipped, &mut folders_created)?;
        let folder_path = crate::organize::folder_path(&self.media_manifest.folders, &top_id);
        let mut out = json!({
            "status": "ready",
            "imported": imported,
            "folder": folder_path,
            "foldersCreated": folders_created,
        });
        if skipped > 0 {
            out["skipped"] = json!(skipped);
        }
        Ok(json!({
            "content": [{
                "type": "text",
                "text": serde_json::to_string(&out).unwrap_or_default()
            }]
        }))
    }

    fn import_dir_recursive(
        &mut self,
        dir: &std::path::Path,
        folder_id: &str,
        depth: usize,
        imported: &mut usize,
        skipped: &mut usize,
        folders_created: &mut usize,
    ) -> Result<(), String> {
        if depth > 32 {
            return Ok(()); // symlink-loop guard
        }
        let mut entries: Vec<std::fs::DirEntry> = std::fs::read_dir(dir)
            .map_err(|e| format!("import_media: can't read directory '{}': {e}", dir.display()))?
            .filter_map(Result::ok)
            .collect();
        entries.sort_by_key(|e| e.file_name());
        for entry in entries {
            let path = entry.path();
            let is_dir = entry.file_type().map(|t| t.is_dir()).unwrap_or(false);
            if is_dir {
                let name = path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("Folder")
                    .to_string();
                let sub = core_model::MediaFolder {
                    id: Uuid::new_v4().to_string(),
                    name,
                    parent_folder_id: Some(folder_id.to_string()),
                };
                let sub_id = sub.id.clone();
                self.media_manifest.folders.push(sub);
                *folders_created += 1;
                self.import_dir_recursive(
                    &path,
                    &sub_id,
                    depth + 1,
                    imported,
                    skipped,
                    folders_created,
                )?;
            } else {
                match self.register_import_file(&path, None, Some(folder_id.to_string())) {
                    Ok(_) => *imported += 1,
                    Err(_) => *skipped += 1,
                }
            }
        }
        Ok(())
    }

    /// Matte import (absorbs create_matte, #242): compute even pixel
    /// dimensions from the aspect preset + timeline size, then hand the colour
    /// + size to the host `MatteWriter` (which renders + persists the PNG) and
    /// register the resulting image asset. Finishes inline (status 'ready').
    fn import_matte(
        &mut self,
        hex: &str,
        aspect_raw: Option<&str>,
        name: Option<&str>,
        folder_id: Option<String>,
    ) -> Result<Value, String> {
        let rgba = TextRgba::from_hex(hex)
            .ok_or_else(|| format!("import_media: invalid matte hex color '{hex}'."))?;
        let aspect = match aspect_raw {
            Some(raw) => MatteAspect::parse(raw).ok_or_else(|| {
                format!(
                    "import_media: unknown matte aspectRatio '{raw}'. Use one of: {}.",
                    MatteAspect::ALL
                        .iter()
                        .map(|a| a.raw_value())
                        .collect::<Vec<_>>()
                        .join(", ")
                )
            })?,
            None => MatteAspect::Project,
        };
        let (width, height) = aspect.pixel_size(self.timeline.width, self.timeline.height);
        let name = name
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .unwrap_or("Matte")
            .to_string();

        let writer = self.matte_writer.clone().ok_or_else(|| {
            "import_media: matte imports are unavailable — no project is connected to write the matte into."
                .to_string()
        })?;
        let to_u8 = |c: f64| (c * 255.0).round().clamp(0.0, 255.0) as u8;
        let px = [to_u8(rgba.r), to_u8(rgba.g), to_u8(rgba.b), 255];
        let source = writer.write_matte(px, width, height, &name)?;

        let entry = MediaManifestEntry {
            id: Uuid::new_v4().to_string(),
            name: name.clone(),
            r#type: ClipType::Image,
            source,
            duration: 5.0,
            generation_input: None,
            source_width: Some(width),
            source_height: Some(height),
            source_fps: None,
            has_audio: Some(false),
            folder_id,
            cached_remote_url: None,
            cached_remote_url_expires_at: None,
            source_timecode_frame: None,
            source_timecode_quanta: None,
            source_timecode_drop_frame: None,
            ai_tags: None,
            ai_description: None,
            ai_label_status: None,
            generation_status: None,
        };
        let id = entry.id.clone();
        self.media_manifest.entries.push(entry);
        Ok(json!({
            "content": [{
                "type": "text",
                "text": serde_json::to_string(&json!({
                    "mediaRef": id, "status": "ready", "name": name, "width": width, "height": height
                }))
                .unwrap_or_default()
            }]
        }))
    }

    /// CREATE_COMPOUND_CLIP: group adjacent same-track clips into a compound
    /// clip whose constituents move into a nested timeline (Issue #155).
    fn cmd_create_compound_clip(&mut self, args: &Value) -> Result<Value, String> {
        let clip_ids: Vec<String> = args
            .get("clipIds")
            .and_then(|v| v.as_array())
            .map(|a| a.iter().filter_map(|v| v.as_str().map(String::from)).collect())
            .unwrap_or_default();
        if clip_ids.is_empty() {
            return Err(
                "create_compound_clip requires 'clipIds' (a non-empty array of clip ids)."
                    .to_string(),
            );
        }
        let name = args
            .get("name")
            .and_then(|v| v.as_str())
            .map(str::trim)
            .filter(|s| !s.is_empty());
        // Swift #255 representation: the group becomes a NEW sibling timeline and
        // a sequence-carrier clip referencing it. (Undo restores the parent
        // timeline; an orphaned child timeline is inert and harmless.)
        let nest = timeline_core::nest_clips(&mut self.timeline, &clip_ids, name)?;
        let child_id = nest.child.id.clone();
        let child_name = nest.child.name.clone();
        self.sibling_timelines.push(nest.child);
        Ok(json!({
            "content": [{
                "type": "text",
                "text": serde_json::to_string(&json!({
                    "compoundClipId": nest.carrier_id,
                    "childTimelineId": child_id,
                    "name": child_name,
                    "groupedClipCount": clip_ids.len(),
                }))
                .unwrap_or_default()
            }]
        }))
    }

    /// DISSOLVE_COMPOUND_CLIP: flatten a compound clip back to its constituent
    /// clips at their absolute frames (Issue #155).
    fn cmd_dissolve_compound_clip(&mut self, args: &Value) -> Result<Value, String> {
        let clip_id = args
            .get("clipId")
            .and_then(|v| v.as_str())
            .ok_or_else(|| "dissolve_compound_clip requires 'clipId'.".to_string())?;
        let child = {
            let loc = timeline_core::find_clip(&self.timeline, clip_id)
                .ok_or_else(|| format!("Clip '{clip_id}' was not found on the timeline."))?;
            let carrier = &self.timeline.tracks[loc.track_index].clips[loc.clip_index];
            if carrier.source_clip_type != ClipType::Sequence {
                return Err("That clip isn't a compound clip.".to_string());
            }
            self.sibling_timelines
                .iter()
                .find(|t| t.id == carrier.media_ref)
                .cloned()
                .ok_or_else(|| "The compound clip's nested timeline is missing.".to_string())?
        };
        let restored = timeline_core::decompose_nest(&mut self.timeline, clip_id, &child)?;
        Ok(json!({
            "content": [{
                "type": "text",
                "text": serde_json::to_string(&json!({
                    "restoredClipIds": restored,
                    "count": restored.len(),
                }))
                .unwrap_or_default()
            }]
        }))
    }

    /// SAVE_CLIP_PRESET: capture a clip's look/grade as a named session preset (#157).
    fn cmd_save_clip_preset(&mut self, args: &Value) -> Result<Value, String> {
        let clip_id = args
            .get("clipId")
            .and_then(|v| v.as_str())
            .ok_or_else(|| "save_clip_preset requires 'clipId'.".to_string())?;
        let name = args
            .get("name")
            .and_then(|v| v.as_str())
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .ok_or_else(|| "save_clip_preset requires a non-empty 'name'.".to_string())?;
        let loc = timeline_core::find_clip(&self.timeline, clip_id)
            .ok_or_else(|| format!("Clip '{clip_id}' was not found on the timeline."))?;
        let clip = &self.timeline.tracks[loc.track_index].clips[loc.clip_index];
        let preset = ClipPreset {
            transform: clip.transform,
            crop: clip.crop,
            opacity: clip.opacity,
            volume: clip.volume,
            speed: clip.speed,
            effects: clip.effects.clone(),
            blend_mode: clip.blend_mode,
            chroma_key: clip.chroma_key.clone(),
        };
        self.clip_presets.insert(name.to_string(), preset);
        Ok(json!({
            "content": [{
                "type": "text",
                "text": serde_json::to_string(&json!({
                    "presetName": name,
                    "capturedFrom": clip_id,
                    "presetCount": self.clip_presets.len(),
                }))
                .unwrap_or_default()
            }]
        }))
    }

    /// APPLY_CLIP_PRESET: apply a saved preset's grade to one or more clips (#157).
    /// Speed goes through `apply_clip_speed` so duration/keyframes stay correct;
    /// the remaining static properties overwrite the clip's own.
    fn cmd_apply_clip_preset(&mut self, args: &Value) -> Result<Value, String> {
        let preset_name = args
            .get("presetName")
            .and_then(|v| v.as_str())
            .ok_or_else(|| "apply_clip_preset requires 'presetName'.".to_string())?;
        let clip_ids: Vec<String> = args
            .get("clipIds")
            .and_then(|v| v.as_array())
            .map(|a| a.iter().filter_map(|v| v.as_str().map(String::from)).collect())
            .unwrap_or_default();
        if clip_ids.is_empty() {
            return Err("apply_clip_preset requires 'clipIds' (a non-empty array of clip ids).".to_string());
        }
        let preset = self.clip_presets.get(preset_name).cloned().ok_or_else(|| {
            format!("No clip preset named '{preset_name}'. Save one first with save_clip_preset.")
        })?;
        let mut applied = 0;
        for clip_id in &clip_ids {
            let Some(loc) = timeline_core::find_clip(&self.timeline, clip_id) else {
                continue;
            };
            let current_speed = self.timeline.tracks[loc.track_index].clips[loc.clip_index].speed;
            if (current_speed - preset.speed).abs() > f64::EPSILON {
                timeline_core::apply_clip_speed(&mut self.timeline, clip_id, preset.speed);
            }
            let Some(loc) = timeline_core::find_clip(&self.timeline, clip_id) else {
                continue;
            };
            let clip = &mut self.timeline.tracks[loc.track_index].clips[loc.clip_index];
            clip.transform = preset.transform;
            clip.crop = preset.crop;
            clip.opacity = preset.opacity;
            clip.volume = preset.volume;
            clip.effects = preset.effects.clone();
            clip.blend_mode = preset.blend_mode;
            clip.chroma_key = preset.chroma_key.clone();
            applied += 1;
        }
        Ok(json!({
            "content": [{
                "type": "text",
                "text": serde_json::to_string(&json!({
                    "presetName": preset_name,
                    "applied": applied,
                }))
                .unwrap_or_default()
            }]
        }))
    }

    /// LIST_CLIP_PRESETS: names of the session's saved clip presets (#157).
    fn cmd_list_clip_presets(&self) -> Result<Value, String> {
        let mut names: Vec<&str> = self.clip_presets.keys().map(String::as_str).collect();
        names.sort_unstable();
        Ok(json!({
            "content": [{
                "type": "text",
                "text": serde_json::to_string(&json!({
                    "presets": names,
                    "count": names.len(),
                }))
                .unwrap_or_default()
            }]
        }))
    }

    /// REMOVE_SILENCE: detect dead air via the host SpeechAnalyzer when present
    /// (span inversion), else on-device RMS, and ripple-delete it (#174).
    /// Decoding/VAD are host seams; the detectors and source→frame mapping are
    /// pure (`audio_core::silence_detector`).
    fn cmd_remove_silence(&mut self, args: &Value) -> Result<Value, String> {
        // Upstream #261 semantics by default: no arguments, whole-timeline
        // dead-air removal with a threshold ADAPTIVE to each recording's own
        // level (RMS approximation of upstream's speech detection - honest in
        // the tool description). clipId + thresholdDb/minSilenceSeconds/
        // edgePaddingSeconds remain as a Rust clip-scoped extension.
        let clip_id = args
            .get("clipId")
            .and_then(|v| v.as_str())
            .map(String::from);
        let threshold_db = args.get("thresholdDb").and_then(|v| v.as_f64());
        let min_silence_seconds = args
            .get("minSilenceSeconds")
            .and_then(|v| v.as_f64())
            .filter(|s| *s >= 0.0)
            .unwrap_or(0.5);
        let edge_padding_seconds = args
            .get("edgePaddingSeconds")
            .and_then(|v| v.as_f64())
            .filter(|s| *s >= 0.0)
            .unwrap_or(0.1);
        let audio = self.audio_source.clone().ok_or_else(|| {
            "remove_silence is unavailable: no audio decoder is connected (run it from the app)."
                .to_string()
        })?;

        // Which tracks to sweep. Clip mode pins one clip on its track.
        let track_indices: Vec<usize> = match clip_id.as_deref() {
            Some(id) => {
                let loc = timeline_core::find_clip(&self.timeline, id)
                    .ok_or_else(|| format!("Clip '{id}' was not found on the timeline."))?;
                vec![loc.track_index]
            }
            None => (0..self.timeline.tracks.len()).collect(),
        };

        // Per track: DETECT AGAINST THE CURRENT STATE, then ripple. An earlier
        // track's ripple moves sync-locked followers, so ranges pre-computed for
        // every track at once would be stale by the time later tracks apply
        // (confirmed by adversarial review) - re-detecting per track keeps every
        // cut aligned, and a span a follower already lost simply stops being
        // detected on its own pass.
        let mut sections = 0usize;
        let mut removed_frames = 0i64;
        let mut analysed_any = false;
        let mut partial: Option<String> = None;
        for ti in track_indices {
            let ranges = self.detect_track_dead_air(
                ti,
                clip_id.as_deref(),
                threshold_db,
                min_silence_seconds,
                edge_padding_seconds,
                audio.as_ref(),
                &mut analysed_any,
            )?;
            if ranges.is_empty() {
                continue;
            }
            sections += ranges.len();
            match self.apply_ripple_delete_on_track(ti, ranges, Default::default()) {
                Ok((frames, _)) => removed_frames += frames,
                Err(reason) => {
                    partial = Some(format!(
                        "A later track refused: {reason}. Earlier tracks were already edited."
                    ));
                    break;
                }
            }
        }

        if clip_id.is_none() && !analysed_any {
            return Err(
                "No dead air on the timeline. The timeline has no audio-bearing clips to analyse."
                    .to_string(),
            );
        }
        if sections == 0 {
            if clip_id.is_some() {
                return Ok(json!({
                    "sectionsRemoved": 0,
                    "removedFrames": 0,
                    "notes": ["No silent regions matched the threshold and minimum duration."],
                }));
            }
            return Err(
                "No dead air on the timeline. The audio has no quiet non-speech sections at the current sensitivity."
                    .to_string(),
            );
        }

        let mut payload = json!({
            "sectionsRemoved": sections,
            "removedFrames": removed_frames,
            "notes": ["Removed dead air and closed the gaps. Frames have shifted - re-read get_timeline or get_transcript before further edits."],
        });
        if let Some(p) = partial {
            payload["partial"] = json!(p);
        }
        if let Some(id) = clip_id {
            payload["clipId"] = json!(id);
        }
        Ok(payload)
    }

    /// Detect dead-air frame ranges on ONE track against the CURRENT timeline
    /// state. `only_clip` restricts to a single clip (the clip-scoped mode);
    /// otherwise every audio-bearing clip on the track is analysed. Per clip
    /// the speech analyzer is consulted first; absent or None → RMS path.
    #[allow(clippy::too_many_arguments)]
    fn detect_track_dead_air(
        &self,
        track_index: usize,
        only_clip: Option<&str>,
        threshold_db: Option<f64>,
        min_silence_seconds: f64,
        edge_padding_seconds: f64,
        audio: &dyn ClipAudioSource,
        analysed_any: &mut bool,
    ) -> Result<Vec<timeline_core::FrameRange>, String> {
        use audio_core::silence_detector as sd;

        let Some(track) = self.timeline.tracks.get(track_index) else {
            return Ok(Vec::new());
        };
        let fps = self.timeline.fps as f64;
        let sample_rate = 44_100u32;
        let channels = 1usize;
        let window = (sample_rate as usize / 100).max(1);
        let mut ranges = Vec::new();
        for clip in &track.clips {
            match only_clip {
                Some(id) if clip.id != id => continue,
                None => {
                    let entry = self.media_manifest.entry_for(&clip.media_ref);
                    let audio_bearing = match entry.map(|e| e.r#type) {
                        Some(ClipType::Audio) => true,
                        Some(ClipType::Video) => {
                            entry.and_then(|e| e.has_audio).unwrap_or(false)
                        }
                        _ => false,
                    };
                    if !audio_bearing {
                        continue;
                    }
                }
                _ => {}
            }
            let Some((source, source_duration)) = self
                .media_manifest
                .entry_for(&clip.media_ref)
                .map(|e| (e.source.clone(), e.duration))
            else {
                if only_clip.is_some() {
                    return Err(
                        "The clip's media isn't in the library, so its audio can't be analysed."
                            .to_string(),
                    );
                }
                continue;
            };
            let spans = self
                .speech_analyzer
                .as_ref()
                .and_then(|a| a.analyze(&source, sample_rate));
            let source_ranges = match spans {
                Some(spans) => {
                    *analysed_any = true;
                    let spans: Vec<(f64, f64)> = spans
                        .iter()
                        .map(|s| (s.start_seconds, s.end_seconds))
                        .collect();
                    sd::speech_spans_to_dead_air(
                        &spans,
                        source_duration,
                        min_silence_seconds,
                        edge_padding_seconds,
                    )
                    .into_iter()
                    .map(|(start_seconds, end_seconds)| sd::SourceRange {
                        start_seconds,
                        end_seconds,
                    })
                    .collect()
                }
                None => {
                    let Some(pcm) = audio.decode_source_pcm(&source, sample_rate, channels)
                    else {
                        if only_clip.is_some() {
                            return Err("Could not decode the clip's audio.".to_string());
                        }
                        continue;
                    };
                    *analysed_any = true;
                    let envelope = sd::rms_envelope(&pcm, channels, window);
                    let envelope_rate = sample_rate as f64 / window as f64;
                    let threshold = match threshold_db {
                        Some(db) => sd::SilenceDetectionConfig::from_db(db),
                        None => sd::adaptive_silence_threshold(&envelope),
                    };
                    let config = sd::SilenceDetectionConfig {
                        threshold,
                        min_silence_seconds,
                        edge_padding_seconds,
                    };
                    sd::detect_silence(&envelope, envelope_rate, &config)
                }
            };
            let placement = sd::ClipPlacement {
                timeline_start_frame: clip.start_frame,
                duration_frames: clip.duration_frames,
                source_offset_seconds: clip.trim_start_frame as f64 / fps,
                speed: clip.speed,
                fps,
            };
            for (start, end) in sd::source_ranges_to_project_frames(&source_ranges, &placement) {
                ranges.push(timeline_core::FrameRange { start, end });
            }
        }
        Ok(ranges)
    }

    fn cmd_sync_clips(&mut self, args: &Value) -> Result<Value, String> {
        use audio_core::audio_sync_correlator::AudioSyncCorrelator;

        let ref_id = args
            .get("referenceClipId")
            .and_then(|v| v.as_str())
            .ok_or_else(|| "sync_clips requires 'referenceClipId'.".to_string())?
            .to_string();
        let mut target_ids: Vec<String> = args
            .get("targetClipIds")
            .and_then(|v| v.as_array())
            .map(|a| a.iter().filter_map(|v| v.as_str().map(String::from)).collect())
            .unwrap_or_default();
        if let Some(single) = args.get("targetClipId").and_then(|v| v.as_str()) {
            if !target_ids.iter().any(|t| t == single) {
                target_ids.push(single.to_string());
            }
        }
        if target_ids.is_empty() {
            return Err("sync_clips requires 'targetClipId' or 'targetClipIds'.".to_string());
        }
        // Multicam guard (upstream #283).
        let is_stamped = |id: &str| {
            timeline_core::find_clip(&self.timeline, id).is_some_and(|loc| {
                self.timeline.tracks[loc.track_index].clips[loc.clip_index]
                    .multicam_group_id
                    .is_some()
            })
        };
        if is_stamped(&ref_id) || target_ids.iter().any(|t| is_stamped(t)) {
            return Err(
                "sync_clips: multicam clips are already aligned by their group's sync maps — re-syncing would move them out of the group."
                    .to_string(),
            );
        }
        let min_confidence = args
            .get("minConfidence")
            .and_then(|v| v.as_f64())
            .unwrap_or(0.5);
        let search_window_seconds = args
            .get("searchWindowSeconds")
            .and_then(|v| v.as_f64())
            .filter(|s| *s > 0.0)
            .unwrap_or(30.0);

        let mode = args
            .get("mode")
            .and_then(|v| v.as_str())
            .unwrap_or("auto");
        if !matches!(mode, "auto" | "audio" | "timecode") {
            return Err(format!("Invalid mode '{mode}'. Use auto, audio, or timecode."));
        }
        // The audio decoder is only required when audio correlation can run;
        // pure-timecode syncs work without it.
        let audio_seam = self.audio_source.clone();
        let audio = match (mode, &audio_seam) {
            ("timecode", _) => None,
            (_, Some(seam)) => Some(seam.clone()),
            ("audio", None) => {
                return Err(
                    "sync_clips is unavailable: no audio decoder is connected (run it from the app)."
                        .to_string(),
                )
            }
            (_, None) => None,
        };
        let sample_rate = 44_100u32;
        let channels = 1usize;
        let frame_size = 1024usize;

        let ref_loc = timeline_core::find_clip(&self.timeline, &ref_id)
            .ok_or_else(|| format!("Reference clip '{ref_id}' was not found on the timeline."))?;
        let ref_clip =
            self.timeline.tracks[ref_loc.track_index].clips[ref_loc.clip_index].clone();
        let ref_source = self
            .media_manifest
            .entry_for(&ref_clip.media_ref)
            .map(|e| e.source.clone())
            .ok_or_else(|| "The reference clip's media isn't in the library.".to_string())?;
        // Audio correlation decodes the reference lazily — timecode syncs
        // never touch the decoder.
        let mut ref_f64: Option<Vec<f64>> = None;
        let ref_anchor = ref_clip.start_frame - ref_clip.trim_start_frame;
        // Embedded source timecode in seconds (frame / quanta), when present.
        let tc_seconds = |media_ref: &str, manifest: &MediaManifest| -> Option<f64> {
            let e = manifest.entry_for(media_ref)?;
            let frame = e.source_timecode_frame? as f64;
            let quanta = e.source_timecode_quanta.filter(|q| *q > 0)? as f64;
            Some(frame / quanta)
        };
        let ref_tc = tc_seconds(&ref_clip.media_ref, &self.media_manifest);

        let fps = self.timeline.fps as f64;
        let mut synced: Vec<Value> = Vec::new();
        let mut skipped: Vec<Value> = Vec::new();
        for tid in &target_ids {
            if *tid == ref_id {
                continue;
            }
            let Some(tloc) = timeline_core::find_clip(&self.timeline, tid) else {
                skipped.push(json!({"clipId": tid, "reason": "not found"}));
                continue;
            };
            let tclip = self.timeline.tracks[tloc.track_index].clips[tloc.clip_index].clone();
            let Some(tsource) = self
                .media_manifest
                .entry_for(&tclip.media_ref)
                .map(|e| e.source.clone())
            else {
                skipped.push(json!({"clipId": tid, "reason": "media not in library"}));
                continue;
            };
            // Timecode first (mode auto/timecode): both files carrying an
            // embedded timecode align exactly, confidence 1.0.
            let tgt_tc = tc_seconds(&tclip.media_ref, &self.media_manifest);
            let timecode_offset: Option<i64> = match (mode, ref_tc, tgt_tc) {
                ("audio", _, _) => None,
                // Correlator sign convention: positive offset = the shared
                // content sits LATER in the target's file = the target
                // started recording EARLIER (smaller timecode).
                (_, Some(r), Some(t)) => Some(((r - t) * fps).round() as i64),
                ("timecode", _, _) => {
                    skipped.push(json!({
                        "clipId": tid,
                        "reason": "timecode mode: both files need an embedded source timecode"
                    }));
                    continue;
                }
                _ => None,
            };

            let (offset_frames, confidence, method) = if let Some(off) = timecode_offset {
                (off, 1.0f64, "timecode")
            } else {
                let Some(audio) = &audio else {
                    skipped.push(json!({
                        "clipId": tid,
                        "reason": "no embedded timecode and no audio decoder connected"
                    }));
                    continue;
                };
                if ref_f64.is_none() {
                    let Some(pcm) = audio.decode_source_pcm(&ref_source, sample_rate, channels)
                    else {
                        return Err("Could not decode the reference clip's audio.".to_string());
                    };
                    ref_f64 = Some(pcm.iter().map(|&s| s as f64).collect());
                }
                let Some(tpcm) = audio.decode_source_pcm(&tsource, sample_rate, channels) else {
                    skipped.push(json!({"clipId": tid, "reason": "could not decode audio"}));
                    continue;
                };
                let tf64: Vec<f64> = tpcm.iter().map(|&s| s as f64).collect();
                match AudioSyncCorrelator::find_sync_offset_windowed(
                    ref_f64.as_ref().expect("decoded above"),
                    &tf64,
                    sample_rate as f64,
                    frame_size,
                    fps,
                    Some(search_window_seconds),
                ) {
                    Some(off) if off.confidence >= min_confidence => {
                        (off.offset_frames, off.confidence, "audio")
                    }
                    Some(off) => {
                        skipped.push(json!({
                            "clipId": tid, "reason": "low confidence", "confidence": off.confidence
                        }));
                        continue;
                    }
                    None => {
                        skipped.push(json!({"clipId": tid, "reason": "no match found"}));
                        continue;
                    }
                }
            };

            // A delayed target (positive offset) must move earlier; align the
            // clips' source-sample-0 anchors (start_frame - trim_start_frame).
            let tgt_anchor = tclip.start_frame - tclip.trim_start_frame;
            let delta = ref_anchor - tgt_anchor - offset_frames;
            let new_start = (tclip.start_frame + delta).max(0);
            // move_clips re-inserts moved clips under NEW ids — report the
            // new id or the agent is left holding a dead reference.
            let placed = timeline_core::move_clips(
                &mut self.timeline,
                &[tid.clone()],
                tloc.track_index,
                new_start,
            );
            let new_id = placed.first().cloned().unwrap_or_else(|| tid.clone());
            synced.push(json!({
                "clipId": tid,
                "newClipId": new_id,
                "offsetFrames": offset_frames,
                "movedToFrame": new_start,
                "confidence": crate::timeline_v2::round3(confidence),
                "method": method,
            }));
        }

        Ok(json!({ "synced": synced, "skipped": skipped }))
    }

    /// DETECT_BEATS (tool-surface-v2): on-device beat/downbeat detection on a
    /// media asset's audio, in SOURCE seconds + estimated bpm. The whole file
    /// is analysed once and cached; startSeconds/endSeconds trim the response.
    /// Decoding runs through the ClipAudioSource host seam.
    fn cmd_detect_beats(&mut self, args: &Value) -> Result<Value, String> {
        let media_ref = args
            .get("mediaRef")
            .and_then(|v| v.as_str())
            .ok_or_else(|| "Missing mediaRef".to_string())?;
        let entry = self
            .media_manifest
            .entry_for(media_ref)
            .ok_or_else(|| format!("Media '{media_ref}' not found"))?;
        if !matches!(entry.r#type, ClipType::Audio | ClipType::Video) {
            return Err(format!(
                "detect_beats works on audio or video assets; '{}' is {}.",
                entry.name,
                entry.r#type.name()
            ));
        }
        if !self.beat_cache.contains_key(media_ref) {
            let audio = self.audio_source.clone().ok_or_else(|| {
                "detect_beats is unavailable: no audio decoder is connected (run it from the app)."
                    .to_string()
            })?;
            let sample_rate = 44_100u32;
            let pcm = audio
                .decode_source_pcm(&entry.source, sample_rate, 1)
                .ok_or_else(|| format!("Could not decode audio from '{}'.", entry.name))?;
            let analysis = audio_core::beat_detector::detect_beats(&pcm, 1, sample_rate);
            self.beat_cache.insert(media_ref.to_string(), analysis);
        }
        let analysis = &self.beat_cache[media_ref];
        let start = args.get("startSeconds").and_then(|v| v.as_f64());
        let end = args.get("endSeconds").and_then(|v| v.as_f64());
        let in_range = |t: f64| -> bool {
            start.is_none_or(|s| t >= s) && end.is_none_or(|e| t <= e)
        };
        let beats: Vec<f64> = analysis
            .beats
            .iter()
            .copied()
            .filter(|t| in_range(*t))
            .map(crate::timeline_v2::round3)
            .collect();
        let downbeats: Vec<f64> = analysis
            .downbeats
            .iter()
            .copied()
            .filter(|t| in_range(*t))
            .map(crate::timeline_v2::round3)
            .collect();
        Ok(Self::json_content(&json!({
            "bpm": crate::timeline_v2::round3(analysis.bpm),
            "beats": beats,
            "downbeats": downbeats,
        })))
    }

    /// DENOISE_AUDIO (upstream #251): toggle the `audio.denoise` effect on audio
    /// clips, mirroring Swift `EditorViewModel.setDenoise` merge semantics exactly:
    /// re-enabling without a strength keeps each clip's existing amount; only
    /// clips with no denoise get the 0.6 default. The DeepFilterNet3 bake itself
    /// is a host concern — the setting round-trips with Palmier Pro.
    fn cmd_denoise_audio(&mut self, args: &Value) -> Result<Value, String> {
        const DENOISE_TYPE: &str = "audio.denoise";
        const DEFAULT_AMOUNT: f64 = 0.6;

        let clip_ids: Vec<String> = args
            .get("clipIds")
            .and_then(|v| v.as_array())
            .map(|a| a.iter().filter_map(|v| v.as_str().map(String::from)).collect())
            .unwrap_or_default();
        if clip_ids.is_empty() {
            return Err("clipIds is empty.".to_string());
        }
        let strength = args.get("strength").and_then(|v| v.as_f64());
        if let Some(s) = strength {
            if !(0.0..=100.0).contains(&s) {
                return Err(format!("strength must be 0–100 (got {s})"));
            }
        }
        let enabled = args.get("enabled").and_then(|v| v.as_bool()).unwrap_or(true);

        // Validate every clip up-front (Swift does) so one bad id mutates nothing.
        for id in &clip_ids {
            let Some(loc) = timeline_core::find_clip(&self.timeline, id) else {
                return Err(format!("Clip not found: {id}"));
            };
            let clip = &self.timeline.tracks[loc.track_index].clips[loc.clip_index];
            if clip.media_type != ClipType::Audio {
                return Err(format!(
                    "Clip {id} is a {:?} clip; denoise_audio needs an audio clip.",
                    clip.media_type
                ));
            }
        }

        let clamped = strength.map(|s| (s / 100.0).clamp(0.0, 1.0));
        for id in &clip_ids {
            let loc = timeline_core::find_clip(&self.timeline, id).expect("validated above");
            let clip = &mut self.timeline.tracks[loc.track_index].clips[loc.clip_index];
            let mut stack = clip.effects.take().unwrap_or_default();
            let current_amount = stack
                .iter()
                .find(|e| e.r#type == DENOISE_TYPE)
                .and_then(|e| e.params.get("amount"))
                .and_then(|p| p.value);
            stack.retain(|e| e.r#type != DENOISE_TYPE);
            if enabled {
                let value = clamped.or(current_amount).unwrap_or(DEFAULT_AMOUNT);
                stack.push(Effect::new(DENOISE_TYPE, vec![("amount", value)]));
            }
            clip.effects = if stack.is_empty() { None } else { Some(stack) };
        }

        Ok(json!({}))
    }

    /// CREATE_TIMELINE (#255): new empty timeline inheriting the active one's
    /// settings; switches to it. Like Swift, the switch itself isn't undoable.
    /// CREATE_TIMELINE (tool-surface-v2, absorbs duplicate_timeline via
    /// 'from'): without 'from' creates an empty timeline inheriting
    /// fps/resolution; with 'from' deep-copies that timeline (all-new
    /// clip/track/link ids). Both switch the active timeline.
    fn cmd_create_timeline(&mut self, args: &Value) -> Result<Value, String> {
        let name_arg = args
            .get("name")
            .and_then(|v| v.as_str())
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(String::from);
        let from = args
            .get("from")
            .and_then(|v| v.as_str())
            .map(str::trim)
            .filter(|s| !s.is_empty());
        let (new_tl, note) = match from {
            Some(source_id) => (
                self.duplicated_timeline(source_id, name_arg)?,
                "Every clip and track id in the copy is new — re-read get_timeline before editing.",
            ),
            None => (
                Timeline {
                    name: name_arg.unwrap_or_else(|| {
                        format!("Timeline {}", self.sibling_timelines.len() + 2)
                    }),
                    fps: self.timeline.fps,
                    width: self.timeline.width,
                    height: self.timeline.height,
                    settings_configured: self.timeline.settings_configured,
                    ..Default::default()
                },
                "The timeline is empty; every read and edit tool now targets it.",
            ),
        };
        let id = new_tl.id.clone();
        let name = new_tl.name.clone();
        let prev = std::mem::replace(&mut self.timeline, new_tl);
        self.sibling_timelines.push(prev);
        // Undo snapshots hold the PREVIOUS timeline's state; applying one to the
        // new active timeline would overwrite it wholesale. Clear on switch.
        self.undo_stack.clear();
        Ok(json!({ "content": [{ "type": "text", "text": serde_json::to_string(&json!({
            "timelineId": id, "name": name, "active": true, "note": note
        })).unwrap_or_default() }]}))
    }

    /// Deep-copy a timeline with fresh clip/track ids, keeping link groups
    /// intact via a per-group remap (create_timeline 'from').
    fn duplicated_timeline(
        &self,
        source_id: &str,
        name: Option<String>,
    ) -> Result<Timeline, String> {
        let source = if self.timeline.id == source_id {
            self.timeline.clone()
        } else {
            self.sibling_timelines
                .iter()
                .find(|t| t.id == source_id)
                .cloned()
                .ok_or_else(|| {
                    format!(
                        "No timeline with id '{source_id}'. get_timeline lists the project's timelines."
                    )
                })?
        };
        let source_name = source.name.clone();
        let mut copy = source;
        copy.id = Uuid::new_v4().to_string();
        copy.name = name.unwrap_or_else(|| format!("{source_name} copy"));
        copy.selected_clip_ids.clear();
        for track in &mut copy.tracks {
            track.id = Uuid::new_v4().to_string();
            let mut group_map: std::collections::HashMap<String, String> =
                std::collections::HashMap::new();
            for clip in &mut track.clips {
                clip.id = Uuid::new_v4().to_string();
                if let Some(g) = &clip.link_group_id {
                    let new_g = group_map
                        .entry(g.clone())
                        .or_insert_with(|| Uuid::new_v4().to_string())
                        .clone();
                    clip.link_group_id = Some(new_g);
                }
            }
        }
        Ok(copy)
    }

    /// SET_ACTIVE_TIMELINE (#255): swap the active timeline. Exempt from undo
    /// (a switch changes the target without registering an undo — Swift parity).
    fn cmd_set_active_timeline(&mut self, args: &Value) -> Result<Value, String> {
        let id = args
            .get("timelineId")
            .and_then(|v| v.as_str())
            .ok_or_else(|| "timelineId is required".to_string())?;
        if self.timeline.id == id {
            let name = self.timeline.name.clone();
            return Ok(json!({ "content": [{ "type": "text", "text": format!(
                "\"{name}\" is already the active timeline."
            )}]}));
        }
        let Some(pos) = self.sibling_timelines.iter().position(|t| t.id == id) else {
            return Err(format!(
                "No timeline with id '{id}'. get_timeline lists the project's timelines."
            ));
        };
        let target = self.sibling_timelines.remove(pos);
        let prev = std::mem::replace(&mut self.timeline, target);
        self.sibling_timelines.push(prev);
        self.undo_stack.clear();
        let name = self.timeline.name.clone();
        let frames = timeline_core::TimelineMathExt::total_frames(&self.timeline);
        let fps = self.timeline.fps;
        Ok(json!({ "content": [{ "type": "text", "text": format!(
            "Active timeline: \"{name}\" ({frames} frames, {fps} fps). Re-read get_timeline before editing."
        )}]}))
    }

    /// Build the carrier clip(s) that place `child` as a nested timeline
    /// (upstream #255): a video sequence carrier, plus a linked audio carrier
    /// when the child has audio clips. Rejects empty children and cycles.
    fn nest_carrier_clips(&self, child: &Timeline, args: &Value) -> Result<Vec<Clip>, String> {
        let (trim_start, duration) = self.nest_window(child, args)?;

        let has_audio = child
            .tracks
            .iter()
            .any(|t| t.r#type == ClipType::Audio && !t.clips.is_empty());
        let link_group = has_audio.then(|| Uuid::new_v4().to_string());

        let base = Clip {
            id: Uuid::new_v4().to_string(),
            media_ref: child.id.clone(),
            media_type: ClipType::Sequence,
            source_clip_type: ClipType::Sequence,
            start_frame: 0,
            duration_frames: duration,
            trim_start_frame: trim_start,
            trim_end_frame: 0,
            speed: 1.0,
            volume: 1.0,
            fade_in_frames: 0,
            fade_out_frames: 0,
            fade_in_interpolation: Interpolation::Linear,
            fade_out_interpolation: Interpolation::Linear,
            opacity: 1.0,
            transform: Transform::default(),
            crop: core_model::Crop::default(),
            link_group_id: link_group.clone(),
            caption_group_id: None,
            text_content: None,
            text_style: None,
            text_animation: None,
            word_timings: None,
            opacity_track: None,
            position_track: None,
            scale_track: None,
            rotation_track: None,
            crop_track: None,
            volume_track: None,
            effects: None,
            shape_style: None,
            stroke_progress_track: None,
            compound_timeline_id: None,
            blend_mode: Default::default(),
            chroma_key: None,
            multicam_group_id: None,
        };
        let mut out = vec![base.clone()];
        if has_audio {
            let mut audio = base;
            audio.id = Uuid::new_v4().to_string();
            audio.media_type = ClipType::Audio;
            out.push(audio);
        }
        Ok(out)
    }

    /// Validate nesting `child` into the active timeline and compute the
    /// carrier's (trim_start, duration) window from the args (upstream #255).
    fn nest_window(&self, child: &Timeline, args: &Value) -> Result<(i64, i64), String> {
        let child_total = timeline_core::TimelineMathExt::total_frames(child);
        if child_total <= 0 {
            return Err(format!(
                "Timeline '{}' is empty — add clips to it before nesting it.",
                child.name
            ));
        }
        if self.timeline_reaches(child, &self.timeline.id) {
            return Err(format!(
                "Placing timeline '{}' here would create a cycle (it contains the active timeline).",
                child.name
            ));
        }
        let trim_start = args
            .get("trimStartFrame")
            .and_then(|v| v.as_i64())
            .unwrap_or(0)
            .clamp(0, (child_total - 1).max(0));
        let duration = args
            .get("durationFrames")
            .and_then(|v| v.as_i64())
            .unwrap_or(child_total - trim_start)
            .clamp(1, child_total - trim_start);
        Ok((trim_start, duration))
    }

    /// True when `target_id` is reachable from `from` through sequence carriers
    /// (via the sibling map), depth-capped like NestFlattener.
    fn timeline_reaches(&self, from: &Timeline, target_id: &str) -> bool {
        let mut queue: Vec<&Timeline> = vec![from];
        let mut visited: std::collections::HashSet<&str> = Default::default();
        let mut depth = 0;
        while !queue.is_empty() && depth < timeline_core::NEST_MAX_DEPTH {
            let mut next = Vec::new();
            for t in queue {
                if !visited.insert(t.id.as_str()) {
                    continue;
                }
                for clip in t.tracks.iter().flat_map(|tr| &tr.clips) {
                    if clip.source_clip_type != ClipType::Sequence {
                        continue;
                    }
                    if clip.media_ref == target_id {
                        return true;
                    }
                    if let Some(c) =
                        self.sibling_timelines.iter().find(|s| s.id == clip.media_ref)
                    {
                        next.push(c);
                    }
                }
            }
            queue = next;
            depth += 1;
        }
        false
    }

    /// UPDATE_TEXT (upstream): merge content/typography/transform/animation
    /// changes into existing text clips, addressed by clipIds or captionGroupId.
    fn cmd_update_text(&mut self, args: &Value) -> Result<Value, String> {
        let mut ids: Vec<String> = args
            .get("clipIds")
            .and_then(|v| v.as_array())
            .map(|a| a.iter().filter_map(|v| v.as_str().map(String::from)).collect())
            .unwrap_or_default();
        if let Some(group) = args.get("captionGroupId").and_then(|v| v.as_str()) {
            for track in &self.timeline.tracks {
                for clip in &track.clips {
                    if clip.caption_group_id.as_deref() == Some(group)
                        && !ids.iter().any(|i| i == &clip.id)
                    {
                        ids.push(clip.id.clone());
                    }
                }
            }
            if ids.is_empty() {
                return Err(format!("No caption clips found for captionGroupId '{group}'."));
            }
        }
        if ids.is_empty() {
            return Err("update_text requires 'clipIds' or 'captionGroupId'.".to_string());
        }

        // Validate all targets first: they must exist and be text clips.
        for id in &ids {
            let Some(loc) = timeline_core::find_clip(&self.timeline, id) else {
                return Err(format!("Clip not found: {id}"));
            };
            let clip = &self.timeline.tracks[loc.track_index].clips[loc.clip_index];
            if clip.media_type != ClipType::Text {
                return Err(format!(
                    "Clip {id} is a {:?} clip; update_text needs text clips.",
                    clip.media_type
                ));
            }
        }

        // Parse shared updates once (invalid values reject before any mutation).
        let content = args.get("content").and_then(|v| v.as_str());
        let font_name = args.get("fontName").and_then(|v| v.as_str());
        let font_size = args.get("fontSize").and_then(|v| v.as_f64());
        let font_weight = args.get("fontWeight").and_then(|v| v.as_f64());
        let color = match args.get("color").and_then(|v| v.as_str()) {
            Some(hex) => Some(core_model::TextRgba::from_hex(hex).ok_or_else(|| {
                format!("invalid color '{hex}'. Expected '#RGB', '#RRGGBB', or '#RRGGBBAA'")
            })?),
            None => None,
        };
        let alignment = match args.get("alignment").and_then(|v| v.as_str()) {
            Some(a) => Some(core_model::TextAlignment::from_name(a).ok_or_else(|| {
                format!("invalid alignment '{a}'. Expected 'left', 'center', or 'right'")
            })?),
            None => None,
        };
        let animation_raw = args.get("animation").and_then(|v| v.as_str());
        let clear_animation = animation_raw == Some("off");
        let animation = if clear_animation {
            None
        } else {
            parse_text_animation(
                animation_raw,
                args.get("highlightColor").and_then(|v| v.as_str()),
            )?
        };
        let partial_transform = args.get("transform").map(|t| timeline_core::PartialTransform {
            center_x: t.get("centerX").and_then(|v| v.as_f64()),
            center_y: t.get("centerY").and_then(|v| v.as_f64()),
            width: t.get("width").and_then(|v| v.as_f64()),
            height: t.get("height").and_then(|v| v.as_f64()),
            rotation: t.get("rotation").and_then(|v| v.as_f64()),
            flip_horizontal: None,
            flip_vertical: None,
        });

        // v2 flattened style additions: isBold/isItalic/borderColor/backgroundColor.
        let is_bold = args.get("isBold").and_then(|v| v.as_bool());
        let is_italic = args.get("isItalic").and_then(|v| v.as_bool());
        let border_color = match args.get("borderColor").and_then(|v| v.as_str()) {
            Some(hex) => Some(core_model::TextRgba::from_hex(hex).ok_or_else(|| {
                format!("invalid borderColor '{hex}'. Expected '#RGB', '#RRGGBB', or '#RRGGBBAA'")
            })?),
            None => None,
        };
        let background_color = match args.get("backgroundColor").and_then(|v| v.as_str()) {
            Some(hex) => Some(core_model::TextRgba::from_hex(hex).ok_or_else(|| {
                format!(
                    "invalid backgroundColor '{hex}'. Expected '#RGB', '#RRGGBB', or '#RRGGBBAA'"
                )
            })?),
            None => None,
        };

        for id in &ids {
            let loc = timeline_core::find_clip(&self.timeline, id).expect("validated above");
            let clip = &mut self.timeline.tracks[loc.track_index].clips[loc.clip_index];
            if let Some(c) = content {
                clip.text_content = Some(c.to_string());
            }
            if font_name.is_some()
                || font_size.is_some()
                || font_weight.is_some()
                || color.is_some()
                || alignment.is_some()
                || is_bold.is_some()
                || is_italic.is_some()
                || border_color.is_some()
                || background_color.is_some()
            {
                let style = clip.text_style.get_or_insert_with(TextStyle::default);
                if let Some(f) = font_name {
                    style.font_name = f.to_string();
                }
                if let Some(sz) = font_size {
                    style.font_size = sz;
                }
                if let Some(w) = font_weight {
                    style.font_weight = w;
                }
                if let Some(b) = is_bold {
                    style.font_weight = if b { 700.0 } else { 400.0 };
                }
                if let Some(i) = is_italic {
                    style.is_italic = i;
                }
                if let Some(c) = color {
                    style.color = c;
                }
                if let Some(a) = alignment {
                    style.alignment = a;
                }
                if let Some(bc) = &border_color {
                    style.border.enabled = true;
                    style.border.color = *bc;
                }
                if let Some(bg) = &background_color {
                    style.background.enabled = true;
                    style.background.color = *bg;
                }
            }
            if let Some(pt) = &partial_transform {
                clip.transform = pt.merge_into(&clip.transform);
            }
            if clear_animation {
                clip.text_animation = None;
            } else if let Some(anim) = &animation {
                clip.text_animation = Some(anim.clone());
            }
        }

        let _ = ids;
        Ok(json!({}))
    }

    /// EXPORT_PROJECT: validate the request and hand it to the host exporter.
    fn cmd_export_project(&mut self, args: &Value) -> Result<Value, String> {
        let mode = args
            .get("mode")
            .and_then(|v| v.as_str())
            .unwrap_or("video")
            .to_string();
        if !matches!(mode.as_str(), "video" | "xml" | "fcpxml" | "palmier") {
            return Err(format!(
                "export_project: unknown mode '{mode}'. Use video, xml, fcpxml, or palmier."
            ));
        }
        let codec = args
            .get("codec")
            .and_then(|v| v.as_str())
            .unwrap_or("H.264")
            .to_string();
        if !matches!(codec.as_str(), "H.264" | "H.265" | "ProRes") {
            return Err(format!(
                "export_project: unknown codec '{codec}'. Use H.264, H.265, or ProRes."
            ));
        }
        let resolution = args
            .get("resolution")
            .and_then(|v| v.as_str())
            .unwrap_or("Match Timeline")
            .to_string();
        if !matches!(
            resolution.as_str(),
            "720p" | "1080p" | "2K" | "4K" | "Match Timeline"
        ) {
            return Err(format!(
                "export_project: unknown resolution '{resolution}'. Use 720p, 1080p, 2K, 4K, or Match Timeline."
            ));
        }
        let fcpxml_target = args
            .get("fcpxmlTarget")
            .and_then(|v| v.as_str())
            .unwrap_or("resolve")
            .to_string();
        if !matches!(fcpxml_target.as_str(), "resolve" | "fcp") {
            return Err(format!(
                "export_project: unknown fcpxmlTarget '{fcpxml_target}'. Use resolve or fcp."
            ));
        }
        let timeline = match args.get("timelineId").and_then(|v| v.as_str()) {
            Some(id) => {
                if mode == "palmier" {
                    return Err(
                        "export_project: timelineId doesn't apply to palmier mode — the package carries every timeline"
                            .to_string(),
                    );
                }
                if self.timeline.id == id {
                    self.timeline.clone()
                } else {
                    self.sibling_timelines
                        .iter()
                        .find(|t| t.id == id)
                        .cloned()
                        .ok_or_else(|| {
                            format!(
                                "export_project: no timeline with id '{id}'. get_timeline lists the project's timelines."
                            )
                        })?
                }
            }
            None => self.timeline.clone(),
        };

        let host = self.export_host.clone().ok_or_else(|| {
            "export_project is unavailable: no exporter is connected (run it from the app)."
                .to_string()
        })?;
        let request = ExportRequest {
            mode,
            codec,
            resolution,
            output_path: args
                .get("outputPath")
                .and_then(|v| v.as_str())
                .map(String::from),
            overwrite: args.get("overwrite").and_then(|v| v.as_bool()).unwrap_or(true),
            fcpxml_target,
            timeline: timeline.clone(),
            sibling_timelines: self.sibling_timelines.clone(),
            manifest: self.media_manifest.clone(),
        };
        let fps = timeline.fps.max(1);
        let total = timeline_core::TimelineMathExt::total_frames(&timeline);
        let (status, path) = match host.export(request)? {
            ExportOutcome::Started { path } => ("started", path),
            ExportOutcome::Completed { path } => ("completed", path),
        };
        Ok(json!({
            "content": [{
                "type": "text",
                "text": serde_json::to_string(&json!({
                    "status": status,
                    "path": path,
                    "timeline": timeline.name,
                    "durationFrames": total,
                    "durationSeconds": total as f64 / fps as f64,
                    "fps": fps,
                }))
                .unwrap_or_default()
            }]
        }))
    }

    /// GET_PROJECTS: read-only recents list + the active project (upstream).
    fn cmd_get_projects(&self) -> Result<Value, String> {
        let lister = self.project_lister.clone().ok_or_else(|| {
            "get_projects is unavailable: no project registry is connected (run it from the app)."
                .to_string()
        })?;
        let (projects, active) = lister.list()?;
        let list: Vec<Value> = projects
            .iter()
            .map(|p| {
                let mut entry = json!({
                    "id": p.id,
                    "name": p.name,
                    "path": p.path,
                    "isOpen": p.is_open,
                });
                if p.is_active {
                    entry["isActive"] = json!(true);
                }
                entry
            })
            .collect();
        let mut out = json!({ "projects": list });
        if let Some((name, path)) = active {
            out["active"] = json!({ "name": name, "path": path });
        }
        Ok(json!({
            "content": [{
                "type": "text",
                "text": serde_json::to_string_pretty(&out).unwrap_or_default()
            }]
        }))
    }

    /// SEND_FEEDBACK: submit product feedback with diagnostics (upstream #152).
    /// Session dedup + cap count successful sends only, so a failed send stays retryable.
    fn cmd_send_feedback(&mut self, args: &Value) -> Result<Value, String> {
        let message = args
            .get("message")
            .and_then(|v| v.as_str())
            .map(str::trim)
            .filter(|m| !m.is_empty())
            .ok_or_else(|| "send_feedback requires a non-empty 'message'.".to_string())?;
        if self.feedback_sent_messages.contains(message) {
            return Err(
                "Duplicate feedback: this exact message was already sent this session.".to_string(),
            );
        }
        if self.feedback_sent_count >= FEEDBACK_SESSION_CAP {
            return Err(format!(
                "Feedback limit reached: at most {FEEDBACK_SESSION_CAP} messages per session."
            ));
        }
        // No feedback backend is run for Fronda — direct the user to file a
        // GitHub issue instead of failing. Nothing is sent, so the dedup/cap
        // state is untouched. (A host may still install a sender.)
        let Some(sender) = self.feedback_sender.clone() else {
            return Ok(json!({ "content": [{ "type": "text", "text": format!(
                "Fronda has no in-app feedback service. Ask the user to file this at {FEEDBACK_ISSUES_URL} — \
                 the app's Send Feedback command opens that page."
            )}]}));
        };
        let clips: usize = self.timeline.tracks.iter().map(|t| t.clips.len()).sum();
        let payload = FeedbackPayload {
            message: message.to_string(),
            app_version: env!("CARGO_PKG_VERSION").to_string(),
            timeline_summary: format!(
                "{} — {}x{} @ {}fps, {} tracks, {} clips, {} frames",
                self.timeline.name,
                self.timeline.width,
                self.timeline.height,
                self.timeline.fps,
                self.timeline.tracks.len(),
                clips,
                timeline_core::TimelineMathExt::total_frames(&self.timeline)
            ),
        };
        sender.send(&payload)?;
        self.feedback_sent_messages.insert(message.to_string());
        self.feedback_sent_count += 1;
        Ok(json!({ "content": [{ "type": "text", "text": format!(
            "Feedback sent ({} of {FEEDBACK_SESSION_CAP} this session).",
            self.feedback_sent_count
        )}]}))
    }

    /// Swap this executor to a navigator-provided project.
    fn adopt_project(&mut self, opened: OpenedProject) -> (String, String) {
        let OpenedProject {
            name,
            root,
            timeline,
            sibling_timelines,
            manifest,
            multicam_groups,
            seams,
        } = opened;
        self.load_project(timeline, manifest);
        self.sibling_timelines = sibling_timelines;
        self.multicam_groups = multicam_groups;
        self.timeline_words = Vec::new();
        self.clip_presets.clear();
        self.search_status = String::new();
        self.matte_writer = Some(seams.matte_writer);
        self.audio_source = Some(seams.audio_source);
        self.export_host = Some(seams.export_host);
        self.project_lister = Some(seams.project_lister);
        (name, root)
    }

    /// OPEN_PROJECT: make another project the active one (upstream).
    fn cmd_open_project(&mut self, args: &Value) -> Result<Value, String> {
        let nav = self.project_navigator.clone().ok_or_else(|| {
            "open_project is unavailable: no project navigator is connected (run it from the app)."
                .to_string()
        })?;
        let id = args.get("id").and_then(|v| v.as_str());
        let path = args.get("path").and_then(|v| v.as_str());
        if id.is_none() && path.is_none() {
            return Err("open_project requires 'id' (from get_projects) or 'path'.".to_string());
        }
        let opened = nav.open(id, path)?;
        let (name, root) = self.adopt_project(opened);
        Ok(json!({ "content": [{ "type": "text", "text": format!(
            "Opened \"{name}\" ({root}) and made it active. Re-read get_timeline and get_media before editing."
        )}]}))
    }

    /// NEW_PROJECT: create an empty project and make it active (upstream).
    fn cmd_new_project(&mut self, args: &Value) -> Result<Value, String> {
        let nav = self.project_navigator.clone().ok_or_else(|| {
            "new_project is unavailable: no project navigator is connected (run it from the app)."
                .to_string()
        })?;
        let opened = nav.create(args.get("name").and_then(|v| v.as_str()))?;
        let (name, root) = self.adopt_project(opened);
        Ok(json!({ "content": [{ "type": "text", "text": format!(
            "Created \"{name}\" ({root}) and made it active. It is empty; all edit tools now target it."
        )}]}))
    }

    /// CLOSE_PROJECT (tool-surface-v2): save-first close via the navigator.
    /// The navigator resolves the target (all-None = the active project),
    /// refuses projects that aren't open, and saves before closing; the
    /// executor then adopts the next active project or resets to the
    /// no-project state.
    fn cmd_close_project(&mut self, args: &Value) -> Result<Value, String> {
        let nav = self.project_navigator.clone().ok_or_else(|| {
            "close_project is unavailable: no project navigator is connected (run it from the app)."
                .to_string()
        })?;
        let name = args.get("name").and_then(|v| v.as_str());
        let id = args.get("id").and_then(|v| v.as_str());
        let path = args.get("path").and_then(|v| v.as_str());
        let active = ActiveProjectState {
            timeline: self.timeline.clone(),
            sibling_timelines: self.sibling_timelines.clone(),
            manifest: self.media_manifest.clone(),
            multicam_groups: self.saved_multicam_groups().unwrap_or_default(),
        };
        let closed = nav.close(name, id, path, active)?;
        let mut out = json!({
            "status": "closed",
            "name": closed.name,
            "openCount": closed.open_count,
        });
        if closed.was_active {
            match closed.next_active {
                Some(opened) => {
                    out["active"] = json!({ "name": opened.name, "path": opened.root });
                    self.adopt_project(opened);
                }
                None => self.reset_to_no_project(closed.lister),
            }
        }
        Ok(json!({
            "content": [{
                "type": "text",
                "text": serde_json::to_string_pretty(&out).unwrap_or_default()
            }]
        }))
    }

    /// Clear project-scoped state and seams after the last project closes
    /// (Home-window state). The account-scoped navigator survives, and the
    /// navigator may hand back a rootless lister, so get_projects /
    /// open_project / new_project keep working.
    fn reset_to_no_project(&mut self, lister: Option<Arc<dyn ProjectLister>>) {
        self.load_project(Timeline::default(), MediaManifest::default());
        self.sibling_timelines.clear();
        self.multicam_groups.clear();
        self.timeline_words = Vec::new();
        self.clip_presets.clear();
        self.search_status = String::new();
        self.matte_writer = None;
        self.audio_source = None;
        self.export_host = None;
        if let Some(l) = lister {
            self.project_lister = Some(l);
        }
    }

    fn cmd_duplicate_project(&mut self) -> Result<Value, String> {
        // Upstream #67: duplicating a project copies its .palmier package on disk and
        // reopens the copy as current. That is host filesystem I/O the in-memory
        // ToolExecutor cannot do (it has no project path or fs handle). The pure plan
        // exists as project_io::project_duplicate::plan_duplicate; the host must
        // execute it. Report honestly rather than claiming a no-op succeeded.
        Ok(json!({
            "content": [{
                "type": "text",
                "text": "Project duplication requires host filesystem support and is not available in this context."
            }],
            "isError": true,
        }))
    }

    // ── Text / annotation tools ────────────────────────────────────────────

    /// Overwrite-place each clip at its own start_frame (ascending start
    /// order, mirroring Swift placeTextClips); returns ids in input order.
    fn place_clips_at_own_starts(
        &mut self,
        track_index: usize,
        clips: &[Clip],
    ) -> Result<Vec<String>, String> {
        let mut ids = vec![String::new(); clips.len()];
        let mut order: Vec<usize> = (0..clips.len()).collect();
        order.sort_by_key(|&i| clips[i].start_frame);
        for i in order {
            let placed = timeline_core::place_clips(
                &mut self.timeline,
                track_index,
                clips[i].start_frame,
                std::slice::from_ref(&clips[i]),
            );
            ids[i] = placed
                .into_iter()
                .next()
                .ok_or_else(|| "Failed to place clip".to_string())?;
        }
        Ok(ids)
    }

    fn cmd_add_texts(&mut self, args: &Value) -> Result<Value, String> {
        // v2 shape: entries with per-entry startFrame/endFrame; the legacy
        // 'texts' array (durationFrames) still parses for older callers.
        let texts_val = args
            .get("entries")
            .or_else(|| args.get("texts"))
            .and_then(|v| v.as_array())
            .ok_or_else(|| "Missing entries array".to_string())?;

        // v2: per-entry trackIndex (all entries must agree today — one track
        // per call); top-level trackIndex still accepted.
        let track_index = args
            .get("trackIndex")
            .or_else(|| texts_val.first().and_then(|e| e.get("trackIndex")))
            .and_then(|v| v.as_i64())
            .unwrap_or(-1) as usize;

        // Find or create a text track
        let ti = if track_index < self.timeline.tracks.len() {
            track_index
        } else {
            // Find existing text/visual track or create one
            let existing = self.timeline.tracks.iter().position(|t| {
                t.r#type == core_model::ClipType::Text || t.r#type == core_model::ClipType::Video
            });
            match existing {
                Some(idx) => idx,
                None => {
                    timeline_core::insert_track_at(
                        &mut self.timeline,
                        0,
                        core_model::ClipType::Video,
                    )
                    .map_err(|_| "Failed to create track".to_string())?;
                    0
                }
            }
        };

        let mut clips: Vec<Clip> = Vec::new();
        let mut current_frame = 0i64;

        // Find the max end frame on this track for placement
        for clip in &self.timeline.tracks[ti].clips {
            let end = clip.start_frame + clip.duration_frames;
            if end > current_frame {
                current_frame = end;
            }
        }

        for t_val in texts_val {
            // Accept Swift's `content` key, falling back to `text`.
            let text = t_val
                .get("content")
                .or_else(|| t_val.get("text"))
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let start_frame = t_val
                .get("startFrame")
                .and_then(|v| v.as_i64())
                .unwrap_or(current_frame);
            // v2: endFrame (exclusive); legacy durationFrames still parses.
            let duration_frames = t_val
                .get("endFrame")
                .and_then(|v| v.as_i64())
                .map(|end| end - start_frame)
                .or_else(|| t_val.get("durationFrames").and_then(|v| v.as_i64()))
                .unwrap_or(150);
            crate::mutation::require_frame_in_bounds(start_frame, "startFrame")?;
            crate::mutation::require_frame_in_bounds(duration_frames, "durationFrames")?;
            if start_frame < 0 {
                return Err(format!("startFrame must be >= 0 (got {start_frame})"));
            }
            if duration_frames < 1 {
                return Err(format!("durationFrames must be >= 1 (got {duration_frames})"));
            }

            // Per-entry text styling (reuses the set_clip_properties parsers).
            let mut style = TextStyle::default();
            if let Some(f) = t_val.get("fontName").and_then(|v| v.as_str()) {
                style.font_name = f.to_string();
            }
            if let Some(s) = t_val.get("fontSize").and_then(|v| v.as_f64()) {
                style.font_size = s;
            }
            if let Some(w) = t_val.get("fontWeight").and_then(|v| v.as_f64()) {
                style.font_weight = w;
            }
            if let Some(b) = t_val.get("isBold").and_then(|v| v.as_bool()) {
                style.font_weight = if b { 700.0 } else { 400.0 };
            }
            if let Some(i) = t_val.get("isItalic").and_then(|v| v.as_bool()) {
                style.is_italic = i;
            }
            if let Some(hex) = t_val.get("color").and_then(|v| v.as_str()) {
                style.color = core_model::TextRgba::from_hex(hex).ok_or_else(|| {
                    format!("invalid color '{hex}'. Expected '#RGB', '#RRGGBB', or '#RRGGBBAA'")
                })?;
            }
            if let Some(a) = t_val.get("alignment").and_then(|v| v.as_str()) {
                style.alignment = core_model::TextAlignment::from_name(a).ok_or_else(|| {
                    format!("invalid alignment '{a}'. Expected 'left', 'center', or 'right'")
                })?;
            }
            if let Some(hex) = t_val.get("borderColor").and_then(|v| v.as_str()) {
                style.border.enabled = true;
                style.border.color = core_model::TextRgba::from_hex(hex).ok_or_else(|| {
                    format!("invalid borderColor '{hex}'. Expected '#RGB', '#RRGGBB', or '#RRGGBBAA'")
                })?;
            }
            if let Some(hex) = t_val.get("backgroundColor").and_then(|v| v.as_str()) {
                style.background.enabled = true;
                style.background.color = core_model::TextRgba::from_hex(hex).ok_or_else(|| {
                    format!("invalid backgroundColor '{hex}'. Expected '#RGB', '#RRGGBB', or '#RRGGBBAA'")
                })?;
            }

            // Explicit box override; partial (centre-only) shifts position, keeping
            // the default size. Auto-fit-to-content sizing is deferred (needs text
            // measurement, which lives in the render layer).
            let mut transform = Transform::default();
            if let Some(t) = t_val.get("transform") {
                transform = timeline_core::PartialTransform {
                    center_x: t.get("centerX").and_then(|v| v.as_f64()),
                    center_y: t.get("centerY").and_then(|v| v.as_f64()),
                    width: t.get("width").and_then(|v| v.as_f64()),
                    height: t.get("height").and_then(|v| v.as_f64()),
                    rotation: t.get("rotation").and_then(|v| v.as_f64()),
                    flip_horizontal: None,
                    flip_vertical: None,
                }
                .merge_into(&transform);
            }

            let text_animation = parse_text_animation(
                t_val.get("animation").and_then(|v| v.as_str()),
                t_val.get("highlightColor").and_then(|v| v.as_str()),
            )?;

            let clip = Clip {
                id: Uuid::new_v4().to_string(),
                media_ref: String::new(),
                media_type: core_model::ClipType::Text,
                source_clip_type: core_model::ClipType::Text,
                start_frame,
                duration_frames,
                trim_start_frame: 0,
                trim_end_frame: 0,
                speed: 1.0,
                volume: 1.0,
                fade_in_frames: 0,
                fade_out_frames: 0,
                fade_in_interpolation: Interpolation::Linear,
                fade_out_interpolation: Interpolation::Linear,
                opacity: 1.0,
                transform,
                crop: core_model::Crop::default(),
                link_group_id: None,
                caption_group_id: None,
                text_content: Some(if text.is_empty() {
                    "Text".to_string()
                } else {
                    text.to_string()
                }),
                text_style: Some(style),
                opacity_track: None,
                position_track: None,
                scale_track: None,
                rotation_track: None,
                crop_track: None,
                volume_track: None,
                effects: None,
                shape_style: None,
                stroke_progress_track: None,
                compound_timeline_id: None,
                blend_mode: Default::default(),
                chroma_key: None,
                multicam_group_id: None,
                text_animation,
                word_timings: None,
            };
            clips.push(clip);
            current_frame = start_frame + duration_frames;
        }

        let created_ids = self.place_clips_at_own_starts(ti, &clips)?;
        let _ = created_ids;
        Ok(json!({}))
    }

    fn cmd_add_shapes(&mut self, args: &Value) -> Result<Value, String> {
        let entries = args
            .get("entries")
            .and_then(|v| v.as_array())
            .ok_or_else(|| "Missing entries array".to_string())?;

        // Find or create a video track
        let ti = match self
            .timeline
            .tracks
            .iter()
            .position(|t| t.r#type == core_model::ClipType::Video)
        {
            Some(idx) => idx,
            None => {
                timeline_core::insert_track_at(&mut self.timeline, 0, core_model::ClipType::Video)
                    .map_err(|_| "Failed to create track".to_string())?;
                0
            }
        };

        let mut current_frame = 0i64;
        for clip in &self.timeline.tracks[ti].clips {
            let end = clip.start_frame + clip.duration_frames;
            if end > current_frame {
                current_frame = end;
            }
        }

        let mut clips: Vec<Clip> = Vec::new();

        for entry in entries {
            let shape_type = entry.get("type").and_then(|v| v.as_str()).unwrap_or("rect");
            let start_frame = entry
                .get("startFrame")
                .and_then(|v| v.as_i64())
                .unwrap_or(current_frame);
            let duration_frames = entry
                .get("durationFrames")
                .and_then(|v| v.as_i64())
                .unwrap_or(150);
            crate::mutation::require_frame_in_bounds(start_frame, "startFrame")?;
            crate::mutation::require_frame_in_bounds(duration_frames, "durationFrames")?;
            if start_frame < 0 {
                return Err(format!("startFrame must be >= 0 (got {start_frame})"));
            }
            if duration_frames < 1 {
                return Err(format!("durationFrames must be >= 1 (got {duration_frames})"));
            }

            let shape_kind = match shape_type.to_lowercase().as_str() {
                "oval" => core_model::ShapeKind::Oval,
                "circle" => core_model::ShapeKind::Circle,
                "arrow" => core_model::ShapeKind::Arrow,
                "line" => core_model::ShapeKind::Line,
                _ => core_model::ShapeKind::Rect,
            };

            let mut shape_style = core_model::ShapeStyle {
                kind: shape_kind,
                ..core_model::ShapeStyle::default()
            };
            // style → stroke (color/width/dashed/arrowheadStyle).
            if let Some(st) = entry.get("style") {
                if let Some(hex) = st.get("color").and_then(|v| v.as_str()) {
                    shape_style.stroke.color = core_model::shape_style::Rgba::from_hex(hex)
                        .ok_or_else(|| format!("invalid style color '{hex}'"))?;
                }
                if let Some(w) = st.get("width").and_then(|v| v.as_f64()) {
                    shape_style.stroke.width = w;
                }
                if let Some(d) = st.get("dashed").and_then(|v| v.as_bool()) {
                    shape_style.stroke.dashed = d;
                }
                if let Some(a) = st.get("arrowheadStyle").and_then(|v| v.as_str()) {
                    shape_style.stroke.arrowhead_style = Some(a.to_string());
                }
            }
            // fill → enabled + colour.
            if let Some(f) = entry.get("fill") {
                if let Some(en) = f.get("enabled").and_then(|v| v.as_bool()) {
                    shape_style.fill.enabled = en;
                }
                if let Some(hex) = f.get("color").and_then(|v| v.as_str()) {
                    shape_style.fill.color = core_model::shape_style::Rgba::from_hex(hex)
                        .ok_or_else(|| format!("invalid fill color '{hex}'"))?;
                }
            }

            let mut transform = Transform::default();
            if let Some(t) = entry.get("transform") {
                transform = timeline_core::PartialTransform {
                    center_x: t.get("centerX").and_then(|v| v.as_f64()),
                    center_y: t.get("centerY").and_then(|v| v.as_f64()),
                    width: t.get("width").and_then(|v| v.as_f64()),
                    height: t.get("height").and_then(|v| v.as_f64()),
                    rotation: t.get("rotation").and_then(|v| v.as_f64()),
                    flip_horizontal: None,
                    flip_vertical: None,
                }
                .merge_into(&transform);
            }

            let clip = Clip {
                id: Uuid::new_v4().to_string(),
                media_ref: String::new(),
                media_type: core_model::ClipType::Shape,
                source_clip_type: core_model::ClipType::Shape,
                start_frame,
                duration_frames,
                trim_start_frame: 0,
                trim_end_frame: 0,
                speed: 1.0,
                volume: 1.0,
                fade_in_frames: 0,
                fade_out_frames: 0,
                fade_in_interpolation: Interpolation::Linear,
                fade_out_interpolation: Interpolation::Linear,
                opacity: 1.0,
                transform,
                crop: core_model::Crop::default(),
                link_group_id: None,
                caption_group_id: None,
                text_content: None,
                text_style: None,
                opacity_track: None,
                position_track: None,
                scale_track: None,
                rotation_track: None,
                crop_track: None,
                volume_track: None,
                effects: None,
                shape_style: Some(shape_style),
                stroke_progress_track: None,
                compound_timeline_id: None,
                blend_mode: Default::default(),
                chroma_key: None,
                multicam_group_id: None,
                text_animation: None,
                word_timings: None,
            };
            clips.push(clip);
            current_frame = start_frame + duration_frames;
        }

        let created_ids = self.place_clips_at_own_starts(ti, &clips)?;

        Ok(json!({
            "content": [{
                "type": "text",
                "text": format!("Added {} shape clip(s): {:?}", created_ids.len(), created_ids)
            }]
        }))
    }

    /// APPLY_COLOR v2 (absorbs set_color_grade): named grading knobs over
    /// clipIds with MERGE semantics; reset starts from neutral; `color`
    /// replaces the whole grade (the grade-copy path). Scalars store as
    /// per-knob `color.<knob>` effects (the compositor's vocabulary); curves,
    /// hue curves, and the LUT store as JSON-string params round-tripped by
    /// get_timeline's `color` object.
    fn cmd_apply_color(&mut self, args: &Value) -> Result<Value, String> {
        let clip_ids: Vec<String> = match args.get("clipIds").and_then(|v| v.as_array()) {
            Some(arr) => arr
                .iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect(),
            None => args
                .get("clipId")
                .and_then(|v| v.as_str())
                .map(|s| vec![s.to_string()])
                .unwrap_or_default(),
        };
        if clip_ids.is_empty() {
            return Err("Missing clipIds".to_string());
        }
        for cid in &clip_ids {
            if timeline_core::find_clip(&self.timeline, cid).is_none() {
                return Err(format!("Clip '{cid}' not found"));
            }
        }

        let reset = args.get("reset").and_then(|v| v.as_bool()).unwrap_or(false);
        let color_copy = args.get("color").and_then(|v| v.as_object()).cloned();

        // (knob, param name, min, max) — apply_color's scalar vocabulary.
        const KNOBS: &[(&str, &str, f64, f64)] = &[
            ("exposure", "ev", -3.0, 3.0),
            ("contrast", "amount", 0.5, 1.5),
            ("saturation", "amount", 0.0, 2.0),
            ("vibrance", "value", -1.0, 1.0),
            ("temperature", "amount", 2000.0, 11000.0),
            ("tint", "value", -100.0, 100.0),
            ("highlights", "value", -1.0, 1.0),
            ("shadows", "value", -1.0, 1.0),
            ("blacks", "value", -1.0, 1.0),
            ("whites", "value", -1.0, 1.0),
            ("shadowsHue", "value", 0.0, 360.0),
            ("shadowsAmount", "value", 0.0, 1.0),
            ("shadowsLum", "value", -0.5, 0.5),
            ("midsHue", "value", 0.0, 360.0),
            ("midsAmount", "value", 0.0, 1.0),
            ("midsGamma", "value", 0.5, 2.0),
            ("highsHue", "value", 0.0, 360.0),
            ("highsAmount", "value", 0.0, 1.0),
            ("highsGain", "value", 0.5, 1.5),
        ];
        let knob_args: Vec<(&str, &str, f64)> = KNOBS
            .iter()
            .filter_map(|(knob, param, lo, hi)| {
                args.get(*knob)
                    .and_then(|v| v.as_f64())
                    .map(|v| (*knob, *param, v.clamp(*lo, *hi)))
            })
            .collect();
        let curve_args: Vec<(&str, Value)> = ["masterCurve", "redCurve", "greenCurve", "blueCurve"]
            .iter()
            .filter_map(|k| args.get(*k).filter(|v| v.is_array()).map(|v| (*k, v.clone())))
            .collect();
        let hue_curves = args
            .get("hueCurves")
            .and_then(|v| v.get("targets"))
            .filter(|v| v.is_array())
            .cloned();
        let lut = args.get("lut").and_then(|v| v.as_object()).cloned();

        let has_knobs = !knob_args.is_empty()
            || !curve_args.is_empty()
            || hue_curves.is_some()
            || lut.is_some();
        if color_copy.is_some() && (reset || has_knobs) {
            return Err(
                "apply_color: 'color' replaces the whole grade — it is mutually exclusive with reset and individual knobs."
                    .to_string(),
            );
        }

        for cid in &clip_ids {
            let loc = timeline_core::find_clip(&self.timeline, cid).expect("checked above");
            let clip = &mut self.timeline.tracks[loc.track_index].clips[loc.clip_index];
            if reset || color_copy.is_some() {
                if let Some(effects) = &mut clip.effects {
                    effects.retain(|e| !e.r#type.starts_with("color."));
                }
            }
            let effects = clip.effects.get_or_insert(Vec::new());

            // The grade-copy path re-applies the pasted object's knobs.
            let copied: Vec<(String, Value)> = color_copy
                .as_ref()
                .map(|obj| obj.iter().map(|(k, v)| (k.clone(), v.clone())).collect())
                .unwrap_or_default();
            let scalar_source: Vec<(&str, &str, f64)> = if color_copy.is_some() {
                KNOBS
                    .iter()
                    .filter_map(|(knob, param, lo, hi)| {
                        copied
                            .iter()
                            .find(|(k, _)| k == knob)
                            .and_then(|(_, v)| v.as_f64())
                            .map(|v| (*knob, *param, v.clamp(*lo, *hi)))
                    })
                    .collect()
            } else {
                knob_args.clone()
            };
            for (knob, param, value) in &scalar_source {
                Self::upsert_effect_param(effects, &format!("color.{knob}"), param, *value);
            }

            let curve_source: Vec<(&str, Value)> = if color_copy.is_some() {
                ["masterCurve", "redCurve", "greenCurve", "blueCurve"]
                    .iter()
                    .filter_map(|k| {
                        copied
                            .iter()
                            .find(|(key, _)| key == k)
                            .map(|(_, v)| (*k, v.clone()))
                    })
                    .collect()
            } else {
                curve_args.clone()
            };
            for (key, value) in &curve_source {
                let channel = key.trim_end_matches("Curve");
                let entry = effects.iter_mut().find(|e| e.r#type == "color.curves");
                let serialized = serde_json::to_string(value).unwrap_or_else(|_| "[]".into());
                match entry {
                    Some(e) => {
                        e.params
                            .insert(channel.to_string(), core_model::EffectParam::string(&serialized));
                    }
                    None => {
                        let mut params = std::collections::HashMap::new();
                        params.insert(
                            channel.to_string(),
                            core_model::EffectParam::string(&serialized),
                        );
                        effects.push(Effect {
                            id: Uuid::new_v4().to_string(),
                            r#type: "color.curves".to_string(),
                            enabled: true,
                            params,
                        });
                    }
                }
            }

            let hue_source = if color_copy.is_some() {
                copied
                    .iter()
                    .find(|(k, _)| k == "hueCurves")
                    .and_then(|(_, v)| v.get("targets"))
                    .cloned()
            } else {
                hue_curves.clone()
            };
            if let Some(targets) = &hue_source {
                let serialized = serde_json::to_string(targets).unwrap_or_else(|_| "[]".into());
                effects.retain(|e| e.r#type != "color.hueCurves");
                let mut params = std::collections::HashMap::new();
                params.insert(
                    "targets".to_string(),
                    core_model::EffectParam::string(&serialized),
                );
                effects.push(Effect {
                    id: Uuid::new_v4().to_string(),
                    r#type: "color.hueCurves".to_string(),
                    enabled: true,
                    params,
                });
            }

            let lut_source = if color_copy.is_some() {
                copied
                    .iter()
                    .find(|(k, _)| k == "lut")
                    .and_then(|(_, v)| v.as_object().cloned())
            } else {
                lut.clone()
            };
            if let Some(lut_obj) = &lut_source {
                let Some(path) = lut_obj.get("path").and_then(|v| v.as_str()) else {
                    return Err("apply_color: lut requires a 'path'".to_string());
                };
                let strength = lut_obj
                    .get("strength")
                    .and_then(|v| v.as_f64())
                    .unwrap_or(1.0)
                    .clamp(0.0, 1.0);
                effects.retain(|e| e.r#type != "color.lut");
                let mut params = std::collections::HashMap::new();
                params.insert("path".to_string(), core_model::EffectParam::string(path));
                params.insert("strength".to_string(), core_model::EffectParam::value(strength));
                effects.push(Effect {
                    id: Uuid::new_v4().to_string(),
                    r#type: "color.lut".to_string(),
                    enabled: true,
                    params,
                });
            }
            if effects.is_empty() {
                clip.effects = None;
            }
        }

        Ok(json!({}))
    }

    fn upsert_effect_param(
        effects: &mut Vec<Effect>,
        effect_type: &str,
        param_name: &str,
        value: f64,
    ) {
        let existing = effects.iter_mut().find(|e| e.r#type == effect_type);
        match existing {
            Some(effect) => {
                effect.params.insert(
                    param_name.to_string(),
                    core_model::EffectParam::value(value),
                );
            }
            None => {
                let mut params = std::collections::HashMap::new();
                params.insert(
                    param_name.to_string(),
                    core_model::EffectParam::value(value),
                );
                effects.push(Effect {
                    id: Uuid::new_v4().to_string(),
                    r#type: effect_type.to_string(),
                    enabled: true,
                    params,
                });
            }
        }
    }

    /// APPLY_EFFECT v2 (absorbs set_chroma_key): merge non-color effects by
    /// type over clipIds; out-of-range params clamp; `remove` deletes by
    /// type; enabled=false bypasses without removing. `key.chroma` also
    /// mirrors into `Clip.chroma_key` so the compositor keys immediately.
    fn cmd_apply_effect(&mut self, args: &Value) -> Result<Value, String> {
        // (type, [(param, min, max, default)]) — Appendix C effect catalog.
        const CATALOG: &[(&str, &[(&str, f64, f64)])] = &[
            ("detail.clarity", &[("clarity", -1.0, 1.0), ("dehaze", -1.0, 1.0)]),
            ("blur.gaussian", &[("radius", 0.0, 100.0)]),
            ("blur.sharpen", &[("amount", 0.0, 2.0)]),
            ("blur.noiseReduction", &[("amount", 0.0, 1.0)]),
            ("blur.motion", &[("radius", 0.0, 100.0), ("angle", -180.0, 180.0)]),
            ("stylize.grain", &[("amount", 0.0, 1.0), ("size", 0.5, 4.0)]),
            (
                "stylize.vignette",
                &[
                    ("amount", -1.0, 1.0),
                    ("midpoint", 0.0, 1.0),
                    ("roundness", -1.0, 1.0),
                    ("feather", 0.0, 1.0),
                ],
            ),
            (
                "stylize.glow",
                &[
                    ("intensity", 0.0, 1.0),
                    ("radius", 0.0, 100.0),
                    ("threshold", 0.0, 1.0),
                    ("warmth", 0.0, 1.0),
                ],
            ),
            (
                "key.chroma",
                &[
                    ("keyHue", 0.0, 1.0),
                    ("tolerance", 0.0, 1.0),
                    ("softness", 0.0, 1.0),
                    ("spill", 0.0, 1.0),
                ],
            ),
        ];

        // Legacy single-clip shape ({clipId, effectType, ...}) still parses.
        if args.get("clipIds").is_none() {
            if let (Some(clip_id), Some(effect_type)) = (
                args.get("clipId").and_then(|v| v.as_str()),
                args.get("effectType").and_then(|v| v.as_str()),
            ) {
                return self.legacy_apply_effect(args, clip_id, effect_type);
            }
        }

        let clip_ids: Vec<String> = args
            .get("clipIds")
            .and_then(|v| v.as_array())
            .map(|a| a.iter().filter_map(|v| v.as_str().map(String::from)).collect())
            .unwrap_or_default();
        if clip_ids.is_empty() {
            return Err("Missing clipIds".to_string());
        }
        for cid in &clip_ids {
            if timeline_core::find_clip(&self.timeline, cid).is_none() {
                return Err(format!("Clip '{cid}' not found"));
            }
        }

        // Parse + clamp every effect entry before mutating anything.
        struct EffectSpec {
            r#type: String,
            params: Vec<(String, f64)>,
            enabled: bool,
        }
        let mut specs: Vec<EffectSpec> = Vec::new();
        if let Some(list) = args.get("effects").and_then(|v| v.as_array()) {
            for (i, e) in list.iter().enumerate() {
                let ty = e
                    .get("type")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| format!("effects[{i}]: missing type"))?;
                let Some((_, catalog_params)) = CATALOG.iter().find(|(t, _)| *t == ty) else {
                    let known: Vec<&str> = CATALOG.iter().map(|(t, _)| *t).collect();
                    return Err(format!(
                        "effects[{i}]: unknown effect type '{ty}'. Available: {}",
                        known.join(", ")
                    ));
                };
                let mut params: Vec<(String, f64)> = Vec::new();
                if let Some(pv) = e.get("params").and_then(|v| v.as_object()) {
                    for (name, value) in pv {
                        let Some((_, lo, hi)) =
                            catalog_params.iter().find(|(n, _, _)| n == name)
                        else {
                            let names: Vec<&str> =
                                catalog_params.iter().map(|(n, _, _)| *n).collect();
                            return Err(format!(
                                "effects[{i}]: '{ty}' has no param '{name}'. Params: {}",
                                names.join(", ")
                            ));
                        };
                        let v = value
                            .as_f64()
                            .ok_or_else(|| format!("effects[{i}]: param '{name}' must be a number"))?;
                        params.push((name.clone(), v.clamp(*lo, *hi)));
                    }
                }
                specs.push(EffectSpec {
                    r#type: ty.to_string(),
                    params,
                    enabled: e.get("enabled").and_then(|v| v.as_bool()).unwrap_or(true),
                });
            }
        }
        let removals: Vec<String> = args
            .get("remove")
            .and_then(|v| v.as_array())
            .map(|a| a.iter().filter_map(|v| v.as_str().map(String::from)).collect())
            .unwrap_or_default();
        if specs.is_empty() && removals.is_empty() {
            return Err("Pass at least one of 'effects' or 'remove'.".to_string());
        }

        for cid in &clip_ids {
            let loc = timeline_core::find_clip(&self.timeline, cid).expect("checked above");
            let clip = &mut self.timeline.tracks[loc.track_index].clips[loc.clip_index];
            for ty in &removals {
                if ty == "key.chroma" {
                    clip.chroma_key = None;
                }
                if let Some(effects) = &mut clip.effects {
                    effects.retain(|e| &e.r#type != ty);
                }
            }
            for spec in &specs {
                let effects = clip.effects.get_or_insert(Vec::new());
                match effects.iter_mut().find(|e| e.r#type == spec.r#type) {
                    Some(e) => {
                        e.enabled = spec.enabled;
                        for (name, value) in &spec.params {
                            e.params
                                .insert(name.clone(), core_model::EffectParam::value(*value));
                        }
                    }
                    None => {
                        let mut params = std::collections::HashMap::new();
                        for (name, value) in &spec.params {
                            params.insert(name.clone(), core_model::EffectParam::value(*value));
                        }
                        effects.push(Effect {
                            id: Uuid::new_v4().to_string(),
                            r#type: spec.r#type.clone(),
                            enabled: spec.enabled,
                            params,
                        });
                    }
                }
                // Mirror the chroma key into the render model.
                if spec.r#type == "key.chroma" {
                    let entry = clip
                        .effects
                        .as_ref()
                        .and_then(|es| es.iter().find(|e| e.r#type == "key.chroma"))
                        .cloned();
                    if let Some(entry) = entry {
                        let get = |name: &str, default: f64| {
                            entry.params.get(name).and_then(|p| p.value).unwrap_or(default)
                        };
                        let hue = get("keyHue", 1.0 / 3.0);
                        let (r, g, b) = hue_to_rgb(hue);
                        clip.chroma_key = Some(core_model::ChromaKey {
                            enabled: spec.enabled,
                            key_r: r,
                            key_g: g,
                            key_b: b,
                            tolerance: get("tolerance", 0.0),
                            spill_suppression: get("spill", 0.5),
                        });
                    }
                }
            }
            if clip.effects.as_ref().is_some_and(|e| e.is_empty()) {
                clip.effects = None;
            }
        }
        Ok(json!({}))
    }

    /// Pre-v2 apply_effect shape ({clipId, effectType, intensity, remove}) —
    /// kept for in-app callers; free-form types, no catalog clamping.
    fn legacy_apply_effect(
        &mut self,
        args: &Value,
        clip_id: &str,
        effect_type: &str,
    ) -> Result<Value, String> {
        let enabled = args
            .get("enabled")
            .and_then(|v| v.as_bool())
            .unwrap_or(true);
        let remove = args
            .get("remove")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        let intensity = args.get("intensity").and_then(|v| v.as_f64());

        let Some(loc) = timeline_core::find_clip(&self.timeline, clip_id) else {
            return Err(format!("Clip '{}' not found", clip_id));
        };
        let clip = &mut self.timeline.tracks[loc.track_index].clips[loc.clip_index];

        if remove {
            if let Some(ref mut effects) = clip.effects {
                effects.retain(|e| e.r#type != effect_type);
                if effects.is_empty() {
                    clip.effects = None;
                }
            }
        } else {
            let effects = clip.effects.get_or_insert(Vec::new());
            let existing = effects.iter_mut().find(|e| e.r#type == effect_type);
            match existing {
                Some(effect) => {
                    effect.enabled = enabled;
                    if let Some(v) = intensity {
                        effect
                            .params
                            .insert("intensity".to_string(), core_model::EffectParam::value(v));
                    }
                }
                None => {
                    let mut params = std::collections::HashMap::new();
                    if let Some(v) = intensity {
                        params.insert("intensity".to_string(), core_model::EffectParam::value(v));
                    }
                    effects.push(Effect {
                        id: Uuid::new_v4().to_string(),
                        r#type: effect_type.to_string(),
                        enabled,
                        params,
                    });
                }
            }
        }
        Ok(json!({}))
    }

    // ── Color inspect (read-only) ──────────────────────────────────────────

    fn cmd_inspect_color(&self, args: &Value) -> Result<Value, String> {
        let clip_id = args.get("clipId").and_then(|v| v.as_str());
        let media_ref = args.get("mediaRef").and_then(|v| v.as_str());

        if clip_id.is_none() && media_ref.is_none() {
            return Err("Provide either clipId or mediaRef".to_string());
        }

        if let Some(cid) = clip_id {
            let loc = timeline_core::find_clip(&self.timeline, cid)
                .ok_or_else(|| format!("Clip '{}' not found", cid))?;
            let clip = &self.timeline.tracks[loc.track_index].clips[loc.clip_index];
            let color_effects: Vec<&Effect> = clip
                .effects
                .as_ref()
                .map(|e| {
                    e.iter()
                        .filter(|ef| ef.r#type.starts_with("color."))
                        .collect()
                })
                .unwrap_or_default();
            return Ok(json!({
                "content": [{
                    "type": "text",
                    "text": format!(
                        "Color info for clip '{}': {} color effect(s) applied",
                        cid, color_effects.len()
                    )
                }]
            }));
        }

        if let Some(mid) = media_ref {
            let in_manifest = self.media_manifest.entries.iter().find(|e| e.id == mid);
            match in_manifest {
                Some(entry) => Ok(json!({
                    "content": [{
                        "type": "text",
                        "text": format!(
                            "Media '{}' ({:?}) — width: {:?}, height: {:?}, fps: {:?}",
                            entry.name, entry.r#type, entry.source_width, entry.source_height, entry.source_fps
                        )
                    }]
                })),
                None => Ok(json!({
                    "content": [{
                        "type": "text",
                        "text": format!("Media '{}' not found in manifest", mid)
                    }],
                    "isError": true,
                })),
            }
        } else {
            Err("No clipId or mediaRef provided".to_string())
        }
    }

    // ── Captions (stub — needs transcription engine) ───────────────────────

    /// ADD_CAPTIONS (v2: no targeting — transcribes the timeline's spoken
    /// audio itself). The transcription engine is a host concern; report the
    /// limitation honestly until it lands.
    fn cmd_add_captions(&mut self, args: &Value) -> Result<Value, String> {
        let _ = args;
        Err(
            "add_captions is unavailable: timeline transcription requires the host transcriber (run it from the app)."
                .to_string(),
        )
    }

    fn cmd_apply_animation(&mut self, args: &Value) -> Result<Value, String> {
        let clip_id = args
            .get("clipId")
            .and_then(|v| v.as_str())
            .ok_or_else(|| "Missing clipId".to_string())?;
        let preset = args
            .get("preset")
            .and_then(|v| v.as_str())
            .ok_or_else(|| "Missing preset".to_string())?;

        let Some(loc) = timeline_core::find_clip(&self.timeline, clip_id) else {
            return Err(format!("Clip '{}' not found", clip_id));
        };
        let clip = &mut self.timeline.tracks[loc.track_index].clips[loc.clip_index];

        let intensity = args
            .get("intensity")
            .and_then(|v| v.as_f64())
            .unwrap_or(1.0);

        // Store animation preset as an effect
        let effects = clip.effects.get_or_insert(Vec::new());
        let anim_effect = Effect {
            id: Uuid::new_v4().to_string(),
            r#type: format!("animation.{}", preset),
            enabled: true,
            params: {
                let mut m = std::collections::HashMap::new();
                m.insert(
                    "intensity".to_string(),
                    core_model::EffectParam::value(intensity),
                );
                m
            },
        };
        effects.push(anim_effect);

        Ok(json!({
            "content": [{
                "type": "text",
                "text": format!("Applied animation '{}' to clip '{}' (intensity: {})", preset, clip_id, intensity)
            }]
        }))
    }

    // ── Generation tools (stub — need remote API) ──────────────────────────

    fn cmd_generate_video(&mut self, args: &Value) -> Result<Value, String> {
        let prompt = args
            .get("prompt")
            .and_then(|v| v.as_str())
            .ok_or_else(|| "Missing prompt".to_string())?;
        let duration = args.get("duration").and_then(|v| v.as_f64()).unwrap_or(5.0);
        let model = self
            .resolve_generation_model(
                generation_core::ModelKind::Video,
                args.get("model").and_then(|v| v.as_str()),
            )?
            .id;


        Ok(json!({
            "content": [{
                "type": "text",
                "text": format!(
                    "Video generation queued (model: {}, duration: {:.1}s, prompt: '{}'). Actual generation requires a remote API.",
                    model, duration, prompt
                )
            }],
            "isError": true,
        }))
    }

    fn cmd_generate_image(&mut self, args: &Value) -> Result<Value, String> {
        let prompt = args
            .get("prompt")
            .and_then(|v| v.as_str())
            .ok_or_else(|| "Missing prompt".to_string())?;
        let model = self
            .resolve_generation_model(
                generation_core::ModelKind::Image,
                args.get("model").and_then(|v| v.as_str()),
            )?
            .id;


        Ok(json!({
            "content": [{
                "type": "text",
                "text": format!(
                    "Image generation queued (model: {}, prompt: '{}'). Actual generation requires a remote API.",
                    model, prompt
                )
            }],
            "isError": true,
        }))
    }

    /// GENERATE_AUDIO v2 (absorbs generate_music): TTS, text-to-music, and
    /// video-to-audio through one tool. The model resolves from the audio
    /// catalog (music-category models included); generation itself still
    /// needs the remote backend, reported honestly.
    fn cmd_generate_audio(&mut self, args: &Value) -> Result<Value, String> {
        let prompt = args.get("prompt").and_then(|v| v.as_str());
        let model = match args.get("model").and_then(|v| v.as_str()) {
            Some(id) => {
                self.resolve_generation_model(generation_core::ModelKind::Audio, Some(id))?
            }
            None => {
                let is_paid = self.is_paid_account();
                model_catalog::catalog()
                    .iter()
                    .filter(|m| m.kind_str() == "audio")
                    .find(|m| model_catalog::model_available(is_paid, m.paid_only))
                    .ok_or_else(|| model_catalog::no_available_model_message("audio"))?
            }
        };
        let is_music = matches!(&model.caps, model_catalog::ModelCaps::Audio(c)
            if c.category == model_catalog::AudioCategory::Music);
        let has_video_source = args.get("videoSourceMediaRef").is_some()
            || args.get("videoSourceStartFrame").is_some();
        if prompt.is_none() && !has_video_source {
            return Err(
                "Missing prompt (required for TTS and text-to-music; optional only for video-to-audio models)."
                    .to_string(),
            );
        }
        let duration = args.get("duration").and_then(|v| v.as_f64());
        // Upstream #288: validate the video-to-audio span against the model's
        // caps BEFORE the backend gap — a too-long/short span must fail fast.
        if has_video_source {
            let span_seconds = if let Some(mref) = args.get("videoSourceMediaRef").and_then(|v| v.as_str()) {
                let entry = self
                    .media_manifest
                    .entry_for(mref)
                    .ok_or_else(|| format!("Media '{mref}' was not found in the library."))?;
                Some(entry.duration)
            } else {
                let start = args.get("videoSourceStartFrame").and_then(|v| v.as_i64());
                let end = args.get("videoSourceEndFrame").and_then(|v| v.as_i64());
                match (start, end) {
                    (Some(s0), Some(e0)) if e0 > s0 => {
                        Some((e0 - s0) as f64 / self.timeline.fps.max(1) as f64)
                    }
                    (Some(_), Some(_)) => {
                        return Err(
                            "videoSourceEndFrame must be greater than videoSourceStartFrame."
                                .to_string(),
                        )
                    }
                    _ => None,
                }
            };
            if let (Some(span), model_catalog::ModelCaps::Audio(c)) = (span_seconds, &model.caps) {
                // #288 defaults when the catalog carries no per-model bounds.
                let min = c.min_seconds.unwrap_or(1.0);
                let max = c.max_seconds.unwrap_or(600.0);
                if span < min {
                    return Err(format!(
                        "Video-to-audio span is {span:.1}s; {} needs at least {min:.0}s.",
                        model.id
                    ));
                }
                if span > max {
                    return Err(format!(
                        "Video-to-audio span is {span:.1}s; {} allows at most {max:.0}s.",
                        model.id
                    ));
                }
            }
        }
        let _ = (
            args.get("voice").and_then(|v| v.as_str()),
            args.get("lyrics").and_then(|v| v.as_str()),
            args.get("instrumental").and_then(|v| v.as_bool()),
            args.get("styleInstructions").and_then(|v| v.as_str()),
        );

        Err(format!(
            "generate_audio is unavailable: the remote generation backend isn't connected (model: {}, {}{}).",
            model.id,
            if is_music { "music" } else { "speech/audio" },
            duration
                .map(|d| format!(", {d:.1}s"))
                .unwrap_or_default(),
        ))
    }

    fn cmd_upscale_media(&mut self, args: &Value) -> Result<Value, String> {
        // v2 key is mediaRef (short-id expansion applies); legacy mediaId
        // still parses for older callers.
        let media_id = args
            .get("mediaRef")
            .or_else(|| args.get("mediaId"))
            .and_then(|v| v.as_str())
            .ok_or_else(|| "Missing mediaRef".to_string())?;

        let entry = self
            .media_manifest
            .entries
            .iter()
            .find(|e| e.id == media_id)
            .ok_or_else(|| format!("Media '{}' not found", media_id))?;

        Ok(json!({
            "content": [{
                "type": "text",
                "text": format!(
                    "Upscale requested for '{}' ({}). Actual upscaling requires a remote API.",
                    entry.name, media_id
                )
            }],
            "isError": true,
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core_model::{MediaManifest, Timeline};

    fn make_executor() -> ToolExecutor {
        ToolExecutor::new(Timeline::default(), MediaManifest::default())
    }

    fn make_executor_with_media() -> ToolExecutor {
        let mut manifest = MediaManifest::default();
        manifest.entries.push(core_model::MediaManifestEntry {
            id: "media-001".to_string(),
            name: "test_video.mp4".to_string(),
            r#type: core_model::ClipType::Video,
            source: core_model::MediaSource::External {
                absolute_path: "/path/to/video.mp4".to_string(),
            },
            duration: 10.0,
            generation_input: None,
            source_width: Some(1920),
            source_height: Some(1080),
            source_fps: Some(30.0),
            has_audio: Some(true),
            folder_id: None,
            cached_remote_url: None,
            cached_remote_url_expires_at: None,
            source_timecode_frame: None,
            source_timecode_quanta: None,
            source_timecode_drop_frame: None,
            ai_tags: None,
            ai_description: None,
            ai_label_status: None,
            generation_status: None,
        });
        manifest.folders.push(core_model::MediaFolder {
            id: "folder-001".to_string(),
            name: "Test Folder".to_string(),
            parent_folder_id: None,
        });
        ToolExecutor::new(Timeline::default(), manifest)
    }

    fn video_media(id: &str, w: i64, h: i64, fps: f64) -> core_model::MediaManifestEntry {
        core_model::MediaManifestEntry {
            id: id.into(),
            name: format!("{id}.mp4"),
            r#type: ClipType::Video,
            source: core_model::MediaSource::External {
                absolute_path: format!("/{id}.mp4"),
            },
            duration: 3.0,
            generation_input: None,
            source_width: Some(w),
            source_height: Some(h),
            source_fps: Some(fps),
            has_audio: Some(false),
            folder_id: None,
            cached_remote_url: None,
            cached_remote_url_expires_at: None,
            source_timecode_frame: None,
            source_timecode_quanta: None,
            source_timecode_drop_frame: None,
            ai_tags: None,
            ai_description: None,
            ai_label_status: None,
            generation_status: None,
        }
    }

    #[test]
    fn add_clips_auto_detects_settings_from_first_video() {
        // First clip ever (settings not configured) → silently adopt its fps/size.
        let mut manifest = MediaManifest::default();
        manifest.entries.push(video_media("v4k", 3840, 2160, 24.0));
        let mut exec = ToolExecutor::new(Timeline::default(), manifest);
        assert!(!exec.timeline().settings_configured);
        let res = exec.execute("add_clips", &json!({"entries": [{"mediaRef": "v4k", "startFrame": 0}]})).unwrap();
        assert_eq!(exec.timeline().fps, 24);
        assert_eq!(exec.timeline().width, 3840);
        assert_eq!(exec.timeline().height, 2160);
        assert!(exec.timeline().settings_configured);
        let text = res["content"][0]["text"].as_str().unwrap();
        assert!(
            text.contains("3840x2160") && text.contains("24fps"),
            "settings note expected: {text}"
        );
        // Clip duration is measured on the DETECTED 24fps grid (3s → 72 frames).
        assert_eq!(exec.timeline().tracks[0].clips[0].duration_frames, 72);
    }

    #[test]
    fn add_clips_keeps_settings_fixed_after_first_clip() {
        let mut manifest = MediaManifest::default();
        manifest.entries.push(video_media("a24", 1920, 1080, 24.0));
        manifest.entries.push(video_media("b60", 1920, 1080, 60.0));
        let mut exec = ToolExecutor::new(Timeline::default(), manifest);
        exec.execute("add_clips", &json!({"entries": [{"mediaRef": "a24", "startFrame": 0}]})).unwrap();
        assert_eq!(exec.timeline().fps, 24, "first clip sets project to 24fps");
        // A later 60fps clip must NOT re-detect: project fps stays fixed (#233).
        let res = exec.execute("add_clips", &json!({"entries": [{"mediaRef": "b60", "startFrame": 0}]})).unwrap();
        assert_eq!(exec.timeline().fps, 24, "settings stay fixed after the first clip");
        let text = res["content"][0]["text"].as_str().unwrap();
        assert!(!text.contains("Set project"), "no re-detect on later adds: {text}");
    }

    #[test]
    fn add_clips_resolves_type_and_full_source_duration() {
        // Upstream #236: omitting trim/duration places the full source length,
        // and the clip type comes from the manifest — not a hardcoded Video/150.
        let mut exec = make_executor_with_media();
        let fps = exec.timeline().fps;
        let _ = timeline_core::insert_track_at(exec.timeline_mut(), 0, ClipType::Video);
        exec.execute("add_clips", &json!({"entries": [{"mediaRef": "media-001", "trackIndex": 0, "startFrame": 0}]}))
            .unwrap();
        let clip = &exec.timeline().tracks[0].clips[0];
        assert!(matches!(clip.media_type, ClipType::Video));
        assert_eq!(clip.duration_frames, (10.0 * fps as f64).round() as i64);
        assert_eq!(clip.trim_start_frame, 0);
        assert_eq!(clip.trim_end_frame, 0);
    }

    #[test]
    fn add_clips_rejects_type_incompatible_track() {
        // An audio asset must not be placed on a video track (and vice versa).
        let mut manifest = MediaManifest::default();
        manifest.entries.push(core_model::MediaManifestEntry {
            id: "aud".into(),
            name: "a.wav".into(),
            r#type: ClipType::Audio,
            source: core_model::MediaSource::External {
                absolute_path: "/a.wav".into(),
            },
            duration: 5.0,
            generation_input: None,
            source_width: None,
            source_height: None,
            source_fps: None,
            has_audio: Some(true),
            folder_id: None,
            cached_remote_url: None,
            cached_remote_url_expires_at: None,
            source_timecode_frame: None,
            source_timecode_quanta: None,
            source_timecode_drop_frame: None,
            ai_tags: None,
            ai_description: None,
            ai_label_status: None,
            generation_status: None,
        });
        let mut exec = ToolExecutor::new(Timeline::default(), manifest);
        let _ = timeline_core::insert_track_at(exec.timeline_mut(), 0, ClipType::Video);
        let err = exec
            .execute("add_clips", &json!({"entries": [{"mediaRef": "aud", "trackIndex": 0, "startFrame": 0}]}))
            .unwrap_err();
        assert!(err.contains("not compatible"), "got: {err}");
        assert!(
            exec.timeline().tracks[0].clips.is_empty(),
            "nothing placed on rejection"
        );
    }

    #[test]
    fn add_clips_auto_creates_linked_audio_for_video_with_audio() {
        // CLP-007/008: a video-with-audio placed on a video track auto-creates a
        // linked audio clip on an audio track (created if needed), sharing a group.
        let mut manifest = MediaManifest::default();
        manifest.entries.push(core_model::MediaManifestEntry {
            id: "vid".into(),
            name: "v.mp4".into(),
            r#type: ClipType::Video,
            source: core_model::MediaSource::External {
                absolute_path: "/v.mp4".into(),
            },
            duration: 4.0,
            generation_input: None,
            source_width: Some(1920),
            source_height: Some(1080),
            source_fps: Some(30.0),
            has_audio: Some(true),
            folder_id: None,
            cached_remote_url: None,
            cached_remote_url_expires_at: None,
            source_timecode_frame: None,
            source_timecode_quanta: None,
            source_timecode_drop_frame: None,
            ai_tags: None,
            ai_description: None,
            ai_label_status: None,
            generation_status: None,
        });
        let mut exec = ToolExecutor::new(Timeline::default(), manifest);
        let _ = timeline_core::insert_track_at(exec.timeline_mut(), 0, ClipType::Video);
        exec.execute("add_clips", &json!({"entries": [{"mediaRef": "vid", "trackIndex": 0, "startFrame": 0}]}))
            .unwrap();

        let video = exec.timeline().tracks[0].clips[0].clone();
        assert!(video.link_group_id.is_some(), "video should be linked");
        let audio_track = exec
            .timeline()
            .tracks
            .iter()
            .find(|t| t.r#type == ClipType::Audio)
            .expect("linked audio track created");
        assert_eq!(audio_track.clips.len(), 1);
        let audio = &audio_track.clips[0];
        assert_eq!(audio.link_group_id, video.link_group_id, "shares link group");
        assert_eq!(audio.start_frame, video.start_frame);
        assert_eq!(audio.duration_frames, video.duration_frames);
        assert!(matches!(audio.media_type, ClipType::Audio));
    }

    fn media_entry(id: &str, ty: ClipType, has_audio: bool, duration: f64) -> core_model::MediaManifestEntry {
        core_model::MediaManifestEntry {
            id: id.into(),
            name: format!("{id}.media"),
            r#type: ty,
            source: core_model::MediaSource::External {
                absolute_path: format!("/{id}"),
            },
            duration,
            generation_input: None,
            source_width: Some(1920),
            source_height: Some(1080),
            source_fps: Some(30.0),
            has_audio: Some(has_audio),
            folder_id: None,
            cached_remote_url: None,
            cached_remote_url_expires_at: None,
            source_timecode_frame: None,
            source_timecode_quanta: None,
            source_timecode_drop_frame: None,
            ai_tags: None,
            ai_description: None,
            ai_label_status: None,
            generation_status: None,
        }
    }

    #[test]
    fn add_clips_omit_track_index_auto_creates_video_track() {
        // MUT-002/003: omitting trackIndex auto-creates a track for the clips.
        let mut manifest = MediaManifest::default();
        manifest.entries.push(media_entry("vid", ClipType::Video, false, 3.0));
        let mut exec = ToolExecutor::new(Timeline::default(), manifest); // zero tracks
        exec.execute("add_clips", &json!({"entries": [{"mediaRef": "vid", "startFrame": 0}]})).unwrap();
        assert_eq!(exec.timeline().tracks.len(), 1, "one track auto-created");
        assert!(matches!(exec.timeline().tracks[0].r#type, ClipType::Video));
        assert_eq!(exec.timeline().tracks[0].clips.len(), 1);
    }

    #[test]
    fn add_clips_omit_track_index_splits_visual_and_audio() {
        // Mixed visual + audio with no trackIndex → a video track for the visual and
        // an audio track for the audio.
        let mut manifest = MediaManifest::default();
        manifest.entries.push(media_entry("vid", ClipType::Video, false, 2.0));
        manifest.entries.push(media_entry("aud", ClipType::Audio, true, 2.0));
        let mut exec = ToolExecutor::new(Timeline::default(), manifest);
        exec.execute("add_clips", &json!({"entries": [{"mediaRef": "vid", "startFrame": 0}, {"mediaRef": "aud", "startFrame": 0}]}))
            .unwrap();
        let video_track = exec
            .timeline()
            .tracks
            .iter()
            .find(|t| t.r#type == ClipType::Video)
            .expect("video track created");
        let audio_track = exec
            .timeline()
            .tracks
            .iter()
            .find(|t| t.r#type == ClipType::Audio)
            .expect("audio track created");
        assert_eq!(video_track.clips.len(), 1);
        assert_eq!(video_track.clips[0].media_ref, "vid");
        assert_eq!(audio_track.clips.len(), 1);
        assert_eq!(audio_track.clips[0].media_ref, "aud");
    }

    #[test]
    fn add_clips_overwrite_linked_pair_clears_stranded_audio_no_extra_track() {
        // Upstream #124: overwriting a linked V+A pair cleared only the video
        // range — the audio fragment stayed (double playback) and the new clip's
        // audio was pushed onto a spurious extra track.
        let mut manifest = MediaManifest::default();
        manifest.entries.push(media_entry("existing", ClipType::Video, true, 3.0)); // 90f
        manifest.entries.push(media_entry("new", ClipType::Video, true, 1.0)); // 30f
        let mut exec = ToolExecutor::new(Timeline::default(), manifest);
        let _ = timeline_core::insert_track_at(exec.timeline_mut(), 0, ClipType::Video);
        exec.execute("add_clips", &json!({"entries": [{"mediaRef": "existing", "trackIndex": 0, "startFrame": 0}]}))
            .unwrap();
        assert_eq!(exec.timeline().tracks.len(), 2, "V + linked A placed");

        exec.execute("add_clips", &json!({"entries": [{"mediaRef": "new", "trackIndex": 0, "startFrame": 0}]}))
            .unwrap();

        assert_eq!(
            exec.timeline().tracks.len(),
            2,
            "no spurious extra audio track"
        );
        let audio_track = &exec.timeline().tracks[1];
        let mut spans: Vec<(i64, i64)> = audio_track
            .clips
            .iter()
            .map(|c| (c.start_frame, c.start_frame + c.duration_frames))
            .collect();
        spans.sort_unstable();
        assert_eq!(
            spans,
            vec![(0, 30), (30, 90)],
            "overwritten audio range cleared; no stranded [0,90) fragment"
        );
        let head = audio_track
            .clips
            .iter()
            .find(|c| c.start_frame == 0)
            .unwrap();
        assert_eq!(head.media_ref, "new", "new clip's audio lands in the cleared range");
    }

    // ─── Upstream #264/#265: frame args are ceiling-bounded so LLM tool calls
    // can't overflow i64 arithmetic (debug panic / release wrap). ───

    #[test]
    fn add_clips_rejects_duration_that_would_overflow() {
        let mut manifest = MediaManifest::default();
        manifest.entries.push(media_entry("i1", ClipType::Image, false, 0.0));
        manifest.entries.push(media_entry("i2", ClipType::Image, false, 0.0));
        let mut exec = ToolExecutor::new(Timeline::default(), manifest);
        let _ = timeline_core::insert_track_at(exec.timeline_mut(), 0, ClipType::Video);
        let err = exec
            .execute(
                "add_clips",
                &json!({"entries": [{"mediaRef": "i1", "trackIndex": 0, "startFrame": 0, "durationFrames": i64::MAX}, {"mediaRef": "i2", "trackIndex": 0, "startFrame": 0, "durationFrames": i64::MAX}]}),
            )
            .unwrap_err();
        assert!(err.contains("exceeds the maximum supported frame"), "err={err}");
    }

    #[test]
    fn insert_clips_rejects_frame_that_would_overflow() {
        let mut manifest = MediaManifest::default();
        manifest.entries.push(media_entry("vid", ClipType::Video, false, 2.0));
        let mut exec = ToolExecutor::new(Timeline::default(), manifest);
        let _ = timeline_core::insert_track_at(exec.timeline_mut(), 0, ClipType::Video);
        let err = exec
            .execute(
                "insert_clips",
                &json!({"entries": [{"mediaRef": "vid"}], "trackIndex": 0, "atFrame": i64::MAX}),
            )
            .unwrap_err();
        assert!(err.contains("exceeds the maximum supported frame"), "err={err}");
    }

    #[test]
    fn move_clips_rejects_to_frame_that_would_overflow() {
        let mut manifest = MediaManifest::default();
        manifest.entries.push(media_entry("vid", ClipType::Video, false, 2.0));
        let mut exec = ToolExecutor::new(Timeline::default(), manifest);
        let _ = timeline_core::insert_track_at(exec.timeline_mut(), 0, ClipType::Video);
        exec.execute("add_clips", &json!({"entries": [{"mediaRef": "vid", "trackIndex": 0, "startFrame": 0}]}))
            .unwrap();
        let clip_id = exec.timeline().tracks[0].clips[0].id.clone();
        let err = exec
            .execute(
                "move_clips",
                &json!({"clipIds": [clip_id], "toTrack": 0, "toFrame": i64::MAX}),
            )
            .unwrap_err();
        assert!(err.contains("exceeds the maximum supported frame"), "err={err}");
    }

    #[test]
    fn split_clips_rejects_at_frame_beyond_ceiling() {
        let mut manifest = MediaManifest::default();
        manifest.entries.push(media_entry("vid", ClipType::Video, false, 2.0));
        let mut exec = ToolExecutor::new(Timeline::default(), manifest);
        let _ = timeline_core::insert_track_at(exec.timeline_mut(), 0, ClipType::Video);
        exec.execute("add_clips", &json!({"entries": [{"mediaRef": "vid", "trackIndex": 0, "startFrame": 0}]}))
            .unwrap();
        let clip_id = exec.timeline().tracks[0].clips[0].id.clone();
        let err = exec
            .execute(
                "split_clips",
                &json!({"splits": [{"clipId": clip_id, "atFrame": i64::MAX}]}),
            )
            .unwrap_err();
        assert!(err.contains("exceeds the maximum supported frame"), "err={err}");
    }

    #[test]
    fn set_clip_properties_rejects_overflowing_duration_and_trims() {
        let mut manifest = MediaManifest::default();
        manifest.entries.push(media_entry("vid", ClipType::Video, false, 2.0));
        let mut exec = ToolExecutor::new(Timeline::default(), manifest);
        let _ = timeline_core::insert_track_at(exec.timeline_mut(), 0, ClipType::Video);
        exec.execute("add_clips", &json!({"entries": [{"mediaRef": "vid", "trackIndex": 0, "startFrame": 0}]}))
            .unwrap();
        let clip_id = exec.timeline().tracks[0].clips[0].id.clone();
        for key in ["durationFrames", "trimStartFrame", "trimEndFrame"] {
            let err = exec
                .execute(
                    "set_clip_properties",
                    &json!({"clipIds": [clip_id], "properties": {key: i64::MAX}}),
                )
                .unwrap_err();
            assert!(
                err.contains("exceeds the maximum supported frame"),
                "{key}: err={err}"
            );
        }
    }

    #[test]
    fn add_texts_rejects_overflowing_start_frame() {
        let mut exec = ToolExecutor::new(Timeline::default(), MediaManifest::default());
        let _ = timeline_core::insert_track_at(exec.timeline_mut(), 0, ClipType::Video);
        let err = exec
            .execute(
                "add_texts",
                &json!({"texts": [{"text": "x", "startFrame": i64::MAX, "durationFrames": 10}], "trackIndex": 0}),
            )
            .unwrap_err();
        assert!(err.contains("exceeds the maximum supported frame"), "err={err}");
    }

    #[test]
    fn apply_layout_rejects_overflowing_start_frame() {
        let mut manifest = MediaManifest::default();
        manifest.entries.push(video_media("m1", 1920, 1080, 30.0));
        let mut exec = ToolExecutor::new(Timeline::default(), manifest);
        let err = exec
            .execute(
                "apply_layout",
                &json!({
                    "layout": "full",
                    "slots": [{"slot": "main", "mediaRef": "m1"}],
                    "startFrame": i64::MAX,
                    "durationFrames": 30
                }),
            )
            .unwrap_err();
        assert!(err.contains("exceeds the maximum supported frame"), "err={err}");
    }

    // ─── Upstream #212: slow speeds below 0.25x are legal; speed must be > 0. ───

    #[test]
    fn set_clip_properties_speed_0_1_slows_clip_10x() {
        let mut manifest = MediaManifest::default();
        manifest.entries.push(media_entry("vid", ClipType::Video, false, 5.0)); // 150f
        let mut exec = ToolExecutor::new(Timeline::default(), manifest);
        let _ = timeline_core::insert_track_at(exec.timeline_mut(), 0, ClipType::Video);
        exec.execute("add_clips", &json!({"entries": [{"mediaRef": "vid", "trackIndex": 0, "startFrame": 0}]}))
            .unwrap();
        let clip_id = exec.timeline().tracks[0].clips[0].id.clone();
        exec.execute(
            "set_clip_properties",
            &json!({"clipIds": [clip_id], "properties": {"speed": 0.1}}),
        )
        .unwrap();
        let clip = &exec.timeline().tracks[0].clips[0];
        assert_eq!(clip.speed, 0.1);
        assert_eq!(
            clip.duration_frames, 1500,
            "0.1x speed stretches 150 source frames to 1500 timeline frames"
        );
    }

    #[test]
    fn set_clip_properties_rejects_non_positive_speed() {
        let mut manifest = MediaManifest::default();
        manifest.entries.push(media_entry("vid", ClipType::Video, false, 2.0));
        let mut exec = ToolExecutor::new(Timeline::default(), manifest);
        let _ = timeline_core::insert_track_at(exec.timeline_mut(), 0, ClipType::Video);
        exec.execute("add_clips", &json!({"entries": [{"mediaRef": "vid", "trackIndex": 0, "startFrame": 0}]}))
            .unwrap();
        let clip_id = exec.timeline().tracks[0].clips[0].id.clone();
        for bad in [0.0, -1.0] {
            let err = exec
                .execute(
                    "set_clip_properties",
                    &json!({"clipIds": [clip_id.clone()], "properties": {"speed": bad}}),
                )
                .unwrap_err();
            // wire-mutation-validators: the #144 validator now rejects first;
            // its message names the tool and field, so prefer it over the old
            // inline "speed must be > 0".
            assert!(err.contains("'speed' must be positive"), "speed {bad}: err={err}");
        }
        assert_eq!(
            exec.timeline().tracks[0].clips[0].speed,
            1.0,
            "rejected speed leaves the clip unchanged"
        );
    }

    // ─── wire-mutation-validators: mutation.rs validators gate execute() ───
    // #144's range checks were dormant (mutation.rs was never called on the
    // live path); these pin them e2e through executor.execute.

    #[test]
    fn set_clip_properties_volume_ceiling_is_the_db_boost_limit() {
        // Ceiling widened to Swift's +15 dB inspector limit: 1.5 (a boost)
        // is VALID here because the Rust inspector writes through this tool
        // layer; values past the +15 dB linear ceiling reject.
        let mut exec = executor_with_clip();
        exec.execute(
            "set_clip_properties",
            &json!({"clipIds": ["c"], "properties": {"volume": 1.5}}),
        )
        .unwrap();
        assert_eq!(exec.timeline().tracks[0].clips[0].volume, 1.5);
        let err = exec
            .execute(
                "set_clip_properties",
                &json!({"clipIds": ["c"], "properties": {"volume": 6.0}}),
            )
            .unwrap_err();
        assert!(err.contains("volume"), "got: {err}");
        assert_eq!(exec.timeline().tracks[0].clips[0].volume, 1.5);
    }

    #[test]
    fn set_clip_properties_rejects_negative_opacity() {
        let mut exec = executor_with_clip();
        let err = exec
            .execute(
                "set_clip_properties",
                &json!({"clipIds": ["c"], "properties": {"opacity": -0.1}}),
            )
            .unwrap_err();
        assert!(err.contains("opacity"), "got: {err}");
        assert_eq!(exec.timeline().tracks[0].clips[0].opacity, 1.0);
    }

    #[test]
    fn set_clip_properties_rejects_negative_trim() {
        let mut exec = executor_with_clip();
        for key in ["trimStartFrame", "trimEndFrame"] {
            let err = exec
                .execute(
                    "set_clip_properties",
                    &json!({"clipIds": ["c"], "properties": {key: -1}}),
                )
                .unwrap_err();
            assert!(err.contains(key), "{key}: got {err}");
        }
        assert_eq!(exec.timeline().tracks[0].clips[0].trim_start_frame, 0);
        assert_eq!(exec.timeline().tracks[0].clips[0].trim_end_frame, 0);
    }

    #[test]
    fn add_texts_rejects_audio_track_target() {
        // MUT-020 live: an explicit audio trackIndex refuses text placement
        // (previously the executor happily put text clips on the audio track).
        let mut exec = make_executor();
        let _ = timeline_core::insert_track_at(exec.timeline_mut(), 0, ClipType::Audio);
        let err = exec
            .execute(
                "add_texts",
                &json!({"texts": [{"content": "x", "startFrame": 0, "durationFrames": 30}], "trackIndex": 0}),
            )
            .unwrap_err();
        assert!(err.contains("audio"), "got: {err}");
        assert!(exec.timeline().tracks[0].clips.is_empty());
    }

    #[test]
    fn add_texts_without_track_index_still_auto_creates() {
        // No trackIndex → no track-type context → the MUT-020 gate must not
        // block the auto-create path even when an audio track exists.
        let mut exec = make_executor();
        let _ = timeline_core::insert_track_at(exec.timeline_mut(), 0, ClipType::Audio);
        exec.execute("add_texts", &json!({"texts": [{"content": "x"}]}))
            .unwrap();
        let placed: usize = exec.timeline().tracks.iter().map(|t| t.clips.len()).sum();
        assert_eq!(placed, 1);
    }

    #[test]
    fn insert_clips_rejects_empty_media_ids() {
        // Shape check the executor never had inline — the wired validator adds it.
        let mut exec = make_executor_with_media();
        let _ = timeline_core::insert_track_at(exec.timeline_mut(), 0, ClipType::Video);
        let err = exec
            .execute(
                "insert_clips",
                &json!({"entries": [], "trackIndex": 0, "atFrame": 0}),
            )
            .unwrap_err();
        assert!(err.contains("entries"), "got: {err}");
    }

    #[test]
    fn generate_audio_validates_video_span_288() {
        // Upstream #288: span checks fire BEFORE the backend-gap error.
        let mut manifest = MediaManifest::default();
        let mut long = audio_media("vid", 700.0);
        long.r#type = ClipType::Video;
        manifest.entries.push(long);
        let mut exec = ToolExecutor::new(Timeline::default(), manifest);

        // mediaRef branch: 700s > minimax-music-v2.6's max span.
        let err = exec
            .execute(
                "generate_audio",
                &json!({"model": "minimax-music-v2.6", "videoSourceMediaRef": "vid"}),
            )
            .unwrap_err();
        assert!(err.contains("at most"), "long span rejected: {err}");

        // frame-span branch at 30fps: 90 frames = 3s, within caps -> falls
        // through to the honest backend gap (NOT a span error).
        let err = exec
            .execute(
                "generate_audio",
                &json!({"model": "minimax-music-v2.6",
                        "videoSourceStartFrame": 0, "videoSourceEndFrame": 90}),
            )
            .unwrap_err();
        assert!(err.contains("backend"), "valid span reaches the gap: {err}");

        // inverted frame span is its own error.
        let err = exec
            .execute(
                "generate_audio",
                &json!({"model": "minimax-music-v2.6",
                        "videoSourceStartFrame": 90, "videoSourceEndFrame": 90}),
            )
            .unwrap_err();
        assert!(err.contains("greater than"), "{err}");
    }

    #[test]
    fn inspector_shaped_volume_boost_roundtrip() {
        // Pins the resolved #144 inspector conflict end-to-end (gpui-free):
        // the inspector converts its dB slider via linear_from_db and commits
        // through this same executor. The full slider range must be accepted;
        // one step past the ceiling must reject without touching the clip.
        let mut exec = executor_with_clip();
        for db in [-60.0, -12.5, 0.0, 7.3, timeline_core::VOLUME_CEILING_DB] {
            let linear = timeline_core::linear_from_db(db);
            exec.execute(
                "set_clip_properties",
                &json!({"clipIds": ["c"], "properties": {"volume": linear}}),
            )
            .unwrap_or_else(|e| panic!("{db} dB (linear {linear}) must be accepted: {e}"));
            let stored = exec.timeline().tracks[0].clips[0].volume;
            assert!((stored - linear).abs() < 1e-9, "{db} dB round-trips");
        }
        let ceiling = crate::mutation::volume_ceiling_linear();
        let err = exec
            .execute(
                "set_clip_properties",
                &json!({"clipIds": ["c"], "properties": {"volume": ceiling + 0.001}}),
            )
            .unwrap_err();
        assert!(err.contains("volume"), "past-ceiling rejects: {err}");
        assert!(
            (exec.timeline().tracks[0].clips[0].volume
                - timeline_core::linear_from_db(timeline_core::VOLUME_CEILING_DB))
            .abs()
                < 1e-9,
            "rejection leaves the last accepted value"
        );
    }

    #[test]
    fn validator_rejection_leaves_revision_unchanged() {
        let mut exec = executor_with_clip();
        let before = exec.revision();
        let _ = exec
            .execute(
                "set_clip_properties",
                &json!({"clipIds": ["c"], "properties": {"volume": 9.9}}),
            )
            .unwrap_err();
        assert_eq!(exec.revision(), before, "rejected call must not bump revision");
    }

    #[test]
    fn add_clips_linked_audio_does_not_clobber_existing_audio() {
        // The existing audio track holds music over the target span; the linked
        // audio must go to a free/new track, never overwriting the music.
        let mut manifest = MediaManifest::default();
        manifest.entries.push(core_model::MediaManifestEntry {
            id: "vid".into(),
            name: "v.mp4".into(),
            r#type: ClipType::Video,
            source: core_model::MediaSource::External {
                absolute_path: "/v.mp4".into(),
            },
            duration: 4.0, // 120 frames @ 30fps
            generation_input: None,
            source_width: Some(1920),
            source_height: Some(1080),
            source_fps: Some(30.0),
            has_audio: Some(true),
            folder_id: None,
            cached_remote_url: None,
            cached_remote_url_expires_at: None,
            source_timecode_frame: None,
            source_timecode_quanta: None,
            source_timecode_drop_frame: None,
            ai_tags: None,
            ai_description: None,
            ai_label_status: None,
            generation_status: None,
        });
        let mut exec = ToolExecutor::new(Timeline::default(), manifest);
        let _ = timeline_core::insert_track_at(exec.timeline_mut(), 0, ClipType::Video);
        let audio_ti =
            timeline_core::insert_track_at(exec.timeline_mut(), 1, ClipType::Audio).unwrap();
        // Pre-existing music spanning the whole target region.
        exec.timeline_mut().tracks[audio_ti].clips.push(only_clip_helper_music());

        exec.execute("add_clips", &json!({"entries": [{"mediaRef": "vid", "trackIndex": 0, "startFrame": 0}]}))
            .unwrap();

        // The music clip must still exist somewhere untouched.
        let music_alive = exec
            .timeline()
            .tracks
            .iter()
            .flat_map(|t| t.clips.iter())
            .any(|c| c.id == "music" && c.start_frame == 0 && c.duration_frames == 300);
        assert!(music_alive, "pre-existing audio was clobbered");
        // The linked audio landed on a different audio track (not over the music).
        let audio_clip_count: usize = exec
            .timeline()
            .tracks
            .iter()
            .filter(|t| t.r#type == ClipType::Audio)
            .map(|t| t.clips.len())
            .sum();
        assert_eq!(audio_clip_count, 2, "music + linked audio both present");
    }

    fn only_clip_helper_music() -> Clip {
        let mut c = executor_with_clip().timeline().tracks[0].clips[0].clone();
        c.id = "music".into();
        c.media_type = ClipType::Audio;
        c.source_clip_type = ClipType::Audio;
        c.start_frame = 0;
        c.duration_frames = 300;
        c.link_group_id = None;
        c
    }

    #[test]
    fn add_clips_honors_symmetric_trim_and_duration() {
        // Upstream #236: trimStartFrame + durationFrames derive trimEndFrame.
        let mut exec = make_executor_with_media();
        let fps = exec.timeline().fps;
        let total = (10.0 * fps as f64).round() as i64;
        let _ = timeline_core::insert_track_at(exec.timeline_mut(), 0, ClipType::Video);
        exec.execute(
            "add_clips",
            &json!({
                "entries": [{"mediaRef": "media-001", "trackIndex": 0, "startFrame": 0,
                             "trimStartFrame": 10, "durationFrames": 50}]
            }),
        )
        .unwrap();
        let clip = &exec.timeline().tracks[0].clips[0];
        assert_eq!(clip.trim_start_frame, 10);
        assert_eq!(clip.duration_frames, 50);
        assert_eq!(clip.trim_end_frame, total - 10 - 50);
    }

    #[test]
    fn add_clips_rejects_duration_and_trim_end_together() {
        // Upstream #236: durationFrames and trimEndFrame are mutually exclusive.
        let mut exec = make_executor_with_media();
        let _ = timeline_core::insert_track_at(exec.timeline_mut(), 0, ClipType::Video);
        let err = exec
            .execute(
                "add_clips",
                &json!({
                    "entries": [{"mediaRef": "media-001", "trackIndex": 0, "startFrame": 0,
                                 "durationFrames": 50, "trimEndFrame": 10}]
                }),
            )
            .unwrap_err();
        assert!(err.contains("not both"), "err={err}");
    }

    #[test]
    fn add_clips_warns_on_source_fps_divergence_without_changing_project_fps() {
        // Upstream #233: on a CONFIGURED project, project fps is authoritative; a
        // divergent source fps only warns and points at set_project_settings (the
        // first-clip auto-detect only fires when settings are not yet configured).
        let mut exec = make_executor_with_media();
        exec.timeline_mut().fps = 24; // source asset is 30 fps
        exec.timeline_mut().settings_configured = true;
        let _ = timeline_core::insert_track_at(exec.timeline_mut(), 0, ClipType::Video);
        let result = exec
            .execute("add_clips", &json!({"entries": [{"mediaRef": "media-001", "trackIndex": 0, "startFrame": 0}]}))
            .unwrap();
        let text = result["content"][0]["text"].as_str().unwrap();
        assert!(text.contains("Source fps"), "text={text}");
        assert!(text.contains("set_project_settings"), "text={text}");
        assert_eq!(exec.timeline().fps, 24, "project fps must be unchanged");
    }

    #[test]
    fn set_project_settings_requires_at_least_one_arg() {
        let mut exec = make_executor();
        let err = exec.execute("set_project_settings", &json!({})).unwrap_err();
        assert!(err.contains("at least one"), "err={err}");
    }

    #[test]
    fn set_project_settings_aspect_and_explicit_dims_conflict() {
        let mut exec = make_executor();
        let err = exec
            .execute(
                "set_project_settings",
                &json!({"aspectRatio": "16:9", "width": 1920}),
            )
            .unwrap_err();
        assert!(err.contains("mutually exclusive"), "err={err}");
    }

    #[test]
    fn set_project_settings_aspect_preset_sets_dims() {
        let mut exec = make_executor(); // default 1920x1080 @30
        exec.execute("set_project_settings", &json!({"aspectRatio": "9:16"}))
            .unwrap();
        assert_eq!(exec.timeline().width, 1080);
        assert_eq!(exec.timeline().height, 1920);
        assert!(exec.timeline().settings_configured);
    }

    #[test]
    fn set_project_settings_quality_scales_short_edge() {
        let mut exec = make_executor(); // 1920x1080 landscape
        exec.execute("set_project_settings", &json!({"quality": "4K"}))
            .unwrap();
        // short edge 2160, landscape -> (2160*1920/1080, 2160)
        assert_eq!(exec.timeline().width, 3840);
        assert_eq!(exec.timeline().height, 2160);
    }

    #[test]
    fn set_project_settings_fps_rescales_clips_and_is_undoable() {
        let mut exec = make_executor_with_media(); // default 30fps
        let _ = timeline_core::insert_track_at(exec.timeline_mut(), 0, ClipType::Video);
        exec.execute(
            "add_clips",
            &json!({"entries": [{"mediaRef": "media-001", "trackIndex": 0, "startFrame": 0, "durationFrames": 50}]}),
        )
        .unwrap();
        assert_eq!(exec.timeline().tracks[0].clips[0].duration_frames, 50);

        exec.execute("set_project_settings", &json!({"fps": 60}))
            .unwrap();
        assert_eq!(exec.timeline().fps, 60);
        assert_eq!(exec.timeline().tracks[0].clips[0].duration_frames, 100);

        exec.execute("undo", &json!({})).unwrap();
        assert_eq!(exec.timeline().fps, 30);
        assert_eq!(exec.timeline().tracks[0].clips[0].duration_frames, 50);
    }

    #[test]
    fn set_project_settings_fps_out_of_range_rejected() {
        let mut exec = make_executor();
        let err = exec
            .execute("set_project_settings", &json!({"fps": 500}))
            .unwrap_err();
        assert!(err.contains("between 1 and 120"), "err={err}");
    }

    #[test]
    fn split_clips_explicit_mode_two_cuts_on_same_clip() {
        let mut exec = make_executor_with_media();
        let _ = timeline_core::insert_track_at(exec.timeline_mut(), 0, ClipType::Video);
        exec.execute(
            "add_clips",
            &json!({"entries": [{"mediaRef": "media-001", "trackIndex": 0, "startFrame": 0, "durationFrames": 100}]}),
        )
        .unwrap();
        let clip_id = exec.timeline().tracks[0].clips[0].id.clone();
        exec.execute(
            "split_clips",
            &json!({"splits": [
                {"clipId": clip_id, "atFrame": 30},
                {"clipId": clip_id, "atFrame": 60}
            ]}),
        )
        .unwrap();
        assert_eq!(exec.timeline().tracks[0].clips.len(), 3);
    }

    #[test]
    fn split_clips_track_mode_and_dedup() {
        let mut exec = make_executor_with_media();
        let _ = timeline_core::insert_track_at(exec.timeline_mut(), 0, ClipType::Video);
        exec.execute(
            "add_clips",
            &json!({"entries": [{"mediaRef": "media-001", "trackIndex": 0, "startFrame": 0, "durationFrames": 100}]}),
        )
        .unwrap();
        // Duplicate cut points must dedup to a single split.
        exec.execute("split_clips", &json!({"trackIndex": 0, "frames": [50, 50, 50]}))
            .unwrap();
        assert_eq!(exec.timeline().tracks[0].clips.len(), 2);
    }

    #[test]
    fn split_clips_rejects_bad_point_with_no_partial_state() {
        let mut exec = make_executor_with_media();
        let _ = timeline_core::insert_track_at(exec.timeline_mut(), 0, ClipType::Video);
        exec.execute(
            "add_clips",
            &json!({"entries": [{"mediaRef": "media-001", "trackIndex": 0, "startFrame": 0, "durationFrames": 100}]}),
        )
        .unwrap();
        let clip_id = exec.timeline().tracks[0].clips[0].id.clone();
        let err = exec
            .execute(
                "split_clips",
                &json!({"splits": [
                    {"clipId": clip_id, "atFrame": 30},
                    {"clipId": clip_id, "atFrame": 999}
                ]}),
            )
            .unwrap_err();
        assert!(err.contains("strictly inside"), "err={err}");
        assert_eq!(
            exec.timeline().tracks[0].clips.len(),
            1,
            "no partial split on rejection"
        );
    }

    #[test]
    fn read_skill_returns_body_for_loaded_skill() {
        let mut exec = make_executor();
        exec.set_skills(vec![AgentSkill {
            id: "captions".into(),
            name: "Captions".into(),
            description: "burn in captions".into(),
            body: "1. Transcribe\n2. Style".into(),
        }]);
        let result = exec.execute("read_skill", &json!({"id": "captions"})).unwrap();
        let text = result["content"][0]["text"].as_str().unwrap();
        assert!(text.contains("Transcribe"));
        assert!(text.contains("Style"));
    }

    #[test]
    fn read_skill_unknown_id_errors() {
        let mut exec = make_executor();
        let err = exec
            .execute("read_skill", &json!({"id": "nope"}))
            .unwrap_err();
        assert!(err.contains("not found"), "err={err}");
    }

    #[test]
    fn read_skill_is_read_only_no_revision_bump() {
        let mut exec = make_executor();
        exec.set_skills(vec![AgentSkill {
            id: "a".into(),
            name: "A".into(),
            description: "d".into(),
            body: "body".into(),
        }]);
        let before = exec.revision();
        exec.execute("read_skill", &json!({"id": "a"})).unwrap();
        assert_eq!(exec.revision(), before, "read_skill must not bump revision");
    }

    #[test]
    fn apply_layout_side_by_side_sets_transforms_and_crop() {
        let mut exec = make_executor_with_media(); // media-001 Video 1920x1080, canvas 1920x1080
        // One clip per track, both starting at frame 0 → co-visible for side_by_side.
        let _ = timeline_core::insert_track_at(exec.timeline_mut(), 0, ClipType::Video);
        let _ = timeline_core::insert_track_at(exec.timeline_mut(), 1, ClipType::Video);
        exec.execute("add_clips", &json!({"entries": [{"mediaRef": "media-001", "trackIndex": 0, "startFrame": 0}]}))
            .unwrap();
        exec.execute("add_clips", &json!({"entries": [{"mediaRef": "media-001", "trackIndex": 1, "startFrame": 0}]}))
            .unwrap();
        let id0 = exec.timeline().tracks[0].clips[0].id.clone();
        let id1 = exec.timeline().tracks[1].clips[0].id.clone();

        exec.execute(
            "apply_layout",
            &json!({
                "layout": "side_by_side",
                "slots": [
                    {"slot": "left", "clipId": id0},
                    {"slot": "right", "clipId": id1}
                ]
            }),
        )
        .unwrap();

        let left = &exec.timeline().tracks[0].clips[0];
        let right = &exec.timeline().tracks[1].clips[0];
        assert!(
            (left.transform.center_x - 0.25).abs() < 1e-6,
            "left cx={}",
            left.transform.center_x
        );
        assert!(
            (right.transform.center_x - 0.75).abs() < 1e-6,
            "right cx={}",
            right.transform.center_x
        );
        // 16:9 source cover-cropped into a half-width slot → 0.25 side crops.
        assert!((left.crop.left - 0.25).abs() < 1e-6, "crop.left={}", left.crop.left);
    }

    #[test]
    fn apply_layout_batches_multiple_clips_per_slot() {
        let mut exec = make_executor_with_media();
        let _ = timeline_core::insert_track_at(exec.timeline_mut(), 0, ClipType::Video);
        let _ = timeline_core::insert_track_at(exec.timeline_mut(), 1, ClipType::Video);
        // Two sequential takes in the left region (same track), one clip on the right.
        exec.execute(
            "add_clips",
            &json!({"entries": [{"mediaRef": "media-001", "trackIndex": 0, "startFrame": 0}, {"mediaRef": "media-001", "trackIndex": 0, "startFrame": 300}]}),
        )
        .unwrap();
        exec.execute("add_clips", &json!({"entries": [{"mediaRef": "media-001", "trackIndex": 1, "startFrame": 0}]}))
            .unwrap();
        let a = exec.timeline().tracks[0].clips[0].id.clone();
        let b = exec.timeline().tracks[0].clips[1].id.clone();
        let c = exec.timeline().tracks[1].clips[0].id.clone();
        exec.execute(
            "apply_layout",
            &json!({
                "layout": "side_by_side",
                "slots": [
                    {"slot": "left", "clipIds": [a, b]},
                    {"slot": "right", "clipIds": [c]}
                ]
            }),
        )
        .unwrap();
        // Both left-slot clips are framed into the left region.
        for clip in &exec.timeline().tracks[0].clips {
            assert!(
                (clip.transform.center_x - 0.25).abs() < 1e-6,
                "left cx={}",
                clip.transform.center_x
            );
        }
        let right = &exec.timeline().tracks[1].clips[0];
        assert!(
            (right.transform.center_x - 0.75).abs() < 1e-6,
            "right cx={}",
            right.transform.center_x
        );
    }

    #[test]
    fn apply_layout_same_track_overlap_errors() {
        let mut exec = make_executor_with_media();
        let _ = timeline_core::insert_track_at(exec.timeline_mut(), 0, ClipType::Video);
        exec.execute(
            "add_clips",
            &json!({"entries": [{"mediaRef": "media-001", "trackIndex": 0, "startFrame": 0}, {"mediaRef": "media-001", "trackIndex": 0, "startFrame": 300}]}),
        )
        .unwrap();
        // Force the two clips to overlap in time on the SAME track.
        exec.timeline_mut().tracks[0].clips[1].start_frame = 100;
        let a = exec.timeline().tracks[0].clips[0].id.clone();
        let b = exec.timeline().tracks[0].clips[1].id.clone();
        let err = exec
            .execute(
                "apply_layout",
                &json!({
                    "layout": "side_by_side",
                    "slots": [
                        {"slot": "left", "clipId": a},
                        {"slot": "right", "clipId": b}
                    ]
                }),
            )
            .unwrap_err();
        assert!(err.contains("same track") && err.contains("overlap"), "err={err}");
    }

    #[test]
    fn apply_layout_non_coincident_clips_error() {
        let mut exec = make_executor_with_media();
        let _ = timeline_core::insert_track_at(exec.timeline_mut(), 0, ClipType::Video);
        let _ = timeline_core::insert_track_at(exec.timeline_mut(), 1, ClipType::Video);
        exec.execute("add_clips", &json!({"entries": [{"mediaRef": "media-001", "trackIndex": 0, "startFrame": 0}]}))
            .unwrap();
        exec.execute("add_clips", &json!({"entries": [{"mediaRef": "media-001", "trackIndex": 1, "startFrame": 0}]}))
            .unwrap();
        // Right clip starts only after the left one ends → never co-visible.
        let left_end = {
            let l = &exec.timeline().tracks[0].clips[0];
            l.start_frame + l.duration_frames
        };
        exec.timeline_mut().tracks[1].clips[0].start_frame = left_end;
        let a = exec.timeline().tracks[0].clips[0].id.clone();
        let b = exec.timeline().tracks[1].clips[0].id.clone();
        let err = exec
            .execute(
                "apply_layout",
                &json!({
                    "layout": "side_by_side",
                    "slots": [
                        {"slot": "left", "clipId": a},
                        {"slot": "right", "clipId": b}
                    ]
                }),
            )
            .unwrap_err();
        assert!(err.contains("never play at the same time"), "err={err}");
    }

    #[test]
    fn apply_layout_unknown_layout_errors() {
        let mut exec = make_executor();
        let err = exec
            .execute(
                "apply_layout",
                &json!({"layout": "nope", "slots": [{"slot": "main", "clipId": "x"}]}),
            )
            .unwrap_err();
        assert!(err.contains("unknown layout"), "err={err}");
    }

    #[test]
    fn apply_layout_missing_slot_errors() {
        let mut exec = make_executor();
        let err = exec
            .execute(
                "apply_layout",
                &json!({"layout": "side_by_side", "slots": [{"slot": "left", "clipId": "x"}]}),
            )
            .unwrap_err();
        assert!(
            err.contains("needs every slot filled") && err.contains("right"),
            "err={err}"
        );
    }

    #[test]
    fn apply_layout_place_new_requires_duration() {
        let mut manifest = MediaManifest::default();
        manifest.entries.push(video_media("m1", 1920, 1080, 30.0));
        let mut exec = ToolExecutor::new(Timeline::default(), manifest);
        let err = exec
            .execute(
                "apply_layout",
                &json!({"layout": "full", "slots": [{"slot": "main", "mediaRef": "m1"}]}),
            )
            .unwrap_err();
        assert!(err.contains("durationFrames >= 1"), "err={err}");
    }

    #[test]
    fn apply_layout_place_new_rejects_missing_asset() {
        let mut exec = make_executor();
        let err = exec
            .execute(
                "apply_layout",
                &json!({"layout": "full", "durationFrames": 30, "slots": [{"slot": "main", "mediaRef": "nope"}]}),
            )
            .unwrap_err();
        assert!(err.contains("asset not found"), "err={err}");
    }

    #[test]
    fn apply_layout_place_new_conflicting_args_leave_timeline_untouched() {
        // durationFrames + trimEndFrame is rejected BEFORE any track is created, so a
        // rejected place-new call does not leave orphaned empty tracks behind.
        let mut manifest = MediaManifest::default();
        manifest.entries.push(video_media("L", 1920, 1080, 30.0));
        manifest.entries.push(video_media("R", 1920, 1080, 30.0));
        let mut exec = ToolExecutor::new(Timeline::default(), manifest);
        let err = exec
            .execute(
                "apply_layout",
                &json!({
                    "layout": "side_by_side",
                    "durationFrames": 60,
                    "trimEndFrame": 10,
                    "slots": [
                        {"slot": "left", "mediaRef": "L"},
                        {"slot": "right", "mediaRef": "R"}
                    ]
                }),
            )
            .unwrap_err();
        assert!(err.contains("trimEndFrame"), "err={err}");
        assert_eq!(exec.timeline().tracks.len(), 0, "no orphaned tracks created");
    }

    #[test]
    fn apply_layout_place_new_creates_tracks_and_frames_clips() {
        let mut manifest = MediaManifest::default();
        manifest.entries.push(video_media("L", 1920, 1080, 30.0));
        manifest.entries.push(video_media("R", 1920, 1080, 30.0));
        let mut exec = ToolExecutor::new(Timeline::default(), manifest);
        let res = exec
            .execute(
                "apply_layout",
                &json!({
                    "layout": "side_by_side",
                    "durationFrames": 60,
                    "slots": [
                        {"slot": "left", "mediaRef": "L"},
                        {"slot": "right", "mediaRef": "R"}
                    ]
                }),
            )
            .unwrap();
        // One new video track per slot, each with one framed clip.
        assert_eq!(exec.timeline().tracks.len(), 2);
        let tl = exec.timeline();
        let clips: Vec<&Clip> = tl.tracks.iter().flat_map(|t| &t.clips).collect();
        let left = clips.iter().find(|c| c.media_ref == "L").expect("left clip");
        let right = clips.iter().find(|c| c.media_ref == "R").expect("right clip");
        assert!((left.transform.center_x - 0.25).abs() < 1e-6, "left cx={}", left.transform.center_x);
        assert!(
            (right.transform.center_x - 0.75).abs() < 1e-6,
            "right cx={}",
            right.transform.center_x
        );
        assert_eq!(left.duration_frames, 60);
        assert_eq!(left.start_frame, 0);
        let env: serde_json::Value =
            serde_json::from_str(res["content"][0]["text"].as_str().unwrap()).unwrap();
        assert_eq!(
            env["createdTracks"].as_array().unwrap().len(),
            2,
            "envelope reports the two created tracks: {env}"
        );
        assert_eq!(env["layout"], json!("side_by_side"));
    }

    #[test]
    fn apply_layout_place_new_stacks_pip_inset_on_top() {
        // The PIP inset has z=1 (main z=0). tracks[0] is the TOP layer, so the inset's
        // track must land at a LOWER index than main's — the insert-at-0 ascending-z
        // stacking put the highest z on top.
        let mut manifest = MediaManifest::default();
        manifest.entries.push(video_media("MAIN", 1920, 1080, 30.0));
        manifest.entries.push(video_media("INSET", 1920, 1080, 30.0));
        let mut exec = ToolExecutor::new(Timeline::default(), manifest);
        exec.execute(
            "apply_layout",
            &json!({
                "layout": "pip_bottom_right",
                "durationFrames": 30,
                "slots": [
                    {"slot": "main", "mediaRef": "MAIN"},
                    {"slot": "inset", "mediaRef": "INSET"}
                ]
            }),
        )
        .unwrap();
        let tl = exec.timeline();
        let inset_track = tl
            .tracks
            .iter()
            .position(|t| t.clips.iter().any(|c| c.media_ref == "INSET"))
            .expect("inset placed");
        let main_track = tl
            .tracks
            .iter()
            .position(|t| t.clips.iter().any(|c| c.media_ref == "MAIN"))
            .expect("main placed");
        assert!(
            inset_track < main_track,
            "PIP inset (z=1) must stack above main (z=0): inset={inset_track}, main={main_track}"
        );
    }

    #[test]
    fn exec_001_get_timeline_returns_default() {
        let mut exec = make_executor();
        let result = exec.execute("get_timeline", &json!({})).unwrap();
        let text = result["content"][0]["text"].as_str().unwrap();
        assert!(text.contains("fps"));
        assert!(text.contains("1920"));
        assert!(text.contains("1080"));
    }

    #[test]
    fn exec_002_unknown_tool_returns_error() {
        let mut exec = make_executor();
        let err = exec.execute("nonexistent", &json!({})).unwrap_err();
        assert!(err.contains("Unknown tool"));
    }

    #[test]
    fn exec_003_split_clips_missing_args() {
        let mut exec = make_executor();
        let err = exec.execute("split_clips", &json!({})).unwrap_err();
        assert!(err.contains("Provide either"), "err={err}");
    }

    #[test]
    fn exec_004_undo_empty_returns_error() {
        let mut exec = make_executor();
        let result = exec.execute("undo", &json!({})).unwrap();
        assert_eq!(result["isError"], true);
    }

    #[test]
    fn exec_005_add_then_remove_track() {
        let mut exec = make_executor();
        let _ = timeline_core::insert_track_at(exec.timeline_mut(), 0, ClipType::Video);
        assert_eq!(exec.timeline().tracks.len(), 1);
        assert!(exec.execute("undo", &json!({})).is_ok());
    }

    #[test]
    fn exec_006_remove_clips_empty_ids() {
        let mut exec = make_executor();
        let err = exec
            .execute("remove_clips", &json!({"clipIds": []}))
            .unwrap_err();
        // wire-mutation-validators: MUT-005 gate message.
        assert!(err.contains("missing or empty 'clipIds'"), "got: {err}");
    }

    #[test]
    fn exec_007_move_clips_no_tracks() {
        let mut exec = make_executor();
        let err = exec
            .execute(
                "move_clips",
                &json!({"clipIds": ["c1"], "toTrack": 0, "toFrame": 10}),
            )
            .unwrap_err();
        assert!(err.contains("out of bounds"));
    }

    #[test]
    fn exec_008_set_clip_properties_missing_ids() {
        let mut exec = make_executor();
        let err = exec
            .execute(
                "set_clip_properties",
                &json!({"clipIds": [], "properties": {}}),
            )
            .unwrap_err();
        // wire-mutation-validators: MUT-009 gate message.
        assert!(err.contains("missing or empty 'clipIds'"), "got: {err}");
    }

    #[test]
    fn exec_009_manage_tracks_empty_call_rejected() {
        let mut exec = make_executor();
        let err = exec.execute("manage_tracks", &json!({})).unwrap_err();
        assert!(err.contains("at least one of reorder, set, or remove"), "got: {err}");
    }

    #[test]
    fn exec_010_undo_tracking_on_mutation() {
        let mut exec = make_executor();
        assert_eq!(exec.undo_stack().len(), 0);

        let _ = timeline_core::insert_track_at(exec.timeline_mut(), 0, ClipType::Video);
        assert_eq!(exec.timeline().tracks.len(), 1);

        let result = exec
            .execute("manage_tracks", &json!({"remove": [0]}))
            .unwrap();
        assert!(result["isError"].is_null() || result["isError"] == false);
        assert_eq!(exec.timeline().tracks.len(), 0);
        assert_eq!(exec.undo_stack().len(), 1);
    }

    #[test]
    fn manage_tracks_resolves_indexes_up_front() {
        // set[].index refers to the CALL-TIME order even after reorder moves
        // tracks around (design C-7: all indexes resolve to ids up front).
        let mut exec = make_executor();
        let _ = timeline_core::insert_track_at(exec.timeline_mut(), 0, ClipType::Video);
        let _ = timeline_core::insert_track_at(exec.timeline_mut(), 1, ClipType::Video);
        let top_id = exec.timeline().tracks[0].id.clone();
        let result = exec
            .execute(
                "manage_tracks",
                &json!({
                    "reorder": [{"index": 0, "to": 1}],
                    "set": [{"index": 0, "hidden": true}],
                }),
            )
            .unwrap();
        // The ORIGINAL track 0 moved to index 1 and is the one hidden.
        assert_eq!(exec.timeline().tracks[1].id, top_id);
        assert!(exec.timeline().tracks[1].hidden, "pre-call index 0 hidden");
        assert!(!exec.timeline().tracks[0].hidden);
        let body: Value = serde_json::from_str(
            result["content"][0]["text"].as_str().unwrap(),
        )
        .unwrap();
        assert_eq!(body["tracks"][1]["hidden"], json!(true));
        assert!(
            body["notes"][0].as_str().unwrap().contains("Track indices changed"),
            "reorder adds the note: {body}"
        );
    }

    #[test]
    fn manage_tracks_reorder_clamps_to_type_zone() {
        // An audio track can't move into the video zone: 'to' clamps.
        let mut exec = make_executor();
        let _ = timeline_core::insert_track_at(exec.timeline_mut(), 0, ClipType::Video);
        let _ = timeline_core::insert_track_at(exec.timeline_mut(), 1, ClipType::Video);
        let _ = timeline_core::insert_track_at(exec.timeline_mut(), 2, ClipType::Audio);
        let _ = timeline_core::insert_track_at(exec.timeline_mut(), 3, ClipType::Audio);
        let audio_id = exec.timeline().tracks[3].id.clone();
        exec.execute("manage_tracks", &json!({"reorder": [{"index": 3, "to": 0}]}))
            .unwrap();
        // Clamped to the top of the AUDIO zone (index 2), not index 0.
        assert_eq!(exec.timeline().tracks[2].id, audio_id);
        assert_eq!(exec.timeline().tracks[2].r#type, ClipType::Audio);
        assert_eq!(exec.timeline().tracks[0].r#type, ClipType::Video);
    }

    #[test]
    fn manage_tracks_remove_keeps_partners_and_reports_order() {
        let mut exec = make_executor();
        let _ = timeline_core::insert_track_at(exec.timeline_mut(), 0, ClipType::Video);
        let _ = timeline_core::insert_track_at(exec.timeline_mut(), 1, ClipType::Audio);
        // Linked A/V pair across the two tracks.
        let group = "grp-1".to_string();
        let mut v = crate::test_helpers::make_clip(0, 100);
        v.link_group_id = Some(group.clone());
        let mut a = crate::test_helpers::make_clip(0, 100);
        a.media_type = ClipType::Audio;
        a.source_clip_type = ClipType::Audio;
        a.link_group_id = Some(group);
        let _ = timeline_core::place_clips(exec.timeline_mut(), 0, 0, &[v]);
        let _ = timeline_core::place_clips(exec.timeline_mut(), 1, 0, &[a]);

        let result = exec
            .execute("manage_tracks", &json!({"remove": [0]}))
            .unwrap();
        assert_eq!(exec.timeline().tracks.len(), 1);
        assert_eq!(
            exec.timeline().tracks[0].clips.len(),
            1,
            "linked partner on the OTHER track stays"
        );
        let body: Value = serde_json::from_str(
            result["content"][0]["text"].as_str().unwrap(),
        )
        .unwrap();
        assert_eq!(body["tracks"][0]["index"], json!(0));
        assert_eq!(body["tracks"][0]["type"], json!("audio"));
        assert_eq!(body["tracks"][0]["label"], json!("A1"));
        assert!(body["tracks"][0].get("muted").is_none(), "defaults stripped");
        assert!(body["tracks"][0].get("syncLocked").is_none(), "true stripped");
    }

    #[test]
    fn manage_tracks_set_reports_sync_locked_false_only() {
        let mut exec = make_executor();
        let _ = timeline_core::insert_track_at(exec.timeline_mut(), 0, ClipType::Audio);
        let result = exec
            .execute(
                "manage_tracks",
                &json!({"set": [{"index": 0, "muted": true, "syncLocked": false}]}),
            )
            .unwrap();
        assert!(exec.timeline().tracks[0].muted);
        assert!(!exec.timeline().tracks[0].sync_locked);
        let body: Value = serde_json::from_str(
            result["content"][0]["text"].as_str().unwrap(),
        )
        .unwrap();
        assert_eq!(body["tracks"][0]["muted"], json!(true));
        assert_eq!(body["tracks"][0]["syncLocked"], json!(false));
        assert!(body.get("notes").is_none(), "set-only call has no reorder note");
    }

    #[test]
    fn manage_tracks_out_of_range_index_refused() {
        let mut exec = make_executor();
        let _ = timeline_core::insert_track_at(exec.timeline_mut(), 0, ClipType::Video);
        let err = exec
            .execute("manage_tracks", &json!({"remove": [5]}))
            .unwrap_err();
        assert!(err.contains("out of range"), "got: {err}");
    }

    // ── duplicate_clips (upstream #176) ──────────────────────────────────

    #[test]
    fn duplicate_clips_creates_exact_copy_at_new_position() {
        let mut exec = make_executor();
        let _ = timeline_core::insert_track_at(exec.timeline_mut(), 0, ClipType::Video);
        let mut src = crate::test_helpers::make_clip(0, 60);
        src.speed = 1.5;
        src.opacity = 0.7;
        let src_id = src.id.clone();
        let src_ref = src.media_ref.clone();
        let _ = timeline_core::place_clips(exec.timeline_mut(), 0, 0, &[src]);
        // place_clips assigns a fresh id; find the placed clip's real id.
        let placed_id = exec.timeline().tracks[0].clips[0].id.clone();

        exec.execute(
            "duplicate_clips",
            &json!({"entries": [{"clipId": placed_id, "toFrame": 100}]}),
        )
        .unwrap();

        let mut clips = exec.timeline().tracks[0].clips.clone();
        clips.sort_by_key(|c| c.start_frame);
        assert_eq!(clips.len(), 2);
        let copy = &clips[1];
        assert_eq!(copy.start_frame, 100);
        assert_ne!(copy.id, placed_id, "copy gets a fresh id");
        assert_ne!(copy.id, src_id);
        assert_eq!(copy.speed, 1.5, "speed preserved");
        assert_eq!(copy.opacity, 0.7, "opacity preserved");
        assert_eq!(copy.media_ref, src_ref, "same source media");
    }

    #[test]
    fn duplicate_clips_preserves_keyframes_and_effects() {
        let mut exec = make_executor();
        let _ = timeline_core::insert_track_at(exec.timeline_mut(), 0, ClipType::Video);
        let mut src = crate::test_helpers::make_clip(0, 60);
        src.opacity_track = Some(KeyframeTrack {
            keyframes: vec![
                Keyframe { frame: 0, value: 1.0, interpolation_out: Interpolation::Linear },
                Keyframe { frame: 30, value: 0.5, interpolation_out: Interpolation::Linear },
            ],
        });
        src.effects = Some(vec![Effect::new("blur", vec![("radius", 10.0)])]);
        src.fade_in_frames = 5;
        let _ = timeline_core::place_clips(exec.timeline_mut(), 0, 0, &[src]);
        let placed_id = exec.timeline().tracks[0].clips[0].id.clone();

        exec.execute(
            "duplicate_clips",
            &json!({"entries": [{"clipId": placed_id, "toFrame": 100}]}),
        )
        .unwrap();

        let copy = exec
            .timeline()
            .tracks[0]
            .clips
            .iter()
            .find(|c| c.start_frame == 100)
            .cloned()
            .expect("copy at frame 100");
        let kf = copy.opacity_track.as_ref().expect("opacity keyframes preserved");
        assert_eq!(kf.keyframes.len(), 2);
        assert_eq!(kf.keyframes[1].value, 0.5);
        let fx = copy.effects.as_ref().expect("effects preserved");
        assert_eq!(fx.len(), 1);
        assert_eq!(fx[0].r#type, "blur");
        assert_eq!(copy.fade_in_frames, 5, "fade preserved");
    }

    #[test]
    fn duplicate_clips_expands_linked_partners() {
        let mut exec = make_executor();
        let _ = timeline_core::insert_track_at(exec.timeline_mut(), 0, ClipType::Video);
        let _ = timeline_core::insert_track_at(exec.timeline_mut(), 1, ClipType::Audio);
        let group = "grp-dup".to_string();
        let mut v = crate::test_helpers::make_clip(0, 60);
        v.link_group_id = Some(group.clone());
        let mut a = crate::test_helpers::make_clip(0, 60);
        a.media_type = ClipType::Audio;
        a.source_clip_type = ClipType::Audio;
        a.link_group_id = Some(group);
        let _ = timeline_core::place_clips(exec.timeline_mut(), 0, 0, &[v]);
        let _ = timeline_core::place_clips(exec.timeline_mut(), 1, 0, &[a]);
        let video_id = exec.timeline().tracks[0].clips[0].id.clone();

        // Only the lead is named; the audio partner is duplicated automatically.
        exec.execute(
            "duplicate_clips",
            &json!({"entries": [{"clipId": video_id, "toFrame": 100}]}),
        )
        .unwrap();

        let mut vclips = exec.timeline().tracks[0].clips.clone();
        let mut aclips = exec.timeline().tracks[1].clips.clone();
        vclips.sort_by_key(|c| c.start_frame);
        aclips.sort_by_key(|c| c.start_frame);
        assert_eq!(vclips.len(), 2, "video duplicated");
        assert_eq!(aclips.len(), 2, "audio partner duplicated");
        assert_eq!(vclips[1].start_frame, 100);
        assert_eq!(aclips[1].start_frame, 100);
        // The two copies share a FRESH link group, not the original.
        let vg = vclips[1].link_group_id.as_ref().expect("copy video linked");
        let ag = aclips[1].link_group_id.as_ref().expect("copy audio linked");
        assert_eq!(vg, ag, "copies stay linked to each other");
        assert_ne!(vg, "grp-dup", "copies get a fresh link group");
    }

    #[test]
    fn duplicate_clips_rejects_invalid_track() {
        let mut exec = make_executor();
        let _ = timeline_core::insert_track_at(exec.timeline_mut(), 0, ClipType::Video);
        let src = crate::test_helpers::make_clip(0, 30);
        let _ = timeline_core::place_clips(exec.timeline_mut(), 0, 0, &[src]);
        let clip_id = exec.timeline().tracks[0].clips[0].id.clone();

        let err = exec
            .execute(
                "duplicate_clips",
                &json!({"entries": [{"clipId": clip_id, "toTrack": 99, "toFrame": 0}]}),
            )
            .unwrap_err();
        assert!(err.contains("out of range"), "got: {err}");
    }

    #[test]
    fn duplicate_clips_rejects_empty_entries() {
        let mut exec = make_executor();
        let err = exec
            .execute("duplicate_clips", &json!({"entries": []}))
            .unwrap_err();
        assert!(err.contains("entries"), "got: {err}");
    }

    #[test]
    fn duplicate_clips_expands_and_shortens_ids() {
        // The nested entries[].clipId is a SCALAR_ID_KEY, so a >= 8-char prefix
        // expands (C-3); output duplicatedClipIds come back shortened.
        let mut exec = make_executor();
        let _ = timeline_core::insert_track_at(exec.timeline_mut(), 0, ClipType::Video);
        let src = crate::test_helpers::make_clip(0, 30);
        let _ = timeline_core::place_clips(exec.timeline_mut(), 0, 0, &[src]);
        let full_id = exec.timeline().tracks[0].clips[0].id.clone();
        let prefix = &full_id[..8];

        let result = exec
            .execute(
                "duplicate_clips",
                &json!({"entries": [{"clipId": prefix, "toFrame": 100}]}),
            )
            .expect("prefix clipId expands to the full id");
        assert_eq!(exec.timeline().tracks[0].clips.len(), 2, "duplicate placed");
        let body: Value =
            serde_json::from_str(result["content"][0]["text"].as_str().unwrap()).unwrap();
        let dup = body["duplicatedClipIds"][0].as_str().unwrap();
        // Output ids are shortened to a unique prefix, not the full 36-char uuid.
        assert!(dup.len() < 36, "output id shortened: {dup}");
    }

    #[test]
    fn exec_011_get_media_found() {
        let mut exec = make_executor_with_media();
        let result = exec
            .execute("get_media", &json!({"ids": ["media-001"]}))
            .unwrap();
        let v: Value = serde_json::from_str(result["content"][0]["text"].as_str().unwrap()).unwrap();
        assert_eq!(v["assets"][0]["name"], json!("test_video.mp4"));
        assert_eq!(v["assets"][0]["type"], json!("video"));
        assert_eq!(v["assets"][0]["hasAudio"], json!(true));
        assert!(v.get("folders").is_none(), "filtered reads skip folders");
    }

    #[test]
    fn get_media_surfaces_generation_status() {
        // #216: get_media surfaces a not-ready async-generation status so the agent
        // waits for 'none' before referencing the asset.
        let mut exec = make_executor_with_media();
        exec.media_manifest.entries[0].generation_status = Some("generating".into());
        let res = exec
            .execute("get_media", &json!({"ids": ["media-001"]}))
            .unwrap();
        let v: Value = serde_json::from_str(res["content"][0]["text"].as_str().unwrap()).unwrap();
        assert_eq!(v["assets"][0]["generationStatus"], json!("generating"));
        // pending:true finds the unresolved asset too.
        let pend = exec.execute("get_media", &json!({"pending": true})).unwrap();
        let pv: Value = serde_json::from_str(pend["content"][0]["text"].as_str().unwrap()).unwrap();
        assert_eq!(pv["assets"].as_array().unwrap().len(), 1);
        // 'none' is ready → not surfaced, and pending no longer matches.
        exec.media_manifest.entries[0].generation_status = Some("none".into());
        let res2 = exec
            .execute("get_media", &json!({"ids": ["media-001"]}))
            .unwrap();
        let v2: Value = serde_json::from_str(res2["content"][0]["text"].as_str().unwrap()).unwrap();
        assert!(v2["assets"][0].get("generationStatus").is_none());
        let pend2 = exec.execute("get_media", &json!({"pending": true})).unwrap();
        let pv2: Value = serde_json::from_str(pend2["content"][0]["text"].as_str().unwrap()).unwrap();
        assert_eq!(pv2["assets"].as_array().unwrap().len(), 0);
    }

    // ── Generation recovery seam (upstream #216) ──

    fn make_recovery_executor() -> ToolExecutor {
        let mut manifest = MediaManifest::default();
        for (id, job) in [
            ("gen-ok", "job-ok"),
            ("gen-fail", "job-fail"),
            ("gen-err", "job-err"),
        ] {
            manifest.entries.push(core_model::MediaManifestEntry {
                id: id.to_string(),
                name: format!("{id}.mp4"),
                r#type: core_model::ClipType::Video,
                source: MediaSource::External {
                    absolute_path: String::new(),
                },
                duration: 5.0,
                generation_input: Some(core_model::GenerationInput {
                    backend_job_id: Some(job.to_string()),
                    ..Default::default()
                }),
                source_width: None,
                source_height: None,
                source_fps: None,
                has_audio: None,
                folder_id: None,
                cached_remote_url: None,
                cached_remote_url_expires_at: None,
                source_timecode_frame: None,
                source_timecode_quanta: None,
                source_timecode_drop_frame: None,
                ai_tags: None,
                ai_description: None,
                ai_label_status: None,
                generation_status: Some("generating".to_string()),
            });
        }
        ToolExecutor::new(Timeline::default(), manifest)
    }

    struct MockGenerationBackend;
    impl GenerationBackend for MockGenerationBackend {
        fn resume_job(&self, job_id: &str) -> Result<generation_core::GenerationOutcome, String> {
            match job_id {
                "job-ok" => Ok(generation_core::GenerationOutcome::Success {
                    result_urls: vec!["https://cdn/out.mp4".into()],
                }),
                "job-fail" => Ok(generation_core::GenerationOutcome::Failure {
                    reason: "job expired".into(),
                }),
                _ => Err("backend unreachable".into()),
            }
        }
    }

    #[test]
    fn generation_recovery_full_chain_applies_outcomes() {
        let mut exec = make_recovery_executor();
        let rev0 = exec.revision();
        exec.set_generation_backend(std::sync::Arc::new(MockGenerationBackend));

        let records = exec.recover_generations();
        assert_eq!(records.len(), 3);
        assert!(matches!(records[0].outcome, Some(Ok(()))));
        assert!(matches!(records[1].outcome, Some(Ok(()))));
        assert!(matches!(records[2].outcome, Some(Err(_))));

        let e0 = &exec.media_manifest().entries[0];
        assert_eq!(e0.generation_status.as_deref(), Some("none"));
        assert_eq!(
            e0.generation_input.as_ref().unwrap().result_urls,
            Some(vec!["https://cdn/out.mp4".to_string()])
        );
        let e1 = &exec.media_manifest().entries[1];
        assert_eq!(e1.generation_status.as_deref(), Some("failed"));
        assert_eq!(e1.generation_input.as_ref().unwrap().result_urls, None);
        // Transport error leaves the entry untouched so a later pass can retry.
        let e2 = &exec.media_manifest().entries[2];
        assert_eq!(e2.generation_status.as_deref(), Some("generating"));
        assert!(
            exec.revision() > rev0,
            "applied outcomes must bump the revision"
        );

        // Applied jobs are terminal — a second pass only retries the unreachable one.
        let again = exec.recover_generations();
        assert_eq!(again.len(), 1);
        assert_eq!(again[0].job.backend_job_id, "job-err");
    }

    #[test]
    fn generation_recovery_without_backend_records_plan_only() {
        let mut exec = make_recovery_executor();
        let rev0 = exec.revision();

        let records = exec.recover_generations();
        assert_eq!(records.len(), 3);
        assert!(records.iter().all(|r| r.outcome.is_none()));
        assert!(records
            .iter()
            .all(|r| r.job.action == generation_core::RecoveryAction::Resubscribe));
        // No backend: nothing applied, no error, manifest untouched.
        assert!(exec
            .media_manifest()
            .entries
            .iter()
            .all(|e| e.generation_status.as_deref() == Some("generating")));
        assert_eq!(exec.revision(), rev0);
    }

    #[test]
    fn exec_012_get_media_unknown_id_returns_empty() {
        // v2: an ids poll for an unknown id returns no assets — the caller
        // learns the placeholder vanished without an error round.
        let mut exec = make_executor_with_media();
        let res = exec
            .execute("get_media", &json!({"ids": ["nonexistent"]}))
            .unwrap();
        let v: Value = serde_json::from_str(res["content"][0]["text"].as_str().unwrap()).unwrap();
        assert_eq!(v["assets"].as_array().unwrap().len(), 0);
    }

    #[test]
    fn exec_013_get_media_unfiltered_lists_folders_and_timelines() {
        // v2 (absorbs list_folders): unfiltered reads include folders as
        // paths and the timelines list.
        let mut exec = make_executor_with_media();
        let res = exec.execute("get_media", &json!({})).unwrap();
        let v: Value = serde_json::from_str(res["content"][0]["text"].as_str().unwrap()).unwrap();
        assert_eq!(v["folders"], json!(["Test Folder"]));
        assert_eq!(v["timelines"][0]["active"], json!(true));
        assert_eq!(v["assets"].as_array().unwrap().len(), 1);
    }

    #[test]
    fn exec_014_search_media_by_name() {
        let mut exec = make_executor_with_media();
        let result = exec
            .execute("search_media", &json!({"query": "test_video"}))
            .unwrap();
        let text = result["content"][0]["text"].as_str().unwrap();
        // Should return structured output with files group
        assert!(text.contains("media-001"));
        assert!(text.contains("\"files\""));
        assert!(text.contains("Found 1 media"));
    }

    #[test]
    fn exec_015_search_media_no_results() {
        let mut exec = make_executor_with_media();
        let result = exec
            .execute("search_media", &json!({"query": "nothing"}))
            .unwrap();
        let text = result["content"][0]["text"].as_str().unwrap();
        let parsed: serde_json::Value = serde_json::from_str(text).unwrap();
        assert!(parsed["files"].as_array().unwrap().is_empty());
        assert_eq!(parsed["status"], "ok");
    }

    #[test]
    fn exec_023_search_media_by_type() {
        let mut exec = make_executor_with_media();
        let result = exec
            .execute("search_media", &json!({"type": "video"}))
            .unwrap();
        let text = result["content"][0]["text"].as_str().unwrap();
        let parsed: serde_json::Value = serde_json::from_str(text).unwrap();
        let files = parsed["files"].as_array().unwrap();
        assert_eq!(files.len(), 1);
        assert_eq!(files[0]["media_id"], "media-001");
    }

    #[test]
    fn exec_024_search_media_no_match_type() {
        let mut exec = make_executor_with_media();
        let result = exec
            .execute("search_media", &json!({"type": "image"}))
            .unwrap();
        let text = result["content"][0]["text"].as_str().unwrap();
        let parsed: serde_json::Value = serde_json::from_str(text).unwrap();
        assert!(parsed["files"].as_array().unwrap().is_empty());
    }

    #[test]
    fn exec_025_search_media_limit() {
        let mut exec = make_executor_with_media();
        let result = exec
            .execute("search_media", &json!({"query": "", "limit": 1}))
            .unwrap();
        let text = result["content"][0]["text"].as_str().unwrap();
        let parsed: serde_json::Value = serde_json::from_str(text).unwrap();
        assert_eq!(parsed["limit"], 1);
        let files = parsed["files"].as_array().unwrap();
        assert_eq!(files.len(), 1);
    }

    #[test]
    fn exec_026_search_media_empty_query_shows_all() {
        let mut exec = make_executor_with_media();
        let result = exec.execute("search_media", &json!({"query": ""})).unwrap();
        let text = result["content"][0]["text"].as_str().unwrap();
        let parsed: serde_json::Value = serde_json::from_str(text).unwrap();
        let files = parsed["files"].as_array().unwrap();
        assert_eq!(files.len(), 1);
        assert_eq!(files[0]["media_id"], "media-001");
    }

    #[test]
    fn exec_027_search_media_with_status() {
        // READ-026: Status reporting for visual indexing
        let mut exec = make_executor_with_media();
        exec.set_search_status("Indexing 1 asset");
        let result = exec
            .execute("search_media", &json!({"query": "test_video"}))
            .unwrap();
        let text = result["content"][0]["text"].as_str().unwrap();
        let parsed: serde_json::Value = serde_json::from_str(text).unwrap();
        assert!(parsed["status"].as_str().unwrap().contains("Indexing"));
    }

    #[test]
    fn exec_028_search_media_no_results_with_status() {
        // READ-026: Status shown even with no results
        let mut exec = make_executor_with_media();
        exec.set_search_status("Model not ready");
        let result = exec
            .execute("search_media", &json!({"query": "nothing"}))
            .unwrap();
        let text = result["content"][0]["text"].as_str().unwrap();
        let parsed: serde_json::Value = serde_json::from_str(text).unwrap();
        assert_eq!(parsed["status"], "Model not ready");
    }

    #[test]
    fn exec_029_search_media_default_status_ok() {
        // READ-026: Default empty status shows ok
        let mut exec = make_executor_with_media();
        let result = exec
            .execute("search_media", &json!({"query": "test_video"}))
            .unwrap();
        let text = result["content"][0]["text"].as_str().unwrap();
        let parsed: serde_json::Value = serde_json::from_str(text).unwrap();
        assert!(parsed["status"].as_str().unwrap().contains("Found"));
    }

    #[test]
    fn exec_016_get_media_folder_filter_includes_subfolders() {
        let mut exec = make_executor_with_media();
        exec.media_manifest.folders.push(core_model::MediaFolder {
            id: "folder-002".to_string(),
            name: "Takes".to_string(),
            parent_folder_id: Some("folder-001".to_string()),
        });
        exec.media_manifest.entries[0].folder_id = Some("folder-002".to_string());
        let res = exec
            .execute("get_media", &json!({"folder": "Test Folder"}))
            .unwrap();
        let v: Value = serde_json::from_str(res["content"][0]["text"].as_str().unwrap()).unwrap();
        assert_eq!(v["assets"].as_array().unwrap().len(), 1, "subfolder asset matches");
        assert_eq!(v["assets"][0]["folder"], json!("Test Folder/Takes"));
    }

    #[test]
    fn exec_017_get_media_empty_library() {
        let mut exec = make_executor();
        let res = exec.execute("get_media", &json!({})).unwrap();
        let v: Value = serde_json::from_str(res["content"][0]["text"].as_str().unwrap()).unwrap();
        assert_eq!(v["assets"].as_array().unwrap().len(), 0);
        assert!(v.get("folders").is_none(), "no folders key when empty");
        assert!(v["timelines"].is_array(), "timelines always listed unfiltered");
    }

    #[test]
    fn exec_018_list_models() {
        // Real catalog (generation_core), not the old hardcoded placeholders.
        let mut exec = make_executor();
        let result = exec.execute("list_models", &json!({})).unwrap();
        let body: Value =
            serde_json::from_str(result["content"][0]["text"].as_str().unwrap()).unwrap();
        assert_eq!(body["loaded"], json!(true));
        let models = body["models"].as_array().unwrap();
        assert_eq!(models.len(), 19, "10 video + 5 image + 4 audio");
        let ids: Vec<&str> = models.iter().map(|m| m["id"].as_str().unwrap()).collect();
        assert!(ids.contains(&"seedance-2"));
        assert!(ids.contains(&"nano-banana-pro"));
        assert!(ids.contains(&"elevenlabs-tts-v3"));
        assert!(!ids.contains(&"gen-3"), "placeholder list removed");
        // No account seam installed → free tier; all shipped models are free.
        assert!(models.iter().all(|m| m["available"] == json!(true)));
    }

    #[test]
    fn exec_018_list_models_video_filter_exact() {
        // Spec: video filter returns exactly the catalog's video entries, in order.
        let mut exec = make_executor();
        let result = exec
            .execute("list_models", &json!({"type": "video"}))
            .unwrap();
        let body: Value =
            serde_json::from_str(result["content"][0]["text"].as_str().unwrap()).unwrap();
        let models = body["models"].as_array().unwrap();
        let ids: Vec<&str> = models.iter().map(|m| m["id"].as_str().unwrap()).collect();
        assert_eq!(
            ids,
            vec![
                "seedance-2",
                "seedance-2-fast",
                "kling-o3",
                "kling-v3",
                "veo3.1-fast",
                "veo3.1",
                "veo3.1-lite",
                "grok-imagine-video",
                "kling-o3-edit",
                "kling-v3-motion-control",
            ]
        );
        assert_eq!(models[0]["displayName"], json!("Seedance 2"));
        assert_eq!(models[0]["type"], json!("video"));
        assert_eq!(models[0]["durations"].as_array().unwrap().len(), 12);
        assert_eq!(models[0]["referenceTagNoun"], json!("Image"));
        assert_eq!(models[2]["displayName"], json!("Kling O3"));
        assert_eq!(models[2]["maxReferenceImages"], json!(7));
    }

    #[test]
    fn exec_018_list_models_emits_aspect_ratio_labels() {
        // #284: every image/video entry carries human-readable aspectRatioLabels
        // parallel to the raw aspectRatios ids.
        let mut exec = make_executor();
        let result = exec
            .execute("list_models", &json!({"type": "image"}))
            .unwrap();
        let body: Value =
            serde_json::from_str(result["content"][0]["text"].as_str().unwrap()).unwrap();
        let models = body["models"].as_array().unwrap();
        let m = models
            .iter()
            .find(|m| m["aspectRatios"].as_array().is_some_and(|a| !a.is_empty()))
            .expect("an image model with aspect ratios");
        let ratios = m["aspectRatios"].as_array().unwrap();
        let labels = m["aspectRatioLabels"]
            .as_array()
            .expect("aspectRatioLabels present");
        assert_eq!(ratios.len(), labels.len(), "labels parallel the raw ids");
        for (r, l) in ratios.iter().zip(labels) {
            let raw = r.as_str().unwrap();
            let label = l.as_str().unwrap();
            if raw == "auto" {
                assert_eq!(label, "Auto");
            } else {
                // Colon-form ids pass through unchanged.
                assert_eq!(label, model_catalog::aspect_ratio_display_label(raw));
            }
        }
    }

    #[test]
    fn exec_018_list_models_audio_filter() {
        let mut exec = make_executor();
        let result = exec
            .execute("list_models", &json!({"type": "audio"}))
            .unwrap();
        let body: Value =
            serde_json::from_str(result["content"][0]["text"].as_str().unwrap()).unwrap();
        let models = body["models"].as_array().unwrap();
        let ids: Vec<&str> = models.iter().map(|m| m["id"].as_str().unwrap()).collect();
        assert_eq!(
            ids,
            vec![
                "elevenlabs-tts-v3",
                "gemini-3.1-flash-tts",
                "minimax-music-v2.6",
                "elevenlabs-music",
            ]
        );
        assert_eq!(models[0]["voiceCount"], json!(21));
        assert_eq!(models[0]["defaultVoice"], json!("Rachel"));
        assert_eq!(models[0]["voicesSample"].as_array().unwrap().len(), 3);
        assert_eq!(models[3]["durations"], json!([15, 30, 60, 90, 120, 180]));
    }

    struct MockAccount {
        paid: bool,
    }
    impl AccountState for MockAccount {
        fn is_paid(&self) -> bool {
            self.paid
        }
    }

    #[test]
    fn model_gating_free_tier_defaults_without_seam() {
        let exec = make_executor();
        assert!(!exec.is_paid_account(), "no seam installed → free tier");
    }

    #[test]
    fn model_gating_seam_reports_paid() {
        let mut exec = make_executor();
        exec.set_account_state(Arc::new(MockAccount { paid: true }));
        assert!(exec.is_paid_account());
        exec.set_account_state(Arc::new(MockAccount { paid: false }));
        assert!(!exec.is_paid_account());
    }

    #[test]
    fn model_gating_paid_only_entry_marked_not_hidden() {
        // Spec: a paid_only model on free tier is marked unavailable/upgrade-required
        // rather than hidden. The shipped catalog has no paid_only entries (the
        // in-repo source predates #249), so exercise the marking with a synthetic one.
        let gated = generation_core::model_catalog::ModelConfig {
            id: "paid-model",
            display_name: "Paid Model",
            paid_only: true,
            caps: generation_core::model_catalog::ModelCaps::Image(
                generation_core::model_catalog::ImageCaps {
                    supports_image_reference: false,
                    max_images: 1,
                    ..Default::default()
                },
            ),
        };
        let free = ToolExecutor::model_entry_json(&gated, false);
        assert_eq!(free["available"], json!(false));
        assert_eq!(free["paidOnly"], json!(true));
        assert!(free["upgrade"].as_str().unwrap().contains("paid plan"));

        let paid = ToolExecutor::model_entry_json(&gated, true);
        assert_eq!(paid["available"], json!(true));
        assert_eq!(paid["paidOnly"], json!(true));
        assert!(paid.get("upgrade").is_none());
    }

    #[test]
    fn model_gating_generate_rejects_gated_model() {
        // Spec: generate with a gated model returns an explicit gating error.
        let gated = generation_core::model_catalog::ModelConfig {
            id: "paid-model",
            display_name: "Paid Model",
            paid_only: true,
            caps: generation_core::model_catalog::ModelCaps::Video(Default::default()),
        };
        let exec = make_executor();
        let err = exec.gate_model(&gated).unwrap_err();
        assert!(err.contains("requires a paid plan"), "got: {err}");
        assert!(err.contains("paid-model"));

        let mut exec = make_executor();
        exec.set_account_state(Arc::new(MockAccount { paid: true }));
        assert!(exec.gate_model(&gated).is_ok(), "paid account passes");
    }

    #[test]
    fn model_gating_generate_unknown_model_errors() {
        let mut exec = make_executor();
        let err = exec
            .execute(
                "generate_video",
                &json!({"prompt": "a fox", "model": "gen-3"}),
            )
            .unwrap_err();
        assert!(err.contains("Unknown model 'gen-3'"), "got: {err}");
        assert!(err.contains("seedance-2"), "lists available ids: {err}");
    }

    #[test]
    fn model_gating_generate_defaults_to_catalog_model() {
        let mut exec = make_executor();
        let result = exec
            .execute("generate_video", &json!({"prompt": "a fox"}))
            .unwrap();
        let text = result["content"][0]["text"].as_str().unwrap();
        assert!(text.contains("seedance-2"), "default is first available");
        assert!(!text.contains("gen-3"));
        // No backend: the stub must not register ghost assets (review F4).
        assert!(exec.media_manifest().entries.is_empty());
    }

    #[test]
    fn model_gating_generate_image_real_model_accepted() {
        let mut exec = make_executor();
        let result = exec
            .execute(
                "generate_image",
                &json!({"prompt": "a fox", "model": "nano-banana-2"}),
            )
            .unwrap();
        let text = result["content"][0]["text"].as_str().unwrap();
        assert!(text.contains("nano-banana-2"));
        assert!(exec.media_manifest().entries.is_empty());
    }

    #[test]
    fn model_gating_generate_audio_resolves_music_model() {
        // v2 (absorbs generate_music): a music-category model resolves through
        // generate_audio and the honest backend-gap error names it.
        let mut exec = make_executor();
        let err = exec
            .execute(
                "generate_audio",
                &json!({"prompt": "calm piano", "model": "minimax-music-v2.6"}),
            )
            .unwrap_err();
        assert!(err.contains("minimax-music-v2.6"), "got: {err}");
        assert!(err.contains("music"), "categorized as music: {err}");
        assert!(exec.media_manifest().entries.is_empty());
    }

    #[test]
    fn exec_019_inspect_media() {
        let mut exec = make_executor_with_media();
        let result = exec
            .execute("inspect_media", &json!({"mediaId": "media-001"}))
            .unwrap();
        let text = result["content"][0]["text"].as_str().unwrap();
        assert!(text.contains("1920"));
    }

    #[test]
    fn exec_019_inspect_media_rejects_text() {
        // READ-013: Text clip rejection
        let mut manifest = core_model::MediaManifest::default();
        manifest.entries.push(core_model::MediaManifestEntry {
            id: "text-media".to_string(),
            name: "text_asset".to_string(),
            r#type: core_model::ClipType::Text,
            source: core_model::MediaSource::External {
                absolute_path: "/tmp/text.txt".to_string(),
            },
            duration: 5.0,
            generation_input: None,
            source_width: None,
            source_height: None,
            source_fps: None,
            has_audio: None,
            folder_id: None,
            cached_remote_url: None,
            cached_remote_url_expires_at: None,
            source_timecode_frame: None,
            source_timecode_quanta: None,
            source_timecode_drop_frame: None,
            ai_tags: None,
            ai_description: None,
            ai_label_status: None,
            generation_status: None,
        });
        let mut exec = ToolExecutor::new(core_model::Timeline::default(), manifest);
        let result = exec
            .execute("inspect_media", &json!({"mediaId": "text-media"}))
            .unwrap();
        assert_eq!(result["isError"], true, "READ-013: text clips return error");
        assert!(
            result["content"][0]["text"]
                .as_str()
                .unwrap()
                .contains("text clip"),
            "READ-013: error mentions text clip"
        );
    }

    #[test]
    fn exec_019_inspect_media_cross_validates_clip_id() {
        // READ-014: clipId → mediaRef cross-validation
        let mut manifest = core_model::MediaManifest::default();
        manifest.entries.push(core_model::MediaManifestEntry {
            id: "media-vid".to_string(),
            name: "video.mp4".to_string(),
            r#type: core_model::ClipType::Video,
            source: core_model::MediaSource::External {
                absolute_path: "/tmp/video.mp4".to_string(),
            },
            duration: 10.0,
            generation_input: None,
            source_width: Some(1920),
            source_height: Some(1080),
            source_fps: Some(30.0),
            has_audio: Some(true),
            folder_id: None,
            cached_remote_url: None,
            cached_remote_url_expires_at: None,
            source_timecode_frame: None,
            source_timecode_quanta: None,
            source_timecode_drop_frame: None,
            ai_tags: None,
            ai_description: None,
            ai_label_status: None,
            generation_status: None,
        });
        let mut timeline = core_model::Timeline::default();
        timeline.tracks.push(core_model::Track {
            id: "track-v".to_string(),
            r#type: core_model::ClipType::Video,
            muted: false,
            hidden: false,
            sync_locked: false,
           display_height: 50.0,
            clips: vec![core_model::Clip {
                id: "clip-vid".to_string(),
                media_ref: "media-vid".to_string(),
                media_type: core_model::ClipType::Video,
                source_clip_type: core_model::ClipType::Video,
                start_frame: 0,
                duration_frames: 100,
                trim_start_frame: 0,
                trim_end_frame: 0,
                speed: 1.0,
                volume: 1.0,
                fade_in_frames: 0,
                fade_out_frames: 0,
                fade_in_interpolation: core_model::Interpolation::Linear,
                fade_out_interpolation: core_model::Interpolation::Linear,
                opacity: 1.0,
                transform: core_model::Transform::default(),
                crop: core_model::Crop::default(),
                link_group_id: None,
                caption_group_id: None,
                text_content: None,
                text_style: None,
                opacity_track: None,
                position_track: None,
                scale_track: None,
                rotation_track: None,
                crop_track: None,
                volume_track: None,
                effects: None,
                shape_style: None,
                stroke_progress_track: None,
                compound_timeline_id: None,
                blend_mode: Default::default(),
                chroma_key: None,
                multicam_group_id: None,
                text_animation: None,
                word_timings: None,
            }],
        });
        let mut exec = ToolExecutor::new(timeline, manifest);
        // Valid clipId → mediaRef should succeed
        let result = exec.execute(
            "inspect_media",
            &json!({"mediaId": "media-vid", "clipId": "clip-vid"}),
        );
        assert!(result.is_ok(), "READ-014: valid clipId should work");

        // Mismatched clipId → mediaRef should fail
        let result = exec.execute(
            "inspect_media",
            &json!({"mediaId": "media-vid", "clipId": "nonexistent"}),
        );
        assert!(result.is_err(), "READ-014: nonexistent clipId should fail");
    }

    #[test]
    fn exec_020_inspect_timeline() {
        let mut exec = make_executor();
        let result = exec.execute("inspect_timeline", &json!({})).unwrap();
        assert!(result["content"][0]["text"]
            .as_str()
            .unwrap()
            .contains("fps"));
    }

    #[test]
    fn exec_021_get_transcript_empty_timeline_returns_empty_clips() {
        // v2: no mediaId targeting — an empty timeline yields an empty
        // transcript payload, not an error.
        let mut exec = make_executor();
        let result = exec.execute("get_transcript", &json!({})).unwrap();
        let v: Value = serde_json::from_str(result["content"][0]["text"].as_str().unwrap()).unwrap();
        assert_eq!(v["clips"], json!([]));
        assert_eq!(v["timing"], json!("projectFrames"));
        assert_eq!(v["wordFormat"], json!(["index", "text", "start"]));
    }

    #[test]
    fn exec_021_get_transcript_with_media_id() {
        let mut exec = make_executor_with_media();
        let result = exec
            .execute("get_transcript", &json!({"mediaId": "media-001"}))
            .unwrap();
        assert!(result.get("isError").is_none(), "no error for known media");
        let text = result["content"][0]["text"].as_str().unwrap();
        assert!(!text.is_empty(), "has result text");
    }

    #[test]
    fn exec_021_get_transcript_tolerates_word_timestamps() {
        let mut exec = make_executor();
        // READ-021: legacy wordTimestamps should not cause errors
        let result = exec
            .execute(
                "get_transcript",
                &json!({"wordTimestamps": true, "mediaId": "media-001"}),
            )
            .unwrap();
        // isError should be absent since mediaId is present
        assert!(
            result.get("isError").is_none(),
            "no error when mediaId provided"
        );
        assert!(
            result["content"][0]["text"]
                .as_str()
                .unwrap()
                .contains("clips"),
            "returns formatted transcript JSON"
        );
    }

    // ---- Issue #39: language resolution in get_transcript / inspect_media --

    #[test]
    fn issue_039_get_transcript_v2_accepts_language_without_echo() {
        // v2: language feeds the (host) transcriber, and the output carries
        // transcriptionSource instead of echoing a language field. The #39
        // language-resolution chain is covered by the transcribe_timeline
        // tests (transcribe_timeline_threads_transcription_language).
        let mut exec = make_executor_with_media();
        exec.timeline.transcription_language = Some("ja".to_string());
        let result = exec
            .execute("get_transcript", &json!({"language": "ko"}))
            .unwrap();
        let v: Value = serde_json::from_str(result["content"][0]["text"].as_str().unwrap()).unwrap();
        assert_eq!(v["transcriptionSource"], json!("local"));
        assert!(v.get("language").is_none(), "no language echo in v2: {v}");
    }

    #[test]
    fn issue_039_get_transcript_no_language_no_field() {
        let mut exec = make_executor_with_media();
        // Neither per-call nor project language set
        exec.timeline.transcription_language = None;
        let result = exec
            .execute("get_transcript", &json!({"mediaId": "media-001"}))
            .unwrap();
        let text = result["content"][0]["text"].as_str().unwrap();
        // language field should be omitted when None
        assert!(
            !text.contains("\"language\""),
            "no language field expected: {text}"
        );
    }

    #[test]
    fn issue_039_inspect_media_accepts_language_param() {
        let mut exec = make_executor_with_media();
        // Should not error — language param accepted
        let result = exec
            .execute(
                "inspect_media",
                &json!({"mediaId": "media-001", "language": "de"}),
            )
            .unwrap();
        assert!(
            result.get("isError").is_none(),
            "no error with language param"
        );
    }

    #[test]
    fn exec_022_organize_media_creates_intermediate_folders() {
        let mut exec = make_executor();
        let result = exec
            .execute(
                "organize_media",
                &json!({"createFolders": ["Hero shots/Takes/Best"]}),
            )
            .unwrap();
        assert_eq!(exec.media_manifest.folders.len(), 3);
        let body: Value =
            serde_json::from_str(result["content"][0]["text"].as_str().unwrap()).unwrap();
        assert_eq!(
            body["createdFolders"],
            json!(["Hero shots", "Hero shots/Takes", "Hero shots/Takes/Best"])
        );
        assert!(body.get("moved").is_none(), "only what happened is reported");
        assert!(body.get("deleted").is_none());
    }

    #[test]
    fn exec_023_organize_media_empty_call_rejected() {
        let mut exec = make_executor();
        let err = exec.execute("organize_media", &json!({})).unwrap_err();
        assert!(err.contains("at least one of"), "got: {err}");
    }

    #[test]
    fn organize_media_existing_folders_left_alone_case_insensitively() {
        let mut exec = make_executor_with_media(); // has folder-001 "Footage"
        let name = exec.media_manifest.folders[0].name.clone();
        let result = exec
            .execute(
                "organize_media",
                &json!({"createFolders": [name.to_uppercase()]}),
            )
            .unwrap();
        assert_eq!(exec.media_manifest.folders.len(), 1, "no duplicate created");
        let body: Value =
            serde_json::from_str(result["content"][0]["text"].as_str().unwrap()).unwrap();
        assert!(body.get("createdFolders").is_none(), "nothing created: {body}");
    }

    #[test]
    fn organize_media_ambiguous_folder_path_refused() {
        let mut exec = make_executor();
        for id in ["x", "y"] {
            exec.media_manifest.folders.push(core_model::MediaFolder {
                id: id.into(),
                name: "Take".into(),
                parent_folder_id: None,
            });
        }
        let err = exec
            .execute("organize_media", &json!({"deletes": ["take"]}))
            .unwrap_err();
        assert!(err.contains("Ambiguous"), "got: {err}");
    }

    #[test]
    fn exec_024_organize_media_renames_folder_by_path() {
        let mut exec = make_executor_with_media();
        let path = exec.media_manifest.folders[0].name.clone();
        exec.execute(
            "organize_media",
            &json!({"renames": [{"item": path, "name": "Renamed"}]}),
        )
        .unwrap();
        assert_eq!(exec.media_manifest.folders[0].name, "Renamed");
    }

    #[test]
    fn organize_media_rename_rejects_path_shaped_folder_names() {
        let mut exec = make_executor_with_media();
        let path = exec.media_manifest.folders[0].name.clone();
        let err = exec
            .execute(
                "organize_media",
                &json!({"renames": [{"item": path, "name": "A/B"}]}),
            )
            .unwrap_err();
        assert!(err.contains("never moves"), "got: {err}");
    }

    #[test]
    fn exec_025_organize_media_deletes_folder_cascade() {
        // v2 semantics: deleting a folder deletes its subfolders AND assets;
        // timelines inside move to the root (design C-7) — unlike the old
        // delete_folder, which re-parented contents.
        let mut exec = make_executor_with_media();
        let root_path = exec.media_manifest.folders[0].name.clone();
        exec.media_manifest.folders.push(core_model::MediaFolder {
            id: "mid".into(),
            name: "Mid".into(),
            parent_folder_id: Some("folder-001".into()),
        });
        exec.media_manifest.entries[0].folder_id = Some("mid".into());
        exec.timeline_mut().folder_id = Some("mid".into());

        let result = exec
            .execute("organize_media", &json!({"deletes": [root_path]}))
            .unwrap();
        assert!(exec.media_manifest.folders.is_empty(), "subtree deleted");
        assert!(exec.media_manifest.entries.is_empty(), "contained asset deleted");
        assert_eq!(exec.timeline().folder_id, None, "timeline moved to root");
        let body: Value =
            serde_json::from_str(result["content"][0]["text"].as_str().unwrap()).unwrap();
        assert_eq!(body["deleted"]["folders"], json!(2));
        assert_eq!(body["deleted"]["assets"], json!(1));
        assert!(body["deleted"].get("timelines").is_none());
    }

    #[test]
    fn exec_026_organize_media_renames_asset_by_id() {
        let mut exec = make_executor_with_media();
        exec.execute(
            "organize_media",
            &json!({"renames": [{"item": "media-001", "name": "renamed.mp4"}]}),
        )
        .unwrap();
        assert_eq!(exec.media_manifest.entries[0].name, "renamed.mp4");
    }

    #[test]
    fn exec_027_organize_media_delete_asset_removes_referencing_clips() {
        let mut exec = make_executor_with_media();
        let _ = timeline_core::insert_track_at(exec.timeline_mut(), 0, ClipType::Video);
        let mut clip = crate::test_helpers::make_clip(0, 100);
        clip.media_ref = "media-001".into();
        let _ = timeline_core::place_clips(exec.timeline_mut(), 0, 0, &[clip]);
        // A sibling timeline referencing the asset is swept too.
        let mut sibling = Timeline::default();
        let _ = timeline_core::insert_track_at(&mut sibling, 0, ClipType::Video);
        let mut sib_clip = crate::test_helpers::make_clip(0, 50);
        sib_clip.media_ref = "media-001".into();
        let _ = timeline_core::place_clips(&mut sibling, 0, 0, &[sib_clip]);
        exec.set_sibling_timelines(vec![sibling]);

        let result = exec
            .execute("organize_media", &json!({"deletes": ["media-001"]}))
            .unwrap();
        assert!(exec.media_manifest.entries.is_empty());
        assert!(exec.timeline().tracks[0].clips.is_empty());
        assert!(exec.sibling_timelines()[0].tracks[0].clips.is_empty());
        let body: Value =
            serde_json::from_str(result["content"][0]["text"].as_str().unwrap()).unwrap();
        assert_eq!(body["deleted"]["assets"], json!(1));
        assert_eq!(body["clipsRemoved"], json!(2));
    }

    #[test]
    fn exec_028_organize_media_moves_asset_into_created_path() {
        let mut exec = make_executor_with_media();
        let result = exec
            .execute(
                "organize_media",
                &json!({"moves": [{"items": ["media-001"], "into": "Archive/2026"}]}),
            )
            .unwrap();
        let dest = exec
            .media_manifest
            .entries[0]
            .folder_id
            .clone()
            .expect("moved into a folder");
        assert_eq!(
            crate::organize::folder_path(&exec.media_manifest.folders, &dest),
            "Archive/2026",
            "destination created on demand"
        );
        let body: Value =
            serde_json::from_str(result["content"][0]["text"].as_str().unwrap()).unwrap();
        assert_eq!(body["moved"], json!(1));
        assert_eq!(body["createdFolders"], json!(["Archive", "Archive/2026"]));
    }

    #[test]
    fn exec_029_organize_media_move_without_into_goes_to_root() {
        let mut exec = make_executor_with_media();
        exec.media_manifest.entries[0].folder_id = Some("folder-001".into());
        exec.execute("organize_media", &json!({"moves": [{"items": ["media-001"]}]}))
            .unwrap();
        assert_eq!(exec.media_manifest.entries[0].folder_id, None);
    }

    #[test]
    fn organize_media_folder_cycle_refused() {
        let mut exec = make_executor();
        exec.execute("organize_media", &json!({"createFolders": ["A/B"]}))
            .unwrap();
        let err = exec
            .execute(
                "organize_media",
                &json!({"moves": [{"items": ["A"], "into": "A/B"}]}),
            )
            .unwrap_err();
        assert!(err.contains("into itself"), "got: {err}");
        let err = exec
            .execute(
                "organize_media",
                &json!({"moves": [{"items": ["A"], "into": "a"}]}),
            )
            .unwrap_err();
        assert!(err.contains("into itself"), "cycle check is case-insensitive: {err}");
    }

    #[test]
    fn organize_media_items_resolve_before_mutations_apply() {
        // Renames apply before deletes, but the delete's item reference was
        // resolved against the PRE-CALL library, so the old path still works.
        let mut exec = make_executor();
        exec.execute("organize_media", &json!({"createFolders": ["Old"]}))
            .unwrap();
        let result = exec
            .execute(
                "organize_media",
                &json!({
                    "renames": [{"item": "Old", "name": "New"}],
                    "deletes": ["Old"],
                }),
            )
            .unwrap();
        assert!(exec.media_manifest.folders.is_empty(), "renamed folder deleted");
        let body: Value =
            serde_json::from_str(result["content"][0]["text"].as_str().unwrap()).unwrap();
        assert_eq!(body["renamed"], json!(1));
        assert_eq!(body["deleted"]["folders"], json!(1));
    }

    #[test]
    fn organize_media_into_may_name_a_folder_created_this_call() {
        let mut exec = make_executor_with_media();
        exec.execute(
            "organize_media",
            &json!({
                "createFolders": ["Fresh"],
                "moves": [{"items": ["media-001"], "into": "Fresh"}],
            }),
        )
        .unwrap();
        let dest = exec.media_manifest.entries[0].folder_id.clone().unwrap();
        assert_eq!(
            crate::organize::folder_path(&exec.media_manifest.folders, &dest),
            "Fresh"
        );
    }

    #[test]
    fn organize_media_unknown_item_refused() {
        let mut exec = make_executor();
        let err = exec
            .execute("organize_media", &json!({"deletes": ["no-such-thing"]}))
            .unwrap_err();
        assert!(
            err.contains("not a media asset id, a timeline id, or an existing folder path"),
            "got: {err}"
        );
    }

    #[test]
    fn exec_030_import_media_source_path_file() {
        let mut exec = make_executor();
        let result = exec
            .execute(
                "import_media",
                &json!({"source": {"path": "/path/to/new.mp4"}, "name": "new.mp4"}),
            )
            .unwrap();
        assert_eq!(exec.media_manifest.entries.len(), 1);
        assert_eq!(exec.media_manifest.entries[0].r#type, ClipType::Video);
        let body: Value =
            serde_json::from_str(result["content"][0]["text"].as_str().unwrap()).unwrap();
        assert_eq!(body["status"], json!("ready"));
        let short = body["mediaRef"].as_str().unwrap();
        assert!(
            exec.media_manifest.entries[0].id.starts_with(short) && short.len() >= 8,
            "mediaRef is the asset id's short prefix (C-3): {body}"
        );
    }

    #[test]
    fn import_media_infers_name_and_type_from_path() {
        let mut exec = make_executor();
        exec.execute("import_media", &json!({"source": {"path": "C:/media/photo.png"}}))
            .unwrap();
        let e = &exec.media_manifest.entries[0];
        assert_eq!(e.name, "photo.png", "name defaults to the filename");
        assert_eq!(e.r#type, ClipType::Image);
        assert!((e.duration - 5.0).abs() < 1e-9, "stills get the 5s default");
    }

    #[test]
    fn import_media_rejects_unsupported_extension_and_shape() {
        let mut exec = make_executor();
        let err = exec
            .execute("import_media", &json!({"source": {"path": "/notes.txt"}}))
            .unwrap_err();
        assert!(err.contains("unsupported file type"), "got: {err}");
        let err = exec
            .execute("import_media", &json!({"path": "/a.mp4"}))
            .unwrap_err();
        assert!(err.contains("source"), "legacy bare-path shape refused: {err}");
        let err = exec
            .execute(
                "import_media",
                &json!({"source": {"path": "/a.mp4", "url": "https://x/y.mp4"}}),
            )
            .unwrap_err();
        assert!(err.contains("exactly one"), "got: {err}");
    }

    #[test]
    fn import_media_url_and_bytes_report_honest_unavailability() {
        let mut exec = make_executor();
        let err = exec
            .execute("import_media", &json!({"source": {"url": "https://x/y.mp4"}}))
            .unwrap_err();
        assert!(err.contains("download"), "got: {err}");
        let err = exec
            .execute(
                "import_media",
                &json!({"source": {"bytes": "aGk=", "mimeType": "image/png"}}),
            )
            .unwrap_err();
        assert!(err.contains("bytes imports"), "got: {err}");
    }

    #[test]
    fn import_media_folder_param_creates_destination_path() {
        let mut exec = make_executor();
        exec.execute(
            "import_media",
            &json!({"source": {"path": "/b.mp4"}, "folder": "B-roll/Sunset"}),
        )
        .unwrap();
        let dest = exec.media_manifest.entries[0].folder_id.clone().unwrap();
        assert_eq!(
            crate::organize::folder_path(&exec.media_manifest.folders, &dest),
            "B-roll/Sunset"
        );
    }

    #[test]
    fn exec_031_import_media_directory_mirrors_subfolders() {
        // source.path may be a directory (absorbs import_folder): supported
        // files register recursively and the tree mirrors as media folders.
        let dir = std::env::temp_dir()
            .join("fronda-import-dir-test")
            .join(format!("run-{}", Uuid::new_v4()));
        std::fs::create_dir_all(dir.join("Sub")).unwrap();
        std::fs::write(dir.join("clip.mp4"), b"x").unwrap();
        std::fs::write(dir.join("notes.txt"), b"x").unwrap();
        std::fs::write(dir.join("Sub").join("photo.png"), b"x").unwrap();

        let mut exec = make_executor();
        let result = exec
            .execute(
                "import_media",
                &json!({"source": {"path": dir.to_string_lossy()}}),
            )
            .unwrap();
        let body: Value =
            serde_json::from_str(result["content"][0]["text"].as_str().unwrap()).unwrap();
        assert_eq!(body["status"], json!("ready"));
        assert_eq!(body["imported"], json!(2));
        assert_eq!(body["skipped"], json!(1), "unsupported .txt skipped");
        assert_eq!(body["foldersCreated"], json!(2), "top dir + Sub");
        assert_eq!(exec.media_manifest.entries.len(), 2);
        let photo = exec
            .media_manifest
            .entries
            .iter()
            .find(|e| e.name == "photo.png")
            .unwrap();
        let photo_folder = photo.folder_id.clone().unwrap();
        assert!(
            crate::organize::folder_path(&exec.media_manifest.folders, &photo_folder)
                .ends_with("/Sub"),
            "subfolder mirrored"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn exec_032_duplicate_project() {
        // Duplication is host filesystem I/O the pure executor can't perform, so it
        // must report an error honestly (not claim a no-op succeeded).
        let mut exec = make_executor_with_media();
        let result = exec.execute("duplicate_project", &json!({})).unwrap();
        assert_eq!(result["isError"], true);
        let text = result["content"][0]["text"].as_str().unwrap();
        assert!(text.contains("requires host filesystem support"), "got: {text}");
    }

    #[test]
    fn exec_033_add_texts() {
        let mut exec = make_executor();
        let _ = timeline_core::insert_track_at(exec.timeline_mut(), 0, ClipType::Video);
        let result = exec
            .execute("add_texts", &json!({"texts": [{"text": "Hello"}]}))
            .unwrap();
        let env: serde_json::Value =
            serde_json::from_str(result["content"][0]["text"].as_str().unwrap()).unwrap();
        assert_eq!(env["clips"][0]["content"], json!("Hello"));
        assert_eq!(env["clips"][0]["mediaType"], json!("text"));
        assert_eq!(exec.timeline.tracks[0].clips.len(), 1);
    }

    #[test]
    fn exec_034_add_texts_missing_texts() {
        let mut exec = make_executor();
        let err = exec.execute("add_texts", &json!({})).unwrap_err();
        // wire-mutation-validators: MUT-019 gate message (also rejects []).
        assert!(err.contains("missing or empty 'texts'"), "got: {err}");
    }

    #[test]
    fn add_texts_honors_start_frame_and_reports_real_ids() {
        let mut exec = make_executor();
        let _ = timeline_core::insert_track_at(exec.timeline_mut(), 0, ClipType::Video);
        let head = crate::test_helpers::make_clip(0, 90);
        let placed = timeline_core::place_clips(exec.timeline_mut(), 0, 0, &[head]);
        let head_id = placed.first().expect("head placed").clone();

        let result = exec
            .execute(
                "add_texts",
                &json!({"texts": [{"content": "T", "startFrame": 120, "durationFrames": 60}]}),
            )
            .unwrap();

        let track = &exec.timeline().tracks[0];
        let text_clip = track
            .clips
            .iter()
            .find(|c| c.media_type == ClipType::Text)
            .expect("text clip placed");
        assert_eq!(text_clip.start_frame, 120, "text must land at startFrame");
        assert_eq!(text_clip.duration_frames, 60);

        let head = track
            .clips
            .iter()
            .find(|c| c.id == head_id)
            .expect("head clip must not be clobbered");
        assert_eq!((head.start_frame, head.duration_frames), (0, 90));

        // The C-4 envelope lists the created clip in get_timeline shape.
        let env: serde_json::Value =
            serde_json::from_str(result["content"][0]["text"].as_str().unwrap()).unwrap();
        let row = env["clips"]
            .as_array()
            .unwrap()
            .iter()
            .find(|c| text_clip.id.starts_with(c["id"].as_str().unwrap()))
            .expect("envelope lists the placed clip");
        assert_eq!(row["frames"], json!([120, 180]));
        assert_eq!(row["track"], json!(0));
    }

    #[test]
    fn add_texts_default_appends_after_existing_content() {
        let mut exec = make_executor();
        let _ = timeline_core::insert_track_at(exec.timeline_mut(), 0, ClipType::Video);
        let head = crate::test_helpers::make_clip(0, 90);
        let _ = timeline_core::place_clips(exec.timeline_mut(), 0, 0, &[head]);

        exec.execute("add_texts", &json!({"texts": [{"content": "T"}]}))
            .unwrap();

        let track = &exec.timeline().tracks[0];
        assert_eq!(track.clips.len(), 2, "head clip untouched");
        let text_clip = track
            .clips
            .iter()
            .find(|c| c.media_type == ClipType::Text)
            .unwrap();
        assert_eq!(text_clip.start_frame, 90, "default start appends after content");
    }

    #[test]
    fn add_texts_reports_ids_in_entry_order_for_unordered_starts() {
        let mut exec = make_executor();
        let _ = timeline_core::insert_track_at(exec.timeline_mut(), 0, ClipType::Video);
        let result = exec
            .execute(
                "add_texts",
                &json!({"texts": [
                    {"content": "B", "startFrame": 300, "durationFrames": 30},
                    {"content": "A", "startFrame": 0, "durationFrames": 30}
                ]}),
            )
            .unwrap();

        let track = &exec.timeline().tracks[0];
        let b = track
            .clips
            .iter()
            .find(|c| c.text_content.as_deref() == Some("B"))
            .unwrap();
        let a = track
            .clips
            .iter()
            .find(|c| c.text_content.as_deref() == Some("A"))
            .unwrap();
        assert_eq!(b.start_frame, 300);
        assert_eq!(a.start_frame, 0);

        // The C-4 envelope lists changed clips sorted by (track, startFrame).
        let env: serde_json::Value =
            serde_json::from_str(result["content"][0]["text"].as_str().unwrap()).unwrap();
        let ids: Vec<&str> = env["clips"]
            .as_array()
            .unwrap()
            .iter()
            .map(|c| c["id"].as_str().unwrap())
            .collect();
        assert_eq!(ids.len(), 2);
        assert!(a.id.starts_with(ids[0]), "first row is A (short id)");
        assert!(b.id.starts_with(ids[1]), "second row is B (short id)");
    }

    #[test]
    fn add_texts_rejects_negative_start_frame() {
        let mut exec = make_executor();
        let _ = timeline_core::insert_track_at(exec.timeline_mut(), 0, ClipType::Video);
        let err = exec
            .execute(
                "add_texts",
                &json!({"texts": [{"content": "x", "startFrame": -5, "durationFrames": 30}]}),
            )
            .unwrap_err();
        assert!(err.contains("startFrame must be >= 0"), "got: {err}");
    }

    #[test]
    fn add_texts_rejects_non_positive_duration() {
        let mut exec = make_executor();
        let _ = timeline_core::insert_track_at(exec.timeline_mut(), 0, ClipType::Video);
        let err = exec
            .execute(
                "add_texts",
                &json!({"texts": [{"content": "x", "startFrame": 0, "durationFrames": 0}]}),
            )
            .unwrap_err();
        assert!(err.contains("durationFrames must be >= 1"), "got: {err}");
    }

    #[test]
    fn add_texts_applies_per_entry_style_and_transform() {
        let mut exec = make_executor();
        let _ = timeline_core::insert_track_at(exec.timeline_mut(), 0, ClipType::Video);
        exec.execute(
            "add_texts",
            &json!({"texts": [{
                "content": "Title",
                "fontName": "Anton",
                "fontSize": 72.0,
                "fontWeight": 700.0,
                "color": "#00FF00",
                "alignment": "left",
                "transform": {"centerX": 0.5, "centerY": 0.9}
            }]}),
        )
        .unwrap();
        let clip = &exec.timeline.tracks[0].clips[0];
        assert_eq!(clip.text_content.as_deref(), Some("Title"));
        let ts = clip.text_style.as_ref().unwrap();
        assert_eq!(ts.font_name, "Anton");
        assert_eq!(ts.font_size, 72.0);
        assert_eq!(ts.font_weight, 700.0);
        assert_eq!((ts.color.r, ts.color.g, ts.color.b), (0.0, 1.0, 0.0));
        assert_eq!(ts.alignment, core_model::TextAlignment::Left);
        // Centre-only transform repositions; the y matches the request.
        assert!((clip.transform.center_y - 0.9).abs() < 1e-9);
    }

    #[test]
    fn add_texts_rejects_bad_color() {
        let mut exec = make_executor();
        let _ = timeline_core::insert_track_at(exec.timeline_mut(), 0, ClipType::Video);
        let err = exec
            .execute("add_texts", &json!({"texts": [{"content": "x", "color": "zzz"}]}))
            .unwrap_err();
        assert!(err.contains("invalid color"), "got: {err}");
    }

    #[test]
    fn add_texts_applies_animation_and_highlight() {
        let mut exec = make_executor();
        let _ = timeline_core::insert_track_at(exec.timeline_mut(), 0, ClipType::Video);
        exec.execute(
            "add_texts",
            &json!({"texts": [{
                "content": "Go",
                "animation": "wordReveal",
                "highlightColor": "#FF8800"
            }]}),
        )
        .unwrap();
        let anim = exec.timeline.tracks[0].clips[0]
            .text_animation
            .as_ref()
            .expect("animation set");
        assert_eq!(anim.preset, core_model::TextAnimationPreset::WordReveal);
        let hl = anim.highlight.as_ref().expect("highlight parsed");
        assert!((hl.r - 1.0).abs() < 1e-6 && hl.g > 0.5 && hl.b == 0.0);
    }

    #[test]
    fn add_texts_animation_off_leaves_none() {
        let mut exec = make_executor();
        let _ = timeline_core::insert_track_at(exec.timeline_mut(), 0, ClipType::Video);
        exec.execute("add_texts", &json!({"texts": [{"content": "x", "animation": "off"}]}))
            .unwrap();
        assert!(exec.timeline.tracks[0].clips[0].text_animation.is_none());
    }

    #[test]
    fn add_texts_rejects_bad_animation() {
        let mut exec = make_executor();
        let _ = timeline_core::insert_track_at(exec.timeline_mut(), 0, ClipType::Video);
        let err = exec
            .execute("add_texts", &json!({"texts": [{"content": "x", "animation": "bogus"}]}))
            .unwrap_err();
        assert!(err.contains("invalid animation"), "got: {err}");
    }

    #[test]
    fn exec_035_add_shapes() {
        let mut exec = make_executor();
        let _ = timeline_core::insert_track_at(exec.timeline_mut(), 0, ClipType::Video);
        let result = exec
            .execute("add_shapes", &json!({"entries": [{"type": "rect"}]}))
            .unwrap();
        let text = result["content"][0]["text"].as_str().unwrap();
        assert!(text.contains("shape clip"));
    }

    #[test]
    fn add_shapes_applies_style_fill_and_transform() {
        let mut exec = make_executor();
        let _ = timeline_core::insert_track_at(exec.timeline_mut(), 0, ClipType::Video);
        exec.execute(
            "add_shapes",
            &json!({"entries": [{
                "type": "oval",
                "style": {"color": "#FF0000", "width": 5.0, "dashed": true},
                "fill": {"enabled": true, "color": "#0000FF"},
                "transform": {"centerX": 0.25, "centerY": 0.75, "width": 0.4, "height": 0.3}
            }]}),
        )
        .unwrap();
        let clip = &exec.timeline.tracks[0].clips[0];
        let ss = clip.shape_style.as_ref().unwrap();
        assert_eq!(ss.kind, core_model::ShapeKind::Oval);
        assert_eq!((ss.stroke.color.r, ss.stroke.color.g, ss.stroke.color.b), (1.0, 0.0, 0.0));
        assert_eq!(ss.stroke.width, 5.0);
        assert!(ss.stroke.dashed);
        assert!(ss.fill.enabled);
        assert_eq!((ss.fill.color.r, ss.fill.color.g, ss.fill.color.b), (0.0, 0.0, 1.0));
        assert!((clip.transform.center_x - 0.25).abs() < 1e-9);
        assert!((clip.transform.width - 0.4).abs() < 1e-9);
    }

    #[test]
    fn add_shapes_rejects_bad_fill_color() {
        let mut exec = make_executor();
        let _ = timeline_core::insert_track_at(exec.timeline_mut(), 0, ClipType::Video);
        let err = exec
            .execute(
                "add_shapes",
                &json!({"entries": [{"type": "rect", "fill": {"color": "zzz"}}]}),
            )
            .unwrap_err();
        assert!(err.contains("invalid fill color"), "got: {err}");
    }

    #[test]
    fn exec_036_add_shapes_empty_entries() {
        let mut exec = make_executor();
        let err = exec
            .execute("add_shapes", &json!({"entries": []}))
            .unwrap_err();
        assert!(err.contains("non-empty"));
    }

    #[test]
    fn add_shapes_honors_start_frame_on_video_track_with_real_ids() {
        let mut exec = make_executor();
        // Text track above the video track: the video track is NOT index 0.
        let _ = timeline_core::insert_track_at(exec.timeline_mut(), 0, ClipType::Text);
        let _ = timeline_core::insert_track_at(exec.timeline_mut(), 1, ClipType::Video);
        assert_eq!(exec.timeline().tracks[1].r#type, ClipType::Video);
        let head = crate::test_helpers::make_clip(0, 90);
        let placed = timeline_core::place_clips(exec.timeline_mut(), 1, 0, &[head]);
        let head_id = placed.first().expect("head placed").clone();

        let result = exec
            .execute(
                "add_shapes",
                &json!({"entries": [{"type": "rect", "startFrame": 120, "durationFrames": 60}]}),
            )
            .unwrap();

        assert!(
            exec.timeline().tracks[0].clips.is_empty(),
            "text track (index 0) must stay untouched"
        );
        let track = &exec.timeline().tracks[1];
        let shape = track
            .clips
            .iter()
            .find(|c| c.media_type == ClipType::Shape)
            .expect("shape placed on the video track");
        assert_eq!(shape.start_frame, 120, "shape must land at startFrame");
        let head = track
            .clips
            .iter()
            .find(|c| c.id == head_id)
            .expect("head clip must not be clobbered");
        assert_eq!((head.start_frame, head.duration_frames), (0, 90));

        let text = result["content"][0]["text"].as_str().unwrap();
        let reported_short = text.split('"').nth(1).expect("response lists the created id");
        assert!(
            shape.id.starts_with(reported_short),
            "reported id must be the placed clip's short id (C-3): {text}"
        );
        // The short prefix must resolve back to a timeline clip (C-3 round trip).
        assert!(
            exec.timeline()
                .tracks
                .iter()
                .flat_map(|t| &t.clips)
                .any(|c| c.id.starts_with(reported_short)),
            "reported short id must resolve on the timeline"
        );
    }

    #[test]
    fn add_shapes_rejects_overflowing_start_frame() {
        let mut exec = make_executor();
        let _ = timeline_core::insert_track_at(exec.timeline_mut(), 0, ClipType::Video);
        let err = exec
            .execute(
                "add_shapes",
                &json!({"entries": [{"type": "rect", "startFrame": i64::MAX, "durationFrames": 10}]}),
            )
            .unwrap_err();
        assert!(err.contains("exceeds the maximum supported frame"), "err={err}");
    }

    #[test]
    fn add_shapes_rejects_negative_start_frame() {
        let mut exec = make_executor();
        let _ = timeline_core::insert_track_at(exec.timeline_mut(), 0, ClipType::Video);
        let err = exec
            .execute(
                "add_shapes",
                &json!({"entries": [{"type": "rect", "startFrame": -1, "durationFrames": 10}]}),
            )
            .unwrap_err();
        assert!(err.contains("startFrame must be >= 0"), "got: {err}");
    }

    #[test]
    fn exec_037_apply_color() {
        let mut exec = make_executor();
        let _ = timeline_core::insert_track_at(exec.timeline_mut(), 0, ClipType::Video);
        let clip = crate::test_helpers::make_clip(0, 150);
        let placed = timeline_core::place_clips(exec.timeline_mut(), 0, 0, &[clip]);
        let clip_id = placed.first().expect("place_clips returned empty");
        let result = exec
            .execute("apply_color", &json!({"clipId": clip_id, "exposure": 0.5}))
            .unwrap();
        assert!(result["content"][0]["text"]
            .as_str()
            .unwrap()
            .contains("color"));
    }

    #[test]
    fn exec_038_apply_color_missing_clip() {
        let mut exec = make_executor();
        let err = exec
            .execute("apply_color", &json!({"clipId": "nonexistent"}))
            .unwrap_err();
        assert!(err.contains("not found"));
    }

    #[test]
    fn exec_039_apply_effect() {
        let mut exec = make_executor();
        let _ = timeline_core::insert_track_at(exec.timeline_mut(), 0, ClipType::Video);
        let clip = crate::test_helpers::make_clip(0, 150);
        let placed = timeline_core::place_clips(exec.timeline_mut(), 0, 0, &[clip]);
        let clip_id = placed.first().expect("place_clips returned empty");
        let result = exec
            .execute(
                "apply_effect",
                &json!({"clipId": clip_id, "effectType": "blur"}),
            )
            .unwrap();
        let env: serde_json::Value =
            serde_json::from_str(result["content"][0]["text"].as_str().unwrap()).unwrap();
        assert_eq!(env["clips"][0]["effects"][0]["type"], json!("blur"));
    }

    #[test]
    fn exec_040_set_chroma_key() {
        let mut exec = make_executor();
        let _ = timeline_core::insert_track_at(exec.timeline_mut(), 0, ClipType::Video);
        let clip = crate::test_helpers::make_clip(0, 150);
        let placed = timeline_core::place_clips(exec.timeline_mut(), 0, 0, &[clip]);
        let clip_id = placed.first().expect("place_clips returned empty");
        // v2: chroma keying is apply_effect's key.chroma (absorbs set_chroma_key).
        let result = exec
            .execute(
                "apply_effect",
                &json!({"clipIds": [clip_id], "effects": [
                    {"type": "key.chroma", "params": {"keyHue": 0.333, "tolerance": 0.4}}
                ]}),
            )
            .unwrap();
        let env: Value = serde_json::from_str(result["content"][0]["text"].as_str().unwrap()).unwrap();
        assert_eq!(env["clips"][0]["effects"][0]["type"], json!("key.chroma"));
        // The renderer's chroma key mirrors the effect (green at hue 1/3).
        let clip = &exec.timeline().tracks[0].clips[0];
        let ck = clip.chroma_key.as_ref().expect("chroma key mirrored");
        assert!(ck.key_g > 0.9 && ck.key_r < 0.1, "keyHue 0.333 = green");
        assert!((ck.tolerance - 0.4).abs() < 1e-9);
    }

    #[test]
    fn exec_041_set_blend_mode() {
        let mut exec = make_executor();
        let _ = timeline_core::insert_track_at(exec.timeline_mut(), 0, ClipType::Video);
        let clip = crate::test_helpers::make_clip(0, 150);
        let placed = timeline_core::place_clips(exec.timeline_mut(), 0, 0, &[clip]);
        let clip_id = placed.first().expect("place_clips returned empty");
        // v2: blend modes are set_clip_properties.blendMode (absorbs set_blend_mode).
        let result = exec
            .execute(
                "set_clip_properties",
                &json!({"clipIds": [clip_id], "blendMode": "multiply"}),
            )
            .unwrap();
        let env: Value = serde_json::from_str(result["content"][0]["text"].as_str().unwrap()).unwrap();
        assert_eq!(env["clips"][0]["blendMode"], json!("multiply"));
        assert_eq!(
            exec.timeline().tracks[0].clips[0].blend_mode,
            core_model::BlendMode::Multiply
        );
        // 'normal' clears the blend.
        exec.execute(
            "set_clip_properties",
            &json!({"clipIds": [clip_id], "blendMode": "normal"}),
        )
        .unwrap();
        assert_eq!(
            exec.timeline().tracks[0].clips[0].blend_mode,
            core_model::BlendMode::Normal
        );
    }

    #[test]
    fn exec_042_set_color_grade() {
        let mut exec = make_executor();
        let _ = timeline_core::insert_track_at(exec.timeline_mut(), 0, ClipType::Video);
        let clip = crate::test_helpers::make_clip(0, 150);
        let placed = timeline_core::place_clips(exec.timeline_mut(), 0, 0, &[clip]);
        let clip_id = placed.first().expect("place_clips returned empty");
        // v2: grading is apply_color (absorbs set_color_grade); the envelope's
        // clip row echoes the resulting grade as the `color` object.
        let result = exec
            .execute(
                "apply_color",
                &json!({"clipIds": [clip_id], "saturation": 1.2}),
            )
            .unwrap();
        let env: Value = serde_json::from_str(result["content"][0]["text"].as_str().unwrap()).unwrap();
        assert_eq!(env["clips"][0]["color"], json!({"saturation": 1.2}));
        // MERGE semantics: a second knob keeps the first.
        exec.execute("apply_color", &json!({"clipIds": [clip_id], "exposure": 0.5}))
            .unwrap();
        let gt = exec.execute("get_timeline", &json!({})).unwrap();
        let tv: Value = serde_json::from_str(gt["content"][0]["text"].as_str().unwrap()).unwrap();
        assert_eq!(
            tv["tracks"][0]["clips"][0]["color"],
            json!({"saturation": 1.2, "exposure": 0.5})
        );
        // reset:true starts from neutral.
        exec.execute(
            "apply_color",
            &json!({"clipIds": [clip_id], "reset": true, "contrast": 1.1}),
        )
        .unwrap();
        let gt2 = exec.execute("get_timeline", &json!({})).unwrap();
        let tv2: Value = serde_json::from_str(gt2["content"][0]["text"].as_str().unwrap()).unwrap();
        assert_eq!(tv2["tracks"][0]["clips"][0]["color"], json!({"contrast": 1.1}));
    }

    #[test]
    fn exec_043_inspect_color() {
        let mut exec = make_executor();
        let _ = timeline_core::insert_track_at(exec.timeline_mut(), 0, ClipType::Video);
        let clip = crate::test_helpers::make_clip(0, 150);
        let placed = timeline_core::place_clips(exec.timeline_mut(), 0, 0, &[clip]);
        let clip_id = placed.first().expect("place_clips returned empty");
        let result = exec
            .execute("inspect_color", &json!({"clipId": clip_id}))
            .unwrap();
        assert!(result["content"][0]["text"]
            .as_str()
            .unwrap()
            .contains("Color info"));
    }

    #[test]
    fn exec_044_inspect_color_no_args() {
        let mut exec = make_executor();
        let err = exec.execute("inspect_color", &json!({})).unwrap_err();
        assert!(err.contains("clipId or mediaRef"));
    }

    #[test]
    fn exec_045_add_captions_stub() {
        let mut exec = make_executor();
        let _ = timeline_core::insert_track_at(exec.timeline_mut(), 0, ClipType::Video);
        let clip = crate::test_helpers::make_clip(0, 150);
        let placed = timeline_core::place_clips(exec.timeline_mut(), 0, 0, &[clip]);
        let clip_id = placed.first().expect("place_clips returned empty");
        let _ = clip_id;
        let err = exec.execute("add_captions", &json!({})).unwrap_err();
        assert!(err.contains("unavailable"), "honest host-gap error: {err}");
    }

    #[test]
    fn exec_046_add_captions_v2_takes_no_targeting() {
        // v2: add_captions finds spoken content itself; clipIds is not part of
        // the schema and its absence must not be the reported failure.
        let mut exec = make_executor();
        let err = exec.execute("add_captions", &json!({})).unwrap_err();
        assert!(err.contains("unavailable"), "got: {err}");
    }

    #[test]
    fn exec_047_apply_animation() {
        let mut exec = make_executor();
        let _ = timeline_core::insert_track_at(exec.timeline_mut(), 0, ClipType::Video);
        let clip = crate::test_helpers::make_clip(0, 150);
        let placed = timeline_core::place_clips(exec.timeline_mut(), 0, 0, &[clip]);
        let clip_id = placed.first().expect("place_clips returned empty");
        let result = exec
            .execute(
                "apply_animation",
                &json!({"clipId": clip_id, "preset": "fadeIn"}),
            )
            .unwrap();
        assert!(result["content"][0]["text"]
            .as_str()
            .unwrap()
            .contains("animation"));
    }

    #[test]
    fn exec_048_generate_video() {
        let mut exec = make_executor();
        let result = exec
            .execute("generate_video", &json!({"prompt": "A cat walking"}))
            .unwrap();
        assert_eq!(result["isError"], true);
        // Unavailable submits are side-effect-free (review F4).
        assert!(exec.media_manifest.entries.is_empty());
    }

    #[test]
    fn exec_049_generate_video_missing_prompt() {
        let mut exec = make_executor();
        let err = exec.execute("generate_video", &json!({})).unwrap_err();
        assert!(err.contains("Missing prompt"));
    }

    #[test]
    fn exec_050_generate_image() {
        let mut exec = make_executor();
        let result = exec
            .execute("generate_image", &json!({"prompt": "A sunset"}))
            .unwrap();
        assert_eq!(result["isError"], true);
    }

    #[test]
    fn exec_051_generate_audio() {
        let mut exec = make_executor();
        let err = exec
            .execute("generate_audio", &json!({"prompt": "Narration"}))
            .unwrap_err();
        assert!(err.contains("unavailable"), "honest backend gap: {err}");
    }

    #[test]
    fn exec_052_generate_audio_requires_prompt_without_video_source() {
        let mut exec = make_executor();
        let err = exec.execute("generate_audio", &json!({})).unwrap_err();
        assert!(err.contains("prompt"), "got: {err}");
    }

    #[test]
    fn exec_053_upscale_media() {
        let mut exec = make_executor_with_media();
        let result = exec
            .execute("upscale_media", &json!({"mediaId": "media-001"}))
            .unwrap();
        assert_eq!(result["isError"], true);
    }

    #[test]
    fn exec_054_upscale_media_not_found() {
        let mut exec = make_executor_with_media();
        let err = exec
            .execute("upscale_media", &json!({"mediaId": "nonexistent"}))
            .unwrap_err();
        assert!(err.contains("not found"));
    }

    #[test]
    fn exec_055_set_keyframes_missing_clip() {
        let mut exec = make_executor();
        let err = exec
            .execute("set_keyframes", &json!({"clipId": "nonexistent", "property": "opacity", "keyframes": [[0, 1.0]]}))
            .unwrap_err();
        assert!(err.contains("not found"));
    }

    fn executor_with_clip() -> ToolExecutor {
        let mut timeline = Timeline::default();
        timeline.tracks.push(core_model::Track {
            id: "t".into(),
            r#type: core_model::ClipType::Video,
            muted: false,
            hidden: false,
            sync_locked: false,
           display_height: 50.0,
            clips: vec![core_model::Clip {
                id: "c".into(),
                media_ref: "m".into(),
                media_type: core_model::ClipType::Video,
                source_clip_type: core_model::ClipType::Video,
                start_frame: 0,
                duration_frames: 100,
                trim_start_frame: 0,
                trim_end_frame: 0,
                speed: 1.0,
                volume: 1.0,
                fade_in_frames: 0,
                fade_out_frames: 0,
                fade_in_interpolation: core_model::Interpolation::Linear,
                fade_out_interpolation: core_model::Interpolation::Linear,
                opacity: 1.0,
                transform: core_model::Transform::default(),
                crop: core_model::Crop::default(),
                link_group_id: None,
                caption_group_id: None,
                text_content: None,
                text_style: None,
                opacity_track: None,
                position_track: None,
                scale_track: None,
                rotation_track: None,
                crop_track: None,
                volume_track: None,
                effects: None,
                shape_style: None,
                stroke_progress_track: None,
                compound_timeline_id: None,
                blend_mode: Default::default(),
                chroma_key: None,
                multicam_group_id: None,
                text_animation: None,
                word_timings: None,
            }],
        });
        ToolExecutor::new(timeline, MediaManifest::default())
    }

    fn only_clip(exec: &ToolExecutor) -> &core_model::Clip {
        &exec.timeline().tracks[0].clips[0]
    }

    #[test]
    fn set_keyframes_scalar_opacity_with_interp_default() {
        let mut exec = executor_with_clip();
        exec.execute(
            "set_keyframes",
            &json!({"clipId": "c", "property": "opacity", "keyframes": [[0, 0.0, "linear"], [100, 1.0]]}),
        )
        .unwrap();
        let kfs = &only_clip(&exec).opacity_track.as_ref().unwrap().keyframes;
        assert_eq!(kfs.len(), 2);
        assert_eq!(kfs[0].frame, 0);
        assert_eq!(kfs[0].value, 0.0);
        assert_eq!(kfs[0].interpolation_out, core_model::Interpolation::Linear);
        // Missing interp defaults to smooth.
        assert_eq!(kfs[1].interpolation_out, core_model::Interpolation::Smooth);
    }

    #[test]
    fn set_keyframes_position_stores_top_left_pair() {
        let mut exec = executor_with_clip();
        exec.execute(
            "set_keyframes",
            &json!({"clipId": "c", "property": "position", "keyframes": [[0, 0.1, 0.2], [50, 0.3, 0.4]]}),
        )
        .unwrap();
        let kfs = &only_clip(&exec).position_track.as_ref().unwrap().keyframes;
        assert_eq!(kfs.len(), 2);
        assert_eq!(kfs[0].value, AnimPair { a: 0.1, b: 0.2 });
        assert_eq!(kfs[1].value, AnimPair { a: 0.3, b: 0.4 });
    }

    #[test]
    fn set_keyframes_scale_stores_pair() {
        let mut exec = executor_with_clip();
        exec.execute(
            "set_keyframes",
            &json!({"clipId": "c", "property": "scale", "keyframes": [[0, 0.5, 0.25]]}),
        )
        .unwrap();
        let kfs = &only_clip(&exec).scale_track.as_ref().unwrap().keyframes;
        assert_eq!(kfs[0].value, AnimPair { a: 0.5, b: 0.25 });
    }

    #[test]
    fn set_keyframes_crop_maps_top_right_bottom_left() {
        let mut exec = executor_with_clip();
        // Input order [top, right, bottom, left].
        exec.execute(
            "set_keyframes",
            &json!({"clipId": "c", "property": "crop", "keyframes": [[0, 0.1, 0.2, 0.3, 0.4]]}),
        )
        .unwrap();
        let c = only_clip(&exec).crop_track.as_ref().unwrap().keyframes[0].value;
        assert_eq!(c.top, 0.1);
        assert_eq!(c.right, 0.2);
        assert_eq!(c.bottom, 0.3);
        assert_eq!(c.left, 0.4);
    }

    #[test]
    fn set_keyframes_empty_array_clears_track() {
        let mut exec = executor_with_clip();
        exec.execute(
            "set_keyframes",
            &json!({"clipId": "c", "property": "opacity", "keyframes": [[0, 0.5]]}),
        )
        .unwrap();
        assert!(only_clip(&exec).opacity_track.is_some());
        exec.execute(
            "set_keyframes",
            &json!({"clipId": "c", "property": "opacity", "keyframes": []}),
        )
        .unwrap();
        assert!(only_clip(&exec).opacity_track.is_none(), "empty clears");
    }

    #[test]
    fn set_keyframes_sorts_and_dedupes_last_wins() {
        let mut exec = executor_with_clip();
        exec.execute(
            "set_keyframes",
            &json!({"clipId": "c", "property": "opacity", "keyframes": [[50, 0.5], [0, 0.1], [0, 0.9]]}),
        )
        .unwrap();
        let kfs = &only_clip(&exec).opacity_track.as_ref().unwrap().keyframes;
        assert_eq!(kfs.len(), 2, "duplicate frame 0 collapsed");
        assert_eq!(kfs[0].frame, 0);
        assert_eq!(kfs[0].value, 0.9, "last row for frame 0 wins");
        assert_eq!(kfs[1].frame, 50);
    }

    #[test]
    fn set_keyframes_wrong_arity_errors() {
        let mut exec = executor_with_clip();
        // position needs two values.
        let err = exec
            .execute(
                "set_keyframes",
                &json!({"clipId": "c", "property": "position", "keyframes": [[0, 0.5]]}),
            )
            .unwrap_err();
        assert!(err.contains("topLeftX, topLeftY"), "got: {err}");
    }

    #[test]
    fn set_keyframes_unknown_property_errors() {
        let mut exec = executor_with_clip();
        let err = exec
            .execute(
                "set_keyframes",
                &json!({"clipId": "c", "property": "warp", "keyframes": [[0, 1.0]]}),
            )
            .unwrap_err();
        assert!(err.contains("Unknown keyframe property"));
    }

    #[test]
    fn set_clip_properties_sets_text_color_and_alignment() {
        let mut exec = executor_with_clip();
        exec.execute(
            "set_clip_properties",
            &json!({"clipIds": ["c"], "properties": {"color": "#FF0000", "alignment": "center"}}),
        )
        .unwrap();
        let ts = only_clip(&exec).text_style.as_ref().expect("text style created");
        assert_eq!((ts.color.r, ts.color.g, ts.color.b), (1.0, 0.0, 0.0));
        assert_eq!(ts.alignment, core_model::TextAlignment::Center);
    }

    #[test]
    fn set_clip_properties_rejects_bad_color() {
        let mut exec = executor_with_clip();
        let err = exec
            .execute(
                "set_clip_properties",
                &json!({"clipIds": ["c"], "properties": {"color": "not-a-color"}}),
            )
            .unwrap_err();
        assert!(err.contains("invalid color"), "got: {err}");
    }

    #[test]
    fn set_clip_properties_rejects_bad_alignment() {
        let mut exec = executor_with_clip();
        let err = exec
            .execute(
                "set_clip_properties",
                &json!({"clipIds": ["c"], "properties": {"alignment": "middle"}}),
            )
            .unwrap_err();
        assert!(err.contains("invalid alignment"), "got: {err}");
    }

    #[test]
    fn set_clip_properties_sets_font_weight_background_border() {
        let mut exec = executor_with_clip();
        exec.execute(
            "set_clip_properties",
            &json!({"clipIds": ["c"], "properties": {
                "fontWeight": 700.0,
                "background": {"enabled": true, "color": "#000000", "padding": 8.0, "cornerRadius": 4.0},
                "border": {"enabled": true, "color": "#FFFFFF"}
            }}),
        )
        .unwrap();
        let ts = only_clip(&exec).text_style.as_ref().unwrap();
        assert_eq!(ts.font_weight, 700.0);
        assert!(ts.background.enabled);
        assert_eq!(ts.background.padding, Some(8.0));
        assert_eq!(ts.background.corner_radius, Some(4.0));
        assert_eq!((ts.background.color.r, ts.background.color.g), (0.0, 0.0));
        assert!(ts.border.enabled);
        assert_eq!((ts.border.color.r, ts.border.color.g, ts.border.color.b), (1.0, 1.0, 1.0));
    }

    #[test]
    fn set_clip_properties_rejects_bad_background_color() {
        let mut exec = executor_with_clip();
        let err = exec
            .execute(
                "set_clip_properties",
                &json!({"clipIds": ["c"], "properties": {"background": {"color": "zzz"}}}),
            )
            .unwrap_err();
        // wire-mutation-validators: the validator's hex check fires before the
        // inline parse_text_fill; its message pinpoints the exact field.
        assert!(
            err.contains("'background.color' is not a valid hex color"),
            "got: {err}"
        );
    }

    #[test]
    fn exec_056_ripple_delete_missing_args() {
        let mut exec = make_executor();
        let err = exec
            .execute("ripple_delete_ranges", &json!({}))
            .unwrap_err();
        assert!(err.contains("exactly one of 'trackIndex' or 'clipId'"), "{err}");
    }

    #[test]
    fn insert_clips_auto_creates_linked_audio_and_pushes() {
        // Inserting a video-with-audio mid-clip: the tail ripples, the video is
        // placed, and a linked audio clip is created on an audio track sharing the
        // group at the same position.
        let timeline = executor_with_clip().timeline().clone(); // track 0, "c" [0,100)
        let mut manifest = MediaManifest::default();
        manifest.entries.push(core_model::MediaManifestEntry {
            id: "vid".into(),
            name: "v.mp4".into(),
            r#type: ClipType::Video,
            source: core_model::MediaSource::External {
                absolute_path: "/v.mp4".into(),
            },
            duration: 2.0, // 60 frames @ 30fps
            generation_input: None,
            source_width: Some(1920),
            source_height: Some(1080),
            source_fps: Some(30.0),
            has_audio: Some(true),
            folder_id: None,
            cached_remote_url: None,
            cached_remote_url_expires_at: None,
            source_timecode_frame: None,
            source_timecode_quanta: None,
            source_timecode_drop_frame: None,
            ai_tags: None,
            ai_description: None,
            ai_label_status: None,
            generation_status: None,
        });
        let mut exec = ToolExecutor::new(timeline, manifest);
        exec.execute(
            "insert_clips",
            &json!({"entries": [{"mediaRef": "vid"}], "trackIndex": 0, "atFrame": 40}),
        )
        .unwrap();

        let video_clips = &exec.timeline().tracks[0].clips;
        // Frame conservation: 100 original + 60 inserted.
        let total: i64 = video_clips.iter().map(|c| c.duration_frames).sum();
        assert_eq!(total, 160, "frames preserved: {video_clips:?}");
        let vid = video_clips
            .iter()
            .find(|c| c.link_group_id.is_some())
            .expect("inserted video is linked");
        assert_eq!(vid.start_frame, 40);
        // Linked audio on an audio track, same group + position.
        let audio_track = exec
            .timeline()
            .tracks
            .iter()
            .find(|t| t.r#type == ClipType::Audio)
            .expect("linked audio track");
        assert_eq!(audio_track.clips.len(), 1);
        assert_eq!(audio_track.clips[0].link_group_id, vid.link_group_id);
        assert_eq!(audio_track.clips[0].start_frame, 40);
        assert_eq!(audio_track.clips[0].duration_frames, vid.duration_frames);
    }

    #[test]
    fn insert_clips_pushes_split_tail_preserving_frames() {
        // Inserting inside a clip must split it and push the tail, not overwrite it.
        // Regression: the tail (a fresh split id) was not shifted, then place_clips
        // trimmed it — destroying original frames.
        let mut exec = executor_with_clip(); // track 0, clip "c" span [0,100)
        exec.execute(
            "insert_clips",
            &json!({"entries": [{"mediaRef": "new-media", "durationFrames": 50}], "trackIndex": 0, "atFrame": 40}),
        )
        .unwrap();
        let clips = &exec.timeline().tracks[0].clips;
        let total: i64 = clips.iter().map(|c| c.duration_frames).sum();
        // 100 original frames preserved + 50 inserted = 150 (not 100 with 50 lost).
        assert_eq!(total, 150, "no frames destroyed: {:?}",
            clips.iter().map(|c| (c.start_frame, c.duration_frames)).collect::<Vec<_>>());
        let mut spans: Vec<(i64, i64)> = clips
            .iter()
            .map(|c| (c.start_frame, c.start_frame + c.duration_frames))
            .collect();
        spans.sort();
        assert_eq!(spans, vec![(0, 40), (40, 90), (90, 150)]);
    }

    #[test]
    fn ripple_delete_partial_range_cuts_fragment_not_whole_clip() {
        // Regression: a partial-overlap range destroyed the whole clip (silent media
        // loss). It must cut only the overlapping fragment and close the gap.
        let mut exec = executor_with_clip(); // track 0, clip "c" span [0,100)
        exec.execute(
            "ripple_delete_ranges",
            &json!({"trackIndex": 0, "ranges": [{"start": 25, "end": 50}]}),
        )
        .unwrap();
        let clips = &exec.timeline().tracks[0].clips;
        let mut spans: Vec<(i64, i64)> = clips
            .iter()
            .map(|c| (c.start_frame, c.start_frame + c.duration_frames))
            .collect();
        spans.sort();
        // Head [0,25) kept; tail [50,100) slid left by 25 → [25,75). Not destroyed.
        assert_eq!(spans, vec![(0, 25), (25, 75)], "fragment cut + gap closed");
    }

    fn ripple_207_two_track_exec() -> ToolExecutor {
        // track 0: anchor video clip [0,100); track 1: sync-locked audio clip [0,100).
        let mut exec = make_executor();
        let _ = timeline_core::insert_track_at(exec.timeline_mut(), 0, ClipType::Video);
        let v = crate::test_helpers::make_clip(0, 100);
        let _ = timeline_core::place_clips(exec.timeline_mut(), 0, 0, &[v]);
        let _ = timeline_core::insert_track_at(exec.timeline_mut(), 1, ClipType::Audio);
        exec.timeline_mut().tracks[1].sync_locked = true;
        let a = crate::test_helpers::make_clip(0, 100);
        let _ = timeline_core::place_clips(exec.timeline_mut(), 1, 0, &[a]);
        exec
    }

    fn track_spans(exec: &ToolExecutor, ti: usize) -> Vec<(i64, i64)> {
        let mut s: Vec<(i64, i64)> = exec.timeline().tracks[ti]
            .clips
            .iter()
            .map(|c| (c.start_frame, c.start_frame + c.duration_frames))
            .collect();
        s.sort();
        s
    }

    #[test]
    fn ripple_delete_207_ignored_sync_locked_track_left_in_place() {
        // #207: a sync-locked track listed in ignoreSyncLockedTracks is treated as
        // unlocked — neither cut nor shifted. Its clips stay exactly where they were.
        let mut exec = ripple_207_two_track_exec();
        let before = track_spans(&exec, 1);
        exec.execute(
            "ripple_delete_ranges",
            &json!({
                "trackIndex": 0,
                "ranges": [{"start": 25, "end": 50}],
                "ignoreSyncLockedTracks": [1]
            }),
        )
        .unwrap();
        // Anchor track 0 is cut+rippled: head [0,25) kept, tail slid → [25,75).
        assert_eq!(track_spans(&exec, 0), vec![(0, 25), (25, 75)], "anchor rippled");
        // Ignored sync-locked track 1 untouched.
        assert_eq!(
            track_spans(&exec, 1),
            before,
            "ignored sync-locked track left in place"
        );
        assert_eq!(before, vec![(0, 100)]);
    }

    #[test]
    fn ripple_delete_207_sync_locked_follower_cut_when_not_ignored() {
        // Without the ignore, the same sync-locked track is cut in sync (#227) and shifted.
        let mut exec = ripple_207_two_track_exec();
        exec.execute(
            "ripple_delete_ranges",
            &json!({"trackIndex": 0, "ranges": [{"start": 25, "end": 50}]}),
        )
        .unwrap();
        assert_eq!(
            track_spans(&exec, 1),
            vec![(0, 25), (25, 75)],
            "sync-locked follower cut+rippled in sync with anchor"
        );
    }

    // ── remove_words (#160/#245) ─────────────────────────────────────────────

    #[test]
    fn remove_words_parses_mixed_spans() {
        // Swift parsesMixedSpans: [3, [12,18], 40] → [(3,3),(12,18),(40,40)].
        let spans = ToolExecutor::parse_word_spans(&[json!(3), json!([12, 18]), json!(40)]).unwrap();
        assert_eq!(spans, vec![(3, 3), (12, 18), (40, 40)]);
    }

    #[test]
    fn remove_words_parse_matches_normalizes() {
        let set = ToolExecutor::parse_word_matches(&[json!("Um,"), json!("  UH  "), json!("...hmm")])
            .unwrap();
        assert!(set.contains("um"));
        assert!(set.contains("uh"));
        assert!(set.contains("hmm"));
        // Empty-after-normalize is rejected.
        assert!(ToolExecutor::parse_word_matches(&[json!("!!!")]).is_err());
        assert!(ToolExecutor::parse_word_matches(&[json!(5)]).is_err());
        // Unicode punctuation is trimmed like Swift's category-P (smart quotes, ellipsis).
        assert_eq!(ToolExecutor::normalized_word_match("\u{2018}um\u{2019}"), "um");
        assert_eq!(ToolExecutor::normalized_word_match("\u{2026}uh\u{2026}"), "uh");
        // A lone Unicode-punctuation token normalizes to empty → rejected.
        assert!(ToolExecutor::parse_word_matches(&[json!("\u{2026}")]).is_err());
        // Internal apostrophe (curly or straight) is preserved.
        assert_eq!(ToolExecutor::normalized_word_match("don\u{2019}t"), "don\u{2019}t");
    }

    #[test]
    fn remove_words_int_from_value_rejects_out_of_range_float() {
        // Swift Int(exactly:) returns nil for an astronomical float; parse fails rather than
        // saturating to i64::MAX.
        assert!(ToolExecutor::parse_word_spans(&[json!(1e19)]).is_err());
        assert_eq!(
            ToolExecutor::parse_word_spans(&[json!(5.0), json!(7)]).unwrap(),
            vec![(5, 5), (7, 7)]
        );
    }

    #[test]
    fn remove_words_rejects_empty_words() {
        let mut exec = make_executor();
        assert!(exec.execute("remove_words", &json!({"words": []})).is_err());
        assert!(exec
            .execute("remove_words", &json!({"matches": []}))
            .is_err());
    }

    #[test]
    fn remove_words_rejects_both_and_neither() {
        let mut exec = make_executor();
        assert!(exec.execute("remove_words", &json!({})).is_err());
        assert!(exec
            .execute("remove_words", &json!({"words": [1], "matches": ["um"]}))
            .is_err());
    }

    #[test]
    fn remove_words_refuses_without_transcription() {
        // No words supplied by the host → the tool can't operate.
        let mut exec = make_executor();
        let _ = timeline_core::insert_track_at(exec.timeline_mut(), 0, ClipType::Video);
        let c = crate::test_helpers::make_clip(0, 100);
        let _ = timeline_core::place_clips(exec.timeline_mut(), 0, 0, &[c]);
        let err = exec.execute("remove_words", &json!({"words": [0]})).unwrap_err();
        assert!(err.contains("No transcribable speech"), "{err}");
    }

    fn tw(index: usize, track: usize, start: i64, end: i64, text: &str) -> timeline_core::TimelineWord {
        timeline_core::TimelineWord {
            index,
            clip_id: "c".into(),
            track_index: track,
            clip_start_frame: 0,
            clip_end_frame: 100,
            text: text.into(),
            start_frame: start,
            end_frame: end,
        }
    }

    fn remove_words_exec() -> ToolExecutor {
        // one video track, one clip [0,100); host supplies 3 timeline words.
        let mut exec = make_executor();
        let _ = timeline_core::insert_track_at(exec.timeline_mut(), 0, ClipType::Video);
        let mut c = crate::test_helpers::make_clip(0, 100);
        c.id = "c".into();
        let _ = timeline_core::place_clips(exec.timeline_mut(), 0, 0, &[c]);
        exec.set_timeline_words(vec![
            tw(0, 0, 0, 10, "hello"),
            tw(1, 0, 11, 20, "um"),
            tw(2, 0, 21, 30, "world"),
        ]);
        exec
    }

    fn report_payload(v: &Value) -> Value {
        let text = v["content"][0]["text"].as_str().unwrap();
        serde_json::from_str(text).unwrap()
    }

    #[test]
    fn remove_words_by_index_cuts_and_reports() {
        let mut exec = remove_words_exec();
        let out = exec.execute("remove_words", &json!({"words": [1]})).unwrap();
        let p = report_payload(&out);
        assert_eq!(p["removedWords"], 1);
        assert_eq!(p["removedFrames"], 9); // frames 11..20
        assert_eq!(p["tracksEdited"], 1);
        assert_eq!(p["cutAggressiveness"], "balanced");
        assert_eq!(p["removedText"], "um");
        // The clip was cut+rippled: word 1's span removed, gap closed.
        let spans = track_spans(&exec, 0);
        assert_eq!(spans, vec![(0, 11), (11, 91)], "cut [11,20) then closed gap");
    }

    #[test]
    fn remove_words_by_matches_selects_token() {
        let mut exec = remove_words_exec();
        let out = exec
            .execute("remove_words", &json!({"matches": ["UM"]}))
            .unwrap();
        let p = report_payload(&out);
        assert_eq!(p["removedWords"], 1);
        assert_eq!(p["removedText"], "um");
    }

    #[test]
    fn remove_words_reports_out_of_range_ignored() {
        let mut exec = remove_words_exec();
        let out = exec
            .execute("remove_words", &json!({"words": [1, 99]}))
            .unwrap();
        let p = report_payload(&out);
        assert_eq!(p["indicesIgnored"], json!([99]));
        assert_eq!(p["removedWords"], 1);
    }

    #[test]
    fn remove_words_all_out_of_range_errors() {
        let mut exec = remove_words_exec();
        let err = exec
            .execute("remove_words", &json!({"words": [99]}))
            .unwrap_err();
        assert!(err.contains("in range 0...2"), "{err}");
    }

    #[test]
    fn remove_words_bad_aggressiveness_errors() {
        let mut exec = remove_words_exec();
        assert!(exec
            .execute(
                "remove_words",
                &json!({"words": [1], "cutAggressiveness": "nuclear"})
            )
            .is_err());
    }

    // ── transcription provider seam (transcription-provider-seam) ───────────

    struct MockTranscriber {
        stamps: Vec<WordStamp>,
        calls: std::sync::Mutex<Vec<(String, Option<String>)>>,
        fail: bool,
    }

    impl MockTranscriber {
        fn new(stamps: Vec<WordStamp>) -> std::sync::Arc<Self> {
            std::sync::Arc::new(Self {
                stamps,
                calls: std::sync::Mutex::new(Vec::new()),
                fail: false,
            })
        }
    }

    impl TranscriptionProvider for MockTranscriber {
        fn transcribe(
            &self,
            source: &MediaSource,
            language: Option<&str>,
        ) -> Result<Vec<WordStamp>, String> {
            let key = match source {
                MediaSource::External { absolute_path } => absolute_path.clone(),
                MediaSource::Project { relative_path } => relative_path.clone(),
            };
            self.calls
                .lock()
                .unwrap()
                .push((key, language.map(String::from)));
            if self.fail {
                return Err("transcription model failed".to_string());
            }
            Ok(self.stamps.clone())
        }
    }

    fn stamp(word: &str, start: f64, end: f64) -> WordStamp {
        WordStamp {
            word: word.to_string(),
            start_seconds: start,
            end_seconds: end,
        }
    }

    /// Executor with one audio-bearing video clip: media "m-audio" (hasAudio),
    /// clip "clip-a" at frame 100, trim 60, duration 300, 30fps — the spec's
    /// trimmed-clip placement example.
    fn transcribe_exec() -> ToolExecutor {
        let mut exec = make_executor();
        exec.media_manifest_mut()
            .entries
            .push(media_entry("m-audio", ClipType::Video, true, 30.0));
        let _ = timeline_core::insert_track_at(exec.timeline_mut(), 0, ClipType::Video);
        let mut c = crate::test_helpers::make_clip(100, 300);
        c.id = "clip-a".into();
        c.media_ref = "m-audio".into();
        c.trim_start_frame = 60;
        exec.timeline_mut().tracks[0].clips.push(c);
        exec
    }

    #[test]
    fn transcribe_timeline_without_provider_is_unavailable() {
        let mut exec = transcribe_exec();
        exec.set_timeline_words(vec![tw(0, 0, 0, 10, "kept")]);
        let err = exec.transcribe_timeline().unwrap_err();
        assert!(err.contains("unavailable"), "{err}");
        assert_eq!(exec.timeline_words().len(), 1, "injected words untouched");
    }

    #[test]
    fn transcribe_timeline_full_chain_to_get_transcript() {
        // Task 2.2: transcribe → placement mapping → get_transcript returns real words.
        let mut exec = transcribe_exec();
        exec.set_transcription_provider(MockTranscriber::new(vec![
            stamp("hello", 3.0, 3.5),
            stamp("world", 4.0, 4.5),
        ]));
        let outcome = exec.transcribe_timeline().unwrap();
        assert_eq!(outcome.clips_transcribed, 1);
        assert_eq!(outcome.words, 2);

        // Stored through the set_timeline_words storage, spec-table placement.
        let words = exec.timeline_words();
        assert_eq!(words.len(), 2);
        assert_eq!((words[0].index, words[0].start_frame, words[0].end_frame), (0, 130, 145));
        assert_eq!(words[0].clip_id, "clip-a");
        assert_eq!((words[1].index, words[1].start_frame), (1, 160));

        let out = exec.execute("get_transcript", &json!({})).unwrap();
        let text = out["content"][0]["text"].as_str().unwrap();
        let payload: Value = serde_json::from_str(text).unwrap();
        // v2 (C-6): clipId is the C-3 short prefix; word rows are
        // [index, text, startFrame] with global indices.
        assert!(payload["clips"][0]["clipId"]
            .as_str()
            .map(|s| "clip-a".starts_with(s) || s == "clip-a")
            .unwrap_or(false));
        assert_eq!(payload["clips"][0]["words"][0], json!([0, "hello", 130]));
        assert_eq!(payload["clips"][0]["words"][1], json!([1, "world", 160]));
        assert_eq!(payload["timing"], json!("projectFrames"));

        // Same storage remove_words reads: cutting by match works end-to-end.
        let cut = exec
            .execute("remove_words", &json!({"matches": ["hello"]}))
            .unwrap();
        assert_eq!(report_payload(&cut)["removedWords"], 1);
    }

    #[test]
    fn transcribe_timeline_threads_transcription_language() {
        let mut exec = transcribe_exec();
        let provider = MockTranscriber::new(vec![stamp("ja-word", 3.0, 3.5)]);
        exec.set_transcription_provider(provider.clone());
        exec.timeline_mut().transcription_language = Some("ja".to_string());
        exec.transcribe_timeline().unwrap();
        let calls = provider.calls.lock().unwrap();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0], ("/m-audio".to_string(), Some("ja".to_string())));
    }

    #[test]
    fn transcribe_timeline_no_language_passes_none() {
        let mut exec = transcribe_exec();
        let provider = MockTranscriber::new(vec![stamp("w", 3.0, 3.5)]);
        exec.set_transcription_provider(provider.clone());
        exec.transcribe_timeline().unwrap();
        assert_eq!(provider.calls.lock().unwrap()[0].1, None);
    }

    #[test]
    fn transcribe_timeline_refuses_without_audio_bearing_clips() {
        // Image media + video media without audio: nothing to transcribe.
        let mut exec = make_executor();
        exec.media_manifest_mut()
            .entries
            .push(media_entry("m-img", ClipType::Image, false, 5.0));
        exec.media_manifest_mut()
            .entries
            .push(media_entry("m-mute", ClipType::Video, false, 5.0));
        let _ = timeline_core::insert_track_at(exec.timeline_mut(), 0, ClipType::Video);
        let mut a = crate::test_helpers::make_clip(0, 60);
        a.media_ref = "m-img".into();
        let mut b = crate::test_helpers::make_clip(60, 60);
        b.media_ref = "m-mute".into();
        exec.timeline_mut().tracks[0].clips.push(a);
        exec.timeline_mut().tracks[0].clips.push(b);
        exec.set_transcription_provider(MockTranscriber::new(vec![stamp("x", 0.0, 1.0)]));
        let err = exec.transcribe_timeline().unwrap_err();
        assert!(err.contains("no audio-bearing clips"), "{err}");
    }

    #[test]
    fn transcribe_timeline_shared_source_transcribed_once() {
        // Two clips over the same source (A shows 0..2s, B shows 2..4s):
        // one provider call, words split by visibility, global indices contiguous.
        let mut exec = make_executor();
        exec.media_manifest_mut()
            .entries
            .push(media_entry("m-audio", ClipType::Audio, true, 30.0));
        let _ = timeline_core::insert_track_at(exec.timeline_mut(), 0, ClipType::Audio);
        let mut a = crate::test_helpers::make_clip(0, 60);
        a.id = "a".into();
        a.media_ref = "m-audio".into();
        let mut b = crate::test_helpers::make_clip(60, 60);
        b.id = "b".into();
        b.media_ref = "m-audio".into();
        b.trim_start_frame = 60;
        exec.timeline_mut().tracks[0].clips.push(a);
        exec.timeline_mut().tracks[0].clips.push(b);
        let provider = MockTranscriber::new(vec![stamp("one", 1.0, 1.5), stamp("two", 3.0, 3.5)]);
        exec.set_transcription_provider(provider.clone());
        let outcome = exec.transcribe_timeline().unwrap();
        assert_eq!(provider.calls.lock().unwrap().len(), 1, "source cached");
        assert_eq!(outcome.clips_transcribed, 2);
        assert_eq!(outcome.words, 2);
        let words = exec.timeline_words();
        assert_eq!((words[0].index, words[0].clip_id.as_str(), words[0].start_frame), (0, "a", 30));
        assert_eq!((words[1].index, words[1].clip_id.as_str(), words[1].start_frame), (1, "b", 90));
    }

    #[test]
    fn transcribe_timeline_provider_error_keeps_existing_words() {
        let mut exec = transcribe_exec();
        exec.set_timeline_words(vec![tw(0, 0, 0, 10, "kept")]);
        exec.set_transcription_provider(std::sync::Arc::new(MockTranscriber {
            stamps: Vec::new(),
            calls: std::sync::Mutex::new(Vec::new()),
            fail: true,
        }));
        let err = exec.transcribe_timeline().unwrap_err();
        assert!(err.contains("transcription model failed"), "{err}");
        assert_eq!(exec.timeline_words().len(), 1, "no partial store on failure");
    }

    #[test]
    fn get_transcript_transcribes_on_demand_when_words_missing() {
        // v2: with a provider installed and no stored words, get_transcript
        // runs the transcriber itself instead of returning an empty payload.
        let mut exec = transcribe_exec();
        exec.set_transcription_provider(MockTranscriber::new(vec![stamp("x", 3.0, 3.5)]));
        let out = exec.execute("get_transcript", &json!({})).unwrap();
        let payload: Value =
            serde_json::from_str(out["content"][0]["text"].as_str().unwrap()).unwrap();
        assert_eq!(payload["clips"][0]["words"][0][1], json!("x"));
    }

    // ── create_matte (#242) ──────────────────────────────────────────────────

    #[derive(Default)]
    struct MockMatte {
        last: std::sync::Mutex<Option<([u8; 4], i64, i64, String)>>,
    }
    impl MatteWriter for MockMatte {
        fn write_matte(
            &self,
            rgba: [u8; 4],
            width: i64,
            height: i64,
            base_name: &str,
        ) -> Result<MediaSource, String> {
            *self.last.lock().unwrap() = Some((rgba, width, height, base_name.to_string()));
            Ok(MediaSource::Project {
                relative_path: format!("media/{base_name}.png"),
            })
        }
    }

    #[test]
    fn import_media_matte_writes_and_registers_image() {
        let mut exec = make_executor(); // Timeline::default() = 1920x1080
        let writer = std::sync::Arc::new(MockMatte::default());
        exec.set_matte_writer(writer.clone());
        let out = exec
            .execute(
                "import_media",
                &json!({"source": {"matte": {"hex": "#FF0000", "aspectRatio": "1:1"}}, "name": "Red"}),
            )
            .unwrap();
        // 1:1 in 1920x1080 → short edge 1080 → 1080x1080; #FF0000 → [255,0,0,255].
        let (rgba, w, h, name) = writer.last.lock().unwrap().clone().unwrap();
        assert_eq!(rgba, [255, 0, 0, 255]);
        assert_eq!((w, h), (1080, 1080));
        assert_eq!(name, "Red");
        // A new image asset is registered with the matte dimensions.
        assert_eq!(exec.media_manifest.entries.len(), 1);
        let e = &exec.media_manifest.entries[0];
        assert_eq!(e.r#type, ClipType::Image);
        assert_eq!(e.source_width, Some(1080));
        assert_eq!(e.source_height, Some(1080));
        let text = out["content"][0]["text"].as_str().unwrap();
        assert!(
            text.contains(&e.id[..8]),
            "result carries the mediaRef's short prefix: {text}"
        );
    }

    #[test]
    fn import_media_matte_defaults_to_project_aspect() {
        let mut exec = make_executor();
        let writer = std::sync::Arc::new(MockMatte::default());
        exec.set_matte_writer(writer.clone());
        exec.execute("import_media", &json!({"source": {"matte": {"hex": "#000"}}}))
            .unwrap();
        let (_, w, h, name) = writer.last.lock().unwrap().clone().unwrap();
        assert_eq!((w, h), (1920, 1080), "default aspect = Project");
        assert_eq!(name, "Matte", "default name");
    }

    #[test]
    fn every_advertised_tool_is_dispatched() {
        // Definitive guard: no tool in the advertised registry should reach the executor's
        // "Unknown tool" fallthrough. Empty args mean tools may error on missing params — that's
        // fine; we only assert each NAME is routed to a handler.
        let mut exec = make_executor();
        let mut undispatched: Vec<&str> = Vec::new();
        for tool in crate::all_tools() {
            if let Err(e) = exec.execute(tool.name, &json!({})) {
                if e.contains("Unknown tool") {
                    undispatched.push(tool.name);
                }
            }
        }
        assert!(
            undispatched.is_empty(),
            "advertised tools with no executor dispatch: {undispatched:?}"
        );
    }

    #[test]
    fn project_nav_tools_unavailable_without_navigator() {
        let mut exec = make_executor();
        let err = exec
            .execute("open_project", &json!({"path": "/x.palmier"}))
            .unwrap_err();
        assert!(err.contains("unavailable"), "{err}");
        let err = exec.execute("new_project", &json!({})).unwrap_err();
        assert!(err.contains("unavailable"), "{err}");
        let err = exec.execute("close_project", &json!({})).unwrap_err();
        assert!(err.contains("unavailable"), "{err}");
        // The speculative names were removed with the v0.6.1 alignment.
        let err = exec.execute("create_project", &json!({})).unwrap_err();
        assert!(err.contains("Unknown tool"), "{err}");
    }

    /// Mock navigator for close_project: records the saved state, returns a
    /// scripted outcome.
    struct MockCloseNav {
        outcome: std::sync::Mutex<Option<Result<ClosedProject, String>>>,
        saved_fps: std::sync::Mutex<Option<i64>>,
    }

    impl ProjectNavigator for MockCloseNav {
        fn open(&self, _id: Option<&str>, _path: Option<&str>) -> Result<OpenedProject, String> {
            Err("not scripted".into())
        }
        fn create(&self, _name: Option<&str>) -> Result<OpenedProject, String> {
            Err("not scripted".into())
        }
        fn close(
            &self,
            _name: Option<&str>,
            _id: Option<&str>,
            _path: Option<&str>,
            active: ActiveProjectState,
        ) -> Result<ClosedProject, String> {
            *self.saved_fps.lock().unwrap() = Some(active.timeline.fps);
            self.outcome
                .lock()
                .unwrap()
                .take()
                .expect("outcome scripted")
        }
    }

    struct MockCloseLister;
    impl ProjectLister for MockCloseLister {
        fn list(&self) -> Result<(Vec<KnownProject>, Option<(String, String)>), String> {
            Ok((Vec::new(), None))
        }
    }

    #[test]
    fn close_project_saves_then_resets_to_no_project_state() {
        let mut exec = make_executor_with_media();
        exec.timeline_mut().fps = 48;
        exec.set_matte_writer(std::sync::Arc::new(MockMatte::default()));
        let nav = std::sync::Arc::new(MockCloseNav {
            outcome: std::sync::Mutex::new(Some(Ok(ClosedProject {
                name: "Demo".into(),
                open_count: 0,
                was_active: true,
                next_active: None,
                lister: Some(std::sync::Arc::new(MockCloseLister)),
            }))),
            saved_fps: std::sync::Mutex::new(None),
        });
        exec.set_project_navigator(nav.clone());

        let res = exec.execute("close_project", &json!({})).unwrap();
        let body: serde_json::Value =
            serde_json::from_str(res["content"][0]["text"].as_str().unwrap()).unwrap();
        assert_eq!(body["status"], json!("closed"));
        assert_eq!(body["name"], json!("Demo"));
        assert_eq!(body["openCount"], json!(0));
        assert!(body.get("active").is_none(), "no next project: {body}");
        // The navigator received the live state for the save-first step.
        assert_eq!(*nav.saved_fps.lock().unwrap(), Some(48));
        // The executor reset to the no-project state: default timeline, empty
        // manifest, project-scoped seams gone (matte imports unavailable) —
        // but get_projects still answers through the replacement lister.
        assert_eq!(exec.timeline().fps, Timeline::default().fps);
        assert!(exec.media_manifest().entries.is_empty());
        let err = exec
            .execute("import_media", &json!({"source": {"matte": {"hex": "#000"}}}))
            .unwrap_err();
        assert!(err.contains("unavailable"), "{err}");
        assert!(exec.execute("get_projects", &json!({})).is_ok());
    }

    #[test]
    fn close_project_save_failure_leaves_the_project_open() {
        let mut exec = make_executor_with_media();
        exec.timeline_mut().fps = 48;
        let nav = std::sync::Arc::new(MockCloseNav {
            outcome: std::sync::Mutex::new(Some(Err(
                "Couldn't save 'Demo' — project left open. disk full".into(),
            ))),
            saved_fps: std::sync::Mutex::new(None),
        });
        exec.set_project_navigator(nav);
        let err = exec.execute("close_project", &json!({})).unwrap_err();
        assert!(err.contains("project left open"), "{err}");
        assert_eq!(exec.timeline().fps, 48, "executor untouched on failure");
        assert_eq!(exec.media_manifest().entries.len(), 1);
    }

    #[test]
    fn close_project_adopts_the_next_active_project() {
        let mut exec = make_executor_with_media();
        let next = OpenedProject {
            name: "Next".into(),
            root: "/tmp/Next.palmier".into(),
            timeline: Timeline {
                fps: 60,
                ..Default::default()
            },
            sibling_timelines: Vec::new(),
            manifest: MediaManifest::default(),
            multicam_groups: Vec::new(),
            seams: ProjectSeams {
                matte_writer: std::sync::Arc::new(MockMatte::default()),
                audio_source: std::sync::Arc::new(MockAudio),
                export_host: std::sync::Arc::new(MockExportHost),
                project_lister: std::sync::Arc::new(MockCloseLister),
            },
        };
        let nav = std::sync::Arc::new(MockCloseNav {
            outcome: std::sync::Mutex::new(Some(Ok(ClosedProject {
                name: "Demo".into(),
                open_count: 1,
                was_active: true,
                next_active: Some(next),
                lister: None,
            }))),
            saved_fps: std::sync::Mutex::new(None),
        });
        exec.set_project_navigator(nav);
        let res = exec.execute("close_project", &json!({})).unwrap();
        let body: serde_json::Value =
            serde_json::from_str(res["content"][0]["text"].as_str().unwrap()).unwrap();
        assert_eq!(body["active"]["name"], json!("Next"));
        assert_eq!(body["openCount"], json!(1));
        assert_eq!(exec.timeline().fps, 60, "adopted the next project");
    }

    #[test]
    fn compound_clip_create_and_dissolve_round_trip_via_executor() {
        let mut manifest = MediaManifest::default();
        manifest.entries.push(video_media("m1", 1920, 1080, 30.0));
        manifest.entries.push(video_media("m2", 1920, 1080, 30.0));
        let mut exec = ToolExecutor::new(Timeline::default(), manifest);
        exec.execute("add_clips", &json!({"entries": [{"mediaRef": "m1", "startFrame": 0}, {"mediaRef": "m2", "startFrame": 90}]}))
            .unwrap();
        let track0 = &exec.timeline().tracks[0];
        assert_eq!(track0.clips.len(), 2, "two clips placed on one track");
        let ids: Vec<String> = track0.clips.iter().map(|c| c.id.clone()).collect();

        let res = exec
            .execute(
                "create_compound_clip",
                &json!({"clipIds": [ids[0], ids[1]], "name": "Scene"}),
            )
            .unwrap();
        let text = res["content"][0]["text"].as_str().unwrap();
        assert!(text.contains("compoundClipId"), "res={text}");
        assert_eq!(exec.timeline().tracks[0].clips.len(), 1, "one carrier clip");
        // Swift #255 representation: a sequence carrier + a sibling timeline.
        let carrier = exec.timeline().tracks[0].clips[0].clone();
        assert_eq!(carrier.source_clip_type, ClipType::Sequence);
        assert_eq!(carrier.media_type, ClipType::Sequence);
        assert!(carrier.compound_timeline_id.is_none(), "no legacy field");
        assert_eq!(exec.sibling_timelines().len(), 1);
        assert_eq!(
            exec.sibling_timelines()[0].id, carrier.media_ref,
            "carrier points at the sibling child timeline"
        );
        assert_eq!(exec.sibling_timelines()[0].name, "Scene");

        let res2 = exec
            .execute("dissolve_compound_clip", &json!({"clipId": carrier.id}))
            .unwrap();
        let text2 = res2["content"][0]["text"].as_str().unwrap();
        assert!(text2.contains("restoredClipIds"), "res={text2}");
        assert_eq!(exec.timeline().tracks[0].clips.len(), 2, "clips restored");
        assert!(
            exec.timeline().tracks[0]
                .clips
                .iter()
                .all(|c| c.source_clip_type != ClipType::Sequence),
            "no carrier left"
        );
    }

    #[test]
    fn compound_clip_errors_surface_not_unknown_tool() {
        let mut exec = make_executor();
        let err = exec
            .execute("create_compound_clip", &json!({"clipIds": ["ghost"]}))
            .unwrap_err();
        assert!(!err.contains("Unknown tool"), "{err}");
        assert!(err.contains("not found"), "{err}");
        let err2 = exec
            .execute("dissolve_compound_clip", &json!({"clipId": "ghost"}))
            .unwrap_err();
        assert!(err2.contains("not found"), "{err2}");
    }

    #[test]
    fn clip_preset_save_list_apply_round_trip() {
        let mut manifest = MediaManifest::default();
        manifest.entries.push(video_media("m1", 1920, 1080, 30.0));
        manifest.entries.push(video_media("m2", 1920, 1080, 30.0));
        let mut exec = ToolExecutor::new(Timeline::default(), manifest);
        exec.execute("add_clips", &json!({"entries": [{"mediaRef": "m1", "startFrame": 0}, {"mediaRef": "m2", "startFrame": 90}]}))
            .unwrap();
        let ids: Vec<String> = exec
            .timeline()
            .tracks[0]
            .clips
            .iter()
            .map(|c| c.id.clone())
            .collect();

        {
            let src = &mut exec.timeline_mut().tracks[0].clips[0];
            src.opacity = 0.5;
            src.transform.rotation = 45.0;
            src.volume = 0.25;
        }

        let res = exec
            .execute("save_clip_preset", &json!({"clipId": ids[0], "name": "Look A"}))
            .unwrap();
        assert!(res["content"][0]["text"].as_str().unwrap().contains("Look A"));

        let list = exec.execute("list_clip_presets", &json!({})).unwrap();
        assert!(list["content"][0]["text"]
            .as_str()
            .unwrap()
            .contains("Look A"));

        let ap = exec
            .execute(
                "apply_clip_preset",
                &json!({"presetName": "Look A", "clipIds": [ids[1]]}),
            )
            .unwrap();
        assert!(ap["content"][0]["text"]
            .as_str()
            .unwrap()
            .contains("\"applied\":1"));

        let dst = &exec.timeline().tracks[0].clips[1];
        assert!((dst.opacity - 0.5).abs() < 1e-9, "opacity applied");
        assert!(
            (dst.transform.rotation - 45.0).abs() < 1e-9,
            "rotation applied"
        );
        assert!((dst.volume - 0.25).abs() < 1e-9, "volume applied");
    }

    #[test]
    fn clip_preset_errors_are_honest_not_unknown_tool() {
        let mut exec = make_executor();
        let e1 = exec
            .execute("save_clip_preset", &json!({"clipId": "ghost", "name": "X"}))
            .unwrap_err();
        assert!(!e1.contains("Unknown tool"), "{e1}");
        assert!(e1.contains("not found"), "{e1}");
        let e2 = exec
            .execute(
                "apply_clip_preset",
                &json!({"presetName": "nope", "clipIds": ["a"]}),
            )
            .unwrap_err();
        assert!(e2.contains("No clip preset named"), "{e2}");
    }

    struct MockAudio;
    impl ClipAudioSource for MockAudio {
        fn decode_source_pcm(
            &self,
            _source: &core_model::MediaSource,
            sample_rate: u32,
            channels: usize,
        ) -> Option<Vec<f32>> {
            // 1s loud, 2s silent, 1s loud (mono at `sample_rate`).
            let sr = sample_rate as usize * channels;
            let mut pcm = Vec::new();
            pcm.extend(std::iter::repeat(0.5f32).take(sr));
            pcm.extend(std::iter::repeat(0.0f32).take(2 * sr));
            pcm.extend(std::iter::repeat(0.5f32).take(sr));
            Some(pcm)
        }
    }

    fn audio_media(id: &str, duration: f64) -> core_model::MediaManifestEntry {
        core_model::MediaManifestEntry {
            id: id.into(),
            name: format!("{id}.wav"),
            r#type: ClipType::Audio,
            source: core_model::MediaSource::External {
                absolute_path: format!("/{id}.wav"),
            },
            duration,
            generation_input: None,
            source_width: None,
            source_height: None,
            source_fps: None,
            has_audio: Some(true),
            folder_id: None,
            cached_remote_url: None,
            cached_remote_url_expires_at: None,
            source_timecode_frame: None,
            source_timecode_quanta: None,
            source_timecode_drop_frame: None,
            ai_tags: None,
            ai_description: None,
            ai_label_status: None,
            generation_status: None,
        }
    }

    #[test]
    fn remove_silence_cuts_the_silent_region() {
        let mut manifest = MediaManifest::default();
        manifest.entries.push(audio_media("a1", 4.0));
        let mut exec = ToolExecutor::new(Timeline::default(), manifest);
        exec.set_audio_source(std::sync::Arc::new(MockAudio));
        exec.execute("add_clips", &json!({"entries": [{"mediaRef": "a1", "startFrame": 0}]})).unwrap();
        let clip_id = exec.timeline().tracks[0].clips[0].id.clone();

        let res = exec
            .execute("remove_silence", &json!({"clipId": clip_id}))
            .unwrap();
        let v: serde_json::Value =
            serde_json::from_str(res["content"][0]["text"].as_str().unwrap()).unwrap();
        assert_eq!(v["sectionsRemoved"], json!(1), "one silent region: {v}");
        assert!(v["removedFrames"].as_i64().unwrap() > 0, "frames removed: {v}");
    }

    struct MockLoudAudio;
    impl ClipAudioSource for MockLoudAudio {
        fn decode_source_pcm(
            &self,
            _source: &core_model::MediaSource,
            sample_rate: u32,
            channels: usize,
        ) -> Option<Vec<f32>> {
            // 4s of steady tone — no silent region.
            Some(vec![0.5f32; sample_rate as usize * channels * 4])
        }
    }

    #[test]
    fn remove_silence_reports_zero_when_no_silence() {
        let mut manifest = MediaManifest::default();
        manifest.entries.push(audio_media("a1", 4.0));
        let mut exec = ToolExecutor::new(Timeline::default(), manifest);
        exec.set_audio_source(std::sync::Arc::new(MockLoudAudio));
        exec.execute("add_clips", &json!({"entries": [{"mediaRef": "a1", "startFrame": 0}]})).unwrap();
        let clip_id = exec.timeline().tracks[0].clips[0].id.clone();
        let before = exec.timeline().tracks[0].clips.len();

        let res = exec
            .execute("remove_silence", &json!({"clipId": clip_id}))
            .unwrap();
        let v: serde_json::Value =
            serde_json::from_str(res["content"][0]["text"].as_str().unwrap()).unwrap();
        assert_eq!(v["sectionsRemoved"], json!(0), "no cut: {v}");
        assert_eq!(
            exec.timeline().tracks[0].clips.len(),
            before,
            "timeline unchanged when nothing is silent"
        );
    }

    #[test]
    fn remove_silence_unavailable_without_audio_source() {
        let mut manifest = MediaManifest::default();
        manifest.entries.push(audio_media("a1", 4.0));
        let mut exec = ToolExecutor::new(Timeline::default(), manifest);
        exec.execute("add_clips", &json!({"entries": [{"mediaRef": "a1", "startFrame": 0}]})).unwrap();
        let clip_id = exec.timeline().tracks[0].clips[0].id.clone();
        let err = exec
            .execute("remove_silence", &json!({"clipId": clip_id}))
            .unwrap_err();
        assert!(!err.contains("Unknown tool"), "{err}");
        assert!(err.contains("unavailable"), "{err}");
    }

    struct MockSpeech(Option<Vec<SpeechSpan>>);
    impl SpeechAnalyzer for MockSpeech {
        fn analyze(
            &self,
            _source: &core_model::MediaSource,
            _sample_rate: u32,
        ) -> Option<Vec<SpeechSpan>> {
            self.0.clone()
        }
    }

    #[test]
    fn remove_silence_uses_analyzer_spans_over_rms() {
        // MockLoudAudio has NO RMS-silent region; the analyzer says speech is
        // only 0–2s of the 4s source, so any cut must come from span inversion.
        let mut manifest = MediaManifest::default();
        manifest.entries.push(audio_media("a1", 4.0));
        let mut exec = ToolExecutor::new(Timeline::default(), manifest);
        exec.set_audio_source(std::sync::Arc::new(MockLoudAudio));
        exec.set_speech_analyzer(std::sync::Arc::new(MockSpeech(Some(vec![SpeechSpan {
            start_seconds: 0.0,
            end_seconds: 2.0,
        }]))));
        exec.execute("add_clips", &json!({"entries": [{"mediaRef": "a1", "startFrame": 0}]})).unwrap();
        let clip_id = exec.timeline().tracks[0].clips[0].id.clone();

        let res = exec
            .execute("remove_silence", &json!({"clipId": clip_id}))
            .unwrap();
        let v: serde_json::Value =
            serde_json::from_str(res["content"][0]["text"].as_str().unwrap()).unwrap();
        assert_eq!(v["sectionsRemoved"], 1, "{v}");
        // Dead air (2.1, 4.0) — trailing gap keeps the clip edge unpadded —
        // ≈ 57 frames at 30fps.
        let removed = v["removedFrames"].as_i64().unwrap();
        assert!((55..=59).contains(&removed), "{v}");
    }

    #[test]
    fn remove_silence_analyzer_none_falls_back_to_rms_identically() {
        let run = |with_none_analyzer: bool| {
            let mut manifest = MediaManifest::default();
            manifest.entries.push(audio_media("a1", 4.0));
            let mut exec = ToolExecutor::new(Timeline::default(), manifest);
            exec.set_audio_source(std::sync::Arc::new(MockAudio));
            if with_none_analyzer {
                exec.set_speech_analyzer(std::sync::Arc::new(MockSpeech(None)));
            }
            exec.execute("add_clips", &json!({"entries": [{"mediaRef": "a1", "startFrame": 0}]})).unwrap();
            let res = exec.execute("remove_silence", &json!({})).unwrap();
            let payload = res["content"][0]["text"].as_str().unwrap().to_string();
            let spans: Vec<(i64, i64)> = exec
                .timeline()
                .tracks
                .iter()
                .flat_map(|t| &t.clips)
                .map(|c| (c.start_frame, c.start_frame + c.duration_frames))
                .collect();
            (payload, spans)
        };
        let (base_payload, base_spans) = run(false);
        let (none_payload, none_spans) = run(true);
        // The envelope's clip rows carry per-run uuids; compare the payloads
        // with ids stripped so the check stays about the DETECTION result.
        let strip = |p: &str| -> serde_json::Value {
            let mut v: serde_json::Value = serde_json::from_str(p).unwrap();
            if let Some(clips) = v.get_mut("clips").and_then(|c| c.as_array_mut()) {
                for c in clips {
                    c.as_object_mut().unwrap().remove("id");
                }
            }
            v
        };
        assert_eq!(strip(&base_payload), strip(&none_payload), "fallback payload identical");
        assert_eq!(base_spans, none_spans, "fallback timeline identical");
    }

    fn sync_noise(n: usize) -> Vec<f32> {
        (0..n)
            .map(|i| {
                let x = i as f64 * 0.137;
                ((x * std::f64::consts::TAU).sin()
                    + (x * 2.71 * std::f64::consts::PI).cos()
                    + (x * 0.37 * std::f64::consts::TAU).sin()) as f32
                    * 0.3
            })
            .collect()
    }

    struct MockSyncAudio;
    impl ClipAudioSource for MockSyncAudio {
        fn decode_source_pcm(
            &self,
            source: &core_model::MediaSource,
            sample_rate: u32,
            _channels: usize,
        ) -> Option<Vec<f32>> {
            let n = sample_rate as usize; // 1s
            let base = sync_noise(n);
            let path = match source {
                core_model::MediaSource::External { absolute_path } => absolute_path.clone(),
                core_model::MediaSource::Project { relative_path } => relative_path.clone(),
            };
            if path.contains("tgt") {
                // 4096 leading silent samples (4 RMS frames) → the target's content
                // is delayed relative to the reference.
                let pad = 4096;
                let mut v = vec![0.0f32; pad];
                v.extend_from_slice(&base[..n - pad]);
                Some(v)
            } else {
                Some(base)
            }
        }
    }

    #[test]
    fn sync_clips_moves_delayed_target_earlier() {
        let mut manifest = MediaManifest::default();
        manifest.entries.push(audio_media("ref", 1.0));
        manifest.entries.push(audio_media("tgt", 1.0));
        let mut exec = ToolExecutor::new(Timeline::default(), manifest);
        exec.set_audio_source(std::sync::Arc::new(MockSyncAudio));
        exec.execute("add_clips", &json!({"entries": [{"mediaRef": "ref", "startFrame": 0}, {"mediaRef": "tgt", "startFrame": 30}]}))
            .unwrap();
        // Dual-system layout: ref and tgt each on their own audio track, both
        // anchored at frame 100 so delta == -offset (clean sign check). Moving
        // them to one shared track/frame would be an overlapping (invalid) state
        // that move_clips' overwrite semantics would clear.
        {
            let tl = exec.timeline_mut();
            let mut tgt_clip = None;
            for t in tl.tracks.iter_mut() {
                if let Some(pos) = t.clips.iter().position(|c| c.media_ref == "tgt") {
                    tgt_clip = Some(t.clips.remove(pos));
                }
            }
            let mut tgt_clip = tgt_clip.expect("tgt placed by add_clips");
            tgt_clip.start_frame = 100;
            for t in tl.tracks.iter_mut() {
                for c in t.clips.iter_mut() {
                    c.start_frame = 100;
                }
            }
            tl.tracks.push(core_model::Track {
                id: "sync-tgt-track".into(),
                r#type: ClipType::Audio,
                muted: false,
                hidden: false,
                sync_locked: true,
               display_height: 50.0,
                clips: vec![tgt_clip],
            });
        }
        let clip_id_by_ref = |exec: &ToolExecutor, r: &str| {
            exec.timeline()
                .tracks
                .iter()
                .flat_map(|t| &t.clips)
                .find(|c| c.media_ref == r)
                .unwrap()
                .id
                .clone()
        };
        let ref_id = clip_id_by_ref(&exec, "ref");
        let tgt_id = clip_id_by_ref(&exec, "tgt");

        let res = exec
            .execute(
                "sync_clips",
                &json!({"referenceClipId": ref_id, "targetClipIds": [tgt_id]}),
            )
            .unwrap();
        let text = res["content"][0]["text"].as_str().unwrap();
        let v: serde_json::Value = serde_json::from_str(text).unwrap();
        let synced = v["synced"].as_array().unwrap();
        assert_eq!(synced.len(), 1, "target synced: {text}");
        let off = synced[0]["offsetFrames"].as_i64().unwrap();
        let moved = synced[0]["movedToFrame"].as_i64().unwrap();
        assert!(off > 0, "delayed target → positive offset: {text}");
        assert_eq!(moved, 100 - off, "moved earlier by the offset");
        assert!(moved < 100, "delayed target moves earlier, not later");
        // move_clips re-inserts under a NEW id; the tool reports it as newClipId.
        let new_id = synced[0]["newClipId"].as_str().unwrap();
        assert!(!tgt_id.starts_with(new_id), "moved clip gets a fresh id");
        let tgt = exec
            .timeline()
            .tracks
            .iter()
            .flat_map(|t| &t.clips)
            .find(|c| c.id.starts_with(new_id))
            .unwrap();
        assert_eq!(tgt.start_frame, moved, "clip actually moved");
    }

    #[test]
    fn sync_clips_skips_below_min_confidence() {
        let mut manifest = MediaManifest::default();
        manifest.entries.push(audio_media("ref", 1.0));
        manifest.entries.push(audio_media("tgt", 1.0));
        let mut exec = ToolExecutor::new(Timeline::default(), manifest);
        exec.set_audio_source(std::sync::Arc::new(MockSyncAudio));
        exec.execute("add_clips", &json!({"entries": [{"mediaRef": "ref", "startFrame": 0}, {"mediaRef": "tgt", "startFrame": 30}]}))
            .unwrap();
        let find_by_ref = |exec: &ToolExecutor, r: &str| {
            exec.timeline().tracks.iter().flat_map(|t| &t.clips).find(|c| c.media_ref == r).unwrap().id.clone()
        };
        let start_of = |exec: &ToolExecutor, id: &str| {
            exec.timeline().tracks.iter().flat_map(|t| &t.clips).find(|c| c.id == id).unwrap().start_frame
        };
        let ref_id = find_by_ref(&exec, "ref");
        let tgt_id = find_by_ref(&exec, "tgt");
        let before = start_of(&exec, &tgt_id);

        // An impossible threshold forces the match into `skipped`.
        let res = exec
            .execute(
                "sync_clips",
                &json!({"referenceClipId": ref_id, "targetClipIds": [tgt_id], "minConfidence": 2.0}),
            )
            .unwrap();
        let v: serde_json::Value =
            serde_json::from_str(res["content"][0]["text"].as_str().unwrap()).unwrap();
        assert_eq!(v["synced"].as_array().unwrap().len(), 0);
        assert_eq!(v["skipped"].as_array().unwrap().len(), 1);
        assert_eq!(before, start_of(&exec, &tgt_id), "low-confidence target is left in place");
    }

    #[test]
    fn sync_clips_unavailable_without_audio_source() {
        let mut manifest = MediaManifest::default();
        manifest.entries.push(audio_media("ref", 1.0));
        manifest.entries.push(audio_media("tgt", 1.0));
        let mut exec = ToolExecutor::new(Timeline::default(), manifest);
        exec.execute(
            "add_clips",
            &json!({"entries": [{"mediaRef": "ref", "startFrame": 0}, {"mediaRef": "tgt", "startFrame": 30}]}),
        )
        .unwrap();
        let ref_id = exec.timeline().tracks[0].clips[0].id.clone();
        let tgt_id = exec.timeline().tracks[0].clips[1].id.clone();
        // mode audio without a decoder: hard error.
        let err = exec
            .execute(
                "sync_clips",
                &json!({"referenceClipId": ref_id, "targetClipIds": [tgt_id], "mode": "audio"}),
            )
            .unwrap_err();
        assert!(!err.contains("Unknown tool"), "{err}");
        assert!(err.contains("unavailable"), "{err}");
        // mode auto without a decoder or timecodes: the target is skipped
        // with the decoder gap named.
        let res = exec
            .execute(
                "sync_clips",
                &json!({"referenceClipId": ref_id, "targetClipIds": [tgt_id]}),
            )
            .unwrap();
        let v: Value = serde_json::from_str(res["content"][0]["text"].as_str().unwrap()).unwrap();
        assert!(v["skipped"][0]["reason"]
            .as_str()
            .unwrap()
            .contains("no audio decoder"));
    }

    #[test]
    fn sync_clips_timecode_mode_aligns_exactly() {
        // Both files carry embedded source timecode: ref at 01:00:00:00 and
        // target 2s later — the target shifts 60 frames (30fps) with
        // confidence 1.0, no audio decoder needed.
        let mut manifest = MediaManifest::default();
        let mut r = audio_media("ref", 10.0);
        r.source_timecode_frame = Some(90_000); // 3600s * 25
        r.source_timecode_quanta = Some(25);
        let mut t = audio_media("tgt", 10.0);
        t.source_timecode_frame = Some(90_050); // +2s
        t.source_timecode_quanta = Some(25);
        manifest.entries.push(r);
        manifest.entries.push(t);
        let mut exec = ToolExecutor::new(Timeline::default(), manifest);
        exec.execute(
            "add_clips",
            &json!({"entries": [{"mediaRef": "ref", "startFrame": 100}, {"mediaRef": "tgt", "startFrame": 400}]}),
        )
        .unwrap();
        let ref_id = exec.timeline().tracks[0].clips[0].id.clone();
        let tgt_id = exec.timeline().tracks[0].clips[1].id.clone();
        let res = exec
            .execute(
                "sync_clips",
                &json!({"referenceClipId": ref_id, "targetClipId": tgt_id, "mode": "timecode"}),
            )
            .unwrap();
        let v: Value = serde_json::from_str(res["content"][0]["text"].as_str().unwrap()).unwrap();
        let synced = &v["synced"][0];
        assert_eq!(synced["method"], json!("timecode"));
        assert_eq!(synced["confidence"], json!(1.0));
        assert_eq!(
            synced["offsetFrames"],
            json!(-60),
            "recording started 2s later = content 2s earlier in file (correlator sign)"
        );
        // Target anchors 60 frames after the reference: 100 + 60 = 160.
        assert_eq!(synced["movedToFrame"], json!(160));
    }

    fn denoise_exec() -> (ToolExecutor, String) {
        let mut manifest = MediaManifest::default();
        manifest.entries.push(audio_media("a1", 4.0));
        let mut exec = ToolExecutor::new(Timeline::default(), manifest);
        exec.execute("add_clips", &json!({"entries": [{"mediaRef": "a1", "startFrame": 0}]})).unwrap();
        let id = exec.timeline().tracks[0].clips[0].id.clone();
        (exec, id)
    }

    fn denoise_amount_of(exec: &ToolExecutor, id: &str) -> Option<f64> {
        exec.timeline()
            .tracks
            .iter()
            .flat_map(|t| &t.clips)
            .find(|c| c.id == id)
            .and_then(|c| c.effects.as_ref())
            .and_then(|es| es.iter().find(|e| e.r#type == "audio.denoise"))
            .and_then(|e| e.params.get("amount"))
            .and_then(|p| p.value)
    }

    #[test]
    fn denoise_audio_sets_effect_with_strength() {
        let (mut exec, id) = denoise_exec();
        let res = exec
            .execute("denoise_audio", &json!({"clipIds": [id], "strength": 80}))
            .unwrap();
        let env: serde_json::Value =
            serde_json::from_str(res["content"][0]["text"].as_str().unwrap()).unwrap();
        assert_eq!(env["clips"][0]["effects"][0]["type"], json!("audio.denoise"));
        assert_eq!(denoise_amount_of(&exec, &id), Some(0.8));
    }

    #[test]
    fn denoise_audio_reenable_without_strength_keeps_custom_amount() {
        // The #251 merge fix: enabling on a clip that already has a custom
        // strength must NOT clobber it with the default.
        let (mut exec, id) = denoise_exec();
        exec.execute("denoise_audio", &json!({"clipIds": [id], "strength": 25}))
            .unwrap();
        exec.execute("denoise_audio", &json!({"clipIds": [id]})).unwrap();
        assert_eq!(denoise_amount_of(&exec, &id), Some(0.25), "custom amount kept");
    }

    #[test]
    fn denoise_audio_disable_removes_effect() {
        let (mut exec, id) = denoise_exec();
        exec.execute("denoise_audio", &json!({"clipIds": [id]})).unwrap();
        assert_eq!(denoise_amount_of(&exec, &id), Some(0.6), "default 60%");
        exec.execute("denoise_audio", &json!({"clipIds": [id], "enabled": false}))
            .unwrap();
        assert_eq!(denoise_amount_of(&exec, &id), None);
        let clip = exec.timeline().tracks[0].clips.iter().find(|c| c.id == id).unwrap();
        assert!(clip.effects.is_none(), "empty stack collapses to None (Swift parity)");
    }

    #[test]
    fn denoise_audio_rejects_non_audio_and_bad_strength() {
        let mut manifest = MediaManifest::default();
        manifest.entries.push(video_media("v1", 1920, 1080, 30.0));
        let mut exec = ToolExecutor::new(Timeline::default(), manifest);
        exec.execute("add_clips", &json!({"entries": [{"mediaRef": "v1", "startFrame": 0}]})).unwrap();
        let vid = exec.timeline().tracks[0].clips[0].id.clone();
        let err = exec
            .execute("denoise_audio", &json!({"clipIds": [vid]}))
            .unwrap_err();
        assert!(err.contains("needs an audio clip"), "{err}");
        let err2 = exec
            .execute("denoise_audio", &json!({"clipIds": [vid], "strength": 150}))
            .unwrap_err();
        assert!(err2.contains("0–100"), "{err2}");
        let err3 = exec.execute("denoise_audio", &json!({"clipIds": []})).unwrap_err();
        assert!(err3.contains("empty"), "{err3}");
    }

    #[test]
    fn timeline_tools_create_switch_duplicate_round_trip() {
        let mut exec = make_executor();
        exec.timeline_mut().fps = 24;
        let original_id = exec.timeline().id.clone();

        // create_timeline inherits settings, switches, and returns the v2
        // payload {timelineId, name, active, note}.
        let res = exec
            .execute("create_timeline", &json!({"name": "Cutdown"}))
            .unwrap();
        let payload: serde_json::Value =
            serde_json::from_str(res["content"][0]["text"].as_str().unwrap()).unwrap();
        assert_eq!(payload["name"], json!("Cutdown"));
        assert_eq!(payload["active"], json!(true));
        let tid_short = payload["timelineId"].as_str().unwrap();
        assert!(exec.timeline().id.starts_with(tid_short), "short timelineId (C-3)");
        assert!(payload["note"].as_str().unwrap().contains("empty"), "{payload}");
        assert_eq!(exec.timeline().name, "Cutdown");
        assert_eq!(exec.timeline().fps, 24, "settings inherited");
        assert!(exec.timeline().tracks.is_empty(), "new timeline is empty");
        assert_eq!(exec.sibling_timelines().len(), 1);
        let new_id = exec.timeline().id.clone();

        // get_timeline lists both, flagging the active one.
        let gt = exec.execute("get_timeline", &json!({})).unwrap();
        let tj: serde_json::Value =
            serde_json::from_str(gt["content"][0]["text"].as_str().unwrap()).unwrap();
        let list = tj["timelines"].as_array().expect("timelines listed when >1");
        assert_eq!(list.len(), 2);
        assert!(new_id.starts_with(list[0]["timelineId"].as_str().unwrap()), "short id (C-3)");
        assert_eq!(list[0]["active"], json!(true));

        // set_active_timeline switches back; already-active early-exits.
        let res = exec
            .execute("set_active_timeline", &json!({"timelineId": original_id}))
            .unwrap();
        assert!(res["content"][0]["text"].as_str().unwrap().contains("Active timeline"));
        assert_eq!(exec.timeline().id, original_id);
        let res = exec
            .execute("set_active_timeline", &json!({"timelineId": original_id}))
            .unwrap();
        assert!(res["content"][0]["text"].as_str().unwrap().contains("already"));
        let err = exec
            .execute("set_active_timeline", &json!({"timelineId": "ghost"}))
            .unwrap_err();
        assert!(err.contains("No timeline"), "{err}");

        // create_timeline 'from' (absorbed duplicate_timeline): fresh ids,
        // switches to the copy, note flags the all-new ids.
        let mut manifest = MediaManifest::default();
        manifest.entries.push(video_media("m1", 1920, 1080, 24.0));
        exec.media_manifest_mut().entries = manifest.entries;
        exec.execute("add_clips", &json!({"entries": [{"mediaRef": "m1", "startFrame": 0}]})).unwrap();
        let src_clip_id = exec.timeline().tracks[0].clips[0].id.clone();
        let res = exec
            .execute("create_timeline", &json!({"from": original_id}))
            .unwrap();
        let payload: serde_json::Value =
            serde_json::from_str(res["content"][0]["text"].as_str().unwrap()).unwrap();
        assert!(payload["name"].as_str().unwrap().contains("copy"), "{payload}");
        assert!(
            payload["note"].as_str().unwrap().contains("re-read get_timeline"),
            "{payload}"
        );
        assert_ne!(exec.timeline().id, original_id, "switched to the copy");
        assert_eq!(exec.timeline().tracks[0].clips.len(), 1, "content copied");
        assert_ne!(
            exec.timeline().tracks[0].clips[0].id, src_clip_id,
            "clip ids are new in the copy"
        );
        assert_eq!(exec.sibling_timelines().len(), 2);
    }

    #[test]
    fn add_clips_nests_a_timeline_by_id() {
        let mut manifest = MediaManifest::default();
        manifest.entries.push(video_media("m1", 1920, 1080, 30.0));
        manifest.entries.push(audio_media("a1", 2.0));
        let mut exec = ToolExecutor::new(Timeline::default(), manifest);

        // Build a child timeline with video+audio, then switch back.
        let root_id = exec.timeline().id.clone();
        exec.execute("create_timeline", &json!({"name": "Insert"})).unwrap();
        let child_id = exec.timeline().id.clone();
        exec.execute("add_clips", &json!({"entries": [{"mediaRef": "m1", "startFrame": 0}, {"mediaRef": "a1", "startFrame": 0}]})).unwrap();
        exec.execute("set_active_timeline", &json!({"timelineId": root_id})).unwrap();

        // Nest it by mediaRef = timelineId: a sequence carrier + linked audio carrier.
        exec.execute("add_clips", &json!({"entries": [{"mediaRef": child_id, "startFrame": 0}]})).unwrap();
        let all: Vec<Clip> = exec
            .timeline()
            .tracks
            .iter()
            .flat_map(|t| t.clips.clone())
            .collect();
        assert_eq!(all.len(), 2, "video carrier + linked audio carrier");
        let video = all.iter().find(|c| c.media_type == ClipType::Sequence).unwrap();
        let audio = all.iter().find(|c| c.media_type == ClipType::Audio).unwrap();
        assert_eq!(video.media_ref, child_id);
        assert_eq!(audio.source_clip_type, ClipType::Sequence);
        assert_eq!(video.link_group_id, audio.link_group_id);
        assert!(video.link_group_id.is_some(), "A/V carriers linked");
        assert!(video.duration_frames > 0, "defaults to the child's length");

        // An empty timeline is rejected.
        exec.execute("create_timeline", &json!({"name": "Empty"})).unwrap();
        let empty_id = exec.timeline().id.clone();
        exec.execute("set_active_timeline", &json!({"timelineId": root_id})).unwrap();
        let err = exec
            .execute("add_clips", &json!({"entries": [{"mediaRef": empty_id, "startFrame": 0}]}))
            .unwrap_err();
        assert!(err.contains("empty"), "{err}");

        // Cycle rejection: the root now carries the child, so nesting the ROOT
        // inside the child (root reaches child... child would reach root) refuses.
        exec.execute("set_active_timeline", &json!({"timelineId": child_id})).unwrap();
        let err = exec
            .execute("add_clips", &json!({"entries": [{"mediaRef": root_id, "startFrame": 0}]}))
            .unwrap_err();
        assert!(err.contains("cycle"), "{err}");
    }

    #[test]
    fn insert_clips_nests_a_timeline_by_id() {
        let mut manifest = MediaManifest::default();
        manifest.entries.push(video_media("m1", 1920, 1080, 30.0));
        manifest.entries.push(audio_media("a1", 2.0));
        let mut exec = ToolExecutor::new(Timeline::default(), manifest);

        // Root gets one video clip; the child gets video+audio.
        let root_id = exec.timeline().id.clone();
        exec.execute("add_clips", &json!({"entries": [{"mediaRef": "m1", "startFrame": 0}]})).unwrap();
        let root_clip_start_before = exec.timeline().tracks[0].clips[0].start_frame;
        exec.execute("create_timeline", &json!({"name": "Chunk"})).unwrap();
        let child_id = exec.timeline().id.clone();
        exec.execute("add_clips", &json!({"entries": [{"mediaRef": "m1", "startFrame": 0}, {"mediaRef": "a1", "startFrame": 0}]})).unwrap();
        let child_total =
            timeline_core::TimelineMathExt::total_frames(exec.timeline());
        exec.execute("set_active_timeline", &json!({"timelineId": root_id})).unwrap();

        // Splice the child in at frame 0 on the video track: the existing clip
        // ripples right and a linked A/V carrier pair lands at 0.
        exec.execute(
            "insert_clips",
            &json!({"entries": [{"mediaRef": child_id}], "trackIndex": 0, "atFrame": 0}),
        )
        .unwrap();
        let all: Vec<Clip> = exec
            .timeline()
            .tracks
            .iter()
            .flat_map(|t| t.clips.clone())
            .collect();
        let video = all
            .iter()
            .find(|c| c.media_type == ClipType::Sequence)
            .expect("video carrier placed");
        assert_eq!(video.media_ref, child_id);
        assert_eq!(video.start_frame, 0);
        assert_eq!(video.duration_frames, child_total);
        let audio = all
            .iter()
            .find(|c| c.media_type == ClipType::Audio && c.source_clip_type == ClipType::Sequence)
            .expect("linked audio carrier placed with sequence source");
        assert_eq!(audio.media_ref, child_id);
        assert_eq!(video.link_group_id, audio.link_group_id);
        assert!(video.link_group_id.is_some());
        let pushed = exec.timeline().tracks[0]
            .clips
            .iter()
            .find(|c| c.media_type == ClipType::Video)
            .unwrap();
        assert_eq!(
            pushed.start_frame,
            root_clip_start_before + child_total,
            "existing clip rippled right by the carrier length"
        );
    }

    #[test]
    fn organize_media_renames_and_deletes_accept_timeline_ids() {
        let mut exec = make_executor();
        let root_id = exec.timeline().id.clone();

        // Rename the active timeline by its id (#255 tab-rename duty lives
        // in organize_media renames now).
        exec.execute(
            "organize_media",
            &json!({"renames": [{"item": root_id, "name": "Main cut"}]}),
        )
        .unwrap();
        assert_eq!(exec.timeline().name, "Main cut");

        // Create a sibling, rename it while it is NOT active, then delete it.
        exec.execute("create_timeline", &json!({"name": "Scrap"})).unwrap();
        let scrap_id = exec.timeline().id.clone();
        exec.execute("set_active_timeline", &json!({"timelineId": root_id})).unwrap();
        exec.execute(
            "organize_media",
            &json!({"renames": [{"item": scrap_id, "name": "Scrap 2"}]}),
        )
        .unwrap();
        assert_eq!(exec.sibling_timelines()[0].name, "Scrap 2");
        exec.execute("organize_media", &json!({"deletes": [scrap_id]}))
            .unwrap();
        assert!(exec.sibling_timelines().is_empty());

        // The last remaining timeline can't be deleted.
        let err = exec
            .execute("organize_media", &json!({"deletes": [root_id.clone()]}))
            .unwrap_err();
        assert!(err.contains("Can't delete every timeline"), "{err}");

        // Deleting the ACTIVE timeline switches to a sibling first and says so.
        exec.execute("create_timeline", &json!({"name": "Other"})).unwrap();
        let other_id = exec.timeline().id.clone();
        let res = exec
            .execute("organize_media", &json!({"deletes": [other_id]}))
            .unwrap();
        let body: serde_json::Value =
            serde_json::from_str(res["content"][0]["text"].as_str().unwrap()).unwrap();
        assert_eq!(body["deleted"]["timelines"], json!(1));
        assert!(
            body["notes"][0]
                .as_str()
                .unwrap()
                .contains("Active timeline changed"),
            "{body}"
        );
        assert_eq!(exec.timeline().id, root_id);
        assert!(exec.sibling_timelines().is_empty());
    }

    #[test]
    fn organize_media_warns_when_deleting_a_nested_timeline() {
        let mut manifest = MediaManifest::default();
        manifest.entries.push(video_media("m1", 1920, 1080, 30.0));
        let mut exec = ToolExecutor::new(Timeline::default(), manifest);
        let root_id = exec.timeline().id.clone();
        exec.execute("create_timeline", &json!({"name": "Insert"})).unwrap();
        let child_id = exec.timeline().id.clone();
        exec.execute("add_clips", &json!({"entries": [{"mediaRef": "m1", "startFrame": 0}]})).unwrap();
        exec.execute("set_active_timeline", &json!({"timelineId": root_id})).unwrap();
        exec.execute("add_clips", &json!({"entries": [{"mediaRef": child_id.clone(), "startFrame": 0}]}))
            .unwrap();

        let res = exec
            .execute("organize_media", &json!({"deletes": [child_id]}))
            .unwrap();
        let body: serde_json::Value =
            serde_json::from_str(res["content"][0]["text"].as_str().unwrap()).unwrap();
        assert!(
            body["warnings"][0]
                .as_str()
                .unwrap()
                .contains("render black"),
            "nest-reference warning present: {body}"
        );
    }

    #[test]
    fn timeline_switch_clears_undo_so_snapshots_never_cross_timelines() {
        let mut manifest = MediaManifest::default();
        manifest.entries.push(video_media("m1", 1920, 1080, 30.0));
        let mut exec = ToolExecutor::new(Timeline::default(), manifest);
        exec.execute("add_clips", &json!({"entries": [{"mediaRef": "m1", "startFrame": 0}]})).unwrap();
        assert!(!exec.undo_stack().is_empty(), "edit recorded");

        // Switching timelines clears the stack: an undo here would otherwise
        // overwrite the NEW active timeline with the OLD one's snapshot.
        exec.execute("create_timeline", &json!({})).unwrap();
        assert!(exec.undo_stack().is_empty(), "cleared on create+switch");
        let err = exec.execute("undo", &json!({})).unwrap();
        let text = err["content"][0]["text"].as_str().unwrap();
        assert!(
            exec.timeline().tracks.is_empty(),
            "undo on the fresh timeline is a no-op, not a cross-timeline restore: {text}"
        );
    }

    #[test]
    fn update_text_merges_fields_and_handles_caption_groups() {
        let mut exec = make_executor();
        exec.execute(
            "add_texts",
            &json!({"texts": [
                {"content": "Hello", "startFrame": 0, "durationFrames": 60,
                 "fontSize": 40.0, "animation": "popIn"},
                {"content": "World", "startFrame": 60, "durationFrames": 60}
            ]}),
        )
        .unwrap();
        let ids: Vec<String> = exec
            .timeline()
            .tracks
            .iter()
            .flat_map(|t| t.clips.iter().map(|c| c.id.clone()))
            .collect();
        assert_eq!(ids.len(), 2);
        // Tag the second clip as part of a caption group.
        {
            let loc = timeline_core::find_clip(exec.timeline(), &ids[1]).unwrap();
            exec.timeline_mut().tracks[loc.track_index].clips[loc.clip_index]
                .caption_group_id = Some("cg1".into());
        }

        // Merge semantics: change color + content on clip 0; fontSize stays.
        exec.execute(
            "update_text",
            &json!({"clipIds": [ids[0]], "content": "Hi", "color": "#ff0000"}),
        )
        .unwrap();
        let c0 = exec.timeline().tracks[0]
            .clips
            .iter()
            .find(|c| c.id == ids[0])
            .unwrap()
            .clone();
        assert_eq!(c0.text_content.as_deref(), Some("Hi"));
        let style = c0.text_style.as_ref().unwrap();
        assert!((style.font_size - 40.0).abs() < 1e-9, "unmentioned field kept");
        assert!((style.color.r - 1.0).abs() < 1e-9, "color applied");

        // 'off' clears the animation.
        assert!(c0.text_animation.is_some());
        exec.execute("update_text", &json!({"clipIds": [ids[0]], "animation": "off"}))
            .unwrap();
        let c0 = exec.timeline().tracks[0]
            .clips
            .iter()
            .find(|c| c.id == ids[0])
            .unwrap()
            .clone();
        assert!(c0.text_animation.is_none(), "animation cleared");

        // captionGroupId addressing.
        exec.execute(
            "update_text",
            &json!({"captionGroupId": "cg1", "fontName": "Anton"}),
        )
        .unwrap();
        let c1 = exec.timeline().tracks[0]
            .clips
            .iter()
            .find(|c| c.id == ids[1])
            .unwrap()
            .clone();
        assert_eq!(c1.text_style.as_ref().unwrap().font_name, "Anton");

        // Non-text targets refuse; missing addressing refuses.
        let err = exec.execute("update_text", &json!({})).unwrap_err();
        assert!(err.contains("clipIds"), "{err}");
        let err = exec
            .execute("update_text", &json!({"captionGroupId": "nope"}))
            .unwrap_err();
        assert!(err.contains("No caption clips"), "{err}");
    }

    struct MockExportHost;
    impl ExportHost for MockExportHost {
        fn export(&self, request: ExportRequest) -> Result<ExportOutcome, String> {
            Ok(ExportOutcome::Completed {
                path: format!("/mock/{}.{}", request.timeline.name, request.mode),
            })
        }
    }

    #[test]
    fn export_project_validates_and_delegates_to_the_host() {
        let mut exec = make_executor();
        // Unavailable without a host.
        let err = exec.execute("export_project", &json!({})).unwrap_err();
        assert!(err.contains("unavailable"), "{err}");

        exec.set_export_host(std::sync::Arc::new(MockExportHost));
        // Bad enum values reject.
        for (k, v) in [
            ("mode", "avi"),
            ("codec", "VP9"),
            ("resolution", "8K"),
            ("fcpxmlTarget", "premiere"),
        ] {
            let err = exec.execute("export_project", &json!({ k: v })).unwrap_err();
            assert!(err.contains("unknown"), "{k}: {err}");
        }
        // palmier + timelineId refuses (Swift parity).
        let tl_id = exec.timeline().id.clone();
        let err = exec
            .execute(
                "export_project",
                &json!({"mode": "palmier", "timelineId": tl_id}),
            )
            .unwrap_err();
        assert!(err.contains("palmier"), "{err}");

        // Happy path reports the host's outcome + timeline stats.
        let res = exec
            .execute("export_project", &json!({"mode": "xml"}))
            .unwrap();
        let v: serde_json::Value =
            serde_json::from_str(res["content"][0]["text"].as_str().unwrap()).unwrap();
        assert_eq!(v["status"], "completed");
        assert!(v["path"].as_str().unwrap().ends_with(".xml"));
        assert!(v["timeline"].is_string());

        // timelineId resolves a sibling.
        exec.execute("create_timeline", &json!({"name": "Alt"})).unwrap();
        let alt = exec.timeline().id.clone();
        exec.execute("set_active_timeline", &json!({"timelineId": exec.sibling_timelines()[0].id.clone()}))
            .unwrap();
        let res = exec
            .execute("export_project", &json!({"mode": "fcpxml", "timelineId": alt}))
            .unwrap();
        let v: serde_json::Value =
            serde_json::from_str(res["content"][0]["text"].as_str().unwrap()).unwrap();
        assert_eq!(v["timeline"], "Alt", "sibling exported by id");
        let err = exec
            .execute("export_project", &json!({"timelineId": "ghost"}))
            .unwrap_err();
        assert!(err.contains("no timeline"), "{err}");
    }

    #[test]
    fn dispatched_tools_are_advertised_or_documented_internal() {
        // Inverse of every_advertised_tool_is_dispatched: scan the dispatch
        // match's string arms so a tool can't quietly exist without being
        // advertised. `redo` and `move_clips_linked` are deliberate UI-internal
        // tools (app_root's history buttons / linked-move gesture) and are NOT
        // part of the agent/MCP surface.
        const INTERNAL: &[&str] = &["redo", "move_clips_linked"];
        let source = include_str!("tool_exec.rs");
        let advertised: std::collections::HashSet<&str> =
            crate::all_tools().iter().map(|t| t.name).collect();
        let mut unknown = Vec::new();
        for line in source.lines() {
            let trimmed = line.trim_start();
            let Some(rest) = trimmed.strip_prefix('"') else {
                continue;
            };
            let Some(end) = rest.find('"') else { continue };
            let name = &rest[..end];
            let after = rest[end + 1..].trim_start();
            // Only tool-dispatch arms (`"name" => self.cmd_..` / `=> self.exec_mut(..)`).
            // Heuristic: requires `=> self.` on the same line, so non-tool string
            // matches ("opacity", "audio", ...) are skipped; a future MULTI-LINE
            // unadvertised arm would evade this - keep new arms single-line.
            if !after.starts_with("=> self.") {
                continue;
            }
            if name.chars().all(|c| c.is_ascii_lowercase() || c == '_')
                && !advertised.contains(name)
                && !INTERNAL.contains(&name)
                && !unknown.contains(&name.to_string())
            {
                unknown.push(name.to_string());
            }
        }
        assert!(
            unknown.is_empty(),
            "dispatched but neither advertised nor documented internal: {unknown:?}"
        );
    }

    #[test]
    fn remove_silence_no_args_sweeps_the_whole_timeline() {
        // Upstream #261 semantics: no arguments, adaptive threshold, payload
        // with sectionsRemoved/removedFrames/note.
        let mut manifest = MediaManifest::default();
        manifest.entries.push(audio_media("a1", 4.0));
        let mut exec = ToolExecutor::new(Timeline::default(), manifest);
        exec.set_audio_source(std::sync::Arc::new(MockAudio));
        exec.execute("add_clips", &json!({"entries": [{"mediaRef": "a1", "startFrame": 0}]})).unwrap();

        let res = exec.execute("remove_silence", &json!({})).unwrap();
        let v: serde_json::Value =
            serde_json::from_str(res["content"][0]["text"].as_str().unwrap()).unwrap();
        assert_eq!(v["sectionsRemoved"], 1, "{v}");
        assert!(v["removedFrames"].as_i64().unwrap() > 0);
        assert!(v["notes"][0].as_str().unwrap().contains("re-read"));

        // All-loud timeline: the no-arg form reports dead-air absence as an
        // error (upstream throws), unlike the clip-scoped zero payload.
        let mut manifest = MediaManifest::default();
        manifest.entries.push(audio_media("a1", 4.0));
        let mut exec = ToolExecutor::new(Timeline::default(), manifest);
        exec.set_audio_source(std::sync::Arc::new(MockLoudAudio));
        exec.execute("add_clips", &json!({"entries": [{"mediaRef": "a1", "startFrame": 0}]})).unwrap();
        let err = exec.execute("remove_silence", &json!({})).unwrap_err();
        assert!(err.contains("No dead air"), "{err}");
    }

    #[test]
    fn remove_silence_multi_track_sync_locked_no_stale_cuts() {
        // Adversarial-review regression: with two sync-locked audio tracks the
        // first track's ripple also cuts+shifts the follower, so the follower's
        // pass must re-detect against the CURRENT state — applying ranges
        // pre-computed before any edit would cut the follower's shifted
        // content at stale positions.
        let mut manifest = MediaManifest::default();
        manifest.entries.push(audio_media("a1", 4.0));
        let mut exec = ToolExecutor::new(Timeline::default(), manifest);
        exec.set_audio_source(std::sync::Arc::new(MockAudio));
        for ti in 0..2usize {
            let _ = timeline_core::insert_track_at(exec.timeline_mut(), ti, ClipType::Audio);
            exec.timeline_mut().tracks[ti].sync_locked = true;
            let mut c = crate::test_helpers::make_clip(0, 120);
            c.media_ref = "a1".into();
            c.media_type = ClipType::Audio;
            c.source_clip_type = ClipType::Audio;
            let _ = timeline_core::place_clips(exec.timeline_mut(), ti, 0, &[c]);
        }

        let res = exec.execute("remove_silence", &json!({})).unwrap();
        let v: serde_json::Value =
            serde_json::from_str(res["content"][0]["text"].as_str().unwrap()).unwrap();
        // ~2s of silence minus 0.1s edge padding each side ≈ 54 frames at
        // 30fps, removed ONCE — the follower re-detects nothing after the
        // synced cut (padding remnants are under minSilenceSeconds).
        let removed = v["removedFrames"].as_i64().unwrap();
        assert!((50..=58).contains(&removed), "{v}");
        assert_eq!(v["sectionsRemoved"], 1, "no stale second pass: {v}");
        let (s0, s1) = (track_spans(&exec, 0), track_spans(&exec, 1));
        assert_eq!(s0, s1, "tracks stay in sync");
        assert_eq!(s0.len(), 2, "head + slid tail: {s0:?}");
        assert_eq!(s0[0].0, 0);
        assert_eq!(s0[1].1, 120 - removed, "total shrank by exactly one cut");
    }

    #[test]
    fn import_media_matte_requires_writer() {
        let mut exec = make_executor(); // no writer set
        let err = exec
            .execute("import_media", &json!({"source": {"matte": {"hex": "#000000"}}}))
            .unwrap_err();
        assert!(err.contains("unavailable"), "{err}");
    }

    #[test]
    fn import_media_matte_validates_hex_and_aspect() {
        let mut exec = make_executor();
        exec.set_matte_writer(std::sync::Arc::new(MockMatte::default()));
        assert!(
            exec.execute("import_media", &json!({"source": {"matte": {}}}))
                .is_err(),
            "no hex"
        );
        assert!(
            exec.execute("import_media", &json!({"source": {"matte": {"hex": "notacolor"}}}))
                .is_err(),
            "bad hex"
        );
        assert!(
            exec.execute(
                "import_media",
                &json!({"source": {"matte": {"hex": "#000", "aspectRatio": "bogus"}}})
            )
            .is_err(),
            "bad aspect"
        );
    }

    #[test]
    fn exec_057_apply_effect_remove() {
        let mut exec = make_executor();
        let _ = timeline_core::insert_track_at(exec.timeline_mut(), 0, ClipType::Video);
        let clip = crate::test_helpers::make_clip(0, 150);
        let placed = timeline_core::place_clips(exec.timeline_mut(), 0, 0, &[clip]);
        let clip_id = placed.first().expect("place_clips returned empty");
        // Apply an effect first
        let _ = exec.execute(
            "apply_effect",
            &json!({"clipId": clip_id, "effectType": "blur"}),
        );
        // Then remove it
        let result = exec
            .execute(
                "apply_effect",
                &json!({"clipId": clip_id, "effectType": "blur", "remove": true}),
            )
            .unwrap();
        let env: serde_json::Value =
            serde_json::from_str(result["content"][0]["text"].as_str().unwrap()).unwrap();
        assert!(
            env["clips"][0].get("effects").is_none(),
            "resulting clip state has no effects: {env}"
        );
    }

    // ── Missing-media helpers (#135) ────────────────────────────────

    #[test]
    fn exec_058_missing_entry_ids_none_missing() {
        let exec = make_executor_with_media();
        let offline = exec.media_offline_ids(chrono::Utc::now(), |_| false);
        assert!(offline.is_empty(), "no entries should be missing");
    }

    #[test]
    fn exec_059_missing_entry_ids_all_missing() {
        let exec = make_executor_with_media();
        // The helper adds one entry with no cached_remote_url.
        let offline = exec.media_offline_ids(chrono::Utc::now(), |_| true);
        assert_eq!(offline.len(), 1, "the one entry should be missing");
    }

    #[test]
    fn exec_060_is_media_offline_true() {
        let exec = make_executor_with_media();
        let id = exec.media_manifest.entries[0].id.clone();
        assert!(exec.is_media_offline(&id, chrono::Utc::now(), |_| true));
    }

    #[test]
    fn exec_061_is_media_offline_false() {
        let exec = make_executor_with_media();
        let id = exec.media_manifest.entries[0].id.clone();
        assert!(!exec.is_media_offline(&id, chrono::Utc::now(), |_| false));
    }

    #[test]
    fn exec_062_is_media_offline_unknown_ref() {
        let exec = make_executor_with_media();
        assert!(!exec.is_media_offline("unknown", chrono::Utc::now(), |_| true));
    }

    #[test]
    fn exec_063_is_media_offline_cached_excluded() {
        let mut exec = make_executor();
        exec.media_manifest
            .entries
            .push(core_model::MediaManifestEntry {
                id: "cached".into(),
                name: "cached".into(),
                r#type: core_model::ClipType::Video,
                source: core_model::MediaSource::External {
                    absolute_path: "/tmp/cached.mp4".into(),
                },
                duration: 10.0,
                generation_input: None,
                source_width: None,
                source_height: None,
                source_fps: None,
                has_audio: None,
                folder_id: None,
                cached_remote_url: Some("https://c".into()),
                cached_remote_url_expires_at: None,
                source_timecode_frame: None,
                source_timecode_quanta: None,
                source_timecode_drop_frame: None,
                ai_tags: None,
                ai_description: None,
                ai_label_status: None,
                generation_status: None,
            });
        assert!(
            !exec.is_media_offline("cached", chrono::Utc::now(), |_| true),
            "cached entries should not be offline"
        );
    }

    #[test]
    fn is_media_offline_expired_cache_is_offline() {
        // An EXPIRED cached URL no longer hides an offline asset (the `now` clock
        // threads through the helper into MediaManifestEntry::cache_is_fresh).
        let mut exec = make_executor();
        let past = chrono::Utc::now() - chrono::Duration::hours(1);
        exec.media_manifest
            .entries
            .push(core_model::MediaManifestEntry {
                id: "stale".into(),
                name: "stale".into(),
                r#type: core_model::ClipType::Video,
                source: core_model::MediaSource::External {
                    absolute_path: "/tmp/stale.mp4".into(),
                },
                duration: 10.0,
                generation_input: None,
                source_width: None,
                source_height: None,
                source_fps: None,
                has_audio: None,
                folder_id: None,
                cached_remote_url: Some("https://c".into()),
                cached_remote_url_expires_at: Some(past),
                source_timecode_frame: None,
                source_timecode_quanta: None,
                source_timecode_drop_frame: None,
                ai_tags: None,
                ai_description: None,
                ai_label_status: None,
                generation_status: None,
            });
        assert!(exec.is_media_offline("stale", chrono::Utc::now(), |_| true));
    }

    #[test]
    fn exec_064_is_media_unprocessable_true() {
        let exec = make_executor_with_media();
        let id = exec.media_manifest.entries[0].id.clone();
        // File exists (not missing) but is unprocessable.
        assert!(exec.is_media_unprocessable(&id, chrono::Utc::now(), |_| false, |_| true));
    }

    #[test]
    fn exec_065_is_media_unprocessable_missing_not_unprocessable() {
        let exec = make_executor_with_media();
        let id = exec.media_manifest.entries[0].id.clone();
        // If file is missing, it's offline, not unprocessable.
        assert!(!exec.is_media_unprocessable(&id, chrono::Utc::now(), |_| true, |_| true));
    }

    // ── Revision counter (shared-editor-state) ─────────────────────────

    #[test]
    fn revision_unchanged_by_read_only_tool() {
        let mut exec = ToolExecutor::new(Timeline::default(), MediaManifest::default());
        assert_eq!(exec.revision(), 0);
        exec.execute("get_timeline", &json!({})).unwrap();
        assert_eq!(exec.revision(), 0);
    }

    #[test]
    fn revision_bumped_by_successful_mutation() {
        let mut exec = ToolExecutor::new(Timeline::default(), MediaManifest::default());
        exec.execute("organize_media", &json!({"createFolders": ["B-roll"]}))
            .unwrap();
        assert_eq!(exec.revision(), 1);
    }

    #[test]
    fn revision_unchanged_by_failed_mutation() {
        let mut exec = ToolExecutor::new(Timeline::default(), MediaManifest::default());
        exec.execute("organize_media", &json!({"createFolders": ["B-roll"]}))
            .unwrap();
        assert!(exec.execute("split_clips", &json!({})).is_err());
        assert_eq!(exec.revision(), 1);
    }

    #[test]
    fn load_project_replaces_state_and_bumps_revision() {
        let mut exec = ToolExecutor::new(Timeline::default(), MediaManifest::default());
        exec.execute("organize_media", &json!({"createFolders": ["B-roll"]}))
            .unwrap();
        let before = exec.revision();
        let timeline = Timeline {
            id: String::new(),
            name: String::new(),
            fps: 60,
            ..Default::default()
        };
        exec.load_project(timeline, MediaManifest::default());
        assert_eq!(exec.revision(), before + 1);
        assert_eq!(exec.timeline().fps, 60);
        assert!(exec.media_manifest().folders.is_empty());
        assert!(exec.undo_stack().is_empty());
    }

    #[test]
    fn adopt_timeline_switches_active_and_keeps_prev_as_sibling() {
        let original = Timeline {
            id: "root".into(),
            name: "Root".into(),
            fps: 30,
            ..Default::default()
        };
        let mut exec = ToolExecutor::new(original, MediaManifest::default());
        exec.execute("organize_media", &json!({"createFolders": ["B-roll"]}))
            .unwrap();
        let before = exec.revision();

        // Imported timeline with no id gets a fresh one; it becomes active.
        let imported = Timeline {
            id: String::new(),
            name: "Imported".into(),
            fps: 24,
            ..Default::default()
        };
        let id = exec.adopt_timeline(imported);

        assert!(!id.is_empty(), "adopt assigns an id");
        assert_eq!(exec.timeline().id, id);
        assert_eq!(exec.timeline().name, "Imported");
        assert_eq!(exec.timeline().fps, 24);
        // Old active is preserved as a sibling — import never drops open work.
        assert_eq!(exec.sibling_timelines().len(), 1);
        assert_eq!(exec.sibling_timelines()[0].id, "root");
        assert_eq!(exec.revision(), before + 1);
        assert!(exec.undo_stack().is_empty());
    }

    #[test]
    fn availability_flags_reflect_installed_seams() {
        let mut exec = ToolExecutor::new(Timeline::default(), MediaManifest::default());
        // Nothing installed on the pure/MCP path.
        assert!(!exec.is_generation_available());
        assert!(!exec.is_transcription_available());
        exec.set_generation_backend(std::sync::Arc::new(MockGenerationBackend));
        assert!(exec.is_generation_available());
        assert!(!exec.is_transcription_available());
    }

    // ── send_feedback (#152: seam + session dedup + cap) ───────────────

    #[derive(Default)]
    struct MockFeedbackSender {
        sent: std::sync::Mutex<Vec<FeedbackPayload>>,
    }

    impl FeedbackSender for MockFeedbackSender {
        fn send(&self, payload: &FeedbackPayload) -> Result<(), String> {
            self.sent.lock().unwrap().push(payload.clone());
            Ok(())
        }
    }

    struct FailingFeedbackSender;
    impl FeedbackSender for FailingFeedbackSender {
        fn send(&self, _: &FeedbackPayload) -> Result<(), String> {
            Err("backend offline".into())
        }
    }

    #[test]
    fn send_feedback_without_sender_points_to_github_issues() {
        // No feedback backend is run — the tool succeeds and directs the user
        // to Fronda's GitHub issues rather than failing.
        let mut exec = ToolExecutor::new(Timeline::default(), MediaManifest::default());
        let res = exec
            .execute("send_feedback", &json!({"message": "The preview flickers"}))
            .unwrap();
        let text = res["content"][0]["text"].as_str().unwrap();
        assert!(text.contains(FEEDBACK_ISSUES_URL), "{text}");
        // Guidance is idempotent — it does not consume dedup/cap budget.
        let again = exec
            .execute("send_feedback", &json!({"message": "The preview flickers"}))
            .unwrap();
        assert!(again["content"][0]["text"]
            .as_str()
            .unwrap()
            .contains(FEEDBACK_ISSUES_URL));
    }

    #[test]
    fn send_feedback_requires_a_message() {
        let sender = std::sync::Arc::new(MockFeedbackSender::default());
        let mut exec = ToolExecutor::new(Timeline::default(), MediaManifest::default());
        exec.set_feedback_sender(sender.clone());
        assert!(exec.execute("send_feedback", &json!({})).is_err());
        assert!(exec
            .execute("send_feedback", &json!({"message": "   "}))
            .is_err());
        assert!(sender.sent.lock().unwrap().is_empty(), "sender never invoked");
    }

    #[test]
    fn send_feedback_delivers_payload_with_diagnostics() {
        let sender = std::sync::Arc::new(MockFeedbackSender::default());
        let timeline = Timeline {
            name: "Cut A".into(),
            fps: 30,
            ..Default::default()
        };
        let mut exec = ToolExecutor::new(timeline, MediaManifest::default());
        exec.set_feedback_sender(sender.clone());
        let res = exec
            .execute("send_feedback", &json!({"message": "Export dialog loses focus"}))
            .unwrap();
        let text = res["content"][0]["text"].as_str().unwrap();
        assert!(text.contains("Feedback sent"), "{text}");
        let sent = sender.sent.lock().unwrap();
        assert_eq!(sent.len(), 1);
        assert_eq!(sent[0].message, "Export dialog loses focus");
        assert!(!sent[0].app_version.is_empty());
        assert!(sent[0].timeline_summary.contains("Cut A"), "{}", sent[0].timeline_summary);
        assert!(sent[0].timeline_summary.contains("30fps"), "{}", sent[0].timeline_summary);
    }

    #[test]
    fn send_feedback_rejects_duplicate_message() {
        let sender = std::sync::Arc::new(MockFeedbackSender::default());
        let mut exec = ToolExecutor::new(Timeline::default(), MediaManifest::default());
        exec.set_feedback_sender(sender.clone());
        exec.execute("send_feedback", &json!({"message": "same words"}))
            .unwrap();
        let err = exec
            .execute("send_feedback", &json!({"message": "same words"}))
            .unwrap_err();
        assert!(err.contains("already"), "{err}");
        assert_eq!(sender.sent.lock().unwrap().len(), 1, "sender not invoked again");
    }

    #[test]
    fn send_feedback_caps_at_eight_successful_sends() {
        let sender = std::sync::Arc::new(MockFeedbackSender::default());
        let mut exec = ToolExecutor::new(Timeline::default(), MediaManifest::default());
        exec.set_feedback_sender(sender.clone());
        exec.execute("send_feedback", &json!({"message": "note 0"}))
            .unwrap();
        // A rejected duplicate must not consume the session budget.
        exec.execute("send_feedback", &json!({"message": "note 0"}))
            .unwrap_err();
        for i in 1..8 {
            exec.execute("send_feedback", &json!({"message": format!("note {i}")}))
                .unwrap();
        }
        let err = exec
            .execute("send_feedback", &json!({"message": "note 8"}))
            .unwrap_err();
        assert!(err.contains("8"), "{err}");
        assert_eq!(sender.sent.lock().unwrap().len(), 8, "sender not invoked past the cap");
    }

    #[test]
    fn send_feedback_failed_send_not_recorded() {
        // Cap and dedup count successful sends only — a failed send stays retryable.
        let mut exec = ToolExecutor::new(Timeline::default(), MediaManifest::default());
        exec.set_feedback_sender(std::sync::Arc::new(FailingFeedbackSender));
        assert!(exec
            .execute("send_feedback", &json!({"message": "retry me"}))
            .is_err());
        exec.set_feedback_sender(std::sync::Arc::new(MockFeedbackSender::default()));
        exec.execute("send_feedback", &json!({"message": "retry me"}))
            .unwrap();
    }

    // ── get_timeline v2 (tool-surface-v2 C-5) ───────────────────────────────

    fn v2_timeline_exec() -> ToolExecutor {
        let mut manifest = MediaManifest::default();
        manifest.entries.push(media_entry("va", ClipType::Video, true, 10.0));
        let mut exec = ToolExecutor::new(Timeline::default(), manifest);
        let _ = timeline_core::insert_track_at(exec.timeline_mut(), 0, ClipType::Video);
        let _ = timeline_core::insert_track_at(exec.timeline_mut(), 1, ClipType::Audio);
        exec
    }

    fn get_timeline_v2(exec: &mut ToolExecutor, args: Value) -> Value {
        let res = exec.execute("get_timeline", &args).unwrap();
        serde_json::from_str(res["content"][0]["text"].as_str().unwrap()).unwrap()
    }

    #[test]
    fn get_timeline_v2_top_level_and_track_shape() {
        let mut exec = v2_timeline_exec();
        exec.execute(
            "add_clips",
            &json!({"entries": [{"mediaRef": "va", "trackIndex": 0, "startFrame": 0, "endFrame": 90}]}),
        )
        .unwrap();
        let v = get_timeline_v2(&mut exec, json!({}));
        assert_eq!(v["totalFrames"], json!(90));
        assert_eq!(v["durationSeconds"], json!(3.0));
        assert_eq!(v["currentFrame"], json!(0));
        assert_eq!(v["canGenerate"], json!(false), "no account seam = free tier");
        assert!(v.get("window").is_none(), "no window key without a window");
        assert!(v.get("settingsConfigured").is_none(), "settingsConfigured stripped");
        let t0 = &v["tracks"][0];
        assert_eq!(t0["index"], json!(0));
        assert_eq!(t0["label"], json!("V1"));
        assert_eq!(t0["type"], json!("video"));
        assert!(t0.get("muted").is_none(), "default flags stripped");
        assert!(t0.get("id").is_none(), "track id not exposed");
        assert!(t0.get("displayHeight").is_none(), "displayHeight not exposed");
        let clip = &t0["clips"][0];
        assert_eq!(clip["frames"], json!([0, 90]));
        assert!(clip.get("startFrame").is_none());
        assert!(clip.get("durationFrames").is_none());
        // Linked audio partner folded into the video clip; audio track
        // reports the folded count instead of repeating the clip.
        assert!(clip["audio"]["id"].is_string(), "audio partner folded: {clip}");
        assert_eq!(clip["audio"]["track"], json!(1));
        let t1 = &v["tracks"][1];
        assert_eq!(t1["linkedClips"], json!(1));
        assert!(t1.get("clips").is_none(), "folded partner not repeated: {t1}");
    }

    #[test]
    fn get_timeline_v2_gaps_and_window() {
        let mut exec = v2_timeline_exec();
        // Two clips with a [30, 60) hole (no linked audio: image media).
        let mut manifest = MediaManifest::default();
        manifest.entries.push(media_entry("img", ClipType::Image, false, 0.0));
        *exec.media_manifest_mut() = manifest;
        exec.execute(
            "add_clips",
            &json!({"entries": [
                {"mediaRef": "img", "trackIndex": 0, "startFrame": 0, "endFrame": 30},
                {"mediaRef": "img", "trackIndex": 0, "startFrame": 60, "endFrame": 90},
            ]}),
        )
        .unwrap();
        let v = get_timeline_v2(&mut exec, json!({}));
        assert_eq!(v["tracks"][0]["gaps"], json!([[30, 60]]));

        // Window [0, 40): only the first clip is visible; totalClips reports 2.
        let w = get_timeline_v2(&mut exec, json!({"startFrame": 0, "endFrame": 40}));
        assert_eq!(w["window"], json!([0, 40]));
        assert_eq!(w["tracks"][0]["clips"].as_array().unwrap().len(), 1);
        assert_eq!(w["tracks"][0]["totalClips"], json!(2));
    }

    #[test]
    fn get_timeline_v2_caption_groups_fold_and_detail() {
        let mut exec = make_executor();
        let _ = timeline_core::insert_track_at(exec.timeline_mut(), 0, ClipType::Video);
        for i in 0..3i64 {
            let mut c = crate::test_helpers::make_clip(i * 60, 60);
            c.media_type = ClipType::Text;
            c.source_clip_type = ClipType::Text;
            c.caption_group_id = Some("cap-group-1".into());
            c.text_content = Some(format!("word{i}"));
            let _ = timeline_core::place_clips(exec.timeline_mut(), 0, i * 60, &[c]);
        }
        let v = get_timeline_v2(&mut exec, json!({}));
        let group = &v["tracks"][0]["captionGroups"][0];
        assert_eq!(group["clipCount"], json!(3));
        assert_eq!(group["frameRange"], json!([0, 180]));
        assert!(group["textPreview"].as_str().unwrap().contains("word0"));
        assert!(group.get("clips").is_none(), "no rows without captionDetail");
        assert!(
            v["tracks"][0].get("clips").is_none(),
            "caption clips not listed individually: {v}"
        );

        let d = get_timeline_v2(&mut exec, json!({"captionDetail": true}));
        let group = &d["tracks"][0]["captionGroups"][0];
        assert_eq!(
            group["clipFormat"],
            json!(["clipId", "startFrame", "endFrame", "text"])
        );
        assert_eq!(group["clips"].as_array().unwrap().len(), 3);
        assert_eq!(group["clips"][0][3], json!("word0"));
    }

    // ── get_transcript v2 (tool-surface-v2 C-6) ─────────────────────────────

    fn tv2w(index: usize, clip_id: &str, start: i64, end: i64, text: &str) -> timeline_core::TimelineWord {
        timeline_core::TimelineWord {
            index,
            clip_id: clip_id.into(),
            track_index: 0,
            clip_start_frame: 0,
            clip_end_frame: 900,
            text: text.into(),
            start_frame: start,
            end_frame: end,
        }
    }

    #[test]
    fn get_transcript_v2_segments_flush_on_punctuation_and_gap() {
        let mut exec = make_executor();
        let _ = timeline_core::insert_track_at(exec.timeline_mut(), 0, ClipType::Audio);
        exec.set_timeline_words(vec![
            tv2w(0, "clip-x", 0, 10, "Hello"),
            tv2w(1, "clip-x", 12, 20, "there."),
            // > 1s gap (fps 30) after "there." → next segment
            tv2w(2, "clip-x", 100, 110, "Second"),
            tv2w(3, "clip-x", 112, 120, "part"),
        ]);
        let res = exec
            .execute("get_transcript", &json!({"granularity": "segments"}))
            .unwrap();
        let v: Value = serde_json::from_str(res["content"][0]["text"].as_str().unwrap()).unwrap();
        assert_eq!(
            v["segmentFormat"],
            json!(["firstWordIndex", "text", "start", "end"])
        );
        let segs = v["clips"][0]["segments"].as_array().unwrap();
        assert_eq!(segs.len(), 2, "punctuation flush splits: {v}");
        assert_eq!(segs[0][0], json!(0));
        assert_eq!(segs[0][1], json!("Hello there."));
        assert_eq!(segs[0][2], json!(0));
        assert_eq!(segs[1][0], json!(2));
        assert_eq!(segs[1][1], json!("Second part"));
    }

    #[test]
    fn get_transcript_v2_caps_at_10000_words_with_paging() {
        let mut exec = make_executor();
        let _ = timeline_core::insert_track_at(exec.timeline_mut(), 0, ClipType::Audio);
        let words: Vec<timeline_core::TimelineWord> = (0..10_005)
            .map(|i| tv2w(i, "clip-x", i as i64 * 2, i as i64 * 2 + 1, "w"))
            .collect();
        exec.set_timeline_words(words);
        let res = exec.execute("get_transcript", &json!({})).unwrap();
        let v: Value = serde_json::from_str(res["content"][0]["text"].as_str().unwrap()).unwrap();
        assert_eq!(v["totalWords"], json!(10_005));
        assert_eq!(v["nextStartFrame"], json!(20_000), "10001st word's start");
        assert_eq!(
            v["clips"][0]["words"].as_array().unwrap().len(),
            10_000,
            "capped at 10000"
        );
        assert!(v["wordsNote"].as_str().unwrap().contains("First 10000 of 10005"));

        // Page 2: indices stay global.
        let res2 = exec
            .execute("get_transcript", &json!({"startFrame": 20_000}))
            .unwrap();
        let v2: Value = serde_json::from_str(res2["content"][0]["text"].as_str().unwrap()).unwrap();
        let first = &v2["clips"][0]["words"][0];
        assert_eq!(first[0], json!(10_000), "global index preserved on page 2");
    }

    #[test]
    fn get_transcript_v2_clip_scope_keeps_global_indices() {
        let mut exec = make_executor();
        let _ = timeline_core::insert_track_at(exec.timeline_mut(), 0, ClipType::Audio);
        let mut a = crate::test_helpers::make_clip(0, 100);
        a.id = "clip-a".into();
        a.media_type = ClipType::Audio;
        let mut b = crate::test_helpers::make_clip(100, 100);
        b.id = "clip-b".into();
        b.media_type = ClipType::Audio;
        exec.timeline_mut().tracks[0].clips = vec![a, b];
        exec.set_timeline_words(vec![
            tv2w(0, "clip-a", 0, 10, "one"),
            tv2w(1, "clip-b", 100, 110, "two"),
        ]);
        let res = exec
            .execute("get_transcript", &json!({"clipId": "clip-b"}))
            .unwrap();
        let v: Value = serde_json::from_str(res["content"][0]["text"].as_str().unwrap()).unwrap();
        assert_eq!(v["clips"].as_array().unwrap().len(), 1);
        assert_eq!(v["clips"][0]["words"][0], json!([1, "two", 100]));
    }

    // ── detect_beats (tool-surface-v2 NEW) ──────────────────────────────────

    struct MockClickAudio;
    impl ClipAudioSource for MockClickAudio {
        fn decode_source_pcm(
            &self,
            _source: &core_model::MediaSource,
            sample_rate: u32,
            channels: usize,
        ) -> Option<Vec<f32>> {
            // 8s click track at 120 BPM (0.5s spacing), every 4th click louder.
            let n = sample_rate as usize * 8;
            let mut pcm = vec![0.0f32; n * channels];
            let mut k = 0usize;
            let mut t = 0.0f64;
            while t < 8.0 {
                let start = (t * sample_rate as f64) as usize;
                let amp = if k % 4 == 0 { 0.9 } else { 0.4 };
                for i in 0..(sample_rate as usize / 100).min(n.saturating_sub(start)) {
                    for c in 0..channels {
                        pcm[(start + i) * channels + c] =
                            amp * (1.0 - i as f32 / (sample_rate as f32 / 100.0));
                    }
                }
                t += 0.5;
                k += 1;
            }
            Some(pcm)
        }
    }

    #[test]
    fn detect_beats_returns_bpm_and_windowed_beats() {
        let mut manifest = MediaManifest::default();
        manifest.entries.push(audio_media("music", 8.0));
        let mut exec = ToolExecutor::new(Timeline::default(), manifest);
        exec.set_audio_source(std::sync::Arc::new(MockClickAudio));
        let res = exec
            .execute("detect_beats", &json!({"mediaRef": "music"}))
            .unwrap();
        let v: Value = serde_json::from_str(res["content"][0]["text"].as_str().unwrap()).unwrap();
        let bpm = v["bpm"].as_f64().unwrap();
        assert!((bpm - 120.0).abs() < 3.0, "bpm={bpm}");
        assert!(v["beats"].as_array().unwrap().len() >= 14);
        assert!(!v["downbeats"].as_array().unwrap().is_empty());

        // Windowing trims the response (the analysis is cached).
        let win = exec
            .execute(
                "detect_beats",
                &json!({"mediaRef": "music", "startSeconds": 2.0, "endSeconds": 4.0}),
            )
            .unwrap();
        let wv: Value = serde_json::from_str(win["content"][0]["text"].as_str().unwrap()).unwrap();
        for b in wv["beats"].as_array().unwrap() {
            let t = b.as_f64().unwrap();
            assert!((2.0..=4.0).contains(&t), "windowed beat {t}");
        }
    }

    #[test]
    fn detect_beats_unavailable_without_audio_source() {
        let mut manifest = MediaManifest::default();
        manifest.entries.push(audio_media("music", 8.0));
        let mut exec = ToolExecutor::new(Timeline::default(), manifest);
        let err = exec
            .execute("detect_beats", &json!({"mediaRef": "music"}))
            .unwrap_err();
        assert!(err.contains("unavailable"), "{err}");
    }

    #[test]
    fn detect_beats_rejects_non_audio_assets() {
        let mut exec = make_executor_with_media(); // video asset is fine
        let mut manifest = MediaManifest::default();
        manifest.entries.push(media_entry("img", ClipType::Image, false, 0.0));
        *exec.media_manifest_mut() = manifest;
        exec.set_audio_source(std::sync::Arc::new(MockClickAudio));
        let err = exec
            .execute("detect_beats", &json!({"mediaRef": "img"}))
            .unwrap_err();
        assert!(err.contains("audio or video"), "{err}");
    }
}
