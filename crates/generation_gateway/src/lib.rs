//! Fronda Generation Gateway — a self-hosted service implementing Fronda
//! Generation Protocol v1.1 (submit, poll, providers catalog) with a pluggable
//! provider registry. Phase 1 ships stub providers so the full
//! Fronda → gateway → result loop runs with no external keys; real provider
//! adapters (Gemini/fal/…) implement the same `GenerationProvider` trait in
//! phase 2.

pub mod config;
pub mod jobs;
pub mod protocol;
pub mod provider;
pub mod registry;
pub mod server;
pub mod stub;

pub use config::GatewayConfig;
pub use protocol::{
    ErrorResponse, GenerateRequest, JobStatusResponse, ProviderCatalogEntry, ProvidersCatalog,
    SubmitResponse,
};
pub use provider::{GenerationProvider, ProviderJob, ProviderKind, ProviderStatus};
pub use registry::{ProviderRegistry, RouteError};
pub use server::{build_router, build_stub_registry, stub_app_state, AppState};
pub use stub::StubProvider;
