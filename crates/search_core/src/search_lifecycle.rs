//! Search lifecycle state machine (SRCH-001..014).
//!
//! Models the search system's enable/disable state, indexing queue, export
//! pause protocol, and asset eligibility rules as a pure state machine with
//! no platform dependencies.

use serde::{Deserialize, Serialize};

/// Current state of the search indexing system.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum IndexingState {
    /// No indexing in progress.
    Idle,
    /// Indexing is actively running.
    Indexing,
    /// Indexing is paused (e.g. while an export is active). SRCH-013.
    Paused,
}

/// Status of a single asset in the indexing queue.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum JobStatus {
    /// Waiting to be indexed.
    Pending,
    /// Currently being indexed.
    InProgress,
    /// Indexing completed successfully.
    Completed,
    /// Indexing failed (SRCH-011: not retried in same batch).
    Failed,
    /// Asset is missing from disk (SRCH-012: treated as completed).
    Missing,
}

impl JobStatus {
    /// Returns true if this job is terminal (won't be retried in current batch).
    pub fn is_terminal(&self) -> bool {
        matches!(self, Self::Completed | Self::Failed | Self::Missing)
    }
}

/// A single asset in the indexing queue.
#[derive(Debug, Clone, PartialEq)]
pub struct IndexJob {
    /// Unique asset identifier.
    pub asset_id: String,
    /// The media type of the asset (for eligibility checks).
    pub media_type: String,
    /// Whether the asset has an audio track.
    pub has_audio: bool,
    /// Whether the asset is currently being generated (SRCH-009).
    pub is_generating: bool,
    /// Current processing status.
    pub status: JobStatus,
}

impl IndexJob {
    /// Returns true if this asset is eligible for visual indexing (SRCH-007).
    /// Only video and image assets participate in visual indexing.
    pub fn is_visual_eligible(&self) -> bool {
        matches!(self.media_type.as_str(), "video" | "image")
    }

    /// Returns true if this asset is eligible for transcript indexing (SRCH-008).
    /// Only audio assets or video assets with audio participate.
    pub fn is_transcript_eligible(&self) -> bool {
        self.media_type == "audio" || (self.media_type == "video" && self.has_audio)
    }

    /// Returns true if this asset is eligible for any indexing at all.
    /// Generating assets are skipped (SRCH-009).
    pub fn is_eligible(&self) -> bool {
        if self.is_generating {
            return false;
        }
        self.is_visual_eligible() || self.is_transcript_eligible()
    }

    /// Returns true if the asset is completely ineligible for all indexing.
    pub fn is_ineligible(&self) -> bool {
        !self.is_eligible()
    }
}

/// Input for registering an asset in the index queue.
#[derive(Debug, Clone)]
pub struct IndexJobInput {
    pub asset_id: String,
    pub media_type: String,
    pub has_audio: bool,
    pub is_generating: bool,
}

/// Search lifecycle state machine (SRCH-001..014).
///
/// Pure state machine with injected callbacks for platform operations
/// (model preparation, unloading, file I/O).
#[derive(Debug, Clone)]
pub struct SearchLifecycle {
    /// SRCH-001/003: Search is enabled by default and persists across launches.
    enabled: bool,
    /// Current indexing state.
    indexing_state: IndexingState,
    /// The asset index queue for the current batch (SRCH-010: deduped).
    index_queue: Vec<IndexJob>,
    /// Batches processed so far — used for tracking whether a failed asset
    /// may be retried in a later sweep (SRCH-011).
    batch_count: u64,
    /// SRCH-014: Export pause is refcounted, not boolean.
    export_pause_refcount: u32,
    /// Whether the search model is currently loaded.
    model_loaded: bool,
}

impl SearchLifecycle {
    /// Create a new search lifecycle. SRCH-001: enabled by default.
    pub fn new() -> Self {
        Self {
            enabled: true,
            indexing_state: IndexingState::Idle,
            index_queue: Vec::new(),
            batch_count: 0,
            export_pause_refcount: 0,
            model_loaded: false,
        }
    }

    // ------------------------------------------------------------------
    // Basic state queries
    // ------------------------------------------------------------------

    /// Whether search is enabled (SRCH-001).
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Current indexing state.
    pub fn indexing_state(&self) -> IndexingState {
        self.indexing_state
    }

    /// Whether the model is currently loaded.
    pub fn is_model_loaded(&self) -> bool {
        self.model_loaded
    }

    /// Current export pause refcount (SRCH-014).
    pub fn export_pause_refcount(&self) -> u32 {
        self.export_pause_refcount
    }

    /// Current number of items in the index queue.
    pub fn queue_len(&self) -> usize {
        self.index_queue.len()
    }

    /// Number of batches processed.
    pub fn batch_count(&self) -> u64 {
        self.batch_count
    }

    /// Current index queue (for inspection).
    pub fn index_queue(&self) -> &[IndexJob] {
        &self.index_queue
    }

    // ------------------------------------------------------------------
    // SRCH-003: Enable/disable state persistence
    // ------------------------------------------------------------------

    /// Enable search (SRCH-005: re-enable prepares model, re-sweeps assets).
    /// Returns the actions the caller must take.
    pub fn enable(&mut self) -> Vec<LifecycleAction> {
        let mut actions = Vec::new();
        if !self.enabled {
            self.enabled = true;
            // SRCH-005: Re-enabling prepares the model and re-sweeps.
            actions.push(LifecycleAction::PrepareModel);
            actions.push(LifecycleAction::SweepAssets);
        }
        actions
    }

    /// Disable search (SRCH-004).
    /// Returns the actions the caller must take.
    pub fn disable(&mut self) -> Vec<LifecycleAction> {
        let mut actions = Vec::new();
        if self.enabled {
            self.enabled = false;
            // SRCH-004: Cancel in-flight indexing.
            if self.indexing_state == IndexingState::Indexing {
                self.indexing_state = IndexingState::Idle;
                actions.push(LifecycleAction::CancelIndexing);
            }
            // SRCH-004: Unload model without deleting stored indexes.
            if self.model_loaded {
                self.model_loaded = false;
                actions.push(LifecycleAction::UnloadModel);
            }
            // Clear the queue but keep stored indexes on disk.
            self.index_queue.clear();
        }
        actions
    }

    // ------------------------------------------------------------------
    // SRCH-006: Remove installed model
    // ------------------------------------------------------------------

    /// Remove the installed search model (SRCH-006).
    /// Resets coordinators, unloads the embedder, deletes model files.
    pub fn remove_model(&mut self) -> Vec<LifecycleAction> {
        let mut actions = Vec::new();
        if self.model_loaded {
            self.model_loaded = false;
        }
        self.indexing_state = IndexingState::Idle;
        self.index_queue.clear();
        // SRCH-006: Delete model files.
        actions.push(LifecycleAction::DeleteModelFiles);
        actions.push(LifecycleAction::ResetCoordinators);
        actions
    }

    // ------------------------------------------------------------------
    // SRCH-002: Project open → prepare model + sweep assets
    // ------------------------------------------------------------------

    /// Notify that a project was opened (SRCH-002).
    /// Attempts to prepare the model and sweep current assets.
    pub fn on_project_opened(&mut self) -> Vec<LifecycleAction> {
        let mut actions = Vec::new();
        if self.enabled {
            actions.push(LifecycleAction::PrepareModel);
            actions.push(LifecycleAction::SweepAssets);
        }
        actions
    }

    // ------------------------------------------------------------------
    // SRCH-007..010: Index queue management
    // ------------------------------------------------------------------

    /// Register assets for indexing (SRCH-007..010).
    ///
    /// - SRCH-007: Only video/image for visual indexing.
    /// - SRCH-008: Only audio/video-with-audio for transcript indexing.
    /// - SRCH-009: Generating assets are skipped.
    /// - SRCH-010: Duplicate asset IDs within a batch are deduped.
    pub fn register_assets(&mut self, inputs: Vec<IndexJobInput>) -> Vec<LifecycleAction> {
        if !self.enabled {
            return Vec::new();
        }

        let mut actions = Vec::new();

        // SRCH-010: Dedupe by asset_id within this batch.
        let mut seen: std::collections::HashSet<String> = self
            .index_queue
            .iter()
            .map(|j| j.asset_id.clone())
            .collect();

        for input in inputs {
            if seen.contains(&input.asset_id) {
                continue; // SRCH-010: dedupe
            }
            seen.insert(input.asset_id.clone());

            let job = IndexJob {
                asset_id: input.asset_id,
                media_type: input.media_type,
                has_audio: input.has_audio,
                is_generating: input.is_generating,
                status: JobStatus::Pending,
            };

            // SRCH-009: Skip generating assets.
            if job.is_generating {
                continue;
            }

            // Skip completely ineligible assets (neither visual nor transcript).
            if job.is_ineligible() {
                continue;
            }

            self.index_queue.push(job);
        }

        // Start indexing if not already running/paused.
        if self.indexing_state == IndexingState::Idle && self.has_pending_jobs() {
            self.indexing_state = IndexingState::Indexing;
            actions.push(LifecycleAction::StartIndexing);
        }

        actions
    }

    /// Returns true if there are pending jobs in the queue.
    fn has_pending_jobs(&self) -> bool {
        self.index_queue
            .iter()
            .any(|j| j.status == JobStatus::Pending)
    }

    // ------------------------------------------------------------------
    // SRCH-011/012: Job status updates
    // ------------------------------------------------------------------

    /// Mark an asset as completed.
    pub fn mark_completed(&mut self, asset_id: &str) {
        if let Some(job) = self.index_queue.iter_mut().find(|j| j.asset_id == asset_id) {
            job.status = JobStatus::Completed;
        }
    }

    /// Mark an asset as failed (SRCH-011: not retried in this batch).
    pub fn mark_failed(&mut self, asset_id: &str) {
        if let Some(job) = self.index_queue.iter_mut().find(|j| j.asset_id == asset_id) {
            job.status = JobStatus::Failed;
        }
    }

    /// Mark an asset as missing (SRCH-012: treated as completed).
    pub fn mark_missing(&mut self, asset_id: &str) {
        if let Some(job) = self.index_queue.iter_mut().find(|j| j.asset_id == asset_id) {
            job.status = JobStatus::Missing;
        }
    }

    /// Get the next pending job (for the caller to process).
    pub fn next_pending_job(&self) -> Option<&IndexJob> {
        self.index_queue
            .iter()
            .find(|j| j.status == JobStatus::Pending)
    }

    /// Check if the current batch is completely finished.
    pub fn is_batch_complete(&self) -> bool {
        self.index_queue.iter().all(|j| j.status.is_terminal())
    }

    // ------------------------------------------------------------------
    // SRCH-013/014: Export pause
    // ------------------------------------------------------------------

    /// Pause indexing for export. SRCH-014: refcounted.
    pub fn pause_for_export(&mut self) -> Vec<LifecycleAction> {
        let mut actions = Vec::new();
        self.export_pause_refcount += 1;
        if self.export_pause_refcount == 1 && self.indexing_state == IndexingState::Indexing {
            self.indexing_state = IndexingState::Paused;
            actions.push(LifecycleAction::PauseIndexing);
        }
        actions
    }

    /// Resume indexing after export. SRCH-014: refcounted.
    pub fn resume_after_export(&mut self) -> Vec<LifecycleAction> {
        let mut actions = Vec::new();
        if self.export_pause_refcount > 0 {
            self.export_pause_refcount -= 1;
        }
        if self.export_pause_refcount == 0 && self.indexing_state == IndexingState::Paused {
            if self.has_pending_jobs() {
                self.indexing_state = IndexingState::Indexing;
                actions.push(LifecycleAction::ResumeIndexing);
            } else {
                self.indexing_state = IndexingState::Idle;
            }
        }
        actions
    }

    // ------------------------------------------------------------------
    // Batch completion
    // ------------------------------------------------------------------

    /// Finish the current batch. Failed items may be retried in a later
    /// sweep (SRCH-011). Clears the queue and increments batch count.
    pub fn finish_batch(&mut self) {
        self.index_queue.clear();
        self.batch_count += 1;
        if self.indexing_state == IndexingState::Indexing {
            self.indexing_state = IndexingState::Idle;
        }
    }
}

impl Default for SearchLifecycle {
    fn default() -> Self {
        Self::new()
    }
}

/// Actions that the lifecycle state machine instructs the caller to perform.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LifecycleAction {
    /// Prepare/search model (download/load).
    PrepareModel,
    /// Unload the model but keep indexes (SRCH-004).
    UnloadModel,
    /// Sweep current assets for indexing.
    SweepAssets,
    /// Start indexing the queue.
    StartIndexing,
    /// Cancel in-flight indexing (SRCH-004).
    CancelIndexing,
    /// Pause indexing (SRCH-013).
    PauseIndexing,
    /// Resume indexing after pause.
    ResumeIndexing,
    /// Delete installed model files (SRCH-006).
    DeleteModelFiles,
    /// Reset coordinators (SRCH-006).
    ResetCoordinators,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn audio_input(id: &str) -> IndexJobInput {
        IndexJobInput {
            asset_id: id.into(),
            media_type: "audio".into(),
            has_audio: true,
            is_generating: false,
        }
    }

    fn _video_input(id: &str) -> IndexJobInput {
        IndexJobInput {
            asset_id: id.into(),
            media_type: "video".into(),
            has_audio: true,
            is_generating: false,
        }
    }

    fn _image_input(id: &str) -> IndexJobInput {
        IndexJobInput {
            asset_id: id.into(),
            media_type: "image".into(),
            has_audio: false,
            is_generating: false,
        }
    }

    fn _silent_video_input(id: &str) -> IndexJobInput {
        IndexJobInput {
            asset_id: id.into(),
            media_type: "video".into(),
            has_audio: false,
            is_generating: false,
        }
    }

    fn text_input(id: &str) -> IndexJobInput {
        IndexJobInput {
            asset_id: id.into(),
            media_type: "text".into(),
            has_audio: false,
            is_generating: false,
        }
    }

    fn generating_input(id: &str) -> IndexJobInput {
        IndexJobInput {
            asset_id: id.into(),
            media_type: "audio".into(),
            has_audio: true,
            is_generating: true,
        }
    }

    #[test]
    fn srch_001_enabled_by_default() {
        let lifecycle = SearchLifecycle::new();
        assert!(lifecycle.is_enabled(), "SRCH-001: enabled by default");
    }

    #[test]
    fn srch_003_state_persists() {
        let mut lifecycle = SearchLifecycle::new();
        assert!(lifecycle.is_enabled());
        lifecycle.disable();
        assert!(!lifecycle.is_enabled());

        // Re-enable
        lifecycle.enable();
        assert!(lifecycle.is_enabled());
    }

    #[test]
    fn srch_004_disable_cancels_indexing_unloads_model() {
        let mut lifecycle = SearchLifecycle::new();
        // Simulate model loaded and indexing active
        lifecycle.model_loaded = true;
        lifecycle.indexing_state = IndexingState::Indexing;
        lifecycle.index_queue.push(IndexJob {
            asset_id: "a1".into(),
            media_type: "video".into(),
            has_audio: true,
            is_generating: false,
            status: JobStatus::Pending,
        });

        let actions = lifecycle.disable();
        assert!(!lifecycle.is_enabled());
        assert!(!lifecycle.is_model_loaded());
        assert_eq!(lifecycle.indexing_state(), IndexingState::Idle);
        assert!(lifecycle.index_queue().is_empty());
        assert!(actions.contains(&LifecycleAction::CancelIndexing));
        assert!(actions.contains(&LifecycleAction::UnloadModel));
    }

    #[test]
    fn srch_005_reenable_prepares_model_sweeps_assets() {
        let mut lifecycle = SearchLifecycle::new();
        lifecycle.disable();

        let actions = lifecycle.enable();
        assert!(lifecycle.is_enabled());
        assert!(actions.contains(&LifecycleAction::PrepareModel));
        assert!(actions.contains(&LifecycleAction::SweepAssets));
    }

    #[test]
    fn srch_006_remove_model_deletes_files() {
        let mut lifecycle = SearchLifecycle::new();
        lifecycle.model_loaded = true;

        let actions = lifecycle.remove_model();
        assert!(!lifecycle.is_model_loaded());
        assert_eq!(lifecycle.indexing_state(), IndexingState::Idle);
        assert!(actions.contains(&LifecycleAction::DeleteModelFiles));
        assert!(actions.contains(&LifecycleAction::ResetCoordinators));
    }

    #[test]
    fn srch_002_project_open_triggers_prepare_and_sweep() {
        let mut lifecycle = SearchLifecycle::new();
        let actions = lifecycle.on_project_opened();
        assert!(actions.contains(&LifecycleAction::PrepareModel));
        assert!(actions.contains(&LifecycleAction::SweepAssets));
    }

    #[test]
    fn srch_002_project_open_disabled_no_actions() {
        let mut lifecycle = SearchLifecycle::new();
        lifecycle.disable();
        let actions = lifecycle.on_project_opened();
        assert!(actions.is_empty());
    }

    #[test]
    fn srch_007_visual_eligibility() {
        assert!(IndexJob {
            asset_id: "v1".into(),
            media_type: "video".into(),
            has_audio: false,
            is_generating: false,
            status: JobStatus::Pending,
        }
        .is_visual_eligible());

        assert!(IndexJob {
            asset_id: "i1".into(),
            media_type: "image".into(),
            has_audio: false,
            is_generating: false,
            status: JobStatus::Pending,
        }
        .is_visual_eligible());

        assert!(!IndexJob {
            asset_id: "a1".into(),
            media_type: "audio".into(),
            has_audio: true,
            is_generating: false,
            status: JobStatus::Pending,
        }
        .is_visual_eligible());
    }

    #[test]
    fn srch_008_transcript_eligibility() {
        // Audio is always transcript-eligible
        assert!(IndexJob {
            asset_id: "a1".into(),
            media_type: "audio".into(),
            has_audio: true,
            is_generating: false,
            status: JobStatus::Pending,
        }
        .is_transcript_eligible());

        // Video with audio is transcript-eligible
        assert!(IndexJob {
            asset_id: "v1".into(),
            media_type: "video".into(),
            has_audio: true,
            is_generating: false,
            status: JobStatus::Pending,
        }
        .is_transcript_eligible());

        // Video without audio is NOT transcript-eligible
        assert!(!IndexJob {
            asset_id: "v2".into(),
            media_type: "video".into(),
            has_audio: false,
            is_generating: false,
            status: JobStatus::Pending,
        }
        .is_transcript_eligible());

        // Image is not transcript-eligible
        assert!(!IndexJob {
            asset_id: "i1".into(),
            media_type: "image".into(),
            has_audio: false,
            is_generating: false,
            status: JobStatus::Pending,
        }
        .is_transcript_eligible());
    }

    #[test]
    fn srch_009_generating_assets_skipped() {
        let mut lifecycle = SearchLifecycle::new();
        let actions = lifecycle.register_assets(vec![generating_input("gen1")]);
        // No indexing should start since the only asset is being generated.
        assert!(actions.is_empty());
        assert_eq!(lifecycle.queue_len(), 0);
    }

    #[test]
    fn srch_010_dedupe_within_batch() {
        let mut lifecycle = SearchLifecycle::new();
        let inputs = vec![audio_input("a1"), audio_input("a1"), audio_input("a2")];
        lifecycle.register_assets(inputs);
        assert_eq!(
            lifecycle.queue_len(),
            2,
            "SRCH-010: deduped to 2 unique assets"
        );
    }

    #[test]
    fn srch_011_failed_not_retried_in_same_batch() {
        let mut lifecycle = SearchLifecycle::new();
        lifecycle.register_assets(vec![audio_input("a1")]);
        lifecycle.mark_failed("a1");
        assert!(lifecycle.index_queue()[0].status == JobStatus::Failed);

        // In the same batch, it remains failed
        let next = lifecycle.next_pending_job();
        assert!(next.is_none());

        // Finish batch, then in a new batch it can be retried
        lifecycle.finish_batch();
        lifecycle.register_assets(vec![audio_input("a1")]);
        assert_eq!(lifecycle.queue_len(), 1);
        assert_eq!(lifecycle.index_queue()[0].status, JobStatus::Pending);
    }

    #[test]
    fn srch_012_missing_treated_as_completed() {
        let mut lifecycle = SearchLifecycle::new();
        lifecycle.register_assets(vec![audio_input("a1")]);
        lifecycle.mark_missing("a1");

        assert_eq!(lifecycle.index_queue()[0].status, JobStatus::Missing);
        assert!(lifecycle.index_queue()[0].status.is_terminal());
        assert!(lifecycle.is_batch_complete());
    }

    #[test]
    fn srch_013_pause_while_exporting() {
        let mut lifecycle = SearchLifecycle::new();
        lifecycle.register_assets(vec![audio_input("a1")]);
        // Should be indexing now
        assert_eq!(lifecycle.indexing_state(), IndexingState::Indexing);

        let actions = lifecycle.pause_for_export();
        assert_eq!(lifecycle.indexing_state(), IndexingState::Paused);
        assert!(actions.contains(&LifecycleAction::PauseIndexing));
    }

    #[test]
    fn srch_014_refcounted_export_pause() {
        let mut lifecycle = SearchLifecycle::new();
        lifecycle.register_assets(vec![audio_input("a1")]);

        // Two concurrent exports
        lifecycle.pause_for_export();
        lifecycle.pause_for_export();
        assert_eq!(lifecycle.export_pause_refcount(), 2);
        assert_eq!(lifecycle.indexing_state(), IndexingState::Paused);

        // Resume one export → still paused
        lifecycle.resume_after_export();
        assert_eq!(lifecycle.export_pause_refcount(), 1);
        assert_eq!(lifecycle.indexing_state(), IndexingState::Paused);

        // Resume the other → should resume indexing
        lifecycle.resume_after_export();
        assert_eq!(lifecycle.export_pause_refcount(), 0);
        assert_eq!(lifecycle.indexing_state(), IndexingState::Indexing);
    }

    #[test]
    fn srch_014_resume_with_no_pending_jobs_goes_idle() {
        let mut lifecycle = SearchLifecycle::new();
        lifecycle.indexing_state = IndexingState::Paused;
        lifecycle.export_pause_refcount = 1;

        lifecycle.resume_after_export();
        assert_eq!(lifecycle.indexing_state(), IndexingState::Idle);
    }

    #[test]
    fn register_assets_starts_indexing() {
        let mut lifecycle = SearchLifecycle::new();
        let actions = lifecycle.register_assets(vec![audio_input("a1")]);
        assert_eq!(lifecycle.indexing_state(), IndexingState::Indexing);
        assert!(actions.contains(&LifecycleAction::StartIndexing));
    }

    #[test]
    fn register_assets_no_eligible_does_not_start() {
        let mut lifecycle = SearchLifecycle::new();
        // Text assets are ineligible
        let actions = lifecycle.register_assets(vec![text_input("t1")]);
        assert!(actions.is_empty());
        assert_eq!(lifecycle.queue_len(), 0);
    }

    #[test]
    fn batch_completion_clears_queue() {
        let mut lifecycle = SearchLifecycle::new();
        lifecycle.register_assets(vec![audio_input("a1"), audio_input("a2")]);
        lifecycle.mark_completed("a1");
        lifecycle.mark_completed("a2");
        assert!(lifecycle.is_batch_complete());

        lifecycle.finish_batch();
        assert_eq!(lifecycle.queue_len(), 0);
        assert_eq!(lifecycle.batch_count(), 1);
        assert_eq!(lifecycle.indexing_state(), IndexingState::Idle);
    }

    #[test]
    fn next_pending_job_returns_none_when_empty() {
        let lifecycle = SearchLifecycle::new();
        assert!(lifecycle.next_pending_job().is_none());
    }

    #[test]
    fn disabled_register_assets_noop() {
        let mut lifecycle = SearchLifecycle::new();
        lifecycle.disable();
        let actions = lifecycle.register_assets(vec![audio_input("a1")]);
        assert!(actions.is_empty());
        assert_eq!(lifecycle.queue_len(), 0);
    }

    #[test]
    fn multiple_batches_track_count() {
        let mut lifecycle = SearchLifecycle::new();
        assert_eq!(lifecycle.batch_count(), 0);
        lifecycle.finish_batch();
        assert_eq!(lifecycle.batch_count(), 1);
        lifecycle.finish_batch();
        assert_eq!(lifecycle.batch_count(), 2);
    }
}
