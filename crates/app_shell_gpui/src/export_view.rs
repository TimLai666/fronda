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

use gpui::prelude::*;
use gpui::*;

use crate::export_model::{ExportMode, ExportViewModel};
use crate::theme::{Accent, Background, BorderColors, FontSize, Radius, Spacing, Text};
use render_core::{ExportFormat, ExportResolution};

/// Export sheet view.
pub struct ExportView {
    pub model: ExportViewModel,
    focus_handle: FocusHandle,
    // UI-only selection state (not in model)
    selected_codec: usize,      // 0=H.264, 1=H.265, 2=ProRes
    selected_resolution: usize, // 0=720p, 1=1080p, 2=2K, 3=4K, 4=Match
    selected_fps: usize,        // 0=24, 1=30, 2=60
    output_path: String,
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
        }
    }
}

impl Focusable for ExportView {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
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
        let progress = self.model.progress_fraction() as f32;
        let selected_codec = self.selected_codec;
        let selected_resolution = self.selected_resolution;

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
            .w(px(860.0))
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
                            }),
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
                    ),
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
                                    // Real pixel render: pick an .mp4 path, then
                                    // composite + encode the timeline off-thread.
                                    let start_dir =
                                        std::env::home_dir().unwrap_or_else(|| ".".into());
                                    let resolution = this.model.panel.settings.resolution;
                                    let (video_codec, ext) = match this.model.panel.settings.format
                                    {
                                        ExportFormat::ProRes => {
                                            (crate::video_export::VideoCodec::ProRes, "mov")
                                        }
                                        ExportFormat::H265 | ExportFormat::H265Hdr => {
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
                                            view.model.start();
                                            cx.notify();
                                        });
                                        // Shared progress (0..=100) the encoder
                                        // updates; a ticker mirrors it to the UI.
                                        let progress = std::sync::Arc::new(
                                            std::sync::atomic::AtomicU64::new(0),
                                        );
                                        let prog_tick = progress.clone();
                                        let ticker_this = this.clone();
                                        cx.spawn(async move |cx| loop {
                                            cx.background_executor()
                                                .timer(std::time::Duration::from_millis(150))
                                                .await;
                                            let p = prog_tick
                                                .load(std::sync::atomic::Ordering::Relaxed)
                                                as f64
                                                / 100.0;
                                            let running = ticker_this
                                                .update(cx, |view, cx| {
                                                    view.model.panel.update_progress(p, None);
                                                    cx.notify();
                                                    view.model.panel.stage
                                                        == generation_core::export_panel::ExportStage::Exporting
                                                })
                                                .unwrap_or(false);
                                            if !running {
                                                break;
                                            }
                                        })
                                        .detach();
                                        let (timeline, manifest, root) = {
                                            let hub =
                                                crate::editor_state_hub::EditorStateHub::global();
                                            let exec = hub.executor();
                                            let guard = exec.lock().unwrap();
                                            let root = hub.project_root().unwrap_or_else(|| {
                                                std::env::home_dir().unwrap_or_else(|| ".".into())
                                            });
                                            (
                                                guard.timeline().clone(),
                                                guard.media_manifest().clone(),
                                                root,
                                            )
                                        };
                                        let out = path.clone();
                                        let prog_enc = progress.clone();
                                        let result = cx
                                            .background_executor()
                                            .spawn(async move {
                                                let size = resolution.render_size(&timeline);
                                                let w = size.width.max(2) as u32;
                                                let h = size.height.max(2) as u32;
                                                crate::audio_export::export_project_with_audio(
                                                    &timeline, &manifest, &root, &out, w, h,
                                                    video_codec, out_fps, &prog_enc,
                                                )
                                                .map(|()| out.clone())
                                            })
                                            .await;
                                        let _ = this.update(cx, |view, cx| {
                                            if let Ok(ref p) = result { crate::platform_adapter::reveal_in_file_manager(p); }
                                            view.model.set_interchange_result(result);
                                            cx.notify();
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
                                    .child(if is_exporting { "Exporting…" } else { "Export" }),
                            ),
                    ),
            )
    }
}
