//! OGC service-level settings (title / abstract / keywords).
//!
//! Persisted in the metadata store (`service_settings` table) and mirrored in
//! `AppState.service_settings` for reads. Consumed by the OGC service
//! capabilities documents (WMS / WFS / WCS / …).

use serde::{Deserialize, Serialize};

/// Settings for one OGC service (e.g. `wms`, `wfs`, `wcs`).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ServiceSettings {
    pub title: Option<String>,
    #[serde(rename = "abstract")]
    pub abstract_text: Option<String>,
    #[serde(default)]
    pub keywords: Vec<String>,
}

impl ServiceSettings {
    /// Whether every field is unset (used to decide if the settings are "empty").
    pub fn is_empty(&self) -> bool {
        self.title.is_none() && self.abstract_text.is_none() && self.keywords.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_service_settings_serde_roundtrip() {
        let s = ServiceSettings {
            title: Some("Terrane WMS".to_string()),
            abstract_text: Some("Web Map Service".to_string()),
            keywords: vec!["WMS".to_string(), "GIS".to_string()],
        };
        let json = serde_json::to_string(&s).unwrap();
        let back: ServiceSettings = serde_json::from_str(&json).unwrap();
        assert_eq!(back.title.as_deref(), Some("Terrane WMS"));
        assert_eq!(back.keywords.len(), 2);
    }

    #[test]
    fn test_service_settings_empty() {
        assert!(ServiceSettings::default().is_empty());
        let s = ServiceSettings {
            title: Some("X".to_string()),
            ..Default::default()
        };
        assert!(!s.is_empty());
    }
}
