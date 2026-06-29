use std::path::PathBuf;
use uuid::Uuid;

/// Result of planning a project duplication.
#[derive(Debug, Clone, PartialEq)]
pub struct DuplicatePlan {
    /// The new UUID for the duplicated project.
    pub new_project_id: Uuid,
    /// The source path of the original project.
    pub source_path: PathBuf,
    /// The destination path for the duplicate.
    pub destination_path: PathBuf,
    /// Whether to register the duplicate in the recent projects list.
    pub register_in_recents: bool,
}

/// Options for project duplication.
#[derive(Debug, Clone, PartialEq)]
pub struct DuplicateOptions {
    /// Optional custom destination directory. If None, derive from source + " (Copy)".
    pub destination_dir: Option<PathBuf>,
    /// Whether to register in recents after duplication. Default: true.
    pub register_in_recents: bool,
    /// Custom new project id. If None, generate a new UUID.
    pub new_project_id: Option<Uuid>,
}

impl Default for DuplicateOptions {
    fn default() -> Self {
        Self {
            destination_dir: None,
            register_in_recents: true,
            new_project_id: None,
        }
    }
}

/// Plan a project duplication without executing it.
///
/// Returns a `DuplicatePlan` describing what would happen. Pure logic:
/// - Generates a new UUID for the project identity
/// - Computes destination path (source parent / "{project_name} (Copy)[.ext]")
/// - Validates that source != destination (error if same)
pub fn plan_duplicate(
    source_path: &std::path::Path,
    options: &DuplicateOptions,
    project_name: &str,
) -> Result<DuplicatePlan, String> {
    let new_project_id = options.new_project_id.unwrap_or_else(Uuid::new_v4);

    let destination_path = match &options.destination_dir {
        Some(dir) => dir.clone(),
        None => {
            let parent = source_path
                .parent()
                .ok_or_else(|| "source path has no parent directory".to_string())?;
            let extension = source_path
                .extension()
                .and_then(|ext| ext.to_str())
                .map(|ext| format!(".{ext}"))
                .unwrap_or_default();
            let dest_name = format!("{project_name} (Copy){extension}");
            parent.join(dest_name)
        }
    };

    if source_path == destination_path {
        return Err("source and destination paths are the same".to_string());
    }

    Ok(DuplicatePlan {
        new_project_id,
        source_path: source_path.to_path_buf(),
        destination_path,
        register_in_recents: options.register_in_recents,
    })
}

/// Check whether a path looks like a valid project bundle root.
///
/// A valid project bundle path must have a parent directory and a non-empty
/// file-name component. This is a pure naming-convention check — no
/// filesystem access is performed.
pub fn is_valid_bundle_path(path: &std::path::Path) -> bool {
    path.parent().is_some()
        && path
            .file_name()
            .and_then(|s| s.to_str())
            .is_some_and(|s| !s.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn plan_duplicate_generates_new_id() {
        let source = Path::new("/tmp/original.palmier");
        let options = DuplicateOptions::default();
        let plan1 = plan_duplicate(source, &options, "Original").unwrap();
        let plan2 = plan_duplicate(source, &options, "Original").unwrap();
        // Each call should generate a fresh UUID
        assert_ne!(plan1.new_project_id, plan2.new_project_id);
    }

    #[test]
    fn plan_duplicate_derives_destination() {
        let source = Path::new("/tmp/MyProject");
        let options = DuplicateOptions::default();
        let plan = plan_duplicate(source, &options, "MyProject").unwrap();
        let expected = Path::new("/tmp/MyProject (Copy)");
        assert_eq!(plan.destination_path, expected);
    }

    #[test]
    fn plan_duplicate_custom_destination() {
        let source = Path::new("/tmp/Original");
        let custom_dest = PathBuf::from("/custom/path/Duplicate");
        let options = DuplicateOptions {
            destination_dir: Some(custom_dest.clone()),
            ..Default::default()
        };
        let plan = plan_duplicate(source, &options, "Original").unwrap();
        assert_eq!(plan.destination_path, custom_dest);
    }

    #[test]
    fn plan_duplicate_register_in_recents_default_true() {
        let source = Path::new("/tmp/project");
        let options = DuplicateOptions::default();
        let plan = plan_duplicate(source, &options, "Project").unwrap();
        assert!(plan.register_in_recents);
    }

    #[test]
    fn plan_duplicate_custom_project_id() {
        let source = Path::new("/tmp/project");
        let custom_id = Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").unwrap();
        let options = DuplicateOptions {
            new_project_id: Some(custom_id),
            ..Default::default()
        };
        let plan = plan_duplicate(source, &options, "Project").unwrap();
        assert_eq!(plan.new_project_id, custom_id);
    }

    #[test]
    fn plan_duplicate_source_destination_same_error() {
        let source = Path::new("/tmp/MyProject");
        let options = DuplicateOptions {
            destination_dir: Some(source.to_path_buf()),
            ..Default::default()
        };
        let result = plan_duplicate(source, &options, "MyProject");
        assert!(result.is_err());
    }

    #[test]
    fn default_options_register_true() {
        let options = DuplicateOptions::default();
        assert!(options.register_in_recents);
        assert!(options.new_project_id.is_none());
        assert!(options.destination_dir.is_none());
    }

    #[test]
    fn is_valid_bundle_path_accepts_normal_names() {
        let path = Path::new("/home/user/My Project (Copy)");
        assert!(is_valid_bundle_path(path));
    }

    #[test]
    fn is_valid_bundle_path_rejects_empty_name() {
        let path = Path::new("");
        assert!(!is_valid_bundle_path(path));
    }

    #[test]
    fn plan_duplicate_preserves_extension() {
        let source = Path::new("/tmp/My Project.palmier");
        let options = DuplicateOptions::default();
        let plan = plan_duplicate(source, &options, "My Project").unwrap();
        let expected = Path::new("/tmp/My Project (Copy).palmier");
        assert_eq!(plan.destination_path, expected);
    }
}
