use ui_components::btn::Btn;
use yew::prelude::*;

pub struct AddSourceComponent {
    error_msg: Option<String>
}

pub enum Msg {
    AddUrl { include_all: bool },
    OpenCloudFilePicker,
}

impl Component for AddSourceComponent {
    type Message = Msg;
    type Properties = ();

    fn create(_ctx: &Context<Self>) -> Self {
        Self { error_msg: None }
    }

    fn update(&mut self, _ctx: &Context<Self>, _msg: Self::Message) -> bool {
        true
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        let link = ctx.link();

        html! {
            <div class="flex flex-col gap-4">
                <div class="flex flex-row gap-4 items-center">
                    <input
                        type="text"
                        class="rounded p-2 text-sm text-neutral-800"
                        placeholder="https://example.com"
                    />
                    <Btn onclick={link.callback(|_| Msg::AddUrl {include_all: false})}>{"Add data from URL"}</Btn>
                    <Btn onclick={link.callback(|_| Msg::AddUrl {include_all: true} )}>{"Add all URLs from Site"}</Btn>
                    <div class="text-sm text-red-700">{self.error_msg.clone()}</div>
                </div>
                <div><Btn onclick={link.callback(|_| Msg::OpenCloudFilePicker)}>{"Add data from Google Drive"}</Btn></div>
            </div>
        }
    }
}