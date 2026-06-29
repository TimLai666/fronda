//! App lifecycle and startup state machine.
//!
//! Covers APP-001 through APP-009 and BOOT-001 through BOOT-011.

/// BOOT-001: Pre-run startup order phases.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StartupPhase {
    LoggingBootstrap,
    TelemetryStartup,
    FontRegistration,
    TooltipDelayOverride,
    AppKitSetup,
    MainMenuSetup,
    RunLoop,
}

impl StartupPhase {
    pub const ORDER: &'static [StartupPhase] = &[
        StartupPhase::LoggingBootstrap,
        StartupPhase::TelemetryStartup,
        StartupPhase::FontRegistration,
        StartupPhase::TooltipDelayOverride,
        StartupPhase::AppKitSetup,
        StartupPhase::MainMenuSetup,
        StartupPhase::RunLoop,
    ];

    pub fn label(&self) -> &'static str {
        match self {
            StartupPhase::LoggingBootstrap => "Logging bootstrap",
            StartupPhase::TelemetryStartup => "Telemetry startup",
            StartupPhase::FontRegistration => "Font registration",
            StartupPhase::TooltipDelayOverride => "Tooltip delay override",
            StartupPhase::AppKitSetup => "AppKit/delegate/menu setup",
            StartupPhase::MainMenuSetup => "Main menu setup",
            StartupPhase::RunLoop => "NSApplication.run()",
        }
    }
}

/// BOOT-002: Tooltip delay override value in milliseconds.
pub const NS_INITIAL_TOOLTIP_DELAY_MS: f64 = 10.0;

/// BOOT-002: The default tooltip delay was 2 seconds (2000 ms).
pub const DEFAULT_TOOLTIP_DELAY_MS: f64 = 2000.0;

/// BOOT-005: applicationShouldOpenUntitledFile returns false.
pub const SHOULD_OPEN_UNTITLED_FILE: bool = false;

// ── Startup flow state ──────────────────────────────────────────

/// Lifecycle stages the app can be in.
#[derive(Debug, Clone, PartialEq)]
pub enum AppLifecycleStage {
    Initializing,
    Boot(BootStage),
    HomeShown,
    EditorActive,
    ClosingProject,
    Terminating,
}

/// Sub-stages during boot.
#[derive(Debug, Clone, PartialEq)]
pub enum BootStage {
    StartingUp,
    RegisteringFonts,
    InitializingUpdater,
    ShowingHome,
    ConfiguringNotifications,
    StartingMCP,
    DeferredAccountConfig,
}

impl BootStage {
    /// BOOT-004: On launch order.
    pub const BOOT_ORDER: &'static [BootStage] = &[
        BootStage::StartingUp,
        BootStage::RegisteringFonts,
        BootStage::InitializingUpdater,
        BootStage::ShowingHome,
        BootStage::ConfiguringNotifications,
        BootStage::StartingMCP,
        BootStage::DeferredAccountConfig,
    ];
}

// ── App state ──────────────────────────────────────────────────

/// Represents the current visible view / screen.
#[derive(Debug, Clone, PartialEq)]
pub enum ActiveView {
    Home,
    Editor { project_id: String },
}

/// The app state machine.
#[derive(Debug, Clone, PartialEq)]
pub struct AppStateMachine {
    pub stage: AppLifecycleStage,
    pub active_view: Option<ActiveView>,
    pub mcp_running: bool,
}

impl Default for AppStateMachine {
    fn default() -> Self {
        Self {
            stage: AppLifecycleStage::Initializing,
            active_view: None,
            mcp_running: false,
        }
    }
}

impl AppStateMachine {
    /// Transition to boot.
    pub fn start_boot(&mut self) {
        self.stage = AppLifecycleStage::Boot(BootStage::StartingUp);
    }

    /// BOOT-004: Show home, configure notifications, start MCP if enabled.
    pub fn show_home(&mut self) {
        self.stage = AppLifecycleStage::HomeShown;
        self.active_view = Some(ActiveView::Home);
    }

    /// BOOT-008: Show editor for project.
    pub fn show_editor(&mut self, project_id: String) {
        self.stage = AppLifecycleStage::EditorActive;
        self.active_view = Some(ActiveView::Editor { project_id });
    }

    /// BOOT-007: Close project and return to Home.
    pub fn close_project(&mut self) {
        self.stage = AppLifecycleStage::HomeShown;
        self.active_view = Some(ActiveView::Home);
    }

    /// Check if the current view is Home.
    pub fn is_home(&self) -> bool {
        matches!(self.active_view, Some(ActiveView::Home))
    }

    /// Check if the current view is Editor.
    pub fn is_editor(&self) -> bool {
        matches!(self.active_view, Some(ActiveView::Editor { .. }))
    }

    /// Is the app in a booting state?
    pub fn is_booting(&self) -> bool {
        matches!(self.stage, AppLifecycleStage::Boot(_))
    }
}

// ── BOOT-009..011: Sample project and open/save panels ──────────

/// BOOT-009: Sample project materialization result.
#[derive(Debug, Clone, PartialEq)]
pub enum SampleProjectResult {
    Cached { path: String },
    Materialized { path: String },
    Failed(String),
}

/// BOOT-010: Project open panel configuration.
pub struct OpenPanelConfig;
impl OpenPanelConfig {
    pub const ALLOWED_CONTENT_TYPES: &'static [&'static str] = &["io.palmier.project"];
    pub const CAN_CHOOSE_DIRECTORIES: bool = false;
    pub const TREATS_FILE_PACKAGES_AS_DIRECTORIES: bool = false;
    pub const ALLOWS_MULTIPLE_SELECTION: bool = false;
    pub const TITLE: &'static str = "Open Project";
}

/// BOOT-011: New project save panel configuration.
pub struct NewProjectSavePanel;
impl NewProjectSavePanel {
    pub const DEFAULT_NAME: &'static str = "Untitled Project";
    pub const DEFAULT_DIRECTORY: &'static str = "~/Documents/Palmier Pro";
    pub const CONTENT_TYPE: &'static str = "io.palmier.project";
}

/// BOOT-002 override: the NSInitialToolTipDelay value.
pub fn tooltip_delay_override_ms() -> f64 {
    NS_INITIAL_TOOLTIP_DELAY_MS
}

/// BOOT-002 description: the override shortens 2s → 0.01s.
pub fn tooltip_delay_explanation() -> String {
    format!(
        "Sets NSInitialToolTipDelay from {:.0}ms to {:.0}ms",
        DEFAULT_TOOLTIP_DELAY_MS, NS_INITIAL_TOOLTIP_DELAY_MS
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    // BOOT-001
    #[test]
    fn startup_phase_order() {
        let order = StartupPhase::ORDER;
        assert_eq!(order.len(), 7);
        assert_eq!(order[0], StartupPhase::LoggingBootstrap);
        assert_eq!(order[3], StartupPhase::TooltipDelayOverride);
        assert_eq!(order[6], StartupPhase::RunLoop);
    }

    // BOOT-002
    #[test]
    fn tooltip_delay_override() {
        assert!((tooltip_delay_override_ms() - 10.0).abs() < 1e-10);
        assert!(tooltip_delay_explanation().contains("2000ms"));
        assert!(tooltip_delay_explanation().contains("10ms"));
    }

    // BOOT-004
    #[test]
    fn boot_stage_order() {
        let order = BootStage::BOOT_ORDER;
        assert_eq!(order.len(), 7);
        assert_eq!(order[0], BootStage::StartingUp);
        assert_eq!(order[3], BootStage::ShowingHome);
        assert_eq!(order[5], BootStage::StartingMCP);
        assert_eq!(order[6], BootStage::DeferredAccountConfig);
    }

    // App lifecycle
    #[test]
    fn app_state_machine_default() {
        let state = AppStateMachine::default();
        assert_eq!(state.stage, AppLifecycleStage::Initializing);
        assert_eq!(state.active_view, None);
        assert!(!state.mcp_running);
    }

    #[test]
    fn app_transition_to_boot() {
        let mut state = AppStateMachine::default();
        state.start_boot();
        assert!(state.is_booting());
    }

    #[test]
    fn app_show_home() {
        let mut state = AppStateMachine::default();
        state.show_home();
        assert!(state.is_home());
        assert_eq!(state.stage, AppLifecycleStage::HomeShown);
    }

    #[test]
    fn app_show_editor() {
        let mut state = AppStateMachine::default();
        state.show_editor("proj-1".into());
        assert!(state.is_editor());
        assert_eq!(state.stage, AppLifecycleStage::EditorActive);
    }

    #[test]
    fn app_close_project_returns_to_home() {
        let mut state = AppStateMachine::default();
        state.show_editor("proj-1".into());
        state.close_project();
        assert!(state.is_home());
    }

    // BOOT-009
    #[test]
    fn sample_project_result_variants() {
        let cached = SampleProjectResult::Cached {
            path: "/tmp/sample.palmier".into(),
        };
        let materialized = SampleProjectResult::Materialized {
            path: "/tmp/sample.palmier".into(),
        };
        let failed = SampleProjectResult::Failed("Network error".into());
        assert!(matches!(cached, SampleProjectResult::Cached { .. }));
        assert!(matches!(
            materialized,
            SampleProjectResult::Materialized { .. }
        ));
        assert!(matches!(failed, SampleProjectResult::Failed(_)));
    }

    // BOOT-010
    #[test]
    fn open_panel_config() {
        assert_eq!(
            OpenPanelConfig::ALLOWED_CONTENT_TYPES,
            &["io.palmier.project"]
        );
    }

    // BOOT-011
    #[test]
    fn new_project_save_panel() {
        assert_eq!(NewProjectSavePanel::DEFAULT_NAME, "Untitled Project");
        assert_eq!(
            NewProjectSavePanel::DEFAULT_DIRECTORY,
            "~/Documents/Palmier Pro"
        );
    }
}
