//! Agent/MCP tool definitions, ID shortening, and system prompt contract.

pub mod id_short;
pub mod read_tools;
pub mod tools;

pub use tools::{all_tools, ToolDefinition, SYSTEM_INSTRUCTION};
