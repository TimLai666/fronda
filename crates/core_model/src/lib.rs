pub mod agent;
pub mod date_serde;
pub mod effect;
pub mod generation_log;
pub mod library;
pub mod matte;
pub mod media_manifest;
pub mod multicam;
pub mod project_file;
pub mod project_registry;
pub mod shape_style;
pub mod text_animation;
pub mod timeline;
pub mod video_layout;

pub use agent::{
    AgentContentBlock, AgentMention, AgentMessage, AgentMessageRole, AgentTimelineRangeMention,
    ChatSession, ToolResultBlock,
};
pub use effect::{CurvePoint, Effect, EffectParam, GradeCurve};
pub use generation_log::{GenerationLog, GenerationLogEntry};
pub use library::{Event, Library, ProjectRef};
pub use matte::MatteAspect;
pub use media_manifest::{
    GenerationInput, MediaFolder, MediaManifest, MediaManifestEntry, MediaSource,
};
pub use multicam::{MulticamMember, MulticamMemberKind, MulticamSource, MulticamSyncMap};
pub use project_file::{ProjectFile, TimelineViewState};
pub use project_registry::{ProjectEntry, ProjectRegistry};
pub use shape_style::{
    Arrowhead, Endpoints, Fill, Point2d, Rgba, ShapeAnimationPreset, ShapeKind, ShapeStyle, Stroke,
};
pub use text_animation::{
    TextAnimation, TextAnimationPreset, TextAnimationRenderMode, WordTiming,
};
pub use timeline::{
    AnimPair, BlendMode, ChromaKey, Clip, ClipType, Crop, Interpolation, Keyframe, KeyframeTrack,
    TextAlignment, TextFill, TextRgba, TextShadow, TextStyle, Timeline, Track, Transform,
};
pub use video_layout::{LayoutFit, LayoutRect, LayoutSlot, VideoLayout};

pub const PROJECT_EXTENSION: &str = "palmier";
pub const TIMELINE_FILENAME: &str = "project.json";
pub const MANIFEST_FILENAME: &str = "media.json";
pub const GENERATION_LOG_FILENAME: &str = "generation-log.json";
pub const THUMBNAIL_FILENAME: &str = "thumbnail.jpg";
pub const MEDIA_DIRECTORY_NAME: &str = "media";
pub const CHAT_DIRECTORY_NAME: &str = "chat";
pub const TRANSCRIPTS_DIRECTORY_NAME: &str = "transcripts";
pub const VISUAL_INDEXES_DIRECTORY_NAME: &str = "visual_indexes";
