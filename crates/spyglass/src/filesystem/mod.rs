use dashmap::DashMap;
use entities::models::crawl_queue::{self, CrawlType, EnqueueSettings};
use entities::models::tag::{TagPair, TagType};
use entities::models::{lens, processed_files};
use entities::sea_orm::entity::prelude::*;
use entities::sea_orm::DatabaseConnection;
use ignore::gitignore::Gitignore;
use ignore::WalkBuilder;

use sha2::{Digest, Sha256};
use url::Url;

use crate::crawler::CrawlResult;
use crate::state::AppState;
use entities::sea_orm::Set;
use entities::sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
use migration::OnConflict;

use std::sync::Arc;
use std::time::Duration;
use std::{
    collections::HashSet,
    path::{Path, PathBuf},
};

use notify::RecommendedWatcher;
use tokio::sync::mpsc::{Receiver, Sender};
use tokio::sync::Mutex;
use uuid::Uuid;

use notify_debouncer_mini::{DebouncedEvent, DebouncedEventKind, Debouncer};

use crate::documents;

pub mod utils;

/// Watcher responsible for processing paths on the file system.
/// All filesystem updates will be run through the debouncer to
/// batch updates then processed through any found git ignore files.
/// Any updates that make it through will be passed to listeners
pub struct SpyglassFileWatcher {
    // The director watcher services
    watcher: Arc<Mutex<Debouncer<RecommendedWatcher>>>,
    // The map of path being watched to the list of watchers
    path_map: DashMap<PathBuf, Vec<WatchPath>>,
    // Map of .gitignore file path to the ignore file processor
    ignore_files: DashMap<PathBuf, Gitignore>,
    // The database connection used to update the database with
    // the state of file processing
    db: DatabaseConnection,
}

/// The watch path represents a watcher of a path. The watcher will
/// be notified of system changes via the send and receiver
#[derive(Debug)]
pub struct WatchPath {
    path: PathBuf,
    _uuid: String,
    extensions: Option<HashSet<String>>,
    tx_channel: Option<Sender<Vec<DebouncedEvent>>>,
}

impl WatchPath {
    /// Constructs a new watch path with the path that is being watched
    /// and the set of extensions to notify the listener with
    pub fn new(path: &Path, extensions: Option<HashSet<String>>) -> Self {
        let uuid = Uuid::new_v4().as_hyphenated().to_string();

        WatchPath {
            path: path.to_path_buf(),
            _uuid: uuid,
            extensions,
            tx_channel: None::<Sender<Vec<DebouncedEvent>>>,
        }
    }

    /// Builds the receiver used to receive file update messages
    pub fn build_rx(&mut self) -> Receiver<Vec<DebouncedEvent>> {
        let (tx, rx) = tokio::sync::mpsc::channel(1);
        self.tx_channel = Some(tx);
        rx
    }

    /// Sends a change notification ot the receiver
    pub async fn send_notify(&self, events: Vec<DebouncedEvent>) -> anyhow::Result<()> {
        if let Some(tx) = &self.tx_channel {
            match &self.extensions {
                Some(ext_list) => {
                    // if there are extension filters only grab files that match
                    // the extension
                    let valid_events = events
                        .iter()
                        .filter_map(|evt| {
                            if let Some(ext) = evt.path.extension() {
                                if let Ok(ext_string) = ext.to_owned().into_string() {
                                    if ext_list.contains(&ext_string) {
                                        return Some(evt.clone());
                                    }
                                }
                            }
                            None
                        })
                        .collect::<Vec<DebouncedEvent>>();

                    // Send all valid updates to the listener
                    if !valid_events.is_empty() {
                        if let Err(error) = tx.send(valid_events).await {
                            log::error!("Error sending event {:?}", error);
                            return Err(anyhow::Error::from(error));
                        }
                    }
                }
                None => {
                    // With no extension filter send all updates to the
                    // listener
                    if let Err(error) = tx.send(events).await {
                        log::error!("Error sending event {:?}", error);
                        return Err(anyhow::Error::from(error));
                    }
                }
            }
        }
        Ok(())
    }
}

/// General helper method used to watch for file change events and shutdown events.
/// This is the top most level watcher that receives all file update events and
/// then send them for appropriate processing
async fn watch_events(
    state: AppState,
    mut file_events: Receiver<Result<Vec<DebouncedEvent>, Vec<notify::Error>>>,
) {
    let mut shutdown_rx = state.shutdown_cmd_tx.lock().await.subscribe();
    loop {
        // Wait for next command / handle shutdown responses
        let next_cmd = tokio::select! {
            // Listen for file change notifications
            file_event = file_events.recv() => {
                if let Some(Ok(file_event)) = file_event {
                    Some(file_event)
                } else {
                    None
                }
            },
            _ = shutdown_rx.recv() => {
                log::info!("🛑 Shutting down file watch loop");

                file_events.close();
                let mut watcher = state.file_watcher.lock().await;
                if let Some(watcher) = watcher.as_mut() {
                    watcher.close().await;
                }
                return;
            }
        };

        let watcher = state.file_watcher.lock().await;
        if let Some(events) = next_cmd {
            // Received some events now process it through the watcher
            if let Some(watcher) = watcher.as_ref() {
                // reduce the events to only ones that should be processed
                // by the system
                let filtered_eventlist = watcher.filter_events(&events);

                // if we found any new .gitignore files add them for
                // further processing. This is normal in the case
                // you git clone in a watched directory, a later
                // build step would require use to filter out ignored
                // target folders
                let ignore_files = &filtered_eventlist
                    .iter()
                    .filter_map(|evt| {
                        if utils::is_ignore_file(evt.path.as_path()) {
                            Some(evt.path.clone())
                        } else {
                            None
                        }
                    })
                    .collect::<Vec<PathBuf>>();
                watcher.add_ignore_files(ignore_files);
                watcher.process_changes(&filtered_eventlist).await;

                // Send chuncks of events to only watchers who care
                for path_ref in &watcher.path_map {
                    let filtered_events = filtered_eventlist
                        .iter()
                        .filter_map(|evt| {
                            if evt.path.starts_with(path_ref.key()) {
                                Some(evt.clone())
                            } else {
                                None
                            }
                        })
                        .collect::<Vec<DebouncedEvent>>();

                    if !filtered_events.is_empty() {
                        let val = path_ref.value();
                        notify_watchers(filtered_events, val).await;
                    }
                }
            }
        }

        // Sleep a little at the end of each cmd
        tokio::time::sleep(tokio::time::Duration::from_millis(300)).await;
    }
}

/// General notification helper used to loop through the watchers and send the
/// events
async fn notify_watchers(events: Vec<DebouncedEvent>, watch_list: &Vec<WatchPath>) {
    log::debug!("Sending {:?} updates to file watchers", events.len());
    for watcher in watch_list {
        let rslt = watcher.send_notify(events.clone()).await;
        if let Err(error) = rslt {
            log::error!("Error sending notify {:?}", error);
        }
    }
}

impl SpyglassFileWatcher {
    /// Creates a new filewatcher that will watch for file changes and send updates
    /// to listeners
    pub fn new(state: &AppState) -> Self {
        let (tx, file_events) = tokio::sync::mpsc::channel(1);

        let watcher =
            notify_debouncer_mini::new_debouncer(Duration::from_secs(5), None, move |res| {
                futures::executor::block_on(async {
                    if !tx.is_closed() {
                        if let Err(err) = tx.send(res).await {
                            log::error!("fseventwatcher error: {}", err.to_string());
                        }
                    }
                })
            })
            .expect("Unable to watch lens directory");

        let spy_watcher = SpyglassFileWatcher {
            watcher: Arc::new(Mutex::new(watcher)),
            path_map: DashMap::new(),
            ignore_files: DashMap::new(),
            db: state.db.clone(),
        };

        tokio::spawn(watch_events(state.clone(), file_events));

        spy_watcher
    }

    /// Helper method used to update the database with newly arrived changes
    async fn process_changes(&self, events: &Vec<DebouncedEvent>) {
        let mut inserts = Vec::new();
        let mut removals = Vec::new();

        for event in events {
            if event.path.exists() {
                let mut model = processed_files::ActiveModel::new();
                model.file_path = Set(utils::path_to_uri(&event.path));
                model.last_modified = Set(utils::last_modified_time(&event.path));
                inserts.push(model);
            } else {
                removals.push(utils::path_to_uri(&event.path));
            }
        }

        if !inserts.is_empty() {
            if let Err(error) = processed_files::Entity::insert_many(inserts)
                .on_conflict(
                    OnConflict::column(processed_files::Column::FilePath)
                        .update_column(processed_files::Column::LastModified)
                        .to_owned(),
                )
                .exec(&self.db)
                .await
            {
                log::error!("Error inserting updates {:?}", error);
            }
        }

        if !removals.is_empty() {
            if let Err(error) = processed_files::Entity::delete_many()
                .filter(processed_files::Column::Id.is_in(removals))
                .exec(&self.db)
                .await
            {
                log::error!("Error processing deletes {:?}", error);
            }
        }
    }

    async fn _remove_path(&mut self, path: &Path) {
        if let Some((_key, _watchers)) = self.path_map.remove(&path.to_path_buf()) {
            let _ = self.watcher.lock().await.watcher().unwatch(path);
        }
    }

    /// Closes the watcher and associated resources
    async fn close(&mut self) {
        self.ignore_files.clear();

        for path_ref in self.path_map.iter() {
            for path in path_ref.value() {
                let _ = self
                    .watcher
                    .lock()
                    .await
                    .watcher()
                    .unwatch(path.path.as_path());
            }
        }
        self.path_map.clear();
    }

    /// Adds .gitignore files
    fn add_ignore_files(&self, files: &Vec<PathBuf>) {
        for path in files {
            if let Ok(patterns) = utils::patterns_from_file(path.as_path()) {
                self.ignore_files.insert(path.to_owned(), patterns);
            }
        }
    }

    /// Adds a single .gitignore file
    fn add_ignore_file(&self, file: &Path) {
        if let Ok(patterns) = utils::patterns_from_file(file) {
            self.ignore_files.insert(file.to_path_buf(), patterns);
        }
    }

    fn is_path_initialized(&self, file: &Path) -> bool {
        self.path_map.contains_key(&file.to_path_buf())
    }

    /// filters the provided events and returns the list of events that should not
    /// be ignored
    fn filter_events(&self, events: &[DebouncedEvent]) -> Vec<DebouncedEvent> {
        events
            .iter()
            .filter_map(|evt| {
                if evt.kind != DebouncedEventKind::AnyContinuous
                    && !self.is_ignored(evt.path.as_path())
                {
                    Some(evt.clone())
                } else {
                    None
                }
            })
            .collect::<Vec<DebouncedEvent>>()
    }

    /// Checks if the path represents a hidden directory or
    /// or file ignored by a .gitignore file
    fn is_ignored(&self, path: &Path) -> bool {
        if utils::is_in_hidden_dir(path) {
            return true;
        }

        // well does this work
        for map_ref in &self.ignore_files {
            let root = map_ref.key();
            let patterns = map_ref.value();
            if let Some(parent) = root.parent() {
                if path.starts_with(parent) {
                    return patterns
                        .matched_path_or_any_parents(path, path.is_dir())
                        .is_ignore();
                }
            }
        }

        false
    }

    /// Sets up a watcher for the specified path. If two watchers are registered
    /// for the same path only one file system watcher is registered and both
    /// listeners are notified
    pub async fn watch_path(
        &mut self,
        path: &Path,
        extensions: Option<HashSet<String>>,
        recursive: bool,
    ) -> Receiver<Vec<DebouncedEvent>> {
        let mut watch_path = WatchPath::new(path, extensions);
        let rx = watch_path.build_rx();

        let path_buf = path.to_path_buf();
        let new_path = !self.path_map.contains_key(&path_buf);
        self.path_map.entry(path_buf).or_default().push(watch_path);

        let mode = if recursive {
            notify::RecursiveMode::Recursive
        } else {
            notify::RecursiveMode::NonRecursive
        };

        if new_path {
            let watch_rslt = self.watcher.lock().await.watcher().watch(path, mode);
            if let Err(error) = watch_rslt {
                log::error!(
                    "Error attempting to watch path: {:?}, Error: {:?}",
                    path,
                    error
                );
            }
        }
        rx
    }

    /// Initializes the path by walking the entire tree. All changed, removed and new files
    /// are returned as debounced events
    pub async fn initialize_path(&mut self, path: &Path) -> Vec<DebouncedEvent> {
        let mut debounced_events = Vec::new();
        let root_uri = utils::path_to_uri(path);
        let files = DashMap::new();

        // will not ignore hidden since we need to include .git files
        let walker = WalkBuilder::new(path).hidden(false).build();
        for entry in walker.flatten() {
            if !utils::is_in_hidden_dir(entry.path()) {
                if utils::is_ignore_file(entry.path()) {
                    self.add_ignore_file(entry.path());
                }

                let uri = utils::path_to_uri(entry.path());
                let time = utils::last_modified_time_for_path(entry.path());
                files.insert(uri, time);
            }
        }

        let processed_files = processed_files::Entity::find()
            .filter(processed_files::Column::FilePath.starts_with(root_uri.as_str()))
            .all(&self.db)
            .await;
        let mut to_delete = Vec::new();
        let mut to_recrawl = Vec::new();

        // Check all items already in the database if it is still in the file system
        // then see if it has updated, if it is not then it has been deleted so
        // add it to the deleted items. All remaining files in the map are new
        if let Ok(processed) = processed_files {
            for item in processed {
                match files.remove(&item.file_path) {
                    Some((file_path, file_last_mod)) => {
                        if file_last_mod > item.last_modified {
                            debounced_events.push(DebouncedEvent {
                                path: utils::uri_to_path(&file_path).unwrap(),
                                kind: DebouncedEventKind::Any,
                            });
                            to_recrawl.push((item.file_path, file_last_mod));
                        }
                    }
                    None => {
                        debounced_events.push(DebouncedEvent {
                            path: utils::uri_to_path(&item.file_path).unwrap(),
                            kind: DebouncedEventKind::Any,
                        });
                        to_delete.push(item.id)
                    }
                }
            }
        }

        log::debug!(
            "Added: {:?} Deleted: {:?} Updated: {:?}",
            files.len(),
            to_delete.len(),
            to_recrawl.len()
        );

        if !to_delete.is_empty() {
            if let Err(error) = processed_files::Entity::delete_many()
                .filter(processed_files::Column::Id.is_in(to_delete))
                .exec(&self.db)
                .await
            {
                log::error!("Error deleting processed files {:?}", error);
            }
        }

        if !files.is_empty() {
            let models = files
                .iter()
                .map(|path_ref| {
                    debounced_events.push(DebouncedEvent {
                        path: utils::uri_to_path(path_ref.key()).unwrap(),
                        kind: DebouncedEventKind::Any,
                    });

                    let mut active_model = processed_files::ActiveModel::new();
                    active_model.file_path = Set(path_ref.key().clone());
                    active_model.last_modified = Set(*path_ref.value());

                    active_model
                })
                .collect::<Vec<processed_files::ActiveModel>>();

            if let Err(error) = processed_files::Entity::insert_many(models)
                .exec(&self.db)
                .await
            {
                log::error!("Error inserting additions {:?}", error);
            }
        }

        if !to_recrawl.is_empty() {
            let updates = to_recrawl
                .iter()
                .map(|(uri, last_modified)| {
                    let mut active_model = processed_files::ActiveModel::new();
                    active_model.file_path = Set(uri.clone());
                    active_model.last_modified = Set(*last_modified);

                    active_model
                })
                .collect::<Vec<processed_files::ActiveModel>>();

            if let Err(error) = processed_files::Entity::insert_many(updates)
                .on_conflict(
                    OnConflict::column(processed_files::Column::FilePath)
                        .update_column(processed_files::Column::LastModified)
                        .to_owned(),
                )
                .exec(&self.db)
                .await
            {
                log::error!("Error updated recrawls {:?}", error);
            }
        }
        log::debug!("Returning {:?} updates", files.len());

        debounced_events
    }
}

/// Configures the file watcher with the user set directories
pub async fn configure_watcher(state: AppState) {
    // temp use plugin configuration
    if let Ok(Some(lens)) = lens::find_by_name("local-file-importer", &state.db).await {
        if lens.is_enabled {
            log::info!("📂 Loading local file watcher");

            let extension = utils::get_supported_file_extensions(&state);
            let paths = utils::get_search_directories(&state);
            let path_names = paths
                .iter()
                .map(|path| utils::path_to_uri(path))
                .collect::<Vec<String>>();

            let mut watcher = state.file_watcher.lock().await;
            if let Some(watcher) = watcher.as_mut() {
                for path in paths {
                    if !watcher.is_path_initialized(path.as_path()) {
                        log::debug!("Adding {:?} to watch list", path);
                        let updates = watcher.initialize_path(path.as_path()).await;
                        let rx1 = watcher.watch_path(path.as_path(), None, true).await;

                        tokio::spawn(_process_messages(
                            state.clone(),
                            rx1,
                            updates,
                            extension.clone(),
                        ));
                    }
                }
            } else {
                log::error!("Watcher is missing");
            }

            match processed_files::remove_unmatched_paths(&state.db, path_names).await {
                Ok(removed) => {
                    let uri_list = removed
                        .iter()
                        .map(|model| model.file_path.clone())
                        .collect::<Vec<String>>();
                    documents::delete_documents_by_uri(&state, uri_list).await;
                }
                Err(error) => log::error!(
                    "Error removing paths that are no longer being watched. {:?}",
                    error
                ),
            }

        // TODO remove the content from extensions that are no longer being processed, this should be the
        // purview of the document handling and not the file handling since we cannot make the assumption
        // here of what happens to files that do not meet the expected extension.

        // At the moment triggering a recrawl will work the best
        } else {
            log::info!("❌ Local file watcher is disabled");

            let mut watcher = state.file_watcher.lock().await;
            if let Some(watcher) = watcher.as_mut() {
                watcher.close().await;
            }

            match processed_files::remove_unmatched_paths(&state.db, Vec::new()).await {
                Ok(removed) => {
                    let uri_list = removed
                        .iter()
                        .map(|model| model.file_path.clone())
                        .collect::<Vec<String>>();
                    documents::delete_documents_by_uri(&state, uri_list).await;
                }
                Err(error) => log::error!(
                    "Error removing paths that are no longer being watched. {:?}",
                    error
                ),
            }
        }
    } else {
        log::info!("❌ Local file watcher not installed");
    }
}

/// Helper method use to process updates from a watched path
async fn _process_messages(
    state: AppState,
    mut rx: Receiver<Vec<DebouncedEvent>>,
    initial: Vec<DebouncedEvent>,
    extensions: HashSet<String>,
) {
    log::debug!("Processing {:?} initial updates.", initial.len());
    if let Err(error) = _process_file_and_dir(&state, initial, &extensions).await {
        log::error!("Error processing initial files {:?}", error);
    }

    loop {
        let msg = rx.recv().await;
        match msg {
            Some(event) => {
                if let Err(error) = _process_file_and_dir(&state, event, &extensions).await {
                    log::error!("Error processing updates {:?}", error);
                }
            }
            None => {
                log::info!("Message queue has closed. Stopping processing");
                break;
            }
        }
    }
}

/// Helper method used process all updated files and directories
async fn _process_file_and_dir(
    state: &AppState,
    events: Vec<DebouncedEvent>,
    extensions: &HashSet<String>,
) -> anyhow::Result<()> {
    let mut enqueue_list = Vec::new();
    let mut general_processing = Vec::new();
    let mut delete_list = Vec::new();
    for event in events {
        let path = event.path;
        let uri = utils::path_to_uri(&path);

        if path.exists() {
            if utils::is_windows_shortcut(path.as_path()) {
                let location = utils::get_shortcut_destination(path.as_path());

                if let Some(location) = location {
                    let ext = &location
                        .extension()
                        .and_then(|x| x.to_str())
                        .map(|x| x.to_string())
                        .unwrap_or_default();

                    // If the shortcut points to a file we can process then
                    // process the file instead of the shortcut
                    if extensions.contains(ext) {
                        let file_uri = utils::path_to_uri(&location);
                        enqueue_list.push(file_uri);
                    }
                }
            }

            let ext = &path
                .extension()
                .and_then(|x| x.to_str())
                .map(|x| x.to_string())
                .unwrap_or_default();
            if extensions.contains(ext) {
                enqueue_list.push(uri);
            } else {
                general_processing.push(uri);
            }
        } else {
            delete_list.push(uri);
        }
    }

    if !enqueue_list.is_empty() {
        let tags = vec![(TagType::Lens, String::from("files"))];
        let enqueue_settings = EnqueueSettings {
            crawl_type: CrawlType::Normal,
            is_recrawl: true,
            tags,
            force_allow: true,
        };
        if let Err(error) =
            crawl_queue::enqueue_local_files(&state.db, &enqueue_list, &enqueue_settings, None)
                .await
        {
            log::error!("Error adding to crawl queue {:?}", error);
        }
    }

    if !general_processing.is_empty() {
        log::debug!("Adding {} general documents", general_processing.len());
        for general_chunk in general_processing.chunks(500) {
            _process_general_file(state, general_chunk).await;
        }
    }

    if !delete_list.is_empty() {
        documents::delete_documents_by_uri(state, delete_list).await;
    }

    Ok(())
}

/// Generates the tags for a file
pub fn build_file_tags(path: &Path) -> Vec<TagPair> {
    let mut tags = Vec::new();
    tags.push((TagType::Lens, String::from("files")));
    if path.is_dir() {
        tags.push((TagType::Type, String::from("directory")));
    } else if path.is_file() {
        tags.push((TagType::Type, String::from("file")));
        let ext = path
            .extension()
            .and_then(|x| x.to_str())
            .map(|x| x.to_string());
        if let Some(ext) = ext {
            tags.push((TagType::FileExt, ext));
        }
    }

    if path.is_symlink() {
        tags.push((TagType::Type, String::from("symlink")))
    }

    let guess = new_mime_guess::from_path(path);
    for mime_guess in guess.iter() {
        tags.push((TagType::MimeType, mime_guess.to_string()));
    }

    tags
}

// Helper method used process files
async fn _process_general_file(state: &AppState, file_uri: &[String]) {
    let mut crawl_results = file_uri
        .iter()
        .filter_map(|uri| match Url::parse(uri) {
            Ok(url) => match url.to_file_path() {
                Ok(path) => _path_to_result(&url, &path),
                Err(_) => None,
            },
            Err(_) => None,
        })
        .collect::<Vec<CrawlResult>>();

    documents::process_crawl_results(state, "files", &mut crawl_results).await;
}

// Process a path to parse result
fn _path_to_result(url: &Url, path: &Path) -> Option<CrawlResult> {
    let file_name = path
        .file_name()
        .and_then(|x| x.to_str())
        .map(|x| x.to_string())
        .expect("Unable to convert path file name to string");
    let mut hasher = Sha256::new();
    hasher.update(file_name.as_bytes());
    let content_hash = hex::encode(&hasher.finalize()[..]);
    let tags = build_file_tags(path);
    if path.is_file() || path.is_dir() {
        Some(CrawlResult {
            content_hash: Some(content_hash),
            content: Some(file_name.clone()),
            // Does a file have a description? Pull the first part of the file
            description: Some(file_name.clone()),
            title: Some(url.to_string()),
            url: url.to_string(),
            open_url: Some(url.to_string()),
            links: Default::default(),
            tags,
        })
    } else {
        None
    }
}
