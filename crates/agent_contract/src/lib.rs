//! Agent/MCP tool definitions, ID shortening, and system prompt contract.

pub mod agent_loop;
pub mod hex_color_parser;
pub mod id_short;
pub mod mention;
pub mod mutation;
pub mod prompt_caching;
pub mod read_tools;
pub mod session;
pub mod test_helpers;
pub mod tool_exec;
pub mod tools;
pub mod undo;

pub use agent_loop::{
    parse_response, run_agent_turn, AgentOutcome, LlmTransport, ParsedResponse, ToolCallRecord,
    ToolUse,
};
pub use tool_exec::{AgentSkill, ClipAudioSource, MatteWriter, ToolExecutor};
pub use tools::{skill_prompt_index, system_instruction_with_skills};
pub use undo::{UndoCommand, UndoError, UndoStack};

pub use prompt_caching::{
    build_agent_request, build_cached_conversation, CacheBreakpoint, CacheStrategy, CachedContent,
    CachedConversation, CachedMessage,
};
pub use tools::{all_tools, ToolDefinition, SYSTEM_INSTRUCTION};

pub use hex_color_parser::parse_hex_color;
pub use mutation::{
    validate_add_captions, validate_add_clips, validate_add_texts, validate_create_folder,
    validate_delete_folder, validate_delete_media, validate_duplicate_project,
    validate_generate_music, validate_hex_color, validate_import_folder, validate_insert_clips,
    validate_move_clips, validate_move_clips_linked, validate_move_to_folder,
    validate_remove_clips, validate_remove_tracks, validate_rename_folder, validate_rename_media,
    validate_ripple_delete_ranges, validate_set_blend_mode, validate_set_chroma_key,
    validate_set_clip_properties, validate_set_color_grade, validate_set_keyframes,
    validate_split_clip, AddCaptionsInput, AddClipsInput, AddTextsInput, CreateFolderInput,
    DeleteFolderInput, DeleteMediaInput, DuplicateProjectInput, GenerateMusicInput,
    ImportFolderInput, InsertClipsInput, MoveClipsInput, MoveToFolderInput, RemoveClipsInput,
    RemoveTracksInput, RenameFolderInput, RenameMediaInput, RippleDeleteRangesInput,
    SetBlendModeInput, SetChromaKeyInput, SetClipPropertiesInput, SetColorGradeInput,
    SetKeyframesInput, SplitClipInput, TextInput,
};
