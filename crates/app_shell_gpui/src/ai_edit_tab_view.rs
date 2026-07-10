//! AI Edit tab content — matches Swift AIEditTab.
//!
//! Layout (top to bottom):
//!   • Scope section (toggles: Replace clip source / Use trimmed portion)
//!   • AI Enhance section (collapsible): Upscale / Edit / Rerun / Create Video (image assets)
//!   • AI Audio section (collapsible, video assets only): Music / SFX
//!
//! Actions dispatch through the shared executor (upscale_media /
//! generate_music / generate_audio; Rerun replays the stored
//! generation_input). Results and errors land on a status line —
//! backend-gated actions report that state honestly, no fake progress.

use crate::generation_view::{interpret_submission, SubmitOutcome};
use crate::theme::{Accent, Background, BorderColors, FontSize, Spacing, Text};
use core_model::{ClipType, GenerationInput};
use generation_core::model_catalog::{self, AudioCategory, ModelCaps};
use gpui::{
    div, prelude::*, px, App, ClickEvent, Context, FocusHandle, Focusable, IntoElement,
    ParentElement, Render, Styled, Window,
};

#[derive(Debug, Clone)]
pub struct AiEditTabState {
    pub enhance_expanded: bool,
    pub audio_expanded: bool,
    pub replace_clip_source: bool,
    pub use_trimmed_portion: bool,
    pub place_audio_on_timeline: bool,
    pub is_video: bool,
    /// GEN-5 analog: whether the Upscale model picker dropdown is open.
    pub show_upscale_picker: bool,
    /// Last-picked upscale model id (persists across picker opens).
    pub selected_upscale_model: Option<String>,
    /// Latest action result or error line.
    pub status: Option<String>,
}

impl Default for AiEditTabState {
    fn default() -> Self {
        Self {
            enhance_expanded: true,
            audio_expanded: true,
            replace_clip_source: false,
            use_trimmed_portion: true,
            place_audio_on_timeline: true,
            is_video: true,
            show_upscale_picker: false,
            selected_upscale_model: None,
            status: None,
        }
    }
}

// ── Pure action logic ────────────────────────────────────────────────

/// Snapshot of the selected media asset, as the tab needs it.
#[derive(Debug, Clone, PartialEq)]
pub struct SelectedAsset {
    pub id: String,
    pub clip_type: ClipType,
    pub duration_seconds: f64,
    pub generation_input: Option<GenerationInput>,
}

/// Spec: no selection → all action rows disabled.
pub fn actions_enabled(selected: Option<&SelectedAsset>) -> bool {
    selected.is_some()
}

/// Rerun additionally needs a stored generation_input.
pub fn rerun_enabled(selected: Option<&SelectedAsset>) -> bool {
    selected.is_some_and(|a| a.generation_input.is_some())
}

/// upscale_media takes mediaId today; the picked model rides along for the
/// future backend (same convention as the generate tools' extra args).
pub fn upscale_tool_call(asset_id: &str, model_id: &str) -> (&'static str, serde_json::Value) {
    (
        "upscale_media",
        serde_json::json!({ "mediaId": asset_id, "model": model_id }),
    )
}

/// Upstream `videoAudioSeed`: empty prompt, the asset's duration, the video
/// itself as the reference. No model id — the Fal-era catalog carries no
/// video-to-audio entries, so the executor picks its default.
fn video_audio_args(asset: &SelectedAsset) -> serde_json::Value {
    let mut args = serde_json::json!({
        "prompt": "",
        "referenceVideoAssetIds": [asset.id],
    });
    let secs = asset.duration_seconds.round() as i64;
    if secs > 0 {
        args["duration"] = serde_json::json!(secs as f64);
    }
    args
}

pub fn music_tool_call(asset: &SelectedAsset) -> (&'static str, serde_json::Value) {
    ("generate_music", video_audio_args(asset))
}

pub fn sfx_tool_call(asset: &SelectedAsset) -> (&'static str, serde_json::Value) {
    ("generate_audio", video_audio_args(asset))
}

/// The stored GenerationInput serialized back to generate-tool args —
/// same key set as `generation_view::generation_tool_call`.
fn generation_args_from_input(input: &GenerationInput) -> serde_json::Value {
    let mut args = serde_json::json!({
        "prompt": input.prompt,
        "model": input.model,
    });
    let obj = args.as_object_mut().expect("args is an object");
    let mut set = |key: &str, value: serde_json::Value| {
        obj.insert(key.to_string(), value);
    };
    if input.duration > 0 {
        set("duration", serde_json::json!(input.duration as f64));
    }
    if !input.aspect_ratio.is_empty() {
        set("aspectRatio", serde_json::json!(input.aspect_ratio));
    }
    if let Some(r) = &input.resolution {
        set("resolution", serde_json::json!(r));
    }
    if let Some(q) = &input.quality {
        set("quality", serde_json::json!(q));
    }
    if let Some(n) = input.num_images {
        set("numImages", serde_json::json!(n));
    }
    if let Some(v) = &input.voice {
        set("voice", serde_json::json!(v));
    }
    if let Some(l) = &input.lyrics {
        set("lyrics", serde_json::json!(l));
    }
    if let Some(s) = &input.style_instructions {
        set("style", serde_json::json!(s));
    }
    if let Some(i) = input.instrumental {
        set("instrumental", serde_json::json!(i));
    }
    if let Some(g) = input.generate_audio {
        set("generateAudio", serde_json::json!(g));
    }
    if let Some(ids) = &input.image_url_asset_ids {
        set("imageURLAssetIds", serde_json::json!(ids));
    }
    if let Some(ids) = &input.reference_image_asset_ids {
        set("referenceImageAssetIds", serde_json::json!(ids));
    }
    if let Some(ids) = &input.reference_video_asset_ids {
        set("referenceVideoAssetIds", serde_json::json!(ids));
    }
    if let Some(ids) = &input.reference_audio_asset_ids {
        set("referenceAudioAssetIds", serde_json::json!(ids));
    }
    args
}

/// Rerun: rebuild the call from the stored generation_input (Swift
/// `EditSubmitter.rerun` branch order — upscale model → catalog kind).
/// A model gone from the catalog dispatches by the asset's kind and lets
/// the executor's model validation report honestly.
pub fn rerun_tool_call(
    asset: &SelectedAsset,
) -> Result<(&'static str, serde_json::Value), String> {
    let input = asset
        .generation_input
        .as_ref()
        .ok_or_else(|| "This asset was not AI-generated".to_string())?;
    if model_catalog::is_upscale_model_id(&input.model) {
        return Ok(upscale_tool_call(&asset.id, &input.model));
    }
    let tool = match model_catalog::model_by_id(&input.model) {
        Some(m) => match &m.caps {
            ModelCaps::Video(_) => "generate_video",
            ModelCaps::Image(_) => "generate_image",
            ModelCaps::Audio(c) => match c.category {
                AudioCategory::Music => "generate_music",
                AudioCategory::Tts => "generate_audio",
            },
        },
        None => match asset.clip_type {
            ClipType::Video => "generate_video",
            ClipType::Image => "generate_image",
            ClipType::Audio => "generate_audio",
            _ => return Err(format!("Model no longer available: {}", input.model)),
        },
    };
    Ok((tool, generation_args_from_input(input)))
}

/// Status line for a tool result. The executor stubs mark a missing backend
/// with "requires a remote API" (SubmitOutcome::Unavailable).
pub fn action_status(action: &str, result: &Result<serde_json::Value, String>) -> String {
    match interpret_submission(result) {
        SubmitOutcome::Queued(text) => text,
        SubmitOutcome::Unavailable => {
            format!("{action} unavailable — no generation backend is connected.")
        }
        SubmitOutcome::Failed(reason) => reason,
    }
}

/// Edit / Create Video are backend-seeded flows upstream; report that
/// honestly instead of a fake dispatch.
pub fn backend_required_status(action: &str) -> String {
    format!("{action} requires a generation backend.")
}

// ── View ─────────────────────────────────────────────────────────────

pub struct AiEditTabView {
    pub state: AiEditTabState,
    /// Selected media-library asset id, forwarded by the inspector.
    pub selected_media_asset_id: Option<String>,
    focus_handle: FocusHandle,
}

impl AiEditTabView {
    pub fn new(cx: &mut Context<Self>) -> Self {
        Self {
            state: AiEditTabState::default(),
            selected_media_asset_id: None,
            focus_handle: cx.focus_handle(),
        }
    }

    /// Snapshot the selected entry from the shared manifest.
    fn selected_asset(&self) -> Option<SelectedAsset> {
        let id = self.selected_media_asset_id.as_deref()?;
        let executor = crate::editor_state_hub::EditorStateHub::global().executor();
        let exec = executor.lock().ok()?;
        let entry = exec.media_manifest().entries.iter().find(|e| e.id == id)?;
        Some(SelectedAsset {
            id: entry.id.clone(),
            clip_type: entry.r#type,
            duration_seconds: entry.duration,
            generation_input: entry.generation_input.clone(),
        })
    }

    /// Run a tool on the shared executor and put the outcome on the status line.
    fn dispatch(&mut self, action: &str, tool: &str, args: serde_json::Value, cx: &mut Context<Self>) {
        let executor = crate::editor_state_hub::EditorStateHub::global().executor();
        let result = match executor.lock() {
            Ok(mut exec) => exec.execute(tool, &args),
            Err(_) => Err("Editor state lock poisoned".to_string()),
        };
        self.state.status = Some(action_status(action, &result));
        cx.notify();
    }

    fn run_upscale(&mut self, model_id: String, cx: &mut Context<Self>) {
        let Some(asset) = self.selected_asset() else {
            return;
        };
        self.state.selected_upscale_model = Some(model_id.clone());
        self.state.show_upscale_picker = false;
        let (tool, args) = upscale_tool_call(&asset.id, &model_id);
        self.dispatch("Upscale", tool, args, cx);
    }

    fn run_music(&mut self, cx: &mut Context<Self>) {
        let Some(asset) = self.selected_asset() else {
            return;
        };
        let (tool, args) = music_tool_call(&asset);
        self.dispatch("Music", tool, args, cx);
    }

    fn run_sfx(&mut self, cx: &mut Context<Self>) {
        let Some(asset) = self.selected_asset() else {
            return;
        };
        let (tool, args) = sfx_tool_call(&asset);
        self.dispatch("Sound Effects", tool, args, cx);
    }

    fn run_rerun(&mut self, cx: &mut Context<Self>) {
        let Some(asset) = self.selected_asset() else {
            return;
        };
        match rerun_tool_call(&asset) {
            Ok((tool, args)) => self.dispatch("Rerun", tool, args, cx),
            Err(reason) => {
                self.state.status = Some(reason);
                cx.notify();
            }
        }
    }

    fn set_backend_required(&mut self, action: &str, cx: &mut Context<Self>) {
        self.state.status = Some(backend_required_status(action));
        cx.notify();
    }
}

impl Focusable for AiEditTabView {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

fn section_header_static(label: &str) -> impl IntoElement {
    div()
        .text_color(Text::MUTED)
        .text_size(px(FontSize::XXS))
        .w_full()
        .child(label.to_uppercase())
}

fn section_header_collapsible(label: &str, expanded: bool) -> impl IntoElement {
    div()
        .flex()
        .flex_row()
        .items_center()
        .gap(px(Spacing::XS))
        .w_full()
        .child(
            div()
                .text_color(Text::MUTED)
                .text_size(px(FontSize::XXS))
                .child(if expanded { "▾" } else { "▸" }),
        )
        .child(
            div()
                .text_color(Text::MUTED)
                .text_size(px(FontSize::XXS))
                .child(label.to_uppercase()),
        )
}

fn toggle_row(icon: &str, label: &str, is_on: bool) -> impl IntoElement {
    let pill_bg = if is_on { Accent::PRIMARY } else { Text::MUTED };
    div()
        .flex()
        .flex_row()
        .items_center()
        .gap(px(Spacing::SM))
        .w_full()
        .child(
            div()
                .w(px(20.0))
                .text_color(if is_on {
                    Accent::PRIMARY
                } else {
                    Text::TERTIARY
                })
                .text_size(px(FontSize::SM))
                .child(icon.to_string()),
        )
        .child(
            div()
                .flex_1()
                .text_color(Text::SECONDARY)
                .text_size(px(FontSize::SM))
                .child(label.to_string()),
        )
        .child(
            div()
                .w(px(28.0))
                .h(px(16.0))
                .rounded_full()
                .bg(pill_bg)
                .flex()
                .items_center()
                .when(is_on, |el| el.justify_end())
                .px(px(2.0))
                .child(
                    div()
                        .w(px(12.0))
                        .h(px(12.0))
                        .rounded_full()
                        .bg(Background::BASE),
                ),
        )
}

fn action_row(
    id: &'static str,
    icon: &str,
    title: &str,
    description: &str,
    trigger: &str,
    enabled: bool,
    on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> impl IntoElement {
    let mut trigger_btn = div()
        .id(id)
        .px(px(Spacing::SM))
        .py(px(Spacing::XXS))
        .rounded_full()
        .border_1()
        .border_color(if enabled {
            BorderColors::PRIMARY
        } else {
            BorderColors::SUBTLE
        })
        .text_color(if enabled {
            Text::SECONDARY
        } else {
            Text::MUTED
        })
        .text_size(px(FontSize::XS))
        .child(trigger.to_string());
    if enabled {
        trigger_btn = trigger_btn.cursor_pointer().on_click(on_click);
    }
    div()
        .flex()
        .flex_row()
        .items_start()
        .gap(px(Spacing::SM))
        .w_full()
        .child(
            div()
                .w(px(20.0))
                .pt(px(2.0))
                .text_color(if enabled {
                    Text::SECONDARY
                } else {
                    Text::MUTED
                })
                .text_size(px(FontSize::MD))
                .child(icon.to_string()),
        )
        .child(
            div()
                .flex()
                .flex_col()
                .flex_1()
                .gap(px(Spacing::XXS))
                .child(
                    div()
                        .text_color(if enabled { Text::PRIMARY } else { Text::MUTED })
                        .text_size(px(FontSize::SM))
                        .child(title.to_string()),
                )
                .child(
                    div()
                        .text_color(Text::TERTIARY)
                        .text_size(px(FontSize::XS))
                        .child(description.to_string()),
                ),
        )
        .child(trigger_btn)
}

impl Render for AiEditTabView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let asset = self.selected_asset();
        // Section gating follows the real asset type when one is selected;
        // the defaults keep the layout stable with nothing selected.
        if let Some(a) = &asset {
            self.state.is_video = a.clip_type == ClipType::Video;
        }
        let enhance_exp = self.state.enhance_expanded;
        let audio_exp = self.state.audio_expanded;
        let replace = self.state.replace_clip_source;
        let trimmed = self.state.use_trimmed_portion;
        let place_audio = self.state.place_audio_on_timeline;
        let is_video = self.state.is_video;
        let is_image = asset.as_ref().is_some_and(|a| a.clip_type == ClipType::Image);
        let enabled = actions_enabled(asset.as_ref());
        let can_rerun = rerun_enabled(asset.as_ref());
        let upscale_models = asset
            .as_ref()
            .map(|a| model_catalog::upscale_models_for(a.clip_type))
            .unwrap_or_default();
        let upscale_enabled = enabled && !upscale_models.is_empty();
        let show_upscale_picker = self.state.show_upscale_picker && upscale_enabled;
        let duration_seconds = asset.as_ref().map(|a| a.duration_seconds).unwrap_or(0.0);
        let status = self.state.status.clone();

        div()
            .track_focus(&self.focus_handle.clone())
            .id("ai-edit-scroll")
            .flex()
            .flex_col()
            .w_full()
            .overflow_y_scroll()
            .px(px(Spacing::LG))
            .py(px(Spacing::MD))
            .gap(px(Spacing::XL))
            // Scope section
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(Spacing::SM_MD))
                    .child(section_header_static("Scope"))
                    .child(toggle_row("↩", "Replace clip source", replace))
                    .child(toggle_row("✂", "Use trimmed portion only", trimmed)),
            )
            // AI Enhance section (collapsible)
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(Spacing::SM_MD))
                    .child(
                        div()
                            .id("btn-enhance-toggle")
                            .w_full()
                            .cursor_pointer()
                            .on_click(cx.listener(|this, _: &ClickEvent, _: &mut Window, cx| {
                                this.state.enhance_expanded = !this.state.enhance_expanded;
                                cx.notify();
                            }))
                            .child(section_header_collapsible("AI Enhance", enhance_exp)),
                    )
                    .when(enhance_exp, |el| {
                        // Upscale row — trigger button opens model picker dropdown (Swift: Menu)
                        let mut upscale_trigger = div()
                            .id("upscale-trigger")
                            .px(px(Spacing::SM))
                            .py(px(Spacing::XXS))
                            .rounded_full()
                            .border_1()
                            .border_color(if upscale_enabled {
                                BorderColors::PRIMARY
                            } else {
                                BorderColors::SUBTLE
                            })
                            .text_color(if upscale_enabled {
                                Text::SECONDARY
                            } else {
                                Text::MUTED
                            })
                            .text_size(px(FontSize::XS))
                            .child("Upscale ⌄");
                        if upscale_enabled {
                            upscale_trigger = upscale_trigger.cursor_pointer().on_click(
                                cx.listener(|this, _, _, cx| {
                                    this.state.show_upscale_picker =
                                        !this.state.show_upscale_picker;
                                    cx.notify();
                                }),
                            );
                        }
                        let upscale_row = div()
                            .flex()
                            .flex_col()
                            .gap(px(0.0))
                            .child(
                                div()
                                    .flex()
                                    .flex_row()
                                    .items_start()
                                    .gap(px(Spacing::SM))
                                    .w_full()
                                    .child(
                                        div()
                                            .w(px(20.0))
                                            .pt(px(2.0))
                                            .text_color(if upscale_enabled {
                                                Text::SECONDARY
                                            } else {
                                                Text::MUTED
                                            })
                                            .text_size(px(FontSize::MD))
                                            .child("✦"),
                                    )
                                    .child(
                                        div()
                                            .flex()
                                            .flex_col()
                                            .flex_1()
                                            .gap(px(Spacing::XXS))
                                            .child(
                                                div()
                                                    .text_color(if upscale_enabled {
                                                        Text::PRIMARY
                                                    } else {
                                                        Text::MUTED
                                                    })
                                                    .text_size(px(FontSize::SM))
                                                    .child("Upscale"),
                                            )
                                            .child(
                                                div()
                                                    .text_color(Text::TERTIARY)
                                                    .text_size(px(FontSize::XS))
                                                    .child("Enhance resolution with AI"),
                                            ),
                                    )
                                    .child(upscale_trigger),
                            )
                            .when(show_upscale_picker, |el| {
                                let mut dropdown = div()
                                    .ml(px(28.0))
                                    .mt(px(Spacing::XXS))
                                    .rounded(px(crate::theme::Radius::SM))
                                    .border_1()
                                    .border_color(BorderColors::SUBTLE)
                                    .bg(crate::theme::Background::RAISED)
                                    .overflow_hidden()
                                    .flex()
                                    .flex_col();
                                for (idx, model) in upscale_models.iter().enumerate() {
                                    let picked = self.state.selected_upscale_model.as_deref()
                                        == Some(model.id);
                                    // Swift menu label: displayName · speed · cost.
                                    let cost = model_catalog::upscale_cost(
                                        model,
                                        duration_seconds.round() as i64,
                                    );
                                    let detail = format!(
                                        "{} · {}",
                                        model.speed,
                                        model_catalog::format_usd(cost)
                                    );
                                    let model_id = model.id.to_string();
                                    dropdown = dropdown.child(
                                        div()
                                            .id(("upscale-model", idx))
                                            .flex()
                                            .flex_row()
                                            .items_center()
                                            .gap(px(Spacing::SM))
                                            .px(px(Spacing::SM_MD))
                                            .py(px(Spacing::XS))
                                            .cursor_pointer()
                                            .on_click(cx.listener(
                                                move |this, _: &ClickEvent, _, cx| {
                                                    this.run_upscale(model_id.clone(), cx);
                                                },
                                            ))
                                            .child(
                                                div()
                                                    .flex_1()
                                                    .text_color(if picked {
                                                        Accent::PRIMARY
                                                    } else {
                                                        Text::PRIMARY
                                                    })
                                                    .text_size(px(FontSize::SM))
                                                    .child(model.display_name.to_string()),
                                            )
                                            .child(
                                                div()
                                                    .text_color(Text::MUTED)
                                                    .text_size(px(FontSize::XXS))
                                                    .child(detail),
                                            ),
                                    );
                                }
                                el.child(dropdown)
                            });
                        el.child(upscale_row)
                            .child(action_row(
                                "btn-edit",
                                "★",
                                "Edit",
                                "Transform with a prompt or motion reference",
                                "Edit",
                                enabled,
                                cx.listener(|this, _: &ClickEvent, _, cx| {
                                    this.set_backend_required("Edit", cx);
                                }),
                            ))
                            .child(action_row(
                                "btn-rerun",
                                "↺",
                                "Rerun",
                                "Regenerate with the same parameters",
                                "Rerun",
                                can_rerun,
                                cx.listener(|this, _: &ClickEvent, _, cx| {
                                    this.run_rerun(cx);
                                }),
                            ))
                            .when(is_image, |el2| {
                                // Swift: Create Video is offered for image assets.
                                el2.child(action_row(
                                    "btn-create-video",
                                    "▷",
                                    "Create Video",
                                    "Use as first frame or reference",
                                    "Create",
                                    enabled,
                                    cx.listener(|this, _: &ClickEvent, _, cx| {
                                        this.set_backend_required("Create Video", cx);
                                    }),
                                ))
                            })
                    }),
            )
            // AI Audio section (video only, collapsible)
            .when(is_video, |el| {
                el.child(
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(Spacing::SM_MD))
                        .child(
                            div()
                                .id("btn-audio-toggle")
                                .w_full()
                                .cursor_pointer()
                                .on_click(cx.listener(
                                    |this, _: &ClickEvent, _: &mut Window, cx| {
                                        this.state.audio_expanded = !this.state.audio_expanded;
                                        cx.notify();
                                    },
                                ))
                                .child(section_header_collapsible("AI Audio", audio_exp)),
                        )
                        .when(audio_exp, |el| {
                            el.child(toggle_row("↗", "Place on timeline", place_audio))
                                .child(action_row(
                                    "btn-music",
                                    "♪",
                                    "Music",
                                    "Generate background music from video",
                                    "Generate",
                                    enabled,
                                    cx.listener(|this, _: &ClickEvent, _, cx| {
                                        this.run_music(cx);
                                    }),
                                ))
                                .child(action_row(
                                    "btn-sfx",
                                    "~",
                                    "Sound Effects",
                                    "Generate sound effects from video",
                                    "Generate",
                                    enabled,
                                    cx.listener(|this, _: &ClickEvent, _, cx| {
                                        this.run_sfx(cx);
                                    }),
                                ))
                        }),
                )
            })
            // Status line — action results, errors, and backend gating.
            .when_some(status, |el, text| {
                el.child(
                    div()
                        .w_full()
                        .text_color(Text::SECONDARY)
                        .text_size(px(FontSize::XS))
                        .child(text),
                )
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn asset(clip_type: ClipType, input: Option<GenerationInput>) -> SelectedAsset {
        SelectedAsset {
            id: "asset-1".to_string(),
            clip_type,
            duration_seconds: 12.4,
            generation_input: input,
        }
    }

    #[test]
    fn enabled_gating() {
        assert!(!actions_enabled(None));
        assert!(!rerun_enabled(None));
        let plain = asset(ClipType::Video, None);
        assert!(actions_enabled(Some(&plain)));
        assert!(!rerun_enabled(Some(&plain)), "no generation_input → Rerun off");
        let generated = asset(ClipType::Video, Some(GenerationInput::default()));
        assert!(rerun_enabled(Some(&generated)));
    }

    #[test]
    fn upscale_call_args() {
        let (tool, args) = upscale_tool_call("asset-1", "topaz-upscaler");
        assert_eq!(tool, "upscale_media");
        assert_eq!(args["mediaId"], "asset-1");
        assert_eq!(args["model"], "topaz-upscaler");
    }

    #[test]
    fn music_and_sfx_calls_carry_video_reference() {
        let a = asset(ClipType::Video, None);
        let (tool, args) = music_tool_call(&a);
        assert_eq!(tool, "generate_music");
        assert_eq!(args["prompt"], "");
        assert_eq!(args["referenceVideoAssetIds"], json!(["asset-1"]));
        assert_eq!(args["duration"], json!(12.0));
        assert!(args.get("model").is_none(), "no video-to-audio model in the catalog");

        let (tool, args) = sfx_tool_call(&a);
        assert_eq!(tool, "generate_audio");
        assert_eq!(args["referenceVideoAssetIds"], json!(["asset-1"]));
    }

    #[test]
    fn music_call_omits_zero_duration() {
        let mut a = asset(ClipType::Video, None);
        a.duration_seconds = 0.0;
        let (_, args) = music_tool_call(&a);
        assert!(args.get("duration").is_none());
    }

    #[test]
    fn rerun_without_input_errors() {
        let a = asset(ClipType::Video, None);
        assert_eq!(
            rerun_tool_call(&a).unwrap_err(),
            "This asset was not AI-generated"
        );
    }

    #[test]
    fn rerun_upscale_model_redispatches_upscale() {
        let input = GenerationInput {
            model: "seedvr-upscaler".to_string(),
            ..Default::default()
        };
        let a = asset(ClipType::Video, Some(input));
        let (tool, args) = rerun_tool_call(&a).unwrap();
        assert_eq!(tool, "upscale_media");
        assert_eq!(args["mediaId"], "asset-1");
        assert_eq!(args["model"], "seedvr-upscaler");
    }

    #[test]
    fn rerun_video_model_replays_stored_parameters() {
        let input = GenerationInput {
            prompt: "a red fox".to_string(),
            model: "seedance-2".to_string(),
            duration: 5,
            aspect_ratio: "16:9".to_string(),
            resolution: Some("720p".to_string()),
            generate_audio: Some(false),
            reference_image_asset_ids: Some(vec!["ref-1".to_string()]),
            ..Default::default()
        };
        let a = asset(ClipType::Video, Some(input));
        let (tool, args) = rerun_tool_call(&a).unwrap();
        assert_eq!(tool, "generate_video");
        assert_eq!(args["prompt"], "a red fox");
        assert_eq!(args["model"], "seedance-2");
        assert_eq!(args["duration"], json!(5.0));
        assert_eq!(args["aspectRatio"], "16:9");
        assert_eq!(args["resolution"], "720p");
        assert_eq!(args["generateAudio"], json!(false));
        assert_eq!(args["referenceImageAssetIds"], json!(["ref-1"]));
    }

    #[test]
    fn rerun_tool_by_catalog_kind() {
        let by_model = |model: &str, ct: ClipType| {
            let input = GenerationInput {
                model: model.to_string(),
                ..Default::default()
            };
            rerun_tool_call(&asset(ct, Some(input))).unwrap().0
        };
        assert_eq!(by_model("nano-banana-pro", ClipType::Image), "generate_image");
        assert_eq!(by_model("minimax-music-v2.6", ClipType::Audio), "generate_music");
        assert_eq!(by_model("elevenlabs-tts-v3", ClipType::Audio), "generate_audio");
    }

    #[test]
    fn rerun_unknown_model_falls_back_to_asset_kind() {
        let input = GenerationInput {
            model: "sonilo-v1.1-video-to-music".to_string(),
            ..Default::default()
        };
        let a = asset(ClipType::Audio, Some(input.clone()));
        let (tool, args) = rerun_tool_call(&a).unwrap();
        assert_eq!(tool, "generate_audio");
        assert_eq!(
            args["model"], "sonilo-v1.1-video-to-music",
            "the executor's model validation reports the unknown id"
        );
        let text_asset = asset(ClipType::Text, Some(input));
        assert_eq!(
            rerun_tool_call(&text_asset).unwrap_err(),
            "Model no longer available: sonilo-v1.1-video-to-music"
        );
    }

    #[test]
    fn action_status_maps_outcomes() {
        let queued = Ok(json!({
            "content": [{"type": "text", "text": "Upscale requested for 'clip.mp4'."}]
        }));
        assert_eq!(
            action_status("Upscale", &queued),
            "Upscale requested for 'clip.mp4'."
        );

        let unavailable = Ok(json!({
            "content": [{"type": "text", "text": "Actual upscaling requires a remote API."}],
            "isError": true,
        }));
        assert_eq!(
            action_status("Upscale", &unavailable),
            "Upscale unavailable — no generation backend is connected."
        );

        let failed: Result<serde_json::Value, String> = Err("Media 'x' not found".to_string());
        assert_eq!(action_status("Upscale", &failed), "Media 'x' not found");

        let tool_error = Ok(json!({
            "content": [{"type": "text", "text": "Some other tool error."}],
            "isError": true,
        }));
        assert_eq!(action_status("Music", &tool_error), "Some other tool error.");
    }

    #[test]
    fn backend_required_wording() {
        assert_eq!(
            backend_required_status("Edit"),
            "Edit requires a generation backend."
        );
    }
}
