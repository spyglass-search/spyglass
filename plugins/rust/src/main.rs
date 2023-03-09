use spyglass_plugin::*;
use url::Url;

#[derive(Default)]
struct Plugin;

register_plugin!(Plugin);

const TAG_TYPE: &str = "type";
const EXT_TAG: &str = "fileext";
const RUST_PROJECT_TAG: &str = "RustProject";
const RUST_PROJECT_TOML_TAG: &str = "RustProjectToml";

impl SpyglassPlugin for Plugin {
    fn load(&mut self) {
        let _ = subscribe_for_documents(DocumentQuery {
            has_tags: Some(vec![(String::from(EXT_TAG), String::from("toml"))]),
            exclude_tags: Some(vec![(
                String::from(TAG_TYPE),
                (String::from(RUST_PROJECT_TOML_TAG)),
            )]),
            ..Default::default()
        });
    }

    fn update(&mut self, event: PluginEvent) {
        match event {
            PluginEvent::DocumentResponse {
                request_id: _,
                page_count: _,
                page: _,
                documents,
            } => {
                let ids = documents
                    .iter()
                    .filter_map(|doc| {
                        if doc.url.ends_with("Cargo.toml") {
                            return Some(doc.doc_id.clone());
                        }
                        None
                    })
                    .collect::<Vec<String>>();
                let parent_urls = documents
                    .iter()
                    .filter_map(|doc| {
                        if doc.url.ends_with("Cargo.toml") {
                            if let Ok(url) = Url::parse(&doc.url) {
                                if let Ok(file_path) = url.to_file_path() {
                                    if let Some(parent) = file_path.parent() {
                                        return Some(utils::path_string_to_uri(
                                            &parent.display().to_string(),
                                        ));
                                    }
                                }
                            }
                        }
                        None
                    })
                    .collect::<Vec<String>>();

                let _result = modify_tags(
                    DocumentQuery {
                        ids: Some(ids),
                        ..Default::default()
                    },
                    TagModification {
                        add: Some(vec![(
                            String::from(TAG_TYPE),
                            String::from(RUST_PROJECT_TOML_TAG),
                        )]),
                        ..Default::default()
                    },
                );

                let _result = modify_tags(
                    DocumentQuery {
                        urls: Some(parent_urls),
                        exclude_tags: Some(vec![(
                            String::from(TAG_TYPE),
                            (String::from(RUST_PROJECT_TAG)),
                        )]),
                        ..Default::default()
                    },
                    TagModification {
                        add: Some(vec![(
                            String::from(TAG_TYPE),
                            String::from(RUST_PROJECT_TAG),
                        )]),
                        ..Default::default()
                    },
                );
            }
        }
    }
}
