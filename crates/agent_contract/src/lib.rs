//! Agent/MCP tool definitions, ID shortening, and system prompt contract.

pub mod id_short;
pub mod mention;
pub mod mutation;
pub mod read_tools;
pub mod session;
pub mod tools;
pub mod undo;

pub use undo::{UndoCommand, UndoError, UndoStack};

pub use tools::{all_tools, ToolDefinition, SYSTEM_INSTRUCTION};

pub use mutation::{
    AddCaptionsInput, AddClipsInput, AddTextsInput, CreateFolderInput, DeleteFolderInput,
    DeleteMediaInput, InsertClipsInput, MoveClipsInput, MoveToFolderInput, RemoveClipsInput,
    RemoveTracksInput, RenameFolderInput, RenameMediaInput, RippleDeleteRangesInput,
    SetClipPropertiesInput, SetKeyframesInput, SplitClipInput, TextInput,
    validate_add_captions, validate_add_clips, validate_add_texts, validate_create_folder,
    validate_delete_folder, validate_delete_media, validate_hex_color, validate_insert_clips,
    validate_move_clips, validate_move_clips_linked, validate_move_to_folder,
    validate_remove_clips, validate_remove_tracks, validate_rename_folder, validate_rename_media,
    validate_ripple_delete_ranges, validate_set_clip_properties, validate_set_keyframes,
    validate_split_clip,
};
