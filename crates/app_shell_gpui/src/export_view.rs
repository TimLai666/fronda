//! Export panel view — matches Swift ExportView layout (Issue #166).
//!
//! Layout: 860×560 sheet
//!   ├── HStack
//!   │   ├── settingsPanel  (360px wide, left)
//!   │   └── previewPanel  (flex, right)
//!   └── bottomBar (48px, footer)
//!
//! The settings panel shows mode-specific options (codec/resolution for Video,
//! description text for XML/Palmier). The bottom bar shows metadata on the left
//! and action buttons on the right.

#![cfg(feature = "desktop-app")]

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;

use gpui::prelude::*;
use gpui::*;

use crate::export_model::{ExportMode, ExportViewModel};
use crate::export_queue::{ExportJobStatus, ExportQueue, JobId};
use crate::theme::{Accent, Background, BorderColors, FontSize, Radius, Spacing, Status, Text};
use render_core::fcpxml_export::FcpxmlTarget;
use render_core::{ExportFormat, ExportResolution};

// Queue pane layout, mirroring Swift `AppTheme.Export` queue constants.
const QUEUE_PANE_WIDTH: f32 = 320.0;
const QUEUE_TIMESTAMP_WIDTH: f32 = 56.0;
const QUEUE_PROGRESS_BAR_WIDTH: f32 = 72.0;
const QUEUE_PROGRESS_WIDTH: f32 = 32.0;
/// Success green (Swift `AppTheme.Status.success` #4FB85F).
const STATUS_SUCCESS: gpui::Hsla = gpui::Hsla {
    h: 129.0 / 360.0,
    s: 0.43,
    l: 0.52,
    a: 1.0,
};

/// Captured settings for a queued video export, applied when its turn comes.
struct QueuedVideoJob {
    codec: crate::video_export::VideoCodec,
    resolution: ExportResolution,
    out_fps: i64,
    path: PathBuf,
}

/// Export sheet view.
pub struct ExportView {
    pub model: ExportViewModel,
    focus_handle: FocusHandle,
    // UI-only selection state (not in model)
    selected_codec: usize,      // 0=H.264, 1=H.265, 2=ProRes
    selected_resolution: usize, // 0=720p, 1=1080p, 2=2K, 3=4K, 4=Match
    selected_fps: usize,        // 0=24, 1=30, 2=60
    output_path: String,
    // Export queue (upstream #298): serialized cancellable video exports.
    queue: ExportQueue,
    pending_jobs: HashMap<JobId, QueuedVideoJob>,
    cancel_flags: HashMap<JobId, Arc<AtomicBool>>,
    submission_error: Option<String>,
}

impl ExportView {
    pub fn new(cx: &mut Context<Self>) -> Self {
        Self {
            model: ExportViewModel::new(),
            focus_handle: cx.focus_handle(),
            selected_codec: 0,
            selected_resolution: 1,
            selected_fps: 1,
            output_path: "~/Desktop/Export.mp4".to_string(),
            queue: ExportQueue::new(),
            pending_jobs: HashMap::new(),
            cancel_flags: HashMap::new(),
            submission_error: None,
        }
    }

    /// Queue scope id for the open project (Swift `exportQueueProjectID`).
    fn project_queue_id() -> String {
        crate::editor_state_hub::EditorStateHub::global()
            .project_root()
            .map(|p| p.to_string_lossy().into_owned())
            .unwrap_or_else(|| "untitled".into())
    }

    /// Reserve the destination and queue a video export; starts it immediately
    /// when nothing is running. A destination conflict surfaces as
    /// `submission_error` instead of a job.
    fn enqueue_video_job(
        &mut self,
        path: PathBuf,
        codec: crate::video_export::VideoCodec,
        resolution: ExportResolution,
        out_fps: i64,
        cx: &mut Context<Self>,
    ) {
        let now_ms = chrono::Utc::now().timestamp_millis().max(0) as u64;
        match self
            .queue
            .enqueue(path.clone(), &Self::project_queue_id(), now_ms)
        {
            Err(e) => self.submission_error = Some(e.to_string()),
            Ok(sub) => {
                self.submission_error = None;
                self.pending_jobs.insert(
                    sub.job_id,
                    QueuedVideoJob {
                        codec,
                        resolution,
                        out_fps,
                        path,
                    },
                );
                self.cancel_flags
                    .insert(sub.job_id, Arc::new(AtomicBool::new(false)));
                self.start_next_job(cx);
            }
        }
        cx.notify();
    }

    /// FIFO driver: start the next waiting job if the slot is free.
    fn start_next_job(&mut self, cx: &mut Context<Self>) {
        let Some(id) = self.queue.next_ready() else {
            return;
        };
        let Some(job) = self.pending_jobs.remove(&id) else {
            // Parameters vanished — fail the job and keep the queue moving.
            let _ = self.queue.mark_preparing(id);
            let _ = self.queue.mark_failed(id, "Export produced no output.");
            return self.start_next_job(cx);
        };
        if self.queue.mark_preparing(id).is_err() {
            return;
        }
        let cancel = self.cancel_flags.entry(id).or_default().clone();
        self.model.start();
        cx.notify();

        // Shared progress (0..=100) the encoder updates; a ticker mirrors it
        // into the queue row + inline progress bar until this job finishes.
        let progress = Arc::new(AtomicU64::new(0));
        let prog_tick = progress.clone();
        cx.spawn(async move |this, cx| loop {
            cx.background_executor()
                .timer(std::time::Duration::from_millis(150))
                .await;
            let p = prog_tick.load(Ordering::Relaxed) as f64 / 100.0;
            let running = this
                .update(cx, |view, cx| {
                    let pending = view.queue.job(id).is_some_and(|j| j.status.is_pending());
                    if pending {
                        let _ = view.queue.set_progress(id, p);
                        view.model.panel.update_progress(p, None);
                        cx.notify();
                    }
                    pending
                })
                .unwrap_or(false);
            if !running {
                break;
            }
        })
        .detach();

        let (codec, resolution, out_fps, out) =
            (job.codec, job.resolution, job.out_fps, job.path.clone());
        let prog_enc = progress.clone();
        cx.spawn(async move |this, cx| {
            let (timeline, manifest, timelines, root) = {
                let hub = crate::editor_state_hub::EditorStateHub::global();
                let exec = hub.executor();
                let guard = exec.lock().unwrap();
                let root = hub
                    .project_root()
                    .unwrap_or_else(|| std::env::home_dir().unwrap_or_else(|| ".".into()));
                (
                    guard.timeline().clone(),
                    guard.media_manifest().clone(),
                    guard.sibling_timeline_map(),
                    root,
                )
            };
            let _ = this.update(cx, |view, cx| {
                let _ = view.queue.mark_exporting(id);
                cx.notify();
            });
            let result = cx
                .background_executor()
                .spawn(async move {
                    let size = resolution.render_size(&timeline);
                    let w = size.width.max(2) as u32;
                    let h = size.height.max(2) as u32;
                    crate::audio_export::export_project_with_audio_cancellable(
                        &timeline, &manifest, &timelines, &root, &out, w, h, codec, out_fps,
                        &prog_enc, &cancel,
                    )
                    .map(|()| out.clone())
                })
                .await;
            let _ = this.update(cx, |view, cx| view.finish_job(id, result, cx));
        })
        .detach();
    }

    /// Resolve a finished background run into its queue state and start the
    /// next job. A cancel that raced past the commit still completes (#298).
    fn finish_job(&mut self, id: JobId, result: Result<PathBuf, String>, cx: &mut Context<Self>) {
        match result {
            Ok(p) => {
                let _ = self.queue.mark_committed(id);
                let _ = self.queue.mark_completed(id);
                crate::platform_adapter::reveal_in_file_manager(&p);
                self.model.set_interchange_result(Ok(p));
            }
            Err(e) if crate::video_export::is_cancel_err(&e) => {
                let _ = self.queue.mark_canceled(id);
                self.model.panel.stage = generation_core::export_panel::ExportStage::Failed;
                self.model.panel.status_message = Some("Export canceled".into());
            }
            Err(e) => {
                let _ = self.queue.mark_failed(id, e.clone());
                self.model.set_interchange_result(Err(e));
            }
        }
        self.cancel_flags.remove(&id);
        self.start_next_job(cx);
        cx.notify();
    }

    /// Cancel a queue row: waiting jobs drop immediately; the running job gets
    /// its cancel flag set and confirms via `finish_job`.
    fn cancel_job(&mut self, id: JobId, cx: &mut Context<Self>) {
        let was_waiting = self.queue.job(id).map(|j| j.status) == Some(ExportJobStatus::Waiting);
        if self.queue.cancel(id) {
            if was_waiting {
                self.pending_jobs.remove(&id);
                self.cancel_flags.remove(&id);
            } else if let Some(flag) = self.cancel_flags.get(&id) {
                flag.store(true, Ordering::Relaxed);
            }
            cx.notify();
        }
    }
}

impl Focusable for ExportView {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl ExportView {
    fn status_color(status: ExportJobStatus) -> gpui::Hsla {
        match status {
            ExportJobStatus::Waiting => Text::TERTIARY,
            ExportJobStatus::Preparing | ExportJobStatus::Canceling => Text::SECONDARY,
            ExportJobStatus::Exporting => Accent::PRIMARY,
            ExportJobStatus::Completed => STATUS_SUCCESS,
            ExportJobStatus::Failed => Status::ERROR,
            ExportJobStatus::Canceled => Text::MUTED,
        }
    }

    /// Right-hand export queue pane (Swift ExportView "Export Queue" log pane):
    /// header with pending count + Clear, rows with time / status / filename /
    /// progress / action.
    fn render_queue_pane(&self, cx: &mut Context<Self>) -> AnyElement {
        use chrono::TimeZone as _;

        let project = Self::project_queue_id();
        let jobs: Vec<crate::export_queue::ExportJob> = self
            .queue
            .jobs_for(&project)
            .into_iter()
            .rev()
            .cloned()
            .collect();
        let pending = jobs.iter().filter(|j| j.status.is_pending()).count();
        let any_finished = jobs.iter().any(|j| j.status.is_finished());

        let header = div()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(Spacing::SM))
            .px(px(Spacing::LG))
            .py(px(Spacing::MD))
            .border_b_1()
            .border_color(BorderColors::PRIMARY)
            .child(
                div()
                    .text_size(px(FontSize::MD))
                    .text_color(Text::PRIMARY)
                    .child("Export Queue"),
            )
            .when(pending > 0, |el| {
                el.child(
                    div()
                        .text_size(px(FontSize::XS))
                        .text_color(Text::SECONDARY)
                        .child(format!("{pending}")),
                )
            })
            .child(div().flex_1())
            .when(any_finished, |el| {
                el.child(
                    div()
                        .id("queue-clear-finished")
                        .px(px(Spacing::SM))
                        .py(px(Spacing::XXS))
                        .rounded(px(Radius::XS_SM))
                        .cursor_pointer()
                        .text_size(px(FontSize::XS))
                        .text_color(Text::SECONDARY)
                        .child("Clear")
                        .on_click(cx.listener(|this, _: &ClickEvent, _: &mut Window, cx| {
                            this.queue.clear_finished(&Self::project_queue_id());
                            cx.notify();
                        })),
                )
            });

        let body = if jobs.is_empty() {
            div()
                .flex_1()
                .flex()
                .items_center()
                .justify_center()
                .child(
                    div()
                        .text_size(px(FontSize::SM))
                        .text_color(Text::MUTED)
                        .child("No exports yet"),
                )
                .into_any_element()
        } else {
            div()
                .id("export-queue-rows")
                .flex_1()
                .flex()
                .flex_col()
                .overflow_y_scroll()
                .children(jobs.into_iter().map(|job| {
                    let id = job.id;
                    let status = job.status;
                    let time = chrono::Local
                        .timestamp_millis_opt(job.created_at_ms as i64)
                        .single()
                        .map(|t| t.format("%H:%M").to_string())
                        .unwrap_or_default();
                    let exporting = status == ExportJobStatus::Exporting;
                    let progress = job.progress as f32;
                    let output_path = job.output_path.clone();

                    let mut row = div()
                        .flex()
                        .flex_col()
                        .px(px(Spacing::LG))
                        .py(px(Spacing::SM))
                        .border_b_1()
                        .border_color(BorderColors::SUBTLE)
                        .child(
                            div()
                                .flex()
                                .flex_row()
                                .items_center()
                                .gap(px(Spacing::SM))
                                .child(
                                    div()
                                        .w(px(QUEUE_TIMESTAMP_WIDTH))
                                        .text_size(px(FontSize::XXS))
                                        .text_color(Text::MUTED)
                                        .child(time),
                                )
                                .child(
                                    div()
                                        .text_size(px(FontSize::XXS))
                                        .text_color(Self::status_color(status))
                                        .child(status.label()),
                                )
                                .child(
                                    div()
                                        .flex_1()
                                        .overflow_hidden()
                                        .text_size(px(FontSize::XS))
                                        .text_color(Text::PRIMARY)
                                        .whitespace_nowrap()
                                        .text_ellipsis()
                                        .child(job.filename.clone()),
                                )
                                .when(exporting, |el| {
                                    el.child(
                                        div()
                                            .relative()
                                            .w(px(QUEUE_PROGRESS_BAR_WIDTH))
                                            .h(px(3.0))
                                            .rounded_full()
                                            .bg(BorderColors::SUBTLE)
                                            .child(
                                                div()
                                                    .absolute()
                                                    .top_0()
                                                    .left_0()
                                                    .h_full()
                                                    .w(relative(progress))
                                                    .rounded_full()
                                                    .bg(Accent::PRIMARY),
                                            ),
                                    )
                                    .child(
                                        div()
                                            .w(px(QUEUE_PROGRESS_WIDTH))
                                            .text_size(px(FontSize::XXS))
                                            .text_color(Text::SECONDARY)
                                            .child(format!("{}%", (progress * 100.0) as u32)),
                                    )
                                })
                                .child(match status {
                                    ExportJobStatus::Waiting
                                    | ExportJobStatus::Preparing
                                    | ExportJobStatus::Exporting => div()
                                        .id(("queue-cancel", id))
                                        .px(px(Spacing::XS))
                                        .py(px(Spacing::XXS))
                                        .rounded(px(Radius::XS_SM))
                                        .cursor_pointer()
                                        .text_size(px(FontSize::XS))
                                        .text_color(Text::SECONDARY)
                                        .child("✕")
                                        .on_click(cx.listener(
                                            move |this, _: &ClickEvent, _: &mut Window, cx| {
                                                this.cancel_job(id, cx);
                                            },
                                        ))
                                        .into_any_element(),
                                    ExportJobStatus::Completed => div()
                                        .id(("queue-reveal", id))
                                        .px(px(Spacing::XS))
                                        .py(px(Spacing::XXS))
                                        .rounded(px(Radius::XS_SM))
                                        .cursor_pointer()
                                        .text_size(px(FontSize::XS))
                                        .text_color(Text::SECONDARY)
                                        .child("Show")
                                        .on_click(cx.listener(
                                            move |_, _: &ClickEvent, _: &mut Window, _| {
                                                crate::platform_adapter::reveal_in_file_manager(
                                                    &output_path,
                                                );
                                            },
                                        ))
                                        .into_any_element(),
                                    ExportJobStatus::Failed | ExportJobStatus::Canceled => div()
                                        .id(("queue-dismiss", id))
                                        .px(px(Spacing::XS))
                                        .py(px(Spacing::XXS))
                                        .rounded(px(Radius::XS_SM))
                                        .cursor_pointer()
                                        .text_size(px(FontSize::XS))
                                        .text_color(Text::MUTED)
                                        .child("✕")
                                        .on_click(cx.listener(
                                            move |this, _: &ClickEvent, _: &mut Window, cx| {
                                                this.queue.remove(id);
                                                cx.notify();
                                            },
                                        ))
                                        .into_any_element(),
                                    ExportJobStatus::Canceling => {
                                        div().w(px(Spacing::LG)).into_any_element()
                                    }
                                }),
                        );
                    if status == ExportJobStatus::Failed {
                        if let Some(error) = job.error.clone() {
                            row = row.child(
                                div()
                                    .pl(px(QUEUE_TIMESTAMP_WIDTH + Spacing::SM))
                                    .text_size(px(FontSize::XXS))
                                    .text_color(Status::ERROR)
                                    .whitespace_nowrap()
                                    .text_ellipsis()
                                    .overflow_hidden()
                                    .child(error),
                            );
                        }
                    }
                    row
                }))
                .into_any_element()
        };

        div()
            .w(px(QUEUE_PANE_WIDTH))
            .h_full()
            .flex()
            .flex_col()
            .border_l_1()
            .border_color(BorderColors::PRIMARY)
            .bg(Background::RAISED)
            .child(header)
            .child(body)
            .into_any_element()
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn option_label(text: &str) -> impl IntoElement {
    div()
        .text_color(Text::MUTED)
        .text_size(px(FontSize::XS))
        .child(text.to_string().to_uppercase())
}

fn picker_option(id: &str, label: &str, is_selected: bool) -> gpui::Stateful<gpui::Div> {
    div()
        .id(id.to_string())
        .flex()
        .flex_row()
        .items_center()
        .gap(px(Spacing::SM))
        .px(px(Spacing::SM))
        .py(px(Spacing::XS))
        .rounded(px(Radius::XS_SM))
        .cursor_pointer()
        .when(is_selected, |el| {
            el.bg(gpui::Hsla {
                h: 0.0,
                s: 0.0,
                l: 1.0,
                a: 0.08,
            })
        })
        // Selection dot
        .child(
            div()
                .w(px(14.0))
                .h(px(14.0))
                .rounded_full()
                .border_1()
                .border_color(if is_selected {
                    Accent::PRIMARY
                } else {
                    BorderColors::SUBTLE
                })
                .flex()
                .items_center()
                .justify_center()
                .when(is_selected, |el| {
                    el.child(
                        div()
                            .w(px(7.0))
                            .h(px(7.0))
                            .rounded_full()
                            .bg(Accent::PRIMARY),
                    )
                }),
        )
        .child(
            div()
                .text_size(px(FontSize::SM))
                .text_color(if is_selected {
                    Text::PRIMARY
                } else {
                    Text::SECONDARY
                })
                .child(label.to_string()),
        )
}

// ── Render ────────────────────────────────────────────────────────────────────

impl Render for ExportView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let can_start = self.model.can_start_export();
        let mode = self.model.mode;
        let is_exporting = !self.model.settings_expanded;
        let queue_busy = self.queue.has_activity();
        let progress = self.model.progress_fraction() as f32;
        let selected_codec = self.selected_codec;
        let selected_resolution = self.selected_resolution;
        // #138: HDR (10-bit HEVC Main10) is only offered for the H.265 codec.
        let hdr_enabled = self.model.hdr;
        let show_hdr = selected_codec == 1;

        let codec_labels = ["H.264", "H.265", "ProRes"];
        let res_labels = ["720p", "1080p", "2K", "4K", "Match Timeline"];
        let fps_labels = ["24 fps", "30 fps", "60 fps"];
        let selected_fps = self.selected_fps;
        let output_path = self.output_path.clone();

        div()
            .id("export-sheet")
            .track_focus(&self.focus_handle.clone())
            .flex()
            .flex_col()
            .w(px(860.0 + QUEUE_PANE_WIDTH))
            .h(px(560.0))
            .bg(Background::RAISED)
            // ── body row ──────────────────────────────────────────────────
            .child(
                div()
                    .flex()
                    .flex_row()
                    .flex_1()
                    // Settings panel (left, 360px)
                    .child(
                        div()
                            .id("export-settings-panel")
                            .w(px(360.0))
                            .h_full()
                            .flex()
                            .flex_col()
                            .border_r_1()
                            .border_color(BorderColors::PRIMARY)
                            // Panel header
                            .child(
                                div()
                                    .px(px(Spacing::XL))
                                    .py(px(Spacing::MD))
                                    .border_b_1()
                                    .border_color(BorderColors::PRIMARY)
                                    .child(
                                        div()
                                            .text_size(px(FontSize::SM_MD))
                                            .text_color(Text::PRIMARY)
                                            .child("Export"),
                                    ),
                            )
                            // Mode picker
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap(px(Spacing::XS))
                                    .px(px(Spacing::LG))
                                    .py(px(Spacing::MD))
                                    .border_b_1()
                                    .border_color(BorderColors::SUBTLE)
                                    .child(option_label("Format"))
                                    .children(ExportMode::all().iter().map(|m| {
                                        let selected = *m == mode;
                                        let m_copy = *m;
                                        div()
                                            .id(format!("mode-{}", m.label()))
                                            .flex()
                                            .items_center()
                                            .gap(px(Spacing::SM))
                                            .px(px(Spacing::SM))
                                            .py(px(Spacing::XS))
                                            .rounded(px(Radius::XS_SM))
                                            .when(selected, |el| {
                                                el.bg(gpui::Hsla { h: 0.0, s: 0.0, l: 1.0, a: 0.08 })
                                            })
                                            .cursor_pointer()
                                            .on_click(cx.listener(move |this, _: &ClickEvent, _: &mut Window, cx| {
                                                this.model.set_mode(m_copy);
                                                cx.notify();
                                            }))
                                            .child(
                                                div()
                                                    .w(px(14.0))
                                                    .h(px(14.0))
                                                    .rounded_full()
                                                    .border_1()
                                                    .border_color(if selected { Accent::PRIMARY } else { BorderColors::SUBTLE })
                                                    .flex()
                                                    .items_center()
                                                    .justify_center()
                                                    .when(selected, |el| {
                                                        el.child(
                                                            div()
                                                                .w(px(7.0))
                                                                .h(px(7.0))
                                                                .rounded_full()
                                                                .bg(Accent::PRIMARY),
                                                        )
                                                    }),
                                            )
                                            .child(
                                                div()
                                                    .text_size(px(FontSize::SM))
                                                    .text_color(if selected { Text::PRIMARY } else { Text::SECONDARY })
                                                    .child(m.label()),
                                            )
                                    })),
                            )
                            // Mode-specific options
                            .child(match mode {
                                ExportMode::Video => div()
                                    .flex()
                                    .flex_col()
                                    .gap(px(Spacing::MD))
                                    .px(px(Spacing::LG))
                                    .py(px(Spacing::MD))
                                    // Codec section
                                    .child(
                                        div()
                                            .flex()
                                            .flex_col()
                                            .gap(px(Spacing::XS))
                                            .child(option_label("Codec"))
                                            .children(codec_labels.iter().enumerate().map(|(i, label)| {
                                                let is_sel = selected_codec == i;
                                                picker_option(
                                                    &format!("codec-{i}"),
                                                    label,
                                                    is_sel,
                                                )
                                                .on_click(cx.listener(move |this, _: &ClickEvent, _: &mut Window, cx| {
                                                    this.selected_codec = i;
                                                    let fmt = match i {
                                                        0 => ExportFormat::H264,
                                                        1 => ExportFormat::H265,
                                                        _ => ExportFormat::ProRes,
                                                    };
                                                    this.model.set_format(fmt);
                                                    cx.notify();
                                                }))
                                            })),
                                    )
                                    // HDR toggle (#138) — HEVC Main10 / BT.2020 HLG, H.265 only
                                    .when(show_hdr, |el| {
                                        el.child(
                                            picker_option("codec-hdr", "HDR (10-bit)", hdr_enabled)
                                                .on_click(cx.listener(
                                                    move |this, _: &ClickEvent, _: &mut Window, cx| {
                                                        this.model.set_hdr(!this.model.hdr);
                                                        cx.notify();
                                                    },
                                                )),
                                        )
                                    })
                                    // Resolution section
                                    .child(
                                        div()
                                            .flex()
                                            .flex_col()
                                            .gap(px(Spacing::XS))
                                            .child(option_label("Resolution"))
                                            .children(res_labels.iter().enumerate().map(|(i, label)| {
                                                let is_sel = selected_resolution == i;
                                                picker_option(
                                                    &format!("res-{i}"),
                                                    label,
                                                    is_sel,
                                                )
                                                .on_click(cx.listener(move |this, _: &ClickEvent, _: &mut Window, cx| {
                                                    this.selected_resolution = i;
                                                    let res = match i {
                                                        0 => ExportResolution::R720p,
                                                        1 => ExportResolution::R1080p,
                                                        2 => ExportResolution::R1440p,
                                                        3 => ExportResolution::R4K,
                                                        _ => ExportResolution::MatchTimeline,
                                                    };
                                                    this.model.set_resolution(res);
                                                    cx.notify();
                                                }))
                                            })),
                                    )
                                    // Frame rate section
                                    .child(
                                        div()
                                            .flex()
                                            .flex_col()
                                            .gap(px(Spacing::XS))
                                            .child(option_label("Frame Rate"))
                                            .children(fps_labels.iter().enumerate().map(|(i, label)| {
                                                let is_sel = selected_fps == i;
                                                picker_option(&format!("fps-{i}"), label, is_sel)
                                                    .on_click(cx.listener(move |this, _: &ClickEvent, _: &mut Window, cx| {
                                                        this.selected_fps = i;
                                                        cx.notify();
                                                    }))
                                            })),
                                    )
                                    // Output destination row
                                    .child(
                                        div()
                                            .flex()
                                            .flex_col()
                                            .gap(px(Spacing::XS))
                                            .child(option_label("Save To"))
                                            .child(
                                                div()
                                                    .flex()
                                                    .flex_row()
                                                    .items_center()
                                                    .gap(px(Spacing::XS))
                                                    .child(
                                                        div()
                                                            .flex_1()
                                                            .px(px(Spacing::SM))
                                                            .py(px(Spacing::XS))
                                                            .rounded(px(crate::theme::Radius::SM))
                                                            .border_1()
                                                            .border_color(BorderColors::SUBTLE)
                                                            .bg(Background::BASE)
                                                            .text_color(Text::TERTIARY)
                                                            .text_size(px(FontSize::XS))
                                                            .child(output_path),
                                                    )
                                                    .child(
                                                        div()
                                                            .id("btn-export-browse")
                                                            .px(px(Spacing::SM))
                                                            .py(px(Spacing::XS))
                                                            .rounded(px(crate::theme::Radius::SM))
                                                            .border_1()
                                                            .border_color(BorderColors::SUBTLE)
                                                            .cursor_pointer()
                                                            .text_color(Text::SECONDARY)
                                                            .text_size(px(FontSize::XS))
                                                            .child("Browse…"),
                                                    ),
                                            ),
                                    )
                                    .into_any_element(),
                                ExportMode::Xml => div()
                                    .flex()
                                    .flex_col()
                                    .gap(px(Spacing::SM))
                                    .px(px(Spacing::LG))
                                    .py(px(Spacing::MD))
                                    .child(
                                        div()
                                            .text_color(Text::SECONDARY)
                                            .text_size(px(FontSize::SM))
                                            .child("Exports an XMEML timeline file compatible with Final Cut Pro 7, Premiere, and DaVinci Resolve."),
                                    )
                                    .children(self.model.status_text().map(|s| {
                                        div()
                                            .text_color(Text::PRIMARY)
                                            .text_size(px(FontSize::XS))
                                            .child(s.to_string())
                                    }))
                                    .into_any_element(),
                                ExportMode::Fcpxml => div()
                                    .flex()
                                    .flex_col()
                                    .gap(px(Spacing::SM))
                                    .px(px(Spacing::LG))
                                    .py(px(Spacing::MD))
                                    .child(
                                        div()
                                            .text_color(Text::SECONDARY)
                                            .text_size(px(FontSize::SM))
                                            .child("Exports an FCPXML 1.10 timeline file for Final Cut Pro X and DaVinci Resolve."),
                                    )
                                    .child(
                                        // #254: pick which NLE the transform/crop values are calibrated for.
                                        div()
                                            .flex()
                                            .items_center()
                                            .gap(px(Spacing::SM))
                                            .child(
                                                div()
                                                    .text_color(Text::SECONDARY)
                                                    .text_size(px(FontSize::SM))
                                                    .child("Calibrate for:"),
                                            )
                                            .children([FcpxmlTarget::Resolve, FcpxmlTarget::Fcp].iter().map(|t| {
                                                let selected = *t == self.model.fcpxml_target;
                                                let t_copy = *t;
                                                let label = match t {
                                                    FcpxmlTarget::Resolve => "DaVinci Resolve",
                                                    FcpxmlTarget::Fcp => "Final Cut Pro",
                                                };
                                                div()
                                                    .id(gpui::SharedString::from(format!("fcp-target-{label}")))
                                                    .px(px(Spacing::SM))
                                                    .py(px(Spacing::XS))
                                                    .rounded(px(Radius::XS_SM))
                                                    .border_1()
                                                    .border_color(if selected { Accent::PRIMARY } else { BorderColors::SUBTLE })
                                                    .when(selected, |el| {
                                                        el.bg(gpui::Hsla { h: 0.0, s: 0.0, l: 1.0, a: 0.08 })
                                                    })
                                                    .cursor_pointer()
                                                    .on_click(cx.listener(move |this, _: &ClickEvent, _: &mut Window, cx| {
                                                        this.model.set_fcpxml_target(t_copy);
                                                        cx.notify();
                                                    }))
                                                    .child(
                                                        div()
                                                            .text_color(Text::PRIMARY)
                                                            .text_size(px(FontSize::SM))
                                                            .child(label),
                                                    )
                                            })),
                                    )
                                    .children(self.model.status_text().map(|s| {
                                        div()
                                            .text_color(Text::PRIMARY)
                                            .text_size(px(FontSize::XS))
                                            .child(s.to_string())
                                    }))
                                    .into_any_element(),
                                ExportMode::PalmierProject => {
                                    let missing = self.model.missing_file_count;
                                    let mut col = div()
                                        .flex()
                                        .flex_col()
                                        .gap(px(Spacing::SM))
                                        .px(px(Spacing::LG))
                                        .py(px(Spacing::MD))
                                        .child(
                                            div()
                                                .text_color(Text::SECONDARY)
                                                .text_size(px(FontSize::SM))
                                                .child("Exports a .palmier project bundle that can be reopened in Palmier Pro or Fronda."),
                                        );
                                    // Matches Swift: if palmierSummary.missing > 0 show errorColor warning
                                    if missing > 0 {
                                        col = col.child(
                                            div()
                                                .flex()
                                                .flex_row()
                                                .items_center()
                                                .gap(px(Spacing::XS))
                                                .px(px(Spacing::SM))
                                                .py(px(Spacing::XS))
                                                .rounded(px(4.0))
                                                .bg(gpui::Hsla { h: 0.0, s: 0.75, l: 0.15, a: 0.6 })
                                                .child(
                                                    div()
                                                        .text_color(gpui::Hsla { h: 0.0, s: 0.85, l: 0.60, a: 1.0 })
                                                        .text_size(px(FontSize::XS))
                                                        .child("⚠"),
                                                )
                                                .child(
                                                    div()
                                                        .flex_1()
                                                        .text_color(gpui::Hsla { h: 0.0, s: 0.85, l: 0.65, a: 1.0 })
                                                        .text_size(px(FontSize::XS))
                                                        .child(format!(
                                                            "{missing} media file{} missing from disk and will not be included.",
                                                            if missing == 1 { "" } else { "s" }
                                                        )),
                                                ),
                                        );
                                    }
                                    if let Some(s) = self.model.status_text() {
                                        col = col.child(
                                            div()
                                                .text_color(Text::PRIMARY)
                                                .text_size(px(FontSize::XS))
                                                .child(s.to_string()),
                                        );
                                    }
                                    col.into_any_element()
                                },
                            })
                            // Destination conflict (queue reservation, #298)
                            .children(self.submission_error.clone().map(|e| {
                                div()
                                    .px(px(Spacing::LG))
                                    .py(px(Spacing::SM))
                                    .text_size(px(FontSize::XS))
                                    .text_color(Status::ERROR)
                                    .child(e)
                            })),
                    )
                    // Preview panel (right, flex)
                    .child(
                        div()
                            .flex_1()
                            .h_full()
                            .flex()
                            .flex_col()
                            .items_center()
                            .justify_center()
                            .bg(Background::BASE)
                            .gap(px(Spacing::MD))
                            // Thumbnail placeholder
                            .child(
                                div()
                                    .w(px(320.0))
                                    .h(px(180.0))
                                    .rounded(px(Radius::SM))
                                    .bg(Background::SURFACE)
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .child(
                                        div()
                                            .text_color(Text::MUTED)
                                            .text_size(px(FontSize::DISPLAY))
                                            .child("▶"),
                                    ),
                            )
                            // Progress bar (when exporting)
                            .when(is_exporting, |el| {
                                el.child(
                                    div()
                                        .flex()
                                        .flex_col()
                                        .items_center()
                                        .gap(px(Spacing::SM))
                                        .w(px(320.0))
                                        .child(
                                            div()
                                                .relative()
                                                .w_full()
                                                .h(px(4.0))
                                                .rounded_full()
                                                .bg(BorderColors::SUBTLE)
                                                .child(
                                                    div()
                                                        .absolute()
                                                        .top_0()
                                                        .left_0()
                                                        .h_full()
                                                        .w(relative(progress))
                                                        .rounded_full()
                                                        .bg(Accent::PRIMARY),
                                                ),
                                        )
                                        .child(
                                            div()
                                                .text_color(Text::TERTIARY)
                                                .text_size(px(FontSize::XS))
                                                .child(format!("{}%", (progress * 100.0) as u32)),
                                        ),
                                )
                            }),
                    )
                    // Export queue pane (right, upstream #298)
                    .child(self.render_queue_pane(cx)),
            )
            // ── bottom bar ───────────────────────────────────────────────
            .child(
                div()
                    .h(px(48.0))
                    .flex()
                    .flex_row()
                    .items_center()
                    .px(px(Spacing::LG))
                    .gap(px(Spacing::MD))
                    .border_t_1()
                    .border_color(BorderColors::PRIMARY)
                    .bg(Background::RAISED)
                    // ── Metadata (left side) ──
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap(px(Spacing::LG))
                            .flex_1()
                            // Duration
                            .child(
                                div()
                                    .flex()
                                    .flex_row()
                                    .items_center()
                                    .gap(px(Spacing::XS))
                                    .child(
                                        div()
                                            .text_color(Text::MUTED)
                                            .text_size(px(FontSize::XS))
                                            .child("⏱"),
                                    )
                                    .child(
                                        div()
                                            .text_color(Text::TERTIARY)
                                            .text_size(px(FontSize::XS))
                                            .child("00:20"),
                                    ),
                            )
                            // Estimated size
                            .child(
                                div()
                                    .flex()
                                    .flex_row()
                                    .items_center()
                                    .gap(px(Spacing::XS))
                                    .child(
                                        div()
                                            .text_color(Text::MUTED)
                                            .text_size(px(FontSize::XS))
                                            .child("~"),
                                    )
                                    .child(
                                        div()
                                            .text_color(Text::TERTIARY)
                                            .text_size(px(FontSize::XS))
                                            .child("5 MB"),
                                    ),
                            )
                            // Resolution
                            .child(
                                div()
                                    .text_color(Text::TERTIARY)
                                    .text_size(px(FontSize::XS))
                                    .child(match selected_resolution {
                                        0 => "1280×720",
                                        1 => "1920×1080",
                                        2 => "2560×1440",
                                        3 => "3840×2160",
                                        _ => "Match Timeline",
                                    }),
                            ),
                    )
                    // ── Action buttons (right side) ──
                    .child(
                        div()
                            .id("btn-export-cancel")
                            .px(px(Spacing::MD))
                            .py(px(Spacing::XS))
                            .rounded(px(Radius::SM))
                            .border_1()
                            .border_color(BorderColors::PRIMARY)
                            .cursor_pointer()
                            .on_click(cx.listener(|this, _: &ClickEvent, _: &mut Window, cx| {
                                this.model.settings_expanded = true;
                                cx.notify();
                            }))
                            .child(
                                div()
                                    .text_size(px(FontSize::SM))
                                    .text_color(Text::SECONDARY)
                                    .child("Cancel"),
                            ),
                    )
                    .child(
                        div()
                            .id("btn-export-start")
                            .px(px(Spacing::LG))
                            .py(px(Spacing::XS))
                            .rounded_full()
                            .bg(if can_start { Accent::PRIMARY } else { Background::PROMINENT })
                            .cursor_pointer()
                            .on_click(cx.listener(|this, _: &ClickEvent, _: &mut Window, cx| {
                                if !this.model.can_start_export() {
                                    return;
                                }
                                let mode = this.model.mode;
                                let fcpxml_target = this.model.fcpxml_target;
                                if let Some(ext) = mode.interchange_extension() {
                                    // Interchange export: pick a path, then generate + write.
                                    let start_dir =
                                        std::env::home_dir().unwrap_or_else(|| ".".into());
                                    let default_name = format!("Timeline.{ext}");
                                    let rx = cx
                                        .prompt_for_new_path(&start_dir, Some(default_name.as_str()));
                                    cx.spawn(async move |this, cx| {
                                        if let Ok(Ok(Some(path))) = rx.await {
                                            let result = {
                                                let hub =
                                                    crate::editor_state_hub::EditorStateHub::global();
                                                let exec = hub.executor();
                                                let guard = exec.lock().unwrap();
                                                crate::export_model::write_interchange(
                                                    mode,
                                                    guard.timeline(),
                                                    guard.media_manifest(),
                                                    &guard.sibling_timeline_map(),
                                                    &path,
                                                    fcpxml_target,
                                                )
                                                .map(|()| path.clone())
                                            };
                                            let _ = this.update(cx, |view, cx| {
                                                if let Ok(ref p) = result { crate::platform_adapter::reveal_in_file_manager(p); }
                                            view.model.set_interchange_result(result);
                                                cx.notify();
                                            });
                                        }
                                    })
                                    .detach();
                                } else if mode == ExportMode::PalmierProject {
                                    // Project bundle export: pick a .palmier path, write it.
                                    let start_dir =
                                        std::env::home_dir().unwrap_or_else(|| ".".into());
                                    let rx = cx
                                        .prompt_for_new_path(&start_dir, Some("Timeline.palmier"));
                                    cx.spawn(async move |this, cx| {
                                        if let Ok(Ok(Some(path))) = rx.await {
                                            let result = {
                                                let hub =
                                                    crate::editor_state_hub::EditorStateHub::global();
                                                let exec = hub.executor();
                                                let guard = exec.lock().unwrap();
                                                crate::export_model::write_palmier_bundle(
                                                    &path,
                                                    guard.timeline(),
                                                    guard.media_manifest(),
                                                )
                                                .map(|()| path.clone())
                                            };
                                            let _ = this.update(cx, |view, cx| {
                                                if let Ok(ref p) = result { crate::platform_adapter::reveal_in_file_manager(p); }
                                            view.model.set_interchange_result(result);
                                                cx.notify();
                                            });
                                        }
                                    })
                                    .detach();
                                } else if mode == ExportMode::Video {
                                    // Real pixel render, routed through the export
                                    // queue (#298): pick a path, reserve it, run FIFO.
                                    let start_dir =
                                        std::env::home_dir().unwrap_or_else(|| ".".into());
                                    let resolution = this.model.panel.settings.resolution;
                                    // #138: the HDR toggle upgrades H.265 to HEVC Main10.
                                    let (video_codec, ext) = match this.model.effective_video_codec()
                                    {
                                        ExportFormat::ProRes => {
                                            (crate::video_export::VideoCodec::ProRes, "mov")
                                        }
                                        ExportFormat::H265Hdr => {
                                            (crate::video_export::VideoCodec::H265Hdr, "mp4")
                                        }
                                        ExportFormat::H265 => {
                                            (crate::video_export::VideoCodec::H265, "mp4")
                                        }
                                        ExportFormat::H264 => {
                                            (crate::video_export::VideoCodec::H264, "mp4")
                                        }
                                    };
                                    let out_fps =
                                        [24i64, 30, 60].get(this.selected_fps).copied().unwrap_or(0);
                                    let rx = cx.prompt_for_new_path(
                                        &start_dir,
                                        Some(&format!("Timeline.{ext}")),
                                    );
                                    cx.spawn(async move |this, cx| {
                                        let Ok(Ok(Some(path))) = rx.await else {
                                            return;
                                        };
                                        let _ = this.update(cx, |view, cx| {
                                            view.enqueue_video_job(
                                                path,
                                                video_codec,
                                                resolution,
                                                out_fps,
                                                cx,
                                            );
                                        });
                                    })
                                    .detach();
                                } else {
                                    this.model.start();
                                    cx.notify();
                                }
                            }))
                            .child(
                                div()
                                    .text_size(px(FontSize::SM))
                                    .text_color(if can_start { Background::BASE } else { Text::MUTED })
                                    // Swift #298: while the queue is busy the
                                    // button reads "Add to Queue".
                                    .child(if mode == ExportMode::Video && queue_busy {
                                        "Add to Queue"
                                    } else if is_exporting && mode != ExportMode::Video {
                                        "Exporting…"
                                    } else {
                                        "Export"
                                    }),
                            ),
                    ),
            )
    }
}
