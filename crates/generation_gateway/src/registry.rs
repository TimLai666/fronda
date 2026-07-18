//! Provider registry: resolves `(kind, optional provider name)` to a concrete
//! provider, defaulting per kind when the name is omitted, and builds the
//! `/v1/providers` catalog.

use std::collections::HashMap;
use std::sync::Arc;

use crate::protocol::{ProviderCatalogEntry, ProvidersCatalog};
use crate::provider::{GenerationProvider, ProviderKind};

/// Why a route failed. Both variants carry enough context for an explicit,
/// client-readable error message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RouteError {
    /// No provider is registered for this kind at all (and no request named one).
    NoProviderForKind(ProviderKind),
    /// A provider name was requested but no provider is registered under it.
    UnknownProvider { kind: ProviderKind, name: String },
}

impl std::fmt::Display for RouteError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RouteError::NoProviderForKind(kind) => {
                write!(f, "no provider registered for kind '{}'", kind.as_str())
            }
            RouteError::UnknownProvider { kind, name } => write!(
                f,
                "unknown provider '{}' for kind '{}'",
                name,
                kind.as_str()
            ),
        }
    }
}

impl std::error::Error for RouteError {}

/// A set of providers keyed by `(kind, name)`, with a chosen default per kind.
#[derive(Default)]
pub struct ProviderRegistry {
    providers: HashMap<(ProviderKind, String), Arc<dyn GenerationProvider>>,
    defaults: HashMap<ProviderKind, String>,
}

impl ProviderRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a provider. The first provider registered for a kind becomes that
    /// kind's default (override later with `set_default`).
    pub fn register(&mut self, provider: Arc<dyn GenerationProvider>) {
        let kind = provider.kind();
        let name = provider.name().to_string();
        self.defaults.entry(kind).or_insert_with(|| name.clone());
        self.providers.insert((kind, name), provider);
    }

    /// Point a kind's default at an already-registered provider name.
    pub fn set_default(&mut self, kind: ProviderKind, name: &str) -> Result<(), RouteError> {
        if self.providers.contains_key(&(kind, name.to_string())) {
            self.defaults.insert(kind, name.to_string());
            Ok(())
        } else {
            Err(RouteError::UnknownProvider {
                kind,
                name: name.to_string(),
            })
        }
    }

    /// Resolve a provider. `None` provider → the kind's default (error if the kind
    /// has none); a named provider that is not registered → an explicit error.
    pub fn route(
        &self,
        kind: ProviderKind,
        provider: Option<&str>,
    ) -> Result<Arc<dyn GenerationProvider>, RouteError> {
        match provider {
            Some(name) => self
                .providers
                .get(&(kind, name.to_string()))
                .cloned()
                .ok_or_else(|| RouteError::UnknownProvider {
                    kind,
                    name: name.to_string(),
                }),
            None => {
                let name = self
                    .defaults
                    .get(&kind)
                    .cloned()
                    .ok_or(RouteError::NoProviderForKind(kind))?;
                self.providers
                    .get(&(kind, name))
                    .cloned()
                    .ok_or(RouteError::NoProviderForKind(kind))
            }
        }
    }

    /// Build the `/v1/providers` catalog, grouped by kind. Entries are sorted by
    /// name for deterministic output (HashMap iteration order is unspecified).
    pub fn catalog(&self) -> ProvidersCatalog {
        let mut grouped: Vec<(ProviderKind, ProviderCatalogEntry)> = self
            .providers
            .iter()
            .map(|((kind, name), provider)| {
                (
                    *kind,
                    ProviderCatalogEntry {
                        name: name.clone(),
                        models: provider.models(),
                    },
                )
            })
            .collect();
        grouped.sort_by(|a, b| a.1.name.cmp(&b.1.name));

        let mut catalog = ProvidersCatalog::default();
        for (kind, entry) in grouped {
            catalog.bucket_mut(kind).push(entry);
        }
        catalog
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::jobs::JobStore;
    use crate::stub::StubProvider;

    fn video_only_registry() -> ProviderRegistry {
        let store = Arc::new(JobStore::new());
        let mut reg = ProviderRegistry::new();
        reg.register(Arc::new(StubProvider::new(ProviderKind::Video, store)));
        reg
    }

    fn full_registry() -> ProviderRegistry {
        let store = Arc::new(JobStore::new());
        let mut reg = ProviderRegistry::new();
        for kind in ProviderKind::ALL {
            reg.register(Arc::new(StubProvider::new(kind, store.clone())));
        }
        reg
    }

    #[test]
    fn route_hits_named_provider() {
        let reg = full_registry();
        let provider = reg.route(ProviderKind::Video, Some("stub")).unwrap();
        assert_eq!(provider.name(), "stub");
        assert_eq!(provider.kind(), ProviderKind::Video);
    }

    #[test]
    fn route_defaults_when_provider_omitted() {
        let reg = full_registry();
        let provider = reg.route(ProviderKind::Audio, None).unwrap();
        assert_eq!(provider.name(), "stub");
        assert_eq!(provider.kind(), ProviderKind::Audio);
    }

    #[test]
    fn route_unknown_provider_is_explicit_error() {
        let reg = full_registry();
        let err = reg.route(ProviderKind::Video, Some("nope")).err().unwrap();
        assert_eq!(
            err,
            RouteError::UnknownProvider {
                kind: ProviderKind::Video,
                name: "nope".into()
            }
        );
        assert!(err.to_string().contains("nope"));
        assert!(err.to_string().contains("video"));
    }

    #[test]
    fn route_no_provider_for_kind_is_explicit_error() {
        let reg = video_only_registry();
        let err = reg.route(ProviderKind::Audio, None).err().unwrap();
        assert_eq!(err, RouteError::NoProviderForKind(ProviderKind::Audio));
        assert!(err.to_string().contains("audio"));
    }

    #[test]
    fn set_default_rejects_unregistered_name() {
        let mut reg = video_only_registry();
        assert!(reg.set_default(ProviderKind::Video, "ghost").is_err());
        assert!(reg.set_default(ProviderKind::Video, "stub").is_ok());
    }

    #[test]
    fn catalog_lists_every_registered_provider_by_kind() {
        let cat = full_registry().catalog();
        assert_eq!(cat.video.len(), 1);
        assert_eq!(cat.video[0].name, "stub");
        assert_eq!(cat.video[0].models, vec!["stub-video".to_string()]);
        assert_eq!(cat.image[0].models, vec!["stub-image".to_string()]);
        assert_eq!(cat.audio[0].models, vec!["stub-audio".to_string()]);
    }
}
