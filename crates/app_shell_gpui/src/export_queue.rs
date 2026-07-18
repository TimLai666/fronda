//! Pure FIFO export queue state machine (upstream #298 `ExportQueue`).
//!
//! No I/O, no clocks, no executor: `now_ms` is injected, job ids are
//! queue-monotonic, and the host drives every transition. At most one job
//! runs at a time; `next_ready` is the FIFO scheduling query. A destination
//! path stays reserved while any job targeting it is pending. Illegal
//! transitions return `Err` — never panic.

use std::fmt;
use std::path::{Path, PathBuf};

pub type JobId = u64;

/// Job lifecycle states, mirroring Swift `ExportJobStatus`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportJobStatus {
    Waiting,
    Preparing,
    Exporting,
    Canceling,
    Completed,
    Failed,
    Canceled,
}

impl ExportJobStatus {
    pub fn is_running(self) -> bool {
        matches!(self, Self::Preparing | Self::Exporting | Self::Canceling)
    }

    pub fn is_finished(self) -> bool {
        matches!(self, Self::Completed | Self::Failed | Self::Canceled)
    }

    pub fn is_pending(self) -> bool {
        self == Self::Waiting || self.is_running()
    }

    /// Display label (Swift rawValues: waiting="queued", exporting="rendering").
    pub fn label(self) -> &'static str {
        match self {
            Self::Waiting => "Queued",
            Self::Preparing => "Preparing",
            Self::Exporting => "Rendering",
            Self::Canceling => "Canceling",
            Self::Completed => "Completed",
            Self::Failed => "Failed",
            Self::Canceled => "Canceled",
        }
    }
}

#[derive(Debug, Clone)]
pub struct ExportJob {
    pub id: JobId,
    pub project_id: String,
    pub filename: String,
    pub output_path: PathBuf,
    pub created_at_ms: u64,
    pub status: ExportJobStatus,
    pub progress: f64,
    pub error: Option<String>,
    committed: bool,
}

impl ExportJob {
    /// Whether the staged output was committed to the destination
    /// (Swift `ExportService.didCommitOutput`). A committed job refuses `cancel`.
    pub fn is_committed(&self) -> bool {
        self.committed
    }
}

/// Result of `enqueue` (Swift `ExportQueueSubmission`). `started` means the job
/// is the immediate `next_ready` candidate when the host drives the queue;
/// `queue_position` is its 1-based place among the jobs that will wait (0 when
/// started).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExportQueueSubmission {
    pub job_id: JobId,
    pub started: bool,
    pub queue_position: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExportQueueError {
    /// A pending job already targets this destination path (carries filename).
    DestinationInUse(String),
    UnknownJob(JobId),
    InvalidTransition {
        job: JobId,
        from: ExportJobStatus,
        to: ExportJobStatus,
    },
    /// `mark_preparing` while a different job is running.
    AnotherJobRunning(JobId),
}

impl fmt::Display for ExportQueueError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DestinationInUse(filename) => write!(
                f,
                "An export to {filename} is already waiting or in progress."
            ),
            Self::UnknownJob(id) => write!(f, "No export job {id}."),
            Self::InvalidTransition { job, from, to } => {
                write!(f, "Export job {job} can't go {from:?} → {to:?}.")
            }
            Self::AnotherJobRunning(id) => write!(f, "Export job {id} is already running."),
        }
    }
}

#[derive(Debug, Default)]
pub struct ExportQueue {
    jobs: Vec<ExportJob>,
    next_id: JobId,
}

impl ExportQueue {
    pub fn new() -> Self {
        Self::default()
    }

    /// Snapshot of every job, oldest first.
    pub fn jobs(&self) -> &[ExportJob] {
        &self.jobs
    }

    pub fn job(&self, id: JobId) -> Option<&ExportJob> {
        self.jobs.iter().find(|j| j.id == id)
    }

    pub fn jobs_for(&self, project_id: &str) -> Vec<&ExportJob> {
        self.jobs
            .iter()
            .filter(|j| j.project_id == project_id)
            .collect()
    }

    /// Any job waiting or running.
    pub fn has_activity(&self) -> bool {
        self.jobs.iter().any(|j| j.status.is_pending())
    }

    /// A job is currently running (preparing / exporting / canceling).
    pub fn is_export_active(&self) -> bool {
        self.jobs.iter().any(|j| j.status.is_running())
    }

    pub fn is_destination_reserved(&self, path: &Path) -> bool {
        self.jobs
            .iter()
            .any(|j| j.status.is_pending() && j.output_path == path)
    }

    pub fn enqueue(
        &mut self,
        output_path: impl Into<PathBuf>,
        project_id: &str,
        now_ms: u64,
    ) -> Result<ExportQueueSubmission, ExportQueueError> {
        let output_path = output_path.into();
        let filename = output_path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| output_path.to_string_lossy().into_owned());
        if self.is_destination_reserved(&output_path) {
            return Err(ExportQueueError::DestinationInUse(filename));
        }

        self.next_id += 1;
        let id = self.next_id;
        self.jobs.push(ExportJob {
            id,
            project_id: project_id.to_string(),
            filename,
            output_path,
            created_at_ms: now_ms,
            status: ExportJobStatus::Waiting,
            progress: 0.0,
            error: None,
            committed: false,
        });

        let head = self.next_ready();
        let started = head == Some(id);
        let queue_position = if started {
            0
        } else {
            self.jobs
                .iter()
                .filter(|j| j.status == ExportJobStatus::Waiting && head != Some(j.id))
                .position(|j| j.id == id)
                .map(|i| i + 1)
                .unwrap_or(0)
        };
        Ok(ExportQueueSubmission {
            job_id: id,
            started,
            queue_position,
        })
    }

    /// FIFO scheduling query: the first waiting job, only when nothing runs.
    pub fn next_ready(&self) -> Option<JobId> {
        if self.is_export_active() {
            return None;
        }
        self.jobs
            .iter()
            .find(|j| j.status == ExportJobStatus::Waiting)
            .map(|j| j.id)
    }

    pub fn mark_preparing(&mut self, id: JobId) -> Result<(), ExportQueueError> {
        self.job_mut(id)?;
        if let Some(running) = self.jobs.iter().find(|j| j.status.is_running() && j.id != id) {
            return Err(ExportQueueError::AnotherJobRunning(running.id));
        }
        self.transition(id, ExportJobStatus::Preparing, |from| {
            from == ExportJobStatus::Waiting
        })
    }

    pub fn mark_exporting(&mut self, id: JobId) -> Result<(), ExportQueueError> {
        let job = self.job_mut(id)?;
        match job.status {
            ExportJobStatus::Preparing => {
                job.status = ExportJobStatus::Exporting;
                Ok(())
            }
            // Late phase updates on a canceling or already-exporting job are
            // ignored (upstream `update(phase)` guard), not errors.
            ExportJobStatus::Canceling | ExportJobStatus::Exporting => Ok(()),
            from => Err(ExportQueueError::InvalidTransition {
                job: id,
                from,
                to: ExportJobStatus::Exporting,
            }),
        }
    }

    /// Clamped to 0..=1; applies only while the job is pending (late updates
    /// after a finish are ignored, mirroring upstream `update(progress)`).
    pub fn set_progress(&mut self, id: JobId, progress: f64) -> Result<(), ExportQueueError> {
        let job = self.job_mut(id)?;
        if job.status.is_pending() {
            job.progress = progress.clamp(0.0, 1.0);
        }
        Ok(())
    }

    /// The host committed the staged output to the destination. From here the
    /// job refuses `cancel` (Swift `ExportService.didCommitOutput`).
    pub fn mark_committed(&mut self, id: JobId) -> Result<(), ExportQueueError> {
        let job = self.job_mut(id)?;
        if !job.status.is_running() {
            return Err(ExportQueueError::InvalidTransition {
                job: id,
                from: job.status,
                to: job.status,
            });
        }
        job.committed = true;
        Ok(())
    }

    /// Running → Completed with progress pinned to 1. Also legal from
    /// Canceling: a commit that raced past a cancel keeps the export completed.
    pub fn mark_completed(&mut self, id: JobId) -> Result<(), ExportQueueError> {
        self.transition(id, ExportJobStatus::Completed, ExportJobStatus::is_running)
            .map(|()| {
                if let Ok(job) = self.job_mut(id) {
                    job.progress = 1.0;
                }
            })
    }

    pub fn mark_failed(
        &mut self,
        id: JobId,
        error: impl Into<String>,
    ) -> Result<(), ExportQueueError> {
        let error = error.into();
        self.transition(id, ExportJobStatus::Failed, ExportJobStatus::is_running)
            .map(|()| {
                if let Ok(job) = self.job_mut(id) {
                    job.error = Some(error);
                }
            })
    }

    /// Request cancellation (Swift `ExportQueue.cancel` bool semantics):
    /// waiting → canceled immediately (true); running & uncommitted → canceling
    /// (true, host confirms via `mark_canceled`); canceling → true; committed,
    /// finished, or unknown → false.
    pub fn cancel(&mut self, id: JobId) -> bool {
        let Ok(job) = self.job_mut(id) else {
            return false;
        };
        match job.status {
            ExportJobStatus::Waiting => {
                job.status = ExportJobStatus::Canceled;
                true
            }
            ExportJobStatus::Preparing | ExportJobStatus::Exporting => {
                if job.committed {
                    return false;
                }
                job.status = ExportJobStatus::Canceling;
                true
            }
            // Already canceling — but a commit may have raced in past the cancel
            // (mark_committed is legal while Canceling). A committed job is going
            // to complete, so it refuses cancel here too, per the contract.
            ExportJobStatus::Canceling => !job.committed,
            _ => false,
        }
    }

    /// Host confirmation that a running job stopped without output.
    pub fn mark_canceled(&mut self, id: JobId) -> Result<(), ExportQueueError> {
        self.transition(id, ExportJobStatus::Canceled, ExportJobStatus::is_running)
    }

    /// Remove a finished job from the list; `false` for pending or unknown jobs.
    pub fn remove(&mut self, id: JobId) -> bool {
        let finished = self.job(id).is_some_and(|j| j.status.is_finished());
        if finished {
            self.jobs.retain(|j| j.id != id);
        }
        finished
    }

    pub fn clear_finished(&mut self, project_id: &str) {
        self.jobs
            .retain(|j| !(j.project_id == project_id && j.status.is_finished()));
    }

    fn job_mut(&mut self, id: JobId) -> Result<&mut ExportJob, ExportQueueError> {
        self.jobs
            .iter_mut()
            .find(|j| j.id == id)
            .ok_or(ExportQueueError::UnknownJob(id))
    }

    fn transition(
        &mut self,
        id: JobId,
        to: ExportJobStatus,
        allowed_from: impl Fn(ExportJobStatus) -> bool,
    ) -> Result<(), ExportQueueError> {
        let job = self.job_mut(id)?;
        if !allowed_from(job.status) {
            return Err(ExportQueueError::InvalidTransition {
                job: id,
                from: job.status,
                to,
            });
        }
        job.status = to;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn enqueue(q: &mut ExportQueue, name: &str) -> ExportQueueSubmission {
        enqueue_for(q, name, "test-project")
    }

    fn enqueue_for(q: &mut ExportQueue, name: &str, project: &str) -> ExportQueueSubmission {
        q.enqueue(PathBuf::from(format!("/tmp/export-queue/{name}")), project, 1_000)
            .expect("enqueue")
    }

    fn status(q: &ExportQueue, id: JobId) -> ExportJobStatus {
        q.job(id).expect("job exists").status
    }

    // Upstream: runsInFIFOOrder — the first job starts, the second waits at
    // position 1, and execution order follows enqueue order.
    #[test]
    fn runs_in_fifo_order() {
        let mut q = ExportQueue::new();
        let first = enqueue(&mut q, "first.mov");
        assert!(first.started);
        assert_eq!(first.queue_position, 0);
        assert_eq!(q.next_ready(), Some(first.job_id));

        q.mark_preparing(first.job_id).unwrap();
        q.mark_exporting(first.job_id).unwrap();

        let second = enqueue(&mut q, "second.mov");
        assert!(!second.started);
        assert_eq!(second.queue_position, 1);
        assert_eq!(q.next_ready(), None, "at most one job runs at a time");

        q.mark_committed(first.job_id).unwrap();
        q.mark_completed(first.job_id).unwrap();
        assert_eq!(q.next_ready(), Some(second.job_id), "FIFO advances");

        q.mark_preparing(second.job_id).unwrap();
        q.mark_exporting(second.job_id).unwrap();
        q.mark_committed(second.job_id).unwrap();
        q.mark_completed(second.job_id).unwrap();
        assert!(!q.has_activity());
        assert_eq!(status(&q, first.job_id), ExportJobStatus::Completed);
        assert_eq!(status(&q, second.job_id), ExportJobStatus::Completed);
    }

    // Upstream: cancelingPreparingJobAdvancesQueueWithoutRunningIt.
    #[test]
    fn canceling_preparing_job_advances_queue_without_running_it() {
        let mut q = ExportQueue::new();
        let first = enqueue(&mut q, "active.mov");
        q.mark_preparing(first.job_id).unwrap();
        let second = enqueue(&mut q, "next.mov");

        assert!(q.cancel(first.job_id));
        assert_eq!(status(&q, first.job_id), ExportJobStatus::Canceling);
        assert_eq!(q.next_ready(), None, "canceling still occupies the slot");

        q.mark_canceled(first.job_id).unwrap();
        assert_eq!(status(&q, first.job_id), ExportJobStatus::Canceled);
        assert_eq!(q.next_ready(), Some(second.job_id), "queue advances");

        // The advanced job produced no output → host fails it (upstream message).
        q.mark_preparing(second.job_id).unwrap();
        q.mark_failed(second.job_id, "Export produced no output.").unwrap();
        assert_eq!(status(&q, second.job_id), ExportJobStatus::Failed);
        assert_eq!(
            q.job(second.job_id).unwrap().error.as_deref(),
            Some("Export produced no output.")
        );
        assert!(!q.has_activity());
    }

    // Upstream: cancelingWaitingJobDoesNotRunIt.
    #[test]
    fn canceling_waiting_job_does_not_run_it() {
        let mut q = ExportQueue::new();
        let blocker = enqueue(&mut q, "waiting-blocker.mov");
        q.mark_preparing(blocker.job_id).unwrap();
        let waiting = enqueue(&mut q, "waiting.mov");

        assert!(q.cancel(waiting.job_id));
        assert_eq!(
            status(&q, waiting.job_id),
            ExportJobStatus::Canceled,
            "waiting jobs cancel immediately"
        );

        assert!(q.cancel(blocker.job_id));
        q.mark_canceled(blocker.job_id).unwrap();
        assert!(!q.has_activity());
        assert_eq!(q.next_ready(), None, "the canceled waiting job never runs");
    }

    // Upstream: scopesHistoryByProject.
    #[test]
    fn scopes_history_by_project() {
        let mut q = ExportQueue::new();
        let first = enqueue_for(&mut q, "project-first.xml", "project-a");
        q.mark_preparing(first.job_id).unwrap();
        q.mark_completed(first.job_id).unwrap();
        let second = enqueue_for(&mut q, "project-second.xml", "project-b");
        q.mark_preparing(second.job_id).unwrap();
        q.mark_completed(second.job_id).unwrap();

        let ids = |jobs: Vec<&ExportJob>| jobs.iter().map(|j| j.id).collect::<Vec<_>>();
        assert_eq!(ids(q.jobs_for("project-a")), vec![first.job_id]);
        assert_eq!(ids(q.jobs_for("project-b")), vec![second.job_id]);

        q.clear_finished("project-a");
        assert!(q.jobs_for("project-a").is_empty());
        assert_eq!(ids(q.jobs_for("project-b")), vec![second.job_id]);
    }

    // Upstream: lateProgressAndCancellationKeepCommittedExportCompleted.
    #[test]
    fn late_progress_and_cancellation_keep_committed_export_completed() {
        let mut q = ExportQueue::new();
        let sub = enqueue(&mut q, "late-cancel.xml");
        q.mark_preparing(sub.job_id).unwrap();
        q.mark_exporting(sub.job_id).unwrap();
        q.mark_committed(sub.job_id).unwrap();

        assert!(!q.cancel(sub.job_id), "committed output refuses cancellation");
        assert_eq!(status(&q, sub.job_id), ExportJobStatus::Exporting);

        q.mark_completed(sub.job_id).unwrap();
        assert_eq!(status(&q, sub.job_id), ExportJobStatus::Completed);
        assert_eq!(q.job(sub.job_id).unwrap().progress, 1.0);

        // A late progress update after completion changes nothing.
        q.set_progress(sub.job_id, 0.25).unwrap();
        assert_eq!(q.job(sub.job_id).unwrap().progress, 1.0);
        assert!(!q.cancel(sub.job_id), "finished jobs refuse cancellation");
    }

    // A cancel raced past the commit: mark_completed still wins from Canceling.
    #[test]
    fn commit_race_resolves_canceling_to_completed() {
        let mut q = ExportQueue::new();
        let sub = enqueue(&mut q, "race.mp4");
        q.mark_preparing(sub.job_id).unwrap();
        q.mark_exporting(sub.job_id).unwrap();
        assert!(q.cancel(sub.job_id));
        assert_eq!(status(&q, sub.job_id), ExportJobStatus::Canceling);
        // Late phase/progress updates while canceling are ignored, not errors.
        q.mark_exporting(sub.job_id).unwrap();
        assert_eq!(status(&q, sub.job_id), ExportJobStatus::Canceling);

        q.mark_completed(sub.job_id).unwrap();
        assert_eq!(status(&q, sub.job_id), ExportJobStatus::Completed);
    }

    // A commit that races in AFTER the cancel (legal while Canceling) makes the
    // job committed; a second cancel must then refuse (contract: committed →
    // false), and the job still completes.
    #[test]
    fn cancel_refuses_once_a_commit_races_in_while_canceling() {
        let mut q = ExportQueue::new();
        let sub = enqueue(&mut q, "commit-then-cancel.mp4");
        q.mark_preparing(sub.job_id).unwrap();
        q.mark_exporting(sub.job_id).unwrap();
        assert!(q.cancel(sub.job_id));
        assert_eq!(status(&q, sub.job_id), ExportJobStatus::Canceling);
        q.mark_committed(sub.job_id).unwrap();
        assert!(
            !q.cancel(sub.job_id),
            "a committed (racing) job refuses a second cancel even while canceling"
        );
        q.mark_completed(sub.job_id).unwrap();
        assert_eq!(status(&q, sub.job_id), ExportJobStatus::Completed);
    }

    // Spec scenario: duplicate destination rejected until the first job
    // releases it (finish or cancel).
    #[test]
    fn duplicate_destination_rejected_until_released() {
        let mut q = ExportQueue::new();
        let path = PathBuf::from("/tmp/export-queue/same.mp4");
        let first = q.enqueue(path.clone(), "p", 0).unwrap();
        assert!(q.is_destination_reserved(&path));
        assert_eq!(
            q.enqueue(path.clone(), "p", 0),
            Err(ExportQueueError::DestinationInUse("same.mp4".into()))
        );

        q.mark_preparing(first.job_id).unwrap();
        assert_eq!(
            q.enqueue(path.clone(), "p", 0),
            Err(ExportQueueError::DestinationInUse("same.mp4".into())),
            "still reserved while running"
        );
        q.mark_completed(first.job_id).unwrap();
        assert!(!q.is_destination_reserved(&path));
        let again = q.enqueue(path.clone(), "p", 0).expect("released after finish");

        // A canceled waiting job releases its destination too.
        let other = PathBuf::from("/tmp/export-queue/other.mp4");
        let waiting = q.enqueue(other.clone(), "p", 0).unwrap();
        assert!(q.cancel(waiting.job_id));
        assert!(!q.is_destination_reserved(&other));
        q.enqueue(other, "p", 0).expect("released after cancel");
        let _ = again;
    }

    #[test]
    fn illegal_transitions_return_err_not_panic() {
        let mut q = ExportQueue::new();
        let a = enqueue(&mut q, "a.mp4");
        let b = enqueue(&mut q, "b.mp4");

        // Exporting/finishing a job that never started.
        assert!(matches!(
            q.mark_exporting(a.job_id),
            Err(ExportQueueError::InvalidTransition { .. })
        ));
        assert!(matches!(
            q.mark_completed(a.job_id),
            Err(ExportQueueError::InvalidTransition { .. })
        ));
        assert!(matches!(
            q.mark_failed(a.job_id, "x"),
            Err(ExportQueueError::InvalidTransition { .. })
        ));
        assert!(matches!(
            q.mark_canceled(a.job_id),
            Err(ExportQueueError::InvalidTransition { .. })
        ));
        assert!(matches!(
            q.mark_committed(a.job_id),
            Err(ExportQueueError::InvalidTransition { .. })
        ));

        // Starting a second job while one runs.
        q.mark_preparing(a.job_id).unwrap();
        assert!(matches!(
            q.mark_preparing(b.job_id),
            Err(ExportQueueError::AnotherJobRunning(_))
        ));

        // Re-preparing a running job.
        assert!(matches!(
            q.mark_preparing(a.job_id),
            Err(ExportQueueError::InvalidTransition { .. })
        ));

        // Unknown ids.
        assert!(matches!(
            q.mark_preparing(9999),
            Err(ExportQueueError::UnknownJob(9999))
        ));
        assert!(matches!(
            q.set_progress(9999, 0.5),
            Err(ExportQueueError::UnknownJob(9999))
        ));
        assert!(!q.cancel(9999));

        // Finished jobs reject further transitions.
        q.mark_completed(a.job_id).unwrap();
        assert!(matches!(
            q.mark_completed(a.job_id),
            Err(ExportQueueError::InvalidTransition { .. })
        ));
        assert!(matches!(
            q.mark_canceled(a.job_id),
            Err(ExportQueueError::InvalidTransition { .. })
        ));
    }

    #[test]
    fn progress_clamps_and_only_applies_while_pending() {
        let mut q = ExportQueue::new();
        let sub = enqueue(&mut q, "p.mp4");
        q.set_progress(sub.job_id, -0.5).unwrap();
        assert_eq!(q.job(sub.job_id).unwrap().progress, 0.0);
        q.set_progress(sub.job_id, 1.5).unwrap();
        assert_eq!(q.job(sub.job_id).unwrap().progress, 1.0);
        q.mark_preparing(sub.job_id).unwrap();
        q.mark_exporting(sub.job_id).unwrap();
        q.set_progress(sub.job_id, 0.4).unwrap();
        assert_eq!(q.job(sub.job_id).unwrap().progress, 0.4);
        q.mark_failed(sub.job_id, "boom").unwrap();
        q.set_progress(sub.job_id, 0.9).unwrap();
        assert_eq!(q.job(sub.job_id).unwrap().progress, 0.4, "frozen after finish");
    }

    #[test]
    fn remove_only_removes_finished_jobs() {
        let mut q = ExportQueue::new();
        let a = enqueue(&mut q, "r1.mp4");
        let b = enqueue(&mut q, "r2.mp4");
        assert!(!q.remove(a.job_id), "pending jobs stay");
        q.mark_preparing(a.job_id).unwrap();
        q.mark_completed(a.job_id).unwrap();
        assert!(q.remove(a.job_id));
        assert!(q.job(a.job_id).is_none());
        assert!(q.job(b.job_id).is_some());
        assert!(!q.remove(9999));
    }

    #[test]
    fn enqueue_records_metadata() {
        let mut q = ExportQueue::new();
        let sub = q
            .enqueue(PathBuf::from("/tmp/export-queue/meta.mp4"), "proj", 42)
            .unwrap();
        let job = q.job(sub.job_id).unwrap();
        assert_eq!(job.filename, "meta.mp4");
        assert_eq!(job.project_id, "proj");
        assert_eq!(job.created_at_ms, 42);
        assert_eq!(job.status, ExportJobStatus::Waiting);
        assert_eq!(job.progress, 0.0);
        assert!(job.error.is_none());
        assert!(!job.is_committed());
        assert!(q.has_activity());
        assert!(!q.is_export_active(), "waiting is pending but not running");
    }
}
