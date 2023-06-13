use std::collections::HashMap;

use gloo::file::callbacks::FileReader;
use gloo::file::File;
use ui_components::btn::{Btn, BtnSize, BtnType};
use ui_components::icons::{
    BadgeCheckIcon, DocumentPlus, FileExtIcon, RefreshIcon, TrashIcon, XCircle,
};
use web_sys::{DragEvent, Event, FileList, HtmlInputElement};
use yew::html::TargetCast;
use yew::platform::spawn_local;
use yew::{classes, html, Callback, Classes, Component, Context, ContextHandle, Html, Properties};

use crate::AuthStatus;

// Maximum number of bytes that can be uploaded 10 MB
const UPLOAD_SIZE_LIMIT: usize = 10_000_000;

/// Details about the file that is being uploaded, including the content
#[derive(Clone)]
pub struct FileDetails {
    pub name: String,
    pub file_type: String,
    pub data: Vec<u8>,
    // Indication if the file has been uploaded
    pub uploaded: bool,
    // Error with the file that will prevent it from being uploaded
    pub error: Option<String>,
}

pub enum Msg {
    // When the file is loaded from disk to memory
    Loaded(String, String, Vec<u8>),
    // The files provided by the user
    Files(Vec<File>),
    // The start of a drag into the drop region
    DragStart,
    // The end of a drag into the region
    DragEnd,
    // Deletes a file at the specified index and indicates if it is
    // due to successful upload or not
    DeleteFile(usize, bool),
    // Sends the upload request to the server
    UploadFiles,
    // Updates the auth status context
    UpdateContext(AuthStatus),
}

pub struct FileUpload {
    processing: bool,
    readers: HashMap<String, FileReader>,
    files: Vec<FileDetails>,
    drag_started: bool,
    auth_status: AuthStatus,
    _context_listener: ContextHandle<AuthStatus>,
}

#[derive(Properties, PartialEq)]
pub struct FileUploadProps {
    // Lens we are uploading the data for
    pub lens_identifier: String,
    // Text to show in the upload drop box
    #[prop_or(Some(String::from("Drop your files here or click to select")))]
    pub upload_text: Option<String>,
    // Height of the upload drop box
    #[prop_or("h-64".into())]
    pub height: String,
    // Width of the upload drop box
    #[prop_or("w-full".into())]
    pub width: String,
    // Classes for the main upload drop area
    #[prop_or_default]
    pub classes: Classes,
    // Callback when a file is uploaded. If multiple files are uploaded
    // this callback will emit a value for each file that is uploaded
    // when the upload is finished
    #[prop_or_default]
    pub on_upload: Callback<Box<FileDetails>>,
}

impl Component for FileUpload {
    type Message = Msg;
    type Properties = FileUploadProps;

    fn create(ctx: &Context<Self>) -> Self {
        // Connect to context for auth details
        let (auth_status, context_listener) = ctx
            .link()
            .context(ctx.link().callback(Msg::UpdateContext))
            .expect("No Message Context Provided");

        Self {
            readers: HashMap::default(),
            files: Vec::default(),
            auth_status,
            processing: false,
            drag_started: false,
            _context_listener: context_listener,
        }
    }

    fn update(&mut self, ctx: &Context<Self>, msg: Self::Message) -> bool {
        let props = ctx.props();

        match msg {
            Msg::Loaded(file_name, file_type, data) => {
                let error = if data.len() > UPLOAD_SIZE_LIMIT {
                    Some("File to large, maximum size 10 MB".to_string())
                } else {
                    None
                };
                self.files.push(FileDetails {
                    data,
                    file_type,
                    name: file_name.clone(),
                    uploaded: false,
                    error,
                });

                self.readers.remove(&file_name);
                if self.readers.is_empty() {
                    self.processing = false;
                }
                self.drag_started = false;

                true
            }
            Msg::Files(files) => {
                self.processing = true;
                for file in files.into_iter() {
                    let file_name = file.name();
                    let file_type = file.raw_mime_type();

                    let task = {
                        let link = ctx.link().clone();
                        let file_name = file_name.clone();

                        gloo::file::callbacks::read_as_bytes(&file, move |res| {
                            link.send_message(Msg::Loaded(
                                file_name,
                                file_type,
                                res.expect("failed to read file"),
                            ))
                        })
                    };
                    self.readers.insert(file_name, task);
                }
                true
            }
            Msg::DragStart => {
                log::error!("Drag start event");
                self.drag_started = true;
                true
            }
            Msg::DragEnd => {
                log::error!("Drag end event");
                self.drag_started = false;
                true
            }
            Msg::DeleteFile(index, uploaded) => {
                if uploaded {
                    if let Some(file) = self.files.get_mut(index) {
                        file.uploaded = true;
                    }

                    if !self.files.iter().any(|file| !file.uploaded) {
                        self.files.clear();
                        self.processing = false;
                    }
                } else {
                    self.files.remove(index);
                }

                true
            }
            Msg::UploadFiles => {
                self.processing = true;

                for (index, file) in self.files.iter().enumerate() {
                    let client = self.auth_status.get_client();
                    let file = Box::new(file.clone());
                    let lens = props.lens_identifier.clone();
                    let upload_callback = props.on_upload.clone();
                    let link = ctx.link().clone();
                    spawn_local(async move {
                        if let Err(error) = client.upload_source_document(&lens, file.clone()).await
                        {
                            log::error!("Got error uploading document {:?}", error);
                        }
                        upload_callback.emit(file);
                        link.send_message(Msg::DeleteFile(index, true))
                    });
                }

                true
            }
            Msg::UpdateContext(auth_status) => {
                self.auth_status = auth_status;
                true
            }
        }
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        let props = ctx.props();
        let classes = classes!(
            "flex",
            "justify-center",
            "items-center",
            "flex-col",
            "flex-grow",
            "relative",
            "hover:cursor-pointer",
            "hover:border-slate-400",
            "hover:stroke-slate-400",
            "hover:text-slate-400",
            "border-dashed",
            "border-2",
            "m-4",
            props.classes.clone(),
            &props.width,
            &props.height,
        );

        let classes = if self.drag_started {
            classes!(
                classes,
                "cursor-pointer",
                "border-slate-400",
                "stroke-slate-400",
                "text-slate-400"
            )
        } else {
            classes!(classes, "border-white/30", "text-white/30",)
        };

        let upload_text = props.upload_text.clone();
        let has_error = self.files.iter().any(|file| file.error.is_some());
        let is_processing = self.processing;
        let upload_documents = ctx.link().callback(move |_| Msg::UploadFiles);
        html! {
            <div class="flex flex-col">
                <p class="m-auto">{"Upload your files"}</p>
                <label for="file-upload">
                    <div
                        class={classes}
                        ondrop={ctx.link().callback(move |event: DragEvent| {
                            event.prevent_default();
                            if !is_processing {
                                let files = event.data_transfer().unwrap().files();
                                Self::upload_files(files)
                            } else {
                                Msg::DragEnd
                            }
                        })}
                        ondragover={Callback::from(|event: DragEvent| {
                            event.prevent_default();
                        })}
                        ondragenter={ctx.link().callback(|event: DragEvent| {
                            event.prevent_default();
                            Msg::DragStart
                        })}
                        ondragleave={ctx.link().callback(|event: DragEvent| {
                            event.prevent_default();
                            Msg::DragEnd
                        })}
                    >
                        {if self.processing {
                            html! {
                                <RefreshIcon
                                    classes={"ml-2 text-cyan-500"}
                                    width="w-16"
                                    height="h-16"
                                    animate_spin={true} />
                            }
                        } else {
                            html! {
                                <DocumentPlus
                                    width="w-16"
                                    height="h-16" />
                            }
                        }}

                        {if upload_text.is_some() {
                            html! {<p>{upload_text.unwrap()}</p>}
                        } else {
                            html! {}
                        }}

                    </div>
                </label>
                <input
                    id="file-upload"
                    class="h-0 w-0 opacity-0"
                    type="file"
                    accept="*"
                    multiple={true}
                    onchange={ctx.link().callback(move |e: Event| {
                        let input: HtmlInputElement = e.target_unchecked_into();
                        Self::upload_files(input.files())
                    })}
                />
                <div class="justify-start mt-4">
                    { for self.files.iter().enumerate().map(|(i, file)| Self::view_file(file, i, ctx)) }
                </div>
                <div class="m-auto">
                    {if !self.files.is_empty() && has_error {
                        html! {
                            <Btn disabled={has_error} size={BtnSize::Lg} _type={BtnType::Primary} >
                                {"Upload"}
                            </Btn>
                        }
                    } else if !self.files.is_empty() {
                        html! {
                            <Btn onclick={upload_documents} disabled={has_error} size={BtnSize::Lg} _type={BtnType::Primary} >
                                {"Upload"}
                            </Btn>
                        }
                    } else {
                        html! {}
                    }}
                </div>
            </div>
        }
    }
}

impl FileUpload {
    fn view_file(file: &FileDetails, index: usize, ctx: &Context<FileUpload>) -> Html {
        let link = ctx.link().clone();
        let on_delete = Callback::from(move |_| {
            link.send_message(Msg::DeleteFile(index, false));
        });
        let ext = file.name.split('.').last().unwrap_or("").to_owned();
        let classes = classes!("w-6", "h-6", "ml-4");

        let icon = if file.error.is_some() {
            html! { <XCircle classes={classes!("ml-4", "text-red-600")} height="h-6" width="w-6" /> }
        } else if file.uploaded {
            html! { <BadgeCheckIcon classes={classes!("ml-4", "text-green-600")} height="h-6" width="w-6" /> }
        } else {
            html! { <FileExtIcon class={classes} ext={ext} /> }
        };

        html! {
            <div class="flex flex-row w-full items-center">
              {icon}
              <div class="p-4 text-white flex-grow">
                { if let Some(error) = &file.error {
                    html! { <p>{ format!("{} - {}", file.name, error) }</p> }
                } else {
                    html! { <p>{ format!("{}", file.name) }</p> }
                }}
              </div>
              <div class="mr-4">
              <Btn size={BtnSize::Sm} onclick={on_delete} _type={BtnType::Danger}>
                <TrashIcon height="h-6" width="w-6" />
              </Btn>
              </div>
            </div>
        }
    }

    fn upload_files(files: Option<FileList>) -> Msg {
        let mut result = Vec::new();

        if let Some(files) = files {
            let files = js_sys::try_iter(&files)
                .unwrap()
                .unwrap()
                .map(|v| web_sys::File::from(v.unwrap()))
                .map(File::from);
            result.extend(files);
        }
        Msg::Files(result)
    }
}
