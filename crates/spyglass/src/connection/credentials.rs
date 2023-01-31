use libgithub::types::AuthScopes as GithubScope;
use libgoog::types::AuthScope;
use shared::response::SupportedConnection;
use std::collections::HashMap;

/// TODO: Move this into a configuration file?
pub fn supported_connections() -> HashMap<String, SupportedConnection> {
    let conns = vec![
        SupportedConnection {
            id: "api.github.com".to_string(),
            label: "Github".to_string(),
            description: "Adds indexing support for Github owned repos, starred repos, and issues."
                .to_string(),
        },
        SupportedConnection {
            id: "calendar.google.com".to_string(),
            label: "Google Calendar".to_string(),
            description: r#"Adds indexing support for Google calendar events."#.to_string(),
        },
        SupportedConnection {
            id: "drive.google.com".to_string(),
            label: "Google Drive".to_string(),
            description: r#"Adds indexing support for Google drive. This will allow you
            to search for through documents, spreadsheets, and presentations."#
                .to_string(),
        },
        // Requires a security audit, lets do this later.
        // SupportedConnection {
        //     id: "mail.google.com".to_string(),
        //     label: "Gmail".to_string(),
        //     description: r#"Adds indexing support for Gmail."#.to_string(),
        //     scopes: Vec::new(),
        //     is_connected: false,
        // },
    ];

    conns
        .into_iter()
        .map(|conn| (conn.id.clone(), conn))
        .collect()
}

/// TODO: Return a client trait that can be used by the crawler to sync with any service.
pub fn connection_secret(id: &str) -> Option<(String, String, Vec<String>)> {
    match id {
        "api.github.com" => Some((
            "597b78b3396e47d71872".to_string(),
            "dfa1a3b482e16ba39c729bc393625291db423d6e".to_string(),
            vec![GithubScope::Repo.to_string(), GithubScope::User.to_string()],
        )),
        "calendar.google.com" => Some((
            "621713166215-621sdvu6vhj4t03u536p3b2u08o72ndh.apps.googleusercontent.com".to_string(),
            "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".to_string(),
            vec![
                AuthScope::Calendar.to_string(),
                AuthScope::Email.to_string(),
            ],
        )),
        "drive.google.com" => Some((
            "621713166215-621sdvu6vhj4t03u536p3b2u08o72ndh.apps.googleusercontent.com".to_string(),
            "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".to_string(),
            vec![AuthScope::Drive.to_string(), AuthScope::Email.to_string()],
        )),
        "mail.google.com" => Some((
            "621713166215-621sdvu6vhj4t03u536p3b2u08o72ndh.apps.googleusercontent.com".to_string(),
            "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".to_string(),
            vec![AuthScope::Gmail.to_string(), AuthScope::Email.to_string()],
        )),
        _ => None,
    }
}
