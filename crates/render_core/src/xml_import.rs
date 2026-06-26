//! XML timeline import model for professional NLE interchange (Issue #154).
//!
//! Supports parsing XMEML (FCP7/FCPX legacy), FCPXML, Premiere XML, and
//! DaVinci Resolve XML into the Fronda timeline model.

use serde::{Deserialize, Serialize};

/// The XML format to import from (Issue #154).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum XmlImportFormat {
    /// XMEML 4 / Final Cut Pro 7 XML (same format as our export).
    Xmeml,
    /// Final Cut Pro X XML (FCPXML 1.x).
    Fcpxml,
    /// Adobe Premiere Pro XML (via File → Export → Final Cut Pro XML).
    PremiereXml,
    /// DaVinci Resolve XML (via Timeline → Export → AAF/XML).
    DavinciXml,
}

impl XmlImportFormat {
    /// Infer the format from the file extension.
    pub fn from_extension(ext: &str) -> Option<Self> {
        match ext.trim_start_matches('.').to_lowercase().as_str() {
            "xml" => Some(XmlImportFormat::Xmeml), // default for .xml
            "fcpxml" => Some(XmlImportFormat::Fcpxml),
            _ => None,
        }
    }

    /// Infer the format from XML content heuristics (root element / namespace).
    pub fn from_xml_content(content: &str) -> Option<Self> {
        if content.contains("<fcpxml") {
            Some(XmlImportFormat::Fcpxml)
        } else if content.contains("<xmeml") {
            Some(XmlImportFormat::Xmeml)
        } else if content.contains("PremiereData") || content.contains("Premiere") {
            Some(XmlImportFormat::PremiereXml)
        } else if content.contains("DaVinci") || content.contains("davinci") {
            Some(XmlImportFormat::DavinciXml)
        } else {
            None
        }
    }

    pub fn display_name(&self) -> &'static str {
        match self {
            XmlImportFormat::Xmeml => "XMEML (FCP7)",
            XmlImportFormat::Fcpxml => "FCPXML (FCP X)",
            XmlImportFormat::PremiereXml => "Premiere Pro XML",
            XmlImportFormat::DavinciXml => "DaVinci Resolve XML",
        }
    }
}

/// Request to import an XML timeline file (Issue #154).
#[derive(Debug, Clone, PartialEq)]
pub struct XmlImportRequest {
    /// Path to the XML file.
    pub path: String,
    /// Detected or user-specified format.
    pub format: XmlImportFormat,
    /// Whether to preserve the original project FPS (true) or adopt
    /// the imported timeline's FPS (false).
    pub preserve_project_fps: bool,
}

impl XmlImportRequest {
    /// Create an import request, inferring the format from the file extension.
    pub fn from_path(path: impl Into<String>) -> Self {
        let path = path.into();
        let ext = path.rsplit('.').next().unwrap_or("");
        let format = XmlImportFormat::from_extension(ext)
            .unwrap_or(XmlImportFormat::Xmeml);
        Self {
            path,
            format,
            preserve_project_fps: false,
        }
    }
}

/// Error types for XML import (Issue #154).
#[derive(Debug, Clone, PartialEq)]
pub enum XmlImportError {
    /// The file could not be read.
    FileReadError { path: String, reason: String },
    /// The XML could not be parsed.
    ParseError { reason: String },
    /// The format was not recognized.
    UnknownFormat,
    /// The format is recognized but import is not yet implemented.
    NotImplemented { format: XmlImportFormat },
}

impl std::fmt::Display for XmlImportError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            XmlImportError::FileReadError { path, reason } => {
                write!(f, "Could not read '{path}': {reason}")
            }
            XmlImportError::ParseError { reason } => {
                write!(f, "XML parse error: {reason}")
            }
            XmlImportError::UnknownFormat => {
                write!(f, "Could not determine XML format from file content")
            }
            XmlImportError::NotImplemented { format } => {
                write!(f, "{} import is not yet implemented", format.display_name())
            }
        }
    }
}

/// Validate an XML import request without performing the actual import.
///
/// Returns `Ok(())` if the request is valid, or an error describing
/// why the import would fail.
pub fn validate_xml_import(request: &XmlImportRequest) -> Result<(), XmlImportError> {
    if request.path.is_empty() {
        return Err(XmlImportError::FileReadError {
            path: request.path.clone(),
            reason: "path must not be empty".into(),
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn xml_import_format_from_extension() {
        assert_eq!(
            XmlImportFormat::from_extension("xml"),
            Some(XmlImportFormat::Xmeml)
        );
        assert_eq!(
            XmlImportFormat::from_extension("fcpxml"),
            Some(XmlImportFormat::Fcpxml)
        );
        assert_eq!(XmlImportFormat::from_extension("pdf"), None);
    }

    #[test]
    fn xml_import_format_from_content_fcpxml() {
        let content = r#"<?xml version="1.0"?><fcpxml version="1.10">"#;
        assert_eq!(
            XmlImportFormat::from_xml_content(content),
            Some(XmlImportFormat::Fcpxml)
        );
    }

    #[test]
    fn xml_import_format_from_content_xmeml() {
        let content = r#"<?xml version="1.0"?><xmeml version="4">"#;
        assert_eq!(
            XmlImportFormat::from_xml_content(content),
            Some(XmlImportFormat::Xmeml)
        );
    }

    #[test]
    fn xml_import_format_from_content_unknown() {
        assert_eq!(XmlImportFormat::from_xml_content("<html>"), None);
    }

    #[test]
    fn xml_import_request_infers_format() {
        let req = XmlImportRequest::from_path("/project.fcpxml");
        assert_eq!(req.format, XmlImportFormat::Fcpxml);
        assert!(!req.preserve_project_fps);
    }

    #[test]
    fn xml_import_request_xml_extension_defaults_xmeml() {
        let req = XmlImportRequest::from_path("/export.xml");
        assert_eq!(req.format, XmlImportFormat::Xmeml);
    }

    #[test]
    fn validate_xml_import_empty_path() {
        let req = XmlImportRequest {
            path: String::new(),
            format: XmlImportFormat::Xmeml,
            preserve_project_fps: false,
        };
        let err = validate_xml_import(&req).unwrap_err();
        assert!(err.to_string().contains("path must not be empty"));
    }

    #[test]
    fn validate_xml_import_valid_path() {
        let req = XmlImportRequest::from_path("/some/file.xml");
        assert!(validate_xml_import(&req).is_ok());
    }

    #[test]
    fn xml_import_error_display() {
        let err = XmlImportError::NotImplemented {
            format: XmlImportFormat::Fcpxml,
        };
        assert!(err.to_string().contains("FCPXML"));
        assert!(err.to_string().contains("not yet implemented"));
    }

    #[test]
    fn xml_import_format_display_names_non_empty() {
        for fmt in [
            XmlImportFormat::Xmeml,
            XmlImportFormat::Fcpxml,
            XmlImportFormat::PremiereXml,
            XmlImportFormat::DavinciXml,
        ] {
            assert!(!fmt.display_name().is_empty(), "{fmt:?} has no display name");
        }
    }
}
