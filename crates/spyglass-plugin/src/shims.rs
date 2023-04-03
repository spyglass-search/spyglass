use crate::{
    Authentication, DocumentQuery, DocumentUpdate, HttpMethod, PluginCommandRequest, Tag,
    TagModification,
};
use serde::{de::DeserializeOwned, Serialize};
use std::io;

pub struct Http {}

/// Provides the ability to build http requests
#[derive(Clone)]
pub struct HttpRequestBuilder {
    request: PluginCommandRequest,
}

impl HttpRequestBuilder {
    pub fn get(&self) -> Self {
        self.change_method(HttpMethod::GET)
    }

    pub fn put(&self) -> Self {
        self.change_method(HttpMethod::PUT)
    }

    pub fn post(&self) -> Self {
        self.change_method(HttpMethod::POST)
    }

    pub fn patch(&self) -> Self {
        self.change_method(HttpMethod::PATCH)
    }

    pub fn delete(&self) -> Self {
        self.change_method(HttpMethod::DELETE)
    }

    pub fn body(&self, new_body: String) -> Self {
        let mut builder = self.clone();
        let request = match builder.request {
            PluginCommandRequest::HttpRequest {
                headers,
                method,
                url,
                body: _,
                auth,
            } => PluginCommandRequest::HttpRequest {
                headers,
                method,
                url,
                body: Some(new_body),
                auth,
            },
            _ => builder.request,
        };
        builder.request = request;
        builder
    }

    pub fn headers(&self, new_headers: Vec<(String, String)>) -> Self {
        let mut builder = self.clone();
        let request = match builder.request {
            PluginCommandRequest::HttpRequest {
                headers: _,
                method,
                url,
                body,
                auth,
            } => PluginCommandRequest::HttpRequest {
                headers: new_headers,
                method,
                url,
                body,
                auth,
            },
            _ => builder.request,
        };
        builder.request = request;
        builder
    }

    pub fn basic_auth(&self, key: &str, val: Option<String>) -> Self {
        let mut builder = self.clone();
        let request = match builder.request {
            PluginCommandRequest::HttpRequest {
                headers,
                method,
                url,
                body,
                auth: _,
            } => PluginCommandRequest::HttpRequest {
                headers,
                method,
                url,
                body,
                auth: Some(Authentication::BASIC(String::from(key), val)),
            },
            _ => builder.request,
        };
        builder.request = request;
        builder
    }

    pub fn bearer_auth(&self, key: &str) -> Self {
        let mut builder = self.clone();
        let request = match builder.request {
            PluginCommandRequest::HttpRequest {
                headers,
                method,
                url,
                body,
                auth: _,
            } => PluginCommandRequest::HttpRequest {
                headers,
                method,
                url,
                body,
                auth: Some(Authentication::BEARER(String::from(key))),
            },
            _ => builder.request,
        };
        builder.request = request;
        builder
    }

    pub fn run(&self) {
        if object_to_stdout(&self.request).is_ok() {
            unsafe {
                plugin_cmd();
            }
        }
    }

    fn change_method(&self, method: HttpMethod) -> Self {
        let mut builder = self.clone();
        let request = match builder.request {
            PluginCommandRequest::HttpRequest {
                headers,
                method: _,
                url,
                body,
                auth,
            } => PluginCommandRequest::HttpRequest {
                headers,
                method,
                url,
                body,
                auth,
            },
            _ => builder.request,
        };
        builder.request = request;
        builder
    }
}

impl Http {
    pub fn get(url: &str, headers: Vec<(String, String)>) {
        Http::request(url).headers(headers).get().run()
    }

    pub fn request(url: &str) -> HttpRequestBuilder {
        HttpRequestBuilder {
            request: PluginCommandRequest::HttpRequest {
                headers: vec![],
                method: HttpMethod::GET,
                url: String::from(url),
                body: None,
                auth: None,
            },
        }
    }
}

pub fn delete_doc(url: &str) {
    if object_to_stdout(&PluginCommandRequest::DeleteDoc {
        url: url.to_string(),
    })
    .is_ok()
    {
        unsafe {
            plugin_cmd();
        }
    }
}

/// Add an item to the Spyglass crawl queue
pub fn enqueue_all(urls: &[String]) {
    if object_to_stdout(&PluginCommandRequest::Enqueue { urls: urls.into() }).is_ok() {
        unsafe {
            plugin_cmd();
        }
    }
}

/// Utility function to log to spyglass logs
pub fn log(msg: &str) {
    println!("{msg}");
    unsafe {
        plugin_log();
    }
}

#[link(wasm_import_module = "spyglass")]
extern "C" {
    fn plugin_cmd();
    fn plugin_log();
}

#[doc(hidden)]
pub fn object_from_stdin<T: DeserializeOwned>() -> Result<T, ron::error::SpannedError> {
    let mut buf = String::new();
    io::stdin().read_line(&mut buf).unwrap();
    ron::from_str(&buf)
}

#[doc(hidden)]
pub fn object_to_stdout(obj: &impl Serialize) -> Result<(), ron::Error> {
    println!("{}", ron::ser::to_string(obj)?);
    Ok(())
}

pub fn modify_tags(query: DocumentQuery, modification: TagModification) -> Result<(), ron::Error> {
    object_to_stdout(&PluginCommandRequest::ModifyTags {
        documents: query,
        tag_modifications: modification,
    })?;

    unsafe {
        plugin_cmd();
    }
    Ok(())
}

pub fn subscribe_for_documents(query: DocumentQuery) -> Result<(), ron::Error> {
    object_to_stdout(&PluginCommandRequest::QueryDocuments {
        query,
        subscribe: true,
    })?;

    unsafe {
        plugin_cmd();
    }
    Ok(())
}

pub fn add_document(documents: Vec<DocumentUpdate>, tags: Vec<Tag>) -> Result<(), ron::Error> {
    object_to_stdout(&PluginCommandRequest::AddDocuments { documents, tags })?;

    unsafe {
        plugin_cmd();
    }
    Ok(())
}

pub fn subscribe_for_updates() -> Result<(), ron::Error> {
    object_to_stdout(&PluginCommandRequest::SubscribeForUpdates)?;

    unsafe {
        plugin_cmd();
    }
    Ok(())
}

pub fn query_documents(query: DocumentQuery) -> Result<(), ron::Error> {
    object_to_stdout(&PluginCommandRequest::QueryDocuments {
        query,
        subscribe: false,
    })?;

    unsafe {
        plugin_cmd();
    }
    Ok(())
}
