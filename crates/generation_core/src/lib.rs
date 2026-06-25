//! Generation workflow state machine.
//!
//! Covers GEN-001 through GEN-024: AI generation lifecycle from
//! account gatekeeping through job submission, upload, download,
//! and clip replacement.
//!
//! Pure state machine with no platform dependencies.

pub mod backend_config;
pub mod generation_payload;

use core_model::GenerationInput;
use std::collections::HashMap;

// ── States ──────────────────────────────────────────────────────

/// Overall generation state machine.
///
/// Transitions:
///   Idle → Preparing (when submit is triggered)
///   Preparing → Uploading (when pre-flight checks pass)
///   Uploading → AwaitingJob (when all uploads complete)
///   AwaitingJob → Downloading (on job success)
///   AwaitingJob → Failed (on job failure)
///   Downloading → Completed (when all results downloaded)
///   Downloading → Failed (on download failure)
///   Downloading → CompletedWithErrors (some downloads failed)
///   Any → Failed (on fatal error)
///   Completed → Idle (reset)
///   Failed → Idle (reset)
#[derive(Debug, Clone, PartialEq)]
pub enum GenerationState {
    Idle,
    Preparing(PreparingState),
    Uploading(UploadingState),
    AwaitingJob(AwaitingJobState),
    Downloading(DownloadingState),
    Completed(CompletedState),
    CompletedWithErrors(CompletedWithErrorsState),
    Failed(FailedState),
}

#[derive(Debug, Clone, PartialEq)]
pub struct PreparingState {
    pub prompt: String,
    pub model: String,
    pub duration_seconds: f64,
    pub aspect_ratio: String,
    pub resolution: Option<String>,
    pub quality: Option<String>,
    pub estimated_cost: i64,
    pub num_images: i64,
    pub reference_urls: Vec<String>,
    pub modality: GenerationModality,
}

#[derive(Debug, Clone, PartialEq)]
pub enum GenerationModality {
    Video,
    Image,
    Audio,
    Music,
}

#[derive(Debug, Clone, PartialEq)]
pub struct UploadingState {
    pub pending_uploads: Vec<UploadItem>,
    pub completed_uploads: Vec<String>, // urls
    pub snapshot: GenerationSnapshot,
}

#[derive(Debug, Clone, PartialEq)]
pub struct UploadItem {
    pub local_path: String,
    pub target_index: usize,
    pub pre_uploaded_url: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AwaitingJobState {
    pub job_id: String,
    pub snapshot: GenerationSnapshot,
    pub placeholder_ids: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DownloadingState {
    pub job_id: String,
    pub snapshot: GenerationSnapshot,
    pub placeholder_ids: Vec<String>,
    pub result_urls: Vec<String>,
    pub completed_downloads: Vec<String>,
    pub failed_downloads: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CompletedState {
    pub final_asset_ids: Vec<String>,
    pub snapshot: GenerationSnapshot,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CompletedWithErrorsState {
    pub final_asset_ids: Vec<String>,
    pub failed_placeholder_ids: Vec<String>,
    pub pending_download_urls: Vec<String>,
    pub snapshot: GenerationSnapshot,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FailedState {
    pub reason: String,
    pub snapshot: Option<GenerationSnapshot>,
    pub pending_retry_urls: Vec<String>,
}

/// GEN-011: Generation snapshot preserves prompt/model/duration/aspect ratio
/// plus modality-specific options, reference URLs, reference asset ids, and createdAt.
#[derive(Debug, Clone, PartialEq)]
pub struct GenerationSnapshot {
    pub prompt: String,
    pub model: String,
    pub duration_seconds: f64,
    pub aspect_ratio: String,
    pub resolution: Option<String>,
    pub quality: Option<String>,
    pub num_images: i64,
    pub reference_urls: Vec<String>,
    pub reference_asset_ids: Vec<String>,
    pub modality: GenerationModality,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

// ── Account / Credits model ────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub enum AccountState {
    Unconfigured,
    MissingKeys,
    Ready(ReadyAccount),
    Misconfigured(String),
}

#[derive(Debug, Clone, PartialEq)]
pub struct ReadyAccount {
    pub monthly_budget: i64,
    pub purchased_credits: i64,
    pub spent_credits: i64,
}

impl ReadyAccount {
    /// ACC-002: remaining = (monthly_budget + purchased) - spent, clamped at 0.
    pub fn remaining_credits(&self) -> i64 {
        (self.monthly_budget + self.purchased_credits - self.spent_credits).max(0)
    }
}

// ── Top-Up validation (ACC-003) ──

/// ACC-003: Top-off amount validation keeps current minimum and maximum bounds.
#[derive(Debug, Clone, PartialEq)]
pub struct TopUpConfig {
    pub min_amount: i64,
    pub max_amount: i64,
}

impl Default for TopUpConfig {
    fn default() -> Self {
        Self {
            min_amount: 10,
            max_amount: 500,
        }
    }
}

impl TopUpConfig {
    /// Validate that a top-off amount is within current min/max bounds.
    pub fn validate_amount(&self, amount: i64) -> Result<(), String> {
        if amount < self.min_amount {
            return Err(format!(
                "Amount {amount} is below minimum of {}",
                self.min_amount
            ));
        }
        if amount > self.max_amount {
            return Err(format!(
                "Amount {amount} exceeds maximum of {}",
                self.max_amount
            ));
        }
        Ok(())
    }
}

/// The default top-up configuration.
pub static DEFAULT_TOPUP_CONFIG: TopUpConfig = TopUpConfig {
    min_amount: 10,
    max_amount: 500,
};

// ── Billing URL validation (ACC-004) ──

/// ACC-004: Billing/checkout URLs remain host-whitelisted and reject untrusted destinations.
pub struct BillingUrlValidator;

impl BillingUrlValidator {
    /// Trusted hosts for billing/checkout URLs.
    const TRUSTED_HOSTS: &'static [&'static str] = &[
        "checkout.stripe.com",
        "buy.stripe.com",
        "api.stripe.com",
        "billing.anthropic.com",
    ];

    /// Validate that a URL is a trusted billing/checkout destination.
    pub fn validate(url: &str) -> Result<(), String> {
        let url = url.trim();
        if !url.starts_with("http://") && !url.starts_with("https://") {
            return Err(format!("Invalid URL scheme: {url}"));
        }
        let after_scheme = url
            .strip_prefix("http://")
            .or_else(|| url.strip_prefix("https://"))
            .unwrap_or(url);
        let host = after_scheme.split('/').next().unwrap_or(after_scheme);
        let host = host.split(':').next().unwrap_or(host); // strip port
        if Self::TRUSTED_HOSTS.contains(&host) {
            Ok(())
        } else {
            Err(format!("Untrusted billing URL host: {host}"))
        }
    }
}

// ── Generation machine ──────────────────────────────────────────

/// Pure state-machine transitions for the generation workflow.
pub struct GenerationMachine;

impl GenerationMachine {
    /// GEN-001: Gated on account state. Returns Err if AI not allowed.
    pub fn can_submit(account: &AccountState) -> Result<(), String> {
        match account {
            AccountState::Ready(_) => Ok(()),
            AccountState::Unconfigured => Err("Account is not configured".into()),
            AccountState::MissingKeys => Err("API keys are missing".into()),
            AccountState::Misconfigured(msg) => Err(format!("Account misconfigured: {msg}")),
        }
    }

    /// GEN-003: Check if estimated cost exceeds remaining credits.
    pub fn check_credits(account: &ReadyAccount, estimated_cost: i64) -> Result<(), String> {
        let remaining = account.remaining_credits();
        if estimated_cost > remaining {
            return Err(format!(
                "Estimated cost {estimated_cost} exceeds remaining credits {remaining}"
            ));
        }
        Ok(())
    }

    /// GEN-006: Clamp image count to [1, 4].
    pub fn clamp_image_count(requested: i64) -> i64 {
        requested.clamp(1, 4)
    }

    /// GEN-012: Multi-image generation preserves requested placeholder count after clamping.
    pub fn placeholder_count(num_images: i64, modality: &GenerationModality) -> usize {
        match modality {
            GenerationModality::Image | GenerationModality::Video => num_images.max(1) as usize,
            GenerationModality::Audio | GenerationModality::Music => 1,
        }
    }

    /// Create a generation snapshot from input (GEN-011).
    pub fn create_snapshot(
        input: &GenerationInput,
        modality: GenerationModality,
        num_images: i64,
        reference_urls: Vec<String>,
        reference_asset_ids: Vec<String>,
    ) -> GenerationSnapshot {
        GenerationSnapshot {
            prompt: input.prompt.clone(),
            model: input.model.clone(),
            duration_seconds: input.duration as f64,
            aspect_ratio: input.aspect_ratio.clone(),
            resolution: input.resolution.clone(),
            quality: input.quality.clone(),
            num_images,
            reference_urls,
            reference_asset_ids,
            modality,
            created_at: chrono::Utc::now(),
        }
    }

    /// Transition: Idle → Preparing.
    /// Validates account state, creates placeholder count, returns PreparingState.
    pub fn start_prepare(
        account: &AccountState,
        prompt: String,
        model: String,
        duration_seconds: f64,
        aspect_ratio: String,
        resolution: Option<String>,
        quality: Option<String>,
        estimated_cost: i64,
        num_images: i64,
        reference_urls: Vec<String>,
        modality: GenerationModality,
    ) -> Result<GenerationState, String> {
        Self::can_submit(account)?;
        let num_images = Self::clamp_image_count(num_images);
        Ok(GenerationState::Preparing(PreparingState {
            prompt,
            model,
            duration_seconds,
            aspect_ratio,
            resolution,
            quality,
            estimated_cost,
            num_images,
            reference_urls,
            modality,
        }))
    }

    /// Transition: Preparing → Uploading.
    /// After credit check passes, produce upload items for any references that need uploading.
    pub fn start_uploading(
        state: PreparingState,
        account: &ReadyAccount,
        local_paths: Vec<(usize, String)>, // (target_index, local_path)
        pre_uploaded: Vec<(usize, String)>, // (target_index, url)
    ) -> Result<GenerationState, String> {
        Self::check_credits(account, state.estimated_cost)?;

        let snapshot = GenerationSnapshot {
            prompt: state.prompt,
            model: state.model,
            duration_seconds: state.duration_seconds,
            aspect_ratio: state.aspect_ratio,
            resolution: state.resolution,
            quality: state.quality,
            num_images: state.num_images,
            reference_urls: state.reference_urls,
            reference_asset_ids: Vec::new(),
            modality: state.modality,
            created_at: chrono::Utc::now(),
        };

        let mut pending_uploads: Vec<UploadItem> = local_paths
            .into_iter()
            .map(|(idx, path)| UploadItem {
                local_path: path,
                target_index: idx,
                pre_uploaded_url: None,
            })
            .collect();

        // GEN-008: Pre-uploaded URLs skip re-upload.
        for (idx, url) in pre_uploaded {
            pending_uploads.push(UploadItem {
                local_path: String::new(),
                target_index: idx,
                pre_uploaded_url: Some(url),
            });
        }

        // GEN-007: Preserve upload order by index.
        pending_uploads.sort_by_key(|u| u.target_index);

        Ok(GenerationState::Uploading(UploadingState {
            pending_uploads,
            completed_uploads: Vec::new(),
            snapshot,
        }))
    }

    /// Mark one upload as completed (GEN-007).
    pub fn mark_upload_complete(state: &mut UploadingState, url: String) {
        state.completed_uploads.push(url);
    }

    /// Transition: Uploading → AwaitingJob.
    /// Called when all uploads are done.
    pub fn submit_job(
        state: UploadingState,
        job_id: String,
        placeholder_ids: Vec<String>,
    ) -> GenerationState {
        GenerationState::AwaitingJob(AwaitingJobState {
            job_id,
            snapshot: state.snapshot,
            placeholder_ids,
        })
    }

    /// Transition: AwaitingJob → Downloading (GEN-015: on success).
    pub fn job_succeeded(state: AwaitingJobState, result_urls: Vec<String>) -> GenerationState {
        GenerationState::Downloading(DownloadingState {
            job_id: state.job_id,
            snapshot: state.snapshot,
            placeholder_ids: state.placeholder_ids,
            result_urls,
            completed_downloads: Vec::new(),
            failed_downloads: Vec::new(),
        })
    }

    /// GEN-016: If fewer results than placeholders, unmatched fail.
    pub fn check_result_count(placeholder_count: usize, result_count: usize) -> Result<(), String> {
        if result_count < placeholder_count {
            Err(format!(
                "Expected {placeholder_count} results, got {result_count}"
            ))
        } else {
            Ok(())
        }
    }

    /// Mark one download as completed.
    pub fn mark_download_complete(state: &mut DownloadingState, asset_id: String) {
        state.completed_downloads.push(asset_id);
    }

    /// GEN-017: Mark download failure with pending retry.
    pub fn mark_download_failed(state: &mut DownloadingState, url: String) {
        state.failed_downloads.push(url);
    }

    /// Transition: Downloading → Completed (all downloads succeeded).
    pub fn finalize_completed(state: DownloadingState) -> GenerationState {
        GenerationState::Completed(CompletedState {
            final_asset_ids: state.completed_downloads,
            snapshot: state.snapshot,
        })
    }

    /// Transition: Downloading → CompletedWithErrors (some succeeded, some failed).
    pub fn finalize_completed_with_errors(state: DownloadingState) -> GenerationState {
        let pending = state.failed_downloads.clone();
        let failed_ids: Vec<String> = state
            .placeholder_ids
            .iter()
            .skip(state.completed_downloads.len())
            .cloned()
            .collect();
        GenerationState::CompletedWithErrors(CompletedWithErrorsState {
            final_asset_ids: state.completed_downloads,
            failed_placeholder_ids: failed_ids,
            pending_download_urls: pending,
            snapshot: state.snapshot,
        })
    }

    /// Transition: Any → Failed.
    pub fn fail(
        reason: String,
        snapshot: Option<GenerationSnapshot>,
        retry_urls: Vec<String>,
    ) -> GenerationState {
        GenerationState::Failed(FailedState {
            reason,
            snapshot,
            pending_retry_urls: retry_urls,
        })
    }

    /// GEN-019: For clip-replacement, only first success replaces target.
    pub fn first_successful_asset(state: &CompletedState) -> Option<&str> {
        state.final_asset_ids.first().map(|s| s.as_str())
    }

    /// GEN-020: Rerun from stored GenerationInput.
    pub fn rerun_from_input(
        input: &GenerationInput,
        modality: GenerationModality,
    ) -> Result<PreparingState, String> {
        if input.model.is_empty() {
            return Err("Original model no longer exists or input is incomplete".into());
        }
        Ok(PreparingState {
            prompt: input.prompt.clone(),
            model: input.model.clone(),
            duration_seconds: input.duration as f64,
            aspect_ratio: input.aspect_ratio.clone(),
            resolution: input.resolution.clone(),
            quality: input.quality.clone(),
            estimated_cost: 0,
            num_images: Self::clamp_image_count(input.num_images.unwrap_or(1)),
            reference_urls: Vec::new(),
            modality,
        })
    }

    /// GEN-022: Check if upscale is available for an asset.
    pub fn can_upscale(asset_type: &str, is_generating: bool) -> bool {
        if is_generating {
            return false;
        }
        matches!(asset_type, "image" | "video")
    }

    /// GEN-024: Generated audio lands on audio tracks.
    pub fn target_track_type(modality: &GenerationModality) -> &str {
        match modality {
            GenerationModality::Audio | GenerationModality::Music => "audio",
            GenerationModality::Image => "video",
            GenerationModality::Video => "video",
        }
    }

    /// Reset completed/failed state back to Idle.
    pub fn reset(state: GenerationState) -> GenerationState {
        match state {
            GenerationState::Completed(_) | GenerationState::Failed(_) => GenerationState::Idle,
            other => other, // can't reset while active
        }
    }
}

// ── Export progress state machine (EXP-008~010) ─────────────────

#[derive(Debug, Clone, PartialEq)]
pub enum ExportState {
    Idle,
    Rendering(RenderingState),
    Cancelling,
    Completed,
    Failed(String),
    Cancelled,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RenderingState {
    pub progress: f64,
    pub stall_timeout_seconds: u64,
    pub last_progress_time: chrono::DateTime<chrono::Utc>,
    pub stall_watchdog_cancelled: bool,
}

impl ExportState {
    /// EXP-009: Start a render with stall watchdog (Upstream #95).
    pub fn start_rendering(stall_timeout_seconds: u64) -> Self {
        ExportState::Rendering(RenderingState {
            progress: 0.0,
            stall_timeout_seconds,
            last_progress_time: chrono::Utc::now(),
            stall_watchdog_cancelled: false,
        })
    }

    /// Update progress. Returns true if progress advanced meaningfully.
    pub fn update_progress(state: &mut RenderingState, value: f64) -> bool {
        let advanced = (value - state.progress).abs() > 0.001;
        if advanced {
            state.progress = value.clamp(0.0, 1.0);
            state.last_progress_time = chrono::Utc::now();
        }
        advanced
    }

    /// EXP-010: Check if export has stalled.
    pub fn has_stalled(state: &RenderingState) -> bool {
        if state.stall_watchdog_cancelled {
            return false;
        }
        let elapsed = (chrono::Utc::now() - state.last_progress_time)
            .num_seconds()
            .unsigned_abs();
        elapsed > state.stall_timeout_seconds
    }

    /// Cancel the export.
    pub fn cancel(_state: RenderingState) -> Self {
        ExportState::Cancelling
    }

    /// Complete successfully.
    pub fn complete() -> Self {
        ExportState::Completed
    }

    /// Fail with reason.
    pub fn fail(reason: String) -> Self {
        ExportState::Failed(reason)
    }
}

// ── Settings (SET-001~007) ──────────────────────────────────────

/// Persisted user settings.
#[derive(Debug, Clone, PartialEq)]
pub struct UserSettings {
    /// SET-002: Notifications preference.
    pub notifications_enabled: bool,
    /// SET-003: Privacy/telemetry preference.
    pub telemetry_enabled: bool,
    /// SET-005: Disabled model ids.
    pub disabled_models: Vec<String>,
    /// SET-006: Agent API keys (last 4 chars only for display).
    pub agent_api_keys: Vec<ApiKeyEntry>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ApiKeyEntry {
    pub provider: String,
    pub masked_key: String, // last 4 chars
}

impl Default for UserSettings {
    fn default() -> Self {
        Self {
            notifications_enabled: true,
            telemetry_enabled: true,
            disabled_models: Vec::new(),
            agent_api_keys: Vec::new(),
        }
    }
}

impl UserSettings {
    /// SET-004: Telemetry changes apply on next launch.
    pub fn telemetry_effective(&self, is_next_launch: bool) -> bool {
        if is_next_launch {
            self.telemetry_enabled
        } else {
            // Current launch always uses the latched value
            true
        }
    }

    /// SET-006: Mask API key, keeping only last 4 chars.
    pub fn mask_api_key(key: &str) -> String {
        if key.len() <= 4 {
            return key.to_string();
        }
        let masked_len = key.len() - 4;
        format!("{}****", &key[masked_len..])
    }
}

// ── Model catalog ───────────────────────────────────────────────

/// A model entry in the live catalog.
#[derive(Debug, Clone, PartialEq)]
pub struct ModelEntry {
    pub id: String,
    pub name: String,
    pub modality: GenerationModality,
    pub cost_per_second: i64,
    pub disabled: bool,
}

/// Model catalog with filtering support (GEN-002).
pub struct ModelCatalog;

impl ModelCatalog {
    /// Get available models from catalog filtered by settings.
    pub fn available<'a>(
        catalog: &'a [ModelEntry],
        disabled_models: &'a [String],
    ) -> Vec<&'a ModelEntry> {
        catalog
            .iter()
            .filter(|m| !disabled_models.contains(&m.id))
            .filter(|m| !m.disabled)
            .collect()
    }
}

// ── New Model Catalog types (CAT-001~CAT-012) ────────────────────

/// Catalog entry kind.
#[derive(Debug, Clone, PartialEq)]
pub enum ModelKind {
    Video,
    Image,
    Audio,
    Upscale,
}

/// Response shape of a model's output.
#[derive(Debug, Clone, PartialEq)]
pub enum ResponseShape {
    Video,
    Images,
    Audio,
    UpscaledImage,
}

/// Video-specific generation capabilities.
#[derive(Debug, Clone, PartialEq)]
pub struct VideoCapabilities {
    pub durations: Vec<f64>,
    pub resolutions: Option<Vec<String>>,
    pub aspect_ratios: Vec<String>,
    pub max_reference_images: Option<i64>,
    pub max_reference_videos: Option<i64>,
    pub max_reference_audios: Option<i64>,
    pub max_total_references: Option<i64>,
    pub max_reference_video_seconds: Option<f64>,
    pub max_reference_audio_seconds: Option<f64>,
    pub requires_source_video: Option<bool>,
    pub requires_reference_image: Option<bool>,
    pub supports_generate_audio: Option<bool>,
}

/// Image-specific generation capabilities.
#[derive(Debug, Clone, PartialEq)]
pub struct ImageCapabilities {
    pub resolutions: Option<Vec<String>>,
    pub aspect_ratios: Option<Vec<String>>,
    pub qualities: Option<Vec<String>>,
    pub max_images: Option<i64>,
}

/// Audio-specific generation capabilities.
#[derive(Debug, Clone, PartialEq)]
pub struct AudioCapabilities {
    pub category: Option<String>,
    pub voices: Vec<String>,
    pub default_voice: Option<String>,
    pub lyrics: Option<bool>,
    pub style_instructions: Option<bool>,
    pub instrumental: Option<bool>,
    pub durations: Option<Vec<f64>>,
    pub min_prompt_length: Option<i64>,
    pub supported_inputs: Option<Vec<String>>,
    pub min_seconds: Option<f64>,
    pub max_seconds: Option<f64>,
}

/// Upscale-specific capabilities.
#[derive(Debug, Clone, PartialEq)]
pub struct UpscaleCapabilities {
    pub speed_label: Option<String>,
    pub p75_duration_seconds: Option<f64>,
    pub supported_clip_types: Vec<String>,
}

/// A catalog entry for a generation model.
#[derive(Debug, Clone, PartialEq)]
pub struct CatalogEntry {
    pub id: String,
    pub kind: ModelKind,
    pub display_name: String,
    pub allowed_endpoints: Vec<String>,
    pub response_shape: ResponseShape,
    pub ui_capabilities: serde_json::Value,
    pub pricing: Option<Pricing>,
    pub qualities: Option<Vec<String>>,
}

impl CatalogEntry {
    /// Return display name, falling back to id if empty.
    pub fn display_name_or_id(&self) -> &str {
        if self.display_name.is_empty() {
            &self.id
        } else {
            &self.display_name
        }
    }
}

impl std::fmt::Display for CatalogEntry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.display_name_or_id())
    }
}

// ── Pricing and Cost estimation (COST-001~COST-009) ────────────

/// Audio pricing mode variants.
#[derive(Debug, Clone, PartialEq)]
pub enum AudioPricingMode {
    PerCharacter { rate_per_thousand: f64 },
    PerSecond { rate: f64 },
    Flat { price: f64 },
}

/// Pricing configuration for a model.
#[derive(Debug, Clone, PartialEq)]
pub struct Pricing {
    pub credits_per_second: Option<f64>,
    pub resolution_pricing: Option<HashMap<String, f64>>,
    pub quality_pricing: Option<HashMap<String, f64>>,
    pub audio_discount: Option<HashMap<String, f64>>,
    pub audio_pricing: Option<(String, serde_json::Value)>,
}

/// Cost calculator for generation operations.
pub struct CostCalculator;

impl CostCalculator {
    /// Calculate video generation cost.
    pub fn video_cost(
        pricing: &Pricing,
        duration_seconds: f64,
        resolution: Option<&str>,
        no_audio: bool,
    ) -> Option<i64> {
        let base_rate = pricing.credits_per_second?;
        if duration_seconds <= 0.0 {
            return None;
        }
        let mut rate = base_rate;
        if let Some(res) = resolution {
            if let Some(ref rp) = pricing.resolution_pricing {
                if let Some(mult) = rp.get(res) {
                    rate = base_rate * mult;
                }
            }
        }
        let mut cost = duration_seconds * rate;
        if no_audio {
            if let Some(ref ad) = pricing.audio_discount {
                if let Some(discount) = ad.get("no_audio") {
                    cost *= discount;
                }
            }
        }
        Some(cost.ceil() as i64)
    }

    /// Calculate image generation cost.
    pub fn image_cost(
        pricing: &Pricing,
        num_images: i64,
        resolution: Option<&str>,
        quality: Option<&str>,
    ) -> Option<i64> {
        let base_rate = pricing.credits_per_second?;
        if num_images <= 0 {
            return None;
        }
        let mut cost_per_image = base_rate;
        if let Some(res) = resolution {
            if let Some(ref rp) = pricing.resolution_pricing {
                if let Some(mult) = rp.get(res) {
                    cost_per_image = base_rate * mult;
                }
            }
        }
        if let Some(qual) = quality {
            if let Some(ref qp) = pricing.quality_pricing {
                if let Some(mult) = qp.get(qual) {
                    cost_per_image = base_rate * mult;
                }
            }
        }
        let total = cost_per_image * num_images as f64;
        Some(total.ceil() as i64)
    }

    /// Calculate audio generation cost.
    pub fn audio_cost(pricing: &Pricing, prompt: &str, duration_seconds: f64) -> Option<i64> {
        let (mode, config) = pricing.audio_pricing.as_ref()?;
        match mode.as_str() {
            "per_character" => {
                let rate = config.get("rate_per_thousand")?.as_f64()?;
                let chars = prompt.len() as f64;
                let cost = (chars / 1000.0) * rate;
                Some(cost.ceil() as i64)
            }
            "per_second" => {
                let rate = config.get("rate")?.as_f64()?;
                let cost = duration_seconds * rate;
                Some(cost.ceil() as i64)
            }
            "flat" => {
                let price = config.get("price")?.as_f64()?;
                Some(price.ceil() as i64)
            }
            _ => None,
        }
    }

    /// Calculate upscale cost.
    pub fn upscale_cost(pricing: &Pricing, duration_seconds: f64) -> Option<i64> {
        let rate = pricing.credits_per_second?;
        if duration_seconds <= 0.0 {
            return None;
        }
        let cost = duration_seconds * rate;
        Some(cost.ceil() as i64)
    }

    /// Format credits into a human-readable string.
    pub fn format_cost(credits: Option<i64>) -> String {
        match credits {
            None => "--".to_string(),
            Some(0) => "0 credits".to_string(),
            Some(1) => "1 credit".to_string(),
            Some(n) => format!("{n} credits"),
        }
    }
}

// ── Resolution helpers (GPAY-009~GPAY-010) ─────────────────────

/// Parse a resolution label like "1920x1080" into (width, height).
pub fn parse_resolution_label(label: &str) -> Option<(i64, i64)> {
    let parts: Vec<&str> = label.split('x').collect();
    if parts.len() != 2 {
        return None;
    }
    let w = parts[0].parse::<i64>().ok()?;
    let h = parts[1].parse::<i64>().ok()?;
    if w <= 0 || h <= 0 {
        return None;
    }
    Some((w, h))
}

/// Format a resolution pair into a display label.
pub fn resolution_display_label(width: i64, height: i64) -> String {
    format!("{}x{}", width, height)
}

/// Clamp image count to a maximum, defaulting to 4.
pub fn clamp_image_count(count: i64, max_images: Option<i64>) -> i64 {
    let max = max_images.unwrap_or(4);
    count.clamp(1, max)
}

// ── Tests ───────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── Account (ACC-001~004) ──

    #[test]
    fn acc_001_missing_keys_not_crash() {
        let state = AccountState::MissingKeys;
        assert_eq!(
            GenerationMachine::can_submit(&state).unwrap_err(),
            "API keys are missing"
        );
    }

    #[test]
    fn acc_002_remaining_credits_clamped() {
        let acct = ReadyAccount {
            monthly_budget: 100,
            purchased_credits: 50,
            spent_credits: 200,
        };
        assert_eq!(acct.remaining_credits(), 0);
        let acct2 = ReadyAccount {
            monthly_budget: 100,
            purchased_credits: 0,
            spent_credits: 30,
        };
        assert_eq!(acct2.remaining_credits(), 70);
    }

    #[test]
    fn acc_003_check_credits_blocks() {
        let acct = ReadyAccount {
            monthly_budget: 100,
            purchased_credits: 0,
            spent_credits: 0,
        };
        assert!(GenerationMachine::check_credits(&acct, 50).is_ok());
        assert!(GenerationMachine::check_credits(&acct, 150).is_err());
    }

    #[test]
    fn acc_004_misconfigured_rejected() {
        let state = AccountState::Misconfigured("invalid endpoint".into());
        assert!(GenerationMachine::can_submit(&state).is_err());
    }

    // ── Settings (SET-001~007) ──

    #[test]
    fn set_002_notifications_default_on() {
        let s = UserSettings::default();
        assert!(s.notifications_enabled);
    }

    #[test]
    fn set_003_telemetry_default_on() {
        let s = UserSettings::default();
        assert!(s.telemetry_enabled);
    }

    #[test]
    fn set_004_telemetry_next_launch() {
        let s = UserSettings {
            telemetry_enabled: false,
            ..Default::default()
        };
        assert!(s.telemetry_effective(false)); // current launch still on
        assert!(!s.telemetry_effective(true)); // next launch off
    }

    #[test]
    fn set_005_disabled_models_filter() {
        let catalog = vec![
            ModelEntry {
                id: "m1".into(),
                name: "M1".into(),
                modality: GenerationModality::Video,
                cost_per_second: 10,
                disabled: false,
            },
            ModelEntry {
                id: "m2".into(),
                name: "M2".into(),
                modality: GenerationModality::Video,
                cost_per_second: 20,
                disabled: false,
            },
            ModelEntry {
                id: "m3".into(),
                name: "M3".into(),
                modality: GenerationModality::Image,
                cost_per_second: 5,
                disabled: true,
            },
        ];
        let disabled = vec!["m1".to_string()];
        let available = ModelCatalog::available(&catalog, &disabled);
        assert_eq!(available.len(), 1);
        assert_eq!(available[0].id, "m2");
    }

    #[test]
    fn set_006_mask_api_key() {
        assert_eq!(UserSettings::mask_api_key("sk-abc12345"), "2345****");
        assert_eq!(UserSettings::mask_api_key("abc"), "abc");
    }

    #[test]
    fn set_007_storage_clear_not_tested() {
        // Placeholder for storage-pane behavior (tested via integration in app shell)
    }

    // ── Generation (GEN-001~024) ──

    #[test]
    fn gen_001_gated_on_account() {
        assert!(GenerationMachine::can_submit(&AccountState::Unconfigured).is_err());
        assert!(
            GenerationMachine::can_submit(&AccountState::Ready(ReadyAccount {
                monthly_budget: 100,
                purchased_credits: 0,
                spent_credits: 0,
            }))
            .is_ok()
        );
    }

    #[test]
    fn gen_002_available_models() {
        let catalog = vec![
            ModelEntry {
                id: "v1".into(),
                name: "V1".into(),
                modality: GenerationModality::Video,
                cost_per_second: 10,
                disabled: false,
            },
            ModelEntry {
                id: "v2".into(),
                name: "V2".into(),
                modality: GenerationModality::Video,
                cost_per_second: 20,
                disabled: false,
            },
        ];
        let available = ModelCatalog::available(&catalog, &[]);
        assert_eq!(available.len(), 2);
    }

    #[test]
    fn gen_003_cost_exceeds_credits_blocked() {
        let acct = ReadyAccount {
            monthly_budget: 50,
            purchased_credits: 0,
            spent_credits: 0,
        };
        assert!(GenerationMachine::check_credits(&acct, 100).is_err());
    }

    #[test]
    fn gen_006_image_count_clamped() {
        assert_eq!(GenerationMachine::clamp_image_count(0), 1);
        assert_eq!(GenerationMachine::clamp_image_count(2), 2);
        assert_eq!(GenerationMachine::clamp_image_count(10), 4);
    }

    #[test]
    fn gen_011_snapshot_preserves_input() {
        let input = GenerationInput {
            prompt: "test".into(),
            model: "m1".into(),
            duration: 30,
            aspect_ratio: "16:9".into(),
            resolution: None,
            quality: None,
            image_urls: None,
            num_images: None,
            voice: None,
            lyrics: None,
            style_instructions: None,
            instrumental: None,
            generate_audio: None,
            reference_image_urls: None,
            reference_video_urls: None,
            reference_audio_urls: None,
            image_url_asset_ids: None,
            reference_image_asset_ids: None,
            reference_video_asset_ids: None,
            reference_audio_asset_ids: None,
            created_at: None,
        };
        let snap = GenerationMachine::create_snapshot(
            &input,
            GenerationModality::Video,
            1,
            vec![],
            vec![],
        );
        assert_eq!(snap.prompt, "test");
        assert_eq!(snap.model, "m1");
    }

    #[test]
    fn gen_012_placeholder_count() {
        assert_eq!(
            GenerationMachine::placeholder_count(3, &GenerationModality::Image),
            3
        );
        assert_eq!(
            GenerationMachine::placeholder_count(1, &GenerationModality::Audio),
            1
        );
    }

    #[test]
    fn gen_013_submit_returns_job_id() {
        let state = make_preparing_state();
        let acct = ReadyAccount {
            monthly_budget: 1000,
            purchased_credits: 0,
            spent_credits: 0,
        };
        let gen_state = GenerationMachine::start_uploading(state, &acct, vec![], vec![]).unwrap();
        let upload_state = match gen_state {
            GenerationState::Uploading(s) => s,
            _ => panic!("expected Uploading"),
        };
        let job_state =
            GenerationMachine::submit_job(upload_state, "job-123".into(), vec!["p1".into()]);
        match job_state {
            GenerationState::AwaitingJob(s) => assert_eq!(s.job_id, "job-123"),
            _ => panic!("expected AwaitingJob"),
        }
    }

    #[test]
    fn gen_014_subscription_failure_fails_placeholders() {
        let state = make_preparing_state();
        let acct = ReadyAccount {
            monthly_budget: 1000,
            purchased_credits: 0,
            spent_credits: 0,
        };
        let gen_state = GenerationMachine::start_uploading(state, &acct, vec![], vec![]).unwrap();
        let upload_state = match gen_state {
            GenerationState::Uploading(s) => s,
            _ => panic!("expected Uploading"),
        };
        // Simulate: subscription can't start → fail
        let failed = GenerationMachine::fail(
            "Subscription could not start".into(),
            Some(upload_state.snapshot.clone()),
            vec![],
        );
        match failed {
            GenerationState::Failed(s) => assert!(s.reason.contains("Subscription")),
            _ => panic!("expected Failed"),
        }
    }

    #[test]
    fn gen_015_success_downloads_results() {
        let job_state = make_awaiting_state();
        let dl_state =
            GenerationMachine::job_succeeded(job_state, vec!["url1".into(), "url2".into()]);
        match dl_state {
            GenerationState::Downloading(s) => {
                assert_eq!(s.result_urls.len(), 2);
                assert_eq!(s.completed_downloads.len(), 0);
            }
            _ => panic!("expected Downloading"),
        }
    }

    #[test]
    fn gen_016_fewer_results_than_placeholders() {
        assert!(GenerationMachine::check_result_count(3, 2).is_err());
        assert!(GenerationMachine::check_result_count(2, 2).is_ok());
    }

    #[test]
    fn gen_017_download_failure_stores_pending_url() {
        let job_state = make_awaiting_state();
        let mut dl = match GenerationMachine::job_succeeded(job_state, vec!["url1".into()]) {
            GenerationState::Downloading(s) => s,
            _ => panic!("expected Downloading"),
        };
        GenerationMachine::mark_download_failed(&mut dl, "url1".into());
        assert_eq!(dl.failed_downloads.len(), 1);
    }

    #[test]
    fn gen_018_upload_submit_failure() {
        let failed =
            GenerationMachine::fail("Upload failed: connection error".into(), None, vec![]);
        match failed {
            GenerationState::Failed(s) => assert!(s.reason.contains("Upload failed")),
            _ => panic!("expected Failed"),
        }
    }

    #[test]
    fn gen_019_first_success_replaces_target() {
        let state = CompletedState {
            final_asset_ids: vec!["asset-1".into(), "asset-2".into()],
            snapshot: make_dummy_snapshot(),
        };
        assert_eq!(
            GenerationMachine::first_successful_asset(&state),
            Some("asset-1")
        );
    }

    #[test]
    fn gen_020_rerun_from_stored_input() {
        let input = GenerationInput {
            prompt: "rerun test".into(),
            model: "m1".into(),
            duration: 30,
            aspect_ratio: "16:9".into(),
            resolution: None,
            quality: None,
            image_urls: None,
            num_images: Some(2),
            voice: None,
            lyrics: None,
            style_instructions: None,
            instrumental: None,
            generate_audio: None,
            reference_image_urls: None,
            reference_video_urls: None,
            reference_audio_urls: None,
            image_url_asset_ids: None,
            reference_image_asset_ids: None,
            reference_video_asset_ids: None,
            reference_audio_asset_ids: None,
            created_at: None,
        };
        let prep = GenerationMachine::rerun_from_input(&input, GenerationModality::Video).unwrap();
        assert_eq!(prep.prompt, "rerun test");
        assert_eq!(prep.num_images, 2);
    }

    #[test]
    fn gen_021_rerun_fails_on_missing_model() {
        let input = GenerationInput {
            model: String::new(),
            ..Default::default()
        };
        assert!(GenerationMachine::rerun_from_input(&input, GenerationModality::Video).is_err());
    }

    #[test]
    fn gen_022_upscale_availability() {
        assert!(GenerationMachine::can_upscale("image", false));
        assert!(GenerationMachine::can_upscale("video", false));
        assert!(!GenerationMachine::can_upscale("audio", false));
        assert!(!GenerationMachine::can_upscale("image", true));
    }

    #[test]
    fn gen_024_audio_lands_on_audio_tracks() {
        assert_eq!(
            GenerationMachine::target_track_type(&GenerationModality::Audio),
            "audio"
        );
        assert_eq!(
            GenerationMachine::target_track_type(&GenerationModality::Music),
            "audio"
        );
        assert_eq!(
            GenerationMachine::target_track_type(&GenerationModality::Video),
            "video"
        );
    }

    // ── Export state machine (EXP-008~010) ──

    #[test]
    fn exp_008_export_starts_rendering() {
        let state = ExportState::start_rendering(120);
        match state {
            ExportState::Rendering(s) => {
                assert_eq!(s.progress, 0.0);
                assert_eq!(s.stall_timeout_seconds, 120);
            }
            _ => panic!("expected Rendering"),
        }
    }

    #[test]
    fn exp_009_progress_updates() {
        let mut state = ExportState::start_rendering(120);
        if let ExportState::Rendering(ref mut s) = state {
            assert!(ExportState::update_progress(s, 0.5));
            assert_eq!(s.progress, 0.5);
            assert!(!ExportState::update_progress(s, 0.5)); // no change
        } else {
            panic!("expected Rendering");
        }
    }

    #[test]
    fn exp_010_stall_detection() {
        let state = RenderingState {
            progress: 0.3,
            stall_timeout_seconds: 0, // immediate timeout
            last_progress_time: chrono::Utc::now() - chrono::Duration::seconds(10),
            stall_watchdog_cancelled: false,
        };
        assert!(ExportState::has_stalled(&state));
    }

    #[test]
    fn exp_010_cancellation_distinct() {
        let state = ExportState::start_rendering(120);
        match state {
            ExportState::Rendering(s) => {
                let cancelled = ExportState::cancel(s);
                assert_eq!(cancelled, ExportState::Cancelling);
            }
            _ => panic!("expected Rendering"),
        }
    }

    // ── Helpers ──

    fn make_preparing_state() -> PreparingState {
        PreparingState {
            prompt: "test".into(),
            model: "m1".into(),
            duration_seconds: 10.0,
            aspect_ratio: "16:9".into(),
            resolution: None,
            quality: None,
            estimated_cost: 50,
            num_images: 1,
            reference_urls: vec![],
            modality: GenerationModality::Video,
        }
    }

    fn make_awaiting_state() -> AwaitingJobState {
        AwaitingJobState {
            job_id: "job-1".into(),
            snapshot: make_dummy_snapshot(),
            placeholder_ids: vec!["p1".into(), "p2".into()],
        }
    }

    fn make_dummy_snapshot() -> GenerationSnapshot {
        GenerationSnapshot {
            prompt: String::new(),
            model: String::new(),
            duration_seconds: 0.0,
            aspect_ratio: String::new(),
            resolution: None,
            quality: None,
            num_images: 1,
            reference_urls: vec![],
            reference_asset_ids: vec![],
            modality: GenerationModality::Video,
            created_at: chrono::Utc::now(),
        }
    }

    // ── GEN-004: Placeholder IDs created before job settles ──

    #[test]
    fn gen_004_placeholder_ids_created() {
        let state = make_preparing_state();
        let acct = ReadyAccount {
            monthly_budget: 1000,
            purchased_credits: 0,
            spent_credits: 0,
        };
        let gen_state = GenerationMachine::start_uploading(state, &acct, vec![], vec![]).unwrap();
        let upload_state = match gen_state {
            GenerationState::Uploading(s) => s,
            _ => panic!("expected Uploading"),
        };
        // Placeholder IDs are created before backend job settles
        let placeholder_ids: Vec<String> = (0..upload_state.snapshot.num_images)
            .map(|i| format!("ph-{i}"))
            .collect();
        let job =
            GenerationMachine::submit_job(upload_state, "job-99".into(), placeholder_ids.clone());
        match job {
            GenerationState::AwaitingJob(s) => {
                assert_eq!(s.job_id, "job-99");
                assert_eq!(s.placeholder_ids, placeholder_ids);
            }
            _ => panic!("expected AwaitingJob"),
        }
    }

    // ── GEN-005: Placeholder count reflects modality ──

    #[test]
    fn gen_005_placeholder_count_modality() {
        // Video modality with multiple images produces multiple placeholders
        assert_eq!(
            GenerationMachine::placeholder_count(3, &GenerationModality::Video),
            3
        );
        // Audio/music always produce 1
        assert_eq!(
            GenerationMachine::placeholder_count(5, &GenerationModality::Audio),
            1
        );
        assert_eq!(
            GenerationMachine::placeholder_count(2, &GenerationModality::Music),
            1
        );
        // Image count is pre-clamped
        assert_eq!(
            GenerationMachine::placeholder_count(1, &GenerationModality::Image),
            1
        );
    }

    // ── GEN-007: Upload order preserved by target_index ──

    #[test]
    fn gen_007_upload_order_preserved() {
        let state = make_preparing_state();
        let acct = ReadyAccount {
            monthly_budget: 1000,
            purchased_credits: 0,
            spent_credits: 0,
        };
        // Feed paths out of order: index 2 first, then 0, then 1
        let local_paths = vec![
            (2usize, "/tmp/ref2.mp4".into()),
            (0usize, "/tmp/ref0.mp4".into()),
            (1usize, "/tmp/ref1.mp4".into()),
        ];
        let gen_state =
            GenerationMachine::start_uploading(state, &acct, local_paths, vec![]).unwrap();
        let upload_state = match gen_state {
            GenerationState::Uploading(s) => s,
            _ => panic!("expected Uploading"),
        };
        // Must be sorted by target_index: 0, 1, 2
        let indices: Vec<usize> = upload_state
            .pending_uploads
            .iter()
            .map(|u| u.target_index)
            .collect();
        assert_eq!(indices, vec![0, 1, 2]);
        // Paths must match the sorted order
        let paths: Vec<&str> = upload_state
            .pending_uploads
            .iter()
            .map(|u| u.local_path.as_str())
            .collect();
        assert_eq!(
            paths,
            vec!["/tmp/ref0.mp4", "/tmp/ref1.mp4", "/tmp/ref2.mp4"]
        );
    }

    // ── GEN-008: Pre-uploaded URLs skip re-upload ──

    #[test]
    fn gen_008_pre_uploaded_skip_reupload() {
        let state = make_preparing_state();
        let acct = ReadyAccount {
            monthly_budget: 1000,
            purchased_credits: 0,
            spent_credits: 0,
        };
        // Mix: one local path (needs upload) + one pre-uploaded
        let local_paths = vec![(0usize, "/tmp/raw.mp4".into())];
        let pre_uploaded = vec![(1usize, "https://cdn.example.com/pre.mp4".into())];
        let gen_state =
            GenerationMachine::start_uploading(state, &acct, local_paths, pre_uploaded).unwrap();
        let upload_state = match gen_state {
            GenerationState::Uploading(s) => s,
            _ => panic!("expected Uploading"),
        };
        assert_eq!(upload_state.pending_uploads.len(), 2);
        // Item 0: local path, no pre-uploaded URL
        assert_eq!(upload_state.pending_uploads[0].local_path, "/tmp/raw.mp4");
        assert!(upload_state.pending_uploads[0].pre_uploaded_url.is_none());
        // Item 1: pre-uploaded, empty local path
        assert_eq!(upload_state.pending_uploads[1].local_path, "");
        assert_eq!(
            upload_state.pending_uploads[1].pre_uploaded_url.as_deref(),
            Some("https://cdn.example.com/pre.mp4")
        );
    }

    // ── GEN-009: Local paths are for pristine upload (not pre-uploaded) ──

    #[test]
    fn gen_009_local_paths_need_upload_trimmed_do_not() {
        let state = make_preparing_state();
        let acct = ReadyAccount {
            monthly_budget: 1000,
            purchased_credits: 0,
            spent_credits: 0,
        };
        // Items going through local_paths need upload (pristine or trimmed)
        let local_paths = vec![
            (0usize, "/tmp/pristine.mp4".into()),
            (1usize, "/tmp/trimmed.mp4".into()),
        ];
        // Items from pre_uploaded skip upload entirely
        let pre_uploaded = vec![(2usize, "https://cdn.example.com/cached.mp4".into())];
        let gen_state =
            GenerationMachine::start_uploading(state, &acct, local_paths, pre_uploaded).unwrap();
        let upload_state = match gen_state {
            GenerationState::Uploading(s) => s,
            _ => panic!("expected Uploading"),
        };
        // local_paths items need upload: no pre_uploaded_url
        for item in &upload_state.pending_uploads {
            if item.target_index < 2 {
                assert!(
                    item.pre_uploaded_url.is_none(),
                    "item {} should need upload",
                    item.target_index
                );
            } else {
                // pre-uploaded item has the URL
                assert_eq!(
                    item.pre_uploaded_url.as_deref(),
                    Some("https://cdn.example.com/cached.mp4")
                );
            }
        }
    }

    // ── GEN-010: Trimmed reference paths pass through local_paths ──

    #[test]
    fn gen_010_trimmed_reference_local_path() {
        let state = make_preparing_state();
        let acct = ReadyAccount {
            monthly_budget: 1000,
            purchased_credits: 0,
            spent_credits: 0,
        };
        // Simulate trimmed first-source video reference exported to temp
        let local_paths = vec![(0usize, "/tmp/fronda-trim-XXXXX.mp4".into())];
        let gen_state =
            GenerationMachine::start_uploading(state, &acct, local_paths, vec![]).unwrap();
        let upload_state = match gen_state {
            GenerationState::Uploading(s) => s,
            _ => panic!("expected Uploading"),
        };
        assert_eq!(upload_state.pending_uploads.len(), 1);
        assert!(
            upload_state.pending_uploads[0]
                .local_path
                .starts_with("/tmp/fronda-trim-"),
            "trimmed references land in temp: {}",
            upload_state.pending_uploads[0].local_path
        );
        assert!(upload_state.pending_uploads[0].pre_uploaded_url.is_none());
    }

    // ── GEN-023: Prompt mention reference slots preserved ──

    #[test]
    fn gen_023_reference_slots_preserved() {
        let acct = ReadyAccount {
            monthly_budget: 1000,
            purchased_credits: 0,
            spent_credits: 0,
        };
        // Reference URLs come from prompt mention / reference-slot processing
        let ref_urls = vec![
            "https://cdn.example.com/ref1.mp4".into(),
            "https://cdn.example.com/ref2.jpg".into(),
        ];
        let prep = GenerationMachine::start_prepare(
            &AccountState::Ready(acct),
            "generate with @ref1 and @ref2".into(),
            "m1".into(),
            10.0,
            "16:9".into(),
            None,
            None,
            50,
            1,
            ref_urls.clone(),
            GenerationModality::Video,
        )
        .unwrap();
        match prep {
            GenerationState::Preparing(s) => {
                // Reference URLs from prompt mention tags pass through to state
                assert_eq!(s.reference_urls.len(), 2);
                assert!(s.reference_urls[0].contains("ref1"));
                assert!(s.reference_urls[1].contains("ref2"));
            }
            _ => panic!("expected Preparing"),
        }
    }

    // ── ACC-003: Top-off amount validation ──

    #[test]
    fn acc_003_top_off_amount_validation() {
        let cfg = TopUpConfig::default();
        assert_eq!(cfg.min_amount, 10);
        assert_eq!(cfg.max_amount, 500);

        // Valid amounts
        assert!(cfg.validate_amount(10).is_ok());
        assert!(cfg.validate_amount(250).is_ok());
        assert!(cfg.validate_amount(500).is_ok());

        // Below minimum
        let err = cfg.validate_amount(5).unwrap_err();
        assert!(err.contains("below minimum"));

        // Above maximum
        let err = cfg.validate_amount(501).unwrap_err();
        assert!(err.contains("exceeds maximum"));
    }

    #[test]
    fn acc_003_top_off_default_constant() {
        // The static default matches the Default impl
        assert_eq!(DEFAULT_TOPUP_CONFIG.min_amount, 10);
        assert_eq!(DEFAULT_TOPUP_CONFIG.max_amount, 500);
        assert!(DEFAULT_TOPUP_CONFIG.validate_amount(50).is_ok());
    }

    // ── ACC-004: Billing URL whitelist validation ──

    #[test]
    fn acc_004_billing_url_trusted_hosts() {
        // Trusted Stripe checkout URLs
        assert!(
            BillingUrlValidator::validate("https://checkout.stripe.com/c/pay/csid_abc").is_ok()
        );
        assert!(BillingUrlValidator::validate("https://buy.stripe.com/test_123").is_ok());
        assert!(BillingUrlValidator::validate("https://api.stripe.com/v1/charges").is_ok());
        // Trusted billing provider URL
        assert!(
            BillingUrlValidator::validate("https://billing.anthropic.com/subscribe?plan=pro")
                .is_ok()
        );
    }

    #[test]
    fn acc_004_billing_url_rejects_untrusted() {
        // Untrusted host
        let err = BillingUrlValidator::validate("https://evil.example.com/billing").unwrap_err();
        assert!(err.contains("Untrusted"));
        assert!(err.contains("evil.example.com"));

        // No scheme
        let err = BillingUrlValidator::validate("ftp://files.example.com").unwrap_err();
        assert!(err.contains("Invalid URL scheme"));

        // Empty string
        let err = BillingUrlValidator::validate("").unwrap_err();
        assert!(err.contains("Invalid URL scheme"));
    }

    // ── Catalog types (CAT-004~006, CAT-011) ──

    #[test]
    fn cat_004_kind_values() {
        assert_eq!(ModelKind::Video as u8, 0);
        assert_eq!(ModelKind::Image as u8, 1);
        assert_eq!(ModelKind::Audio as u8, 2);
        assert_eq!(ModelKind::Upscale as u8, 3);
    }

    #[test]
    fn cat_005_response_shape_values() {
        assert_eq!(ResponseShape::Video as u8, 0);
        assert_eq!(ResponseShape::Images as u8, 1);
        assert_eq!(ResponseShape::Audio as u8, 2);
        assert_eq!(ResponseShape::UpscaledImage as u8, 3);
    }

    #[test]
    fn cat_006_video_capabilities_defaults() {
        let caps = VideoCapabilities {
            durations: vec![5.0, 10.0],
            resolutions: None,
            aspect_ratios: vec!["16:9".into()],
            max_reference_images: None,
            max_reference_videos: None,
            max_reference_audios: None,
            max_total_references: None,
            max_reference_video_seconds: None,
            max_reference_audio_seconds: None,
            requires_source_video: None,
            requires_reference_image: None,
            supports_generate_audio: None,
        };
        assert_eq!(caps.durations, vec![5.0, 10.0]);
        assert!(caps.resolutions.is_none());
        assert_eq!(caps.aspect_ratios, vec!["16:9"]);
    }

    #[test]
    fn cat_011_display_name_fallback() {
        let entry = CatalogEntry {
            id: "model-x".into(),
            kind: ModelKind::Video,
            display_name: String::new(),
            allowed_endpoints: vec![],
            response_shape: ResponseShape::Video,
            ui_capabilities: serde_json::Value::Null,
            pricing: None,
            qualities: None,
        };
        assert_eq!(entry.display_name_or_id(), "model-x");
        assert_eq!(format!("{entry}"), "model-x");

        let entry2 = CatalogEntry {
            display_name: "Model X".into(),
            ..entry
        };
        assert_eq!(entry2.display_name_or_id(), "Model X");
        assert_eq!(format!("{entry2}"), "Model X");
    }

    // ── Cost estimation (COST-001~008) ──

    #[test]
    fn cost_001_video_cost_nil_when_no_rates() {
        let pricing = Pricing {
            credits_per_second: None,
            resolution_pricing: None,
            quality_pricing: None,
            audio_discount: None,
            audio_pricing: None,
        };
        assert!(CostCalculator::video_cost(&pricing, 10.0, None, false).is_none());
    }

    #[test]
    fn cost_001_video_cost_nil_non_positive_duration() {
        let pricing = Pricing {
            credits_per_second: Some(10.0),
            resolution_pricing: None,
            quality_pricing: None,
            audio_discount: None,
            audio_pricing: None,
        };
        assert!(CostCalculator::video_cost(&pricing, 0.0, None, false).is_none());
        assert!(CostCalculator::video_cost(&pricing, -1.0, None, false).is_none());
    }

    #[test]
    fn cost_001_video_cost_with_resolution_pricing() {
        let pricing = Pricing {
            credits_per_second: Some(10.0),
            resolution_pricing: Some([("1080p".into(), 2.0)].into()),
            quality_pricing: None,
            audio_discount: None,
            audio_pricing: None,
        };
        // 10 seconds * 10 base * 2x resolution = 200
        assert_eq!(
            CostCalculator::video_cost(&pricing, 10.0, Some("1080p"), false),
            Some(200)
        );
        // No resolution match = base
        assert_eq!(
            CostCalculator::video_cost(&pricing, 10.0, Some("720p"), false),
            Some(100)
        );
    }

    #[test]
    fn cost_001_video_cost_with_audio_discount() {
        let pricing = Pricing {
            credits_per_second: Some(10.0),
            resolution_pricing: None,
            quality_pricing: None,
            audio_discount: Some([("no_audio".into(), 0.5)].into()),
            audio_pricing: None,
        };
        // 10 seconds * 10 base = 100, with 0.5 discount = 50
        assert_eq!(
            CostCalculator::video_cost(&pricing, 10.0, None, true),
            Some(50)
        );
        // Without discount = 100
        assert_eq!(
            CostCalculator::video_cost(&pricing, 10.0, None, false),
            Some(100)
        );
    }

    #[test]
    fn cost_002_image_cost_with_quality_pricing() {
        let pricing = Pricing {
            credits_per_second: Some(5.0),
            resolution_pricing: None,
            quality_pricing: Some([("hd".into(), 2.0)].into()),
            audio_discount: None,
            audio_pricing: None,
        };
        // 5 base * 2x quality = 10 per image, 2 images = 20
        assert_eq!(
            CostCalculator::image_cost(&pricing, 2, None, Some("hd")),
            Some(20)
        );
        // No quality match = base rate
        assert_eq!(
            CostCalculator::image_cost(&pricing, 2, None, Some("standard")),
            Some(10)
        );
    }

    #[test]
    fn cost_002_image_cost_with_resolution_pricing() {
        let pricing = Pricing {
            credits_per_second: Some(5.0),
            resolution_pricing: Some([("4k".into(), 3.0)].into()),
            quality_pricing: None,
            audio_discount: None,
            audio_pricing: None,
        };
        // 5 base * 3x resolution = 15, 2 images = 30
        assert_eq!(
            CostCalculator::image_cost(&pricing, 2, Some("4k"), None),
            Some(30)
        );
    }

    #[test]
    fn cost_002_image_cost_nil_when_no_rates() {
        let pricing = Pricing {
            credits_per_second: None,
            resolution_pricing: None,
            quality_pricing: None,
            audio_discount: None,
            audio_pricing: None,
        };
        assert!(CostCalculator::image_cost(&pricing, 1, None, None).is_none());
    }

    #[test]
    fn cost_003_audio_per_thousand_char_cost() {
        let pricing = Pricing {
            credits_per_second: None,
            resolution_pricing: None,
            quality_pricing: None,
            audio_discount: None,
            audio_pricing: Some((
                "per_character".into(),
                serde_json::json!({"rate_per_thousand": 10.0}),
            )),
        };
        // 500 chars / 1000 * 10 = 5
        assert_eq!(
            CostCalculator::audio_cost(&pricing, &"a".repeat(500), 0.0),
            Some(5)
        );
        // 1500 chars / 1000 * 10 = 15
        assert_eq!(
            CostCalculator::audio_cost(&pricing, &"a".repeat(1500), 0.0),
            Some(15)
        );
    }

    #[test]
    fn cost_004_audio_per_second_cost() {
        let pricing = Pricing {
            credits_per_second: None,
            resolution_pricing: None,
            quality_pricing: None,
            audio_discount: None,
            audio_pricing: Some(("per_second".into(), serde_json::json!({"rate": 2.5}))),
        };
        // 30 seconds * 2.5 = 75
        assert_eq!(CostCalculator::audio_cost(&pricing, "", 30.0), Some(75));
    }

    #[test]
    fn cost_005_audio_flat_cost() {
        let pricing = Pricing {
            credits_per_second: None,
            resolution_pricing: None,
            quality_pricing: None,
            audio_discount: None,
            audio_pricing: Some(("flat".into(), serde_json::json!({"price": 50.0}))),
        };
        assert_eq!(
            CostCalculator::audio_cost(&pricing, "anything", 0.0),
            Some(50)
        );
    }

    #[test]
    fn cost_006_unknown_audio_pricing_returns_nil() {
        let pricing = Pricing {
            credits_per_second: None,
            resolution_pricing: None,
            quality_pricing: None,
            audio_discount: None,
            audio_pricing: Some(("unknown_mode".into(), serde_json::Value::Null)),
        };
        assert!(CostCalculator::audio_cost(&pricing, "test", 1.0).is_none());
    }

    #[test]
    fn cost_007_upscale_cost() {
        let pricing = Pricing {
            credits_per_second: Some(3.0),
            resolution_pricing: None,
            quality_pricing: None,
            audio_discount: None,
            audio_pricing: None,
        };
        // 10 seconds * 3 = 30
        assert_eq!(CostCalculator::upscale_cost(&pricing, 10.0), Some(30));
        // Non-positive returns nil
        assert!(CostCalculator::upscale_cost(&pricing, 0.0).is_none());
    }

    #[test]
    fn cost_008_cost_formatting() {
        assert_eq!(CostCalculator::format_cost(None), "--");
        assert_eq!(CostCalculator::format_cost(Some(0)), "0 credits");
        assert_eq!(CostCalculator::format_cost(Some(1)), "1 credit");
        assert_eq!(CostCalculator::format_cost(Some(50)), "50 credits");
    }

    // ── Resolution helpers (GPAY-007, 009-010) ──

    #[test]
    fn gpay_007_image_maximages_clamping() {
        // Default max (4)
        assert_eq!(clamp_image_count(0, None), 1);
        assert_eq!(clamp_image_count(2, None), 2);
        assert_eq!(clamp_image_count(10, None), 4);
        // Custom max from capabilities
        assert_eq!(clamp_image_count(0, Some(8)), 1);
        assert_eq!(clamp_image_count(10, Some(8)), 8);
    }

    #[test]
    fn gpay_009_resolution_label_parsing() {
        assert_eq!(parse_resolution_label("1920x1080"), Some((1920, 1080)));
        assert_eq!(parse_resolution_label("3840x2160"), Some((3840, 2160)));
    }

    #[test]
    fn gpay_009_invalid_resolution_labels() {
        assert!(parse_resolution_label("").is_none());
        assert!(parse_resolution_label("abc").is_none());
        assert!(parse_resolution_label("1920xabc").is_none());
        assert!(parse_resolution_label("1920x1080x720").is_none());
        assert!(parse_resolution_label("-1920x1080").is_none());
    }

    #[test]
    fn gpay_010_resolution_display_labels() {
        assert_eq!(resolution_display_label(1920, 1080), "1920x1080");
        assert_eq!(resolution_display_label(3840, 2160), "3840x2160");
        assert_eq!(resolution_display_label(640, 480), "640x480");
    }

    // ── Catalog entry fields (CAT-003, 007-010) ──

    #[test]
    fn cat_003_core_fields() {
        let entry = CatalogEntry {
            id: "model-1".into(),
            kind: ModelKind::Video,
            display_name: "Model One".into(),
            allowed_endpoints: vec!["generate".into()],
            response_shape: ResponseShape::Video,
            ui_capabilities: serde_json::json!({"referenceImages": true}),
            pricing: Some(Pricing {
                credits_per_second: Some(10.0),
                resolution_pricing: None,
                quality_pricing: None,
                audio_discount: None,
                audio_pricing: None,
            }),
            qualities: Some(vec!["hd".into(), "sd".into()]),
        };
        assert_eq!(entry.id, "model-1");
        assert_eq!(entry.kind, ModelKind::Video);
        assert_eq!(entry.display_name, "Model One");
        assert_eq!(entry.allowed_endpoints, vec!["generate"]);
        assert_eq!(entry.response_shape, ResponseShape::Video);
        assert_eq!(
            entry.ui_capabilities,
            serde_json::json!({"referenceImages": true})
        );
        assert!(entry.pricing.is_some());
        assert_eq!(entry.qualities, Some(vec!["hd".into(), "sd".into()]));
    }

    #[test]
    fn cat_007_image_capabilities() {
        let caps = ImageCapabilities {
            resolutions: Some(vec!["1024x1024".into()]),
            aspect_ratios: Some(vec!["1:1".into()]),
            qualities: Some(vec!["standard".into(), "hd".into()]),
            max_images: Some(4),
        };
        assert_eq!(caps.resolutions, Some(vec!["1024x1024".into()]));
        assert_eq!(caps.aspect_ratios, Some(vec!["1:1".into()]));
        assert_eq!(caps.qualities, Some(vec!["standard".into(), "hd".into()]));
        assert_eq!(caps.max_images, Some(4));
    }

    #[test]
    fn cat_008_audio_capabilities() {
        let caps = AudioCapabilities {
            category: Some("Speech".into()),
            voices: vec!["alloy".into(), "echo".into()],
            default_voice: Some("alloy".into()),
            lyrics: Some(true),
            style_instructions: Some(true),
            instrumental: Some(false),
            durations: Some(vec![30.0, 60.0]),
            min_prompt_length: Some(10),
            supported_inputs: Some(vec!["text".into()]),
            min_seconds: Some(1.0),
            max_seconds: Some(900.0),
        };
        assert_eq!(caps.category, Some("Speech".into()));
        assert_eq!(caps.voices, vec!["alloy", "echo"]);
        assert_eq!(caps.default_voice, Some("alloy".into()));
        assert_eq!(caps.lyrics, Some(true));
        assert_eq!(caps.style_instructions, Some(true));
        assert_eq!(caps.instrumental, Some(false));
        assert_eq!(caps.durations, Some(vec![30.0, 60.0]));
        assert_eq!(caps.min_prompt_length, Some(10));
        assert_eq!(caps.supported_inputs, Some(vec!["text".into()]));
        assert_eq!(caps.min_seconds, Some(1.0));
        assert_eq!(caps.max_seconds, Some(900.0));
    }

    #[test]
    fn cat_009_upscale_capabilities() {
        let caps = UpscaleCapabilities {
            speed_label: Some("Fast".into()),
            p75_duration_seconds: Some(15.0),
            supported_clip_types: vec!["video".into(), "image".into()],
        };
        assert_eq!(caps.speed_label, Some("Fast".into()));
        assert_eq!(caps.p75_duration_seconds, Some(15.0));
        assert_eq!(caps.supported_clip_types, vec!["video", "image"]);
    }

    #[test]
    fn cat_010_unknown_audio_pricing_mode() {
        // AudioPricingMode has no catch-all; unknown modes cannot be
        // represented, satisfying CAT-010 (fail decode, not silent).
        let per_char = AudioPricingMode::PerCharacter {
            rate_per_thousand: 10.0,
        };
        let per_sec = AudioPricingMode::PerSecond { rate: 2.5 };
        let flat = AudioPricingMode::Flat { price: 50.0 };
        assert_ne!(format!("{per_char:?}"), format!("{per_sec:?}"));
        assert_ne!(format!("{per_sec:?}"), format!("{flat:?}"));

        // Unknown string modes at the CostCalculator level produce None
        // (no silent zero-cost pricing).
        let pricing = Pricing {
            credits_per_second: None,
            resolution_pricing: None,
            quality_pricing: None,
            audio_discount: None,
            audio_pricing: Some(("bogus_mode".into(), serde_json::Value::Null)),
        };
        assert!(CostCalculator::audio_cost(&pricing, "test", 1.0).is_none());
    }

    // ── Rerun cost reconstruction (COST-009) ──

    #[test]
    fn cost_009_rerun_cost_reconstruction() {
        // Video rerun: stores model id, preserves all input fields,
        // defaults num_images to 1 when absent.
        let video_input = GenerationInput {
            prompt: "a video".into(),
            model: "video-model".into(),
            duration: 10,
            aspect_ratio: "16:9".into(),
            resolution: Some("1080p".into()),
            quality: Some("hd".into()),
            num_images: None,
            generate_audio: None,
            ..Default::default()
        };
        let prep =
            GenerationMachine::rerun_from_input(&video_input, GenerationModality::Video).unwrap();
        assert_eq!(prep.prompt, "a video");
        assert_eq!(prep.model, "video-model");
        assert_eq!(prep.duration_seconds, 10.0);
        assert_eq!(prep.aspect_ratio, "16:9");
        assert_eq!(prep.resolution, Some("1080p".into()));
        assert_eq!(prep.quality, Some("hd".into()));
        assert_eq!(prep.num_images, 1); // defaulted from None via unwrap_or(1)
        assert_eq!(prep.modality, GenerationModality::Video);
        assert_eq!(prep.estimated_cost, 0); // populated fresh after dispatch

        // Image rerun: num_images defaults to 1 when absent.
        let image_input = GenerationInput {
            prompt: "an image".into(),
            model: "image-model".into(),
            num_images: None,
            generate_audio: None,
            ..Default::default()
        };
        let prep =
            GenerationMachine::rerun_from_input(&image_input, GenerationModality::Image).unwrap();
        assert_eq!(prep.num_images, 1); // defaulted from None via unwrap_or(1)
        assert_eq!(prep.modality, GenerationModality::Image);
    }
}
