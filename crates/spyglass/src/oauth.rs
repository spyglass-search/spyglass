use shared::response::ConnectionResult;
use std::collections::HashMap;

/// TODO: Move this into a configuration file?
pub fn supported_connections() -> HashMap<String, ConnectionResult> {
    let conns = vec![
        ConnectionResult {
            id: "calendar.google.com".to_string(),
            label: "Google Calendar".to_string(),
            description: r#"Adds indexing support for Google calendar events."#.to_string(),
            scopes: Vec::new(),
            is_connected: false,
        },
        ConnectionResult {
            id: "drive.google.com".to_string(),
            label: "Google Drive".to_string(),
            description: r#"Adds indexing support for Google drive. This will allow you
            to search for through documents, spreadsheets, & presentations."#
                .to_string(),
            scopes: Vec::new(),
            is_connected: false,
        },
        ConnectionResult {
            id: "mail.google.com".to_string(),
            label: "Gmail".to_string(),
            description: r#"Adds indexing support for Gmail."#.to_string(),
            scopes: Vec::new(),
            is_connected: false,
        },
    ];

    conns
        .into_iter()
        .map(|conn| (conn.id.clone(), conn.clone()))
        .collect()
}

/// TODO: Return a client trait that can be used by the crawler to sync with any service.
pub fn connection_secret(id: &str) -> (String, String) {
    if id.ends_with("google.com") {
        (
            "621713166215-621sdvu6vhj4t03u536p3b2u08o72ndh.apps.googleusercontent.com".to_string(),
            "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".to_string(),
        )
    } else {
        ("".to_string(), "".to_string())
    }
}
