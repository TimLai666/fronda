//! Agent/MCP tool definitions, ID shortening, and system prompt contract.

pub mod id_short;
pub mod mention;
pub mod read_tools;
pub mod session;
pub mod tools;

pub use tools::{all_tools, ToolDefinition, SYSTEM_INSTRUCTION};
