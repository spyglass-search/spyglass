use serde_json::{self, Value};
use spyglass_plugin::*;

#[derive(Default)]
struct Plugin;

register_plugin!(Plugin);

// Example plugin used to illustrate basic features for spyglass plugins
// Load: Is always called when the plugin is loaded. You can expect this method to be
//       called when spyglass is started or the plugin is toggled on.
// Update: Update is called when new asynchronous events are received. The events can
//         be response to requests or generate interval updates received after subscribing
// All plugin functions are completely asynchronous and do not return a result directly. The
// update method can be used to receive updates to api calls
impl SpyglassPlugin for Plugin {
    fn load(&mut self) {
        // Requests a set of documents that match the document query. This example checks for
        // all documents that have the tag named lens with the value of nuget. This method
        // will execute the query at a regular interval and call the update method. A onetime
        // request can also be made using the query_documents function.
        let _ = subscribe_for_documents(DocumentQuery {
            has_tags: Some(vec![(String::from("lens"), String::from("nuget"))]),
            ..Default::default()
        });

        // Request that the system calls update at a regular interval. This is used to allow
        // the plugin to check for updates on a poll interval.
        let _ = subscribe_for_updates();

        // Configuration properties defined in the manifest file are provide to the plugin
        // via environmental variables. Here we access a string configuration and boolean
        // configuration
        if let Ok(api_key) = std::env::var("API_KEY") {
            log(format!("API Key {api_key}").as_str());
        }

        if let Ok(enable_api) = std::env::var("ENABLE_API") {
            log(format!("Enable API {enable_api}").as_str());
        }
    }

    fn update(&mut self, event: PluginEvent) {
        match event {
            PluginEvent::IntervalUpdate => {
                // The log function is used for debug logging. Since the plugin is loaded in as a
                // wasm module normal println! and log::error! will not work. The log statement can
                // be found in the spyglass server log.
                // Note avoid any direct calls to stdout since that is what is used to communicate
                // between spyglass and the wasm module.
                log("Got Interval Update");

                // Creates an http request to the following url. The wasm runtime used does not
                // support direct http access at the moment so common libraries like reqwest
                // will not work. We provide a very minimal set of methods to allow for requesting
                // http resources.
                Http::request("https://azuresearch-usnc.nuget.org/query")
                    .get()
                    .run();
            }
            PluginEvent::HttpResponse { url: _, result } => {
                // When a response is received from an http call it will be received asynchronously
                if let Ok(rslt) = result {
                    if let Some(json) = rslt.as_json() {
                        let packages = json["data"].as_array().unwrap();
                        let mut docs = Vec::new();
                        for package in packages {
                            let tags = build_tags(
                                package["tags"].as_array(),
                                package["authors"].as_array(),
                                package["owners"].as_array(),
                                package["version"].as_str(),
                            );
                            let license_url = package["licenseUrl"].as_str().unwrap();
                            if let Ok(mut url) = url::Url::parse(license_url) {
                                url.path_segments_mut().unwrap().pop();
                                let url_name = url.as_str();
                                let doc = DocumentUpdate {
                                    content: Some(String::from(
                                        package["description"].as_str().unwrap(),
                                    )),
                                    description: Some(String::from(
                                        package["description"].as_str().unwrap(),
                                    )),
                                    title: Some(String::from(package["title"].as_str().unwrap())),
                                    url: String::from(url_name),
                                    open_url: Some(String::from(url_name)),
                                    tags,
                                };
                                docs.push(doc);
                            }
                            log(package["id"].as_str().unwrap());
                        }

                        // adds the set of documents to the index. If the url already exists the document
                        // will be updated instead of created. The tags specified in the add call will
                        // apply to all documents
                        let _ =
                            add_document(docs, vec![(String::from("lens"), String::from("nuget"))]);
                    }
                }
            }
            PluginEvent::DocumentResponse {
                request_id: _,
                page_count: _,
                page: _,
                documents,
            } => {
                // Response to a request for documents, there are also methods used to modify the tags on
                // documents without modifying the full document. This can be useful for conditionally adding
                // tags to documents that already exist.
                let urls = documents
                    .iter()
                    .map(|doc| doc.url.clone())
                    .collect::<Vec<String>>();
                log(format!("Saved documents {:?}", urls).as_str());
            }
        }
    }
}

fn build_tags(
    tags: Option<&Vec<Value>>,
    authors: Option<&Vec<Value>>,
    owners: Option<&Vec<Value>>,
    version: Option<&str>,
) -> Vec<(String, String)> {
    let mut doc_tags = Vec::new();
    if let Some(tag_list) = tags {
        for tag in tag_list {
            if let Some(tag_val) = tag.as_str() {
                doc_tags.push((String::from("tag"), String::from(tag_val)));
            }
        }
    }

    if let Some(authors) = authors {
        for author in authors {
            if let Some(author_val) = author.as_str() {
                doc_tags.push((String::from("author"), String::from(author_val)));
            }
        }
    }

    if let Some(owners) = owners {
        for owner in owners {
            if let Some(owner_val) = owner.as_str() {
                doc_tags.push((String::from("owner"), String::from(owner_val)));
            }
        }
    }

    if let Some(version) = version {
        doc_tags.push((String::from("version"), String::from(version)));
    }

    doc_tags
}
