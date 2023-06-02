use yew::prelude::*;
use yew::virtual_dom::VNode;

#[derive(Properties, PartialEq)]
pub struct ChatBubbleProps {
    pub text: String,
    pub background: Option<String>,
    pub icon: Option<VNode>,
    #[prop_or_default]
    pub classes: Classes,
    pub align: ChatAlign,
}

#[derive(PartialEq)]
pub enum ChatAlign {
    Right,
    Left,
}

#[function_component(ChatBubble)]
pub fn nav_bar_component(props: &ChatBubbleProps) -> Html {
    let bg_classes = match &props.background {
        Some(color) => classes!(color, "text-gray-900"),
        None => match &props.align {
            ChatAlign::Left => classes!("bg-blue-100", "text-blue-900"),
            ChatAlign::Right => classes!("bg-gray-200", "text-gray-900"),
        },
    };

    let style_classes = classes!(bg_classes, "ml-2", "rounded-lg", "p-2");
    match &props.align {
        ChatAlign::Left => {
            html! {
                <div class="flex items-start">
                  <div class="flex-shrink-0">
                    <img src="user-avatar.jpg" alt="User Avatar" class="h-6 w-6 rounded-full"/>
                  </div>
                  <div class={style_classes}>
                    <p>{props.text.clone()}</p>
                  </div>
                </div>
            }
        }
        ChatAlign::Right => {
            html! {
                <div class="flex items-end justify-end">
                  <div class={style_classes}>
                    <p>{props.text.clone()}</p>
                  </div>
                  <div class="flex-shrink-0">
                    <img src="product-image.jpg" alt="Product Image" class="h-6 w-6 rounded-full"/>
                  </div>
                </div>
            }
        }
    }
}
