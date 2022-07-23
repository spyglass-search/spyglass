use yew::function_component;
use yew::prelude::*;

#[derive(Properties, PartialEq)]
pub struct IconProps {
    #[prop_or_default]
    pub animate_spin: bool,
    #[prop_or("h-5".into())]
    pub height: String,
    #[prop_or("w-5".into())]
    pub width: String,
}

impl IconProps {
    pub fn class(&self) -> Vec<Option<String>> {
        let animated = if self.animate_spin {
            Some("animate-spin".to_string())
        } else {
            None
        };

        vec![animated, Some(format!("{} {}", self.height, self.width))]
    }
}

#[function_component(BadgeCheckIcon)]
pub fn badge_check_icon(props: &IconProps) -> Html {
    html! {
        <svg xmlns="http://www.w3.org/2000/svg" class={props.class()} viewBox="0 0 20 20" fill="currentColor">
            <path fill-rule="evenodd" d="M6.267 3.455a3.066 3.066 0 001.745-.723 3.066 3.066 0 013.976 0 3.066 3.066 0 001.745.723 3.066 3.066 0 012.812 2.812c.051.643.304 1.254.723 1.745a3.066 3.066 0 010 3.976 3.066 3.066 0 00-.723 1.745 3.066 3.066 0 01-2.812 2.812 3.066 3.066 0 00-1.745.723 3.066 3.066 0 01-3.976 0 3.066 3.066 0 00-1.745-.723 3.066 3.066 0 01-2.812-2.812 3.066 3.066 0 00-.723-1.745 3.066 3.066 0 010-3.976 3.066 3.066 0 00.723-1.745 3.066 3.066 0 012.812-2.812zm7.44 5.252a1 1 0 00-1.414-1.414L9 10.586 7.707 9.293a1 1 0 00-1.414 1.414l2 2a1 1 0 001.414 0l4-4z" clip-rule="evenodd" />
        </svg>
    }
}

#[function_component(DocumentDownloadIcon)]
pub fn document_download_icon(props: &IconProps) -> Html {
    html! {
        <svg xmlns="http://www.w3.org/2000/svg" class={props.class()} viewBox="0 0 20 20" fill="currentColor">
            <path fill-rule="evenodd" d="M6 2a2 2 0 00-2 2v12a2 2 0 002 2h8a2 2 0 002-2V7.414A2 2 0 0015.414 6L12 2.586A2 2 0 0010.586 2H6zm5 6a1 1 0 10-2 0v3.586l-1.293-1.293a1 1 0 10-1.414 1.414l3 3a1 1 0 001.414 0l3-3a1 1 0 00-1.414-1.414L11 11.586V8z" clip-rule="evenodd" />
        </svg>
    }
}

#[function_component(EmojiSadIcon)]
pub fn emoji_sad_icon(props: &IconProps) -> Html {
    html! {
        <svg xmlns="http://www.w3.org/2000/svg" class={props.class()} fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
            <path stroke-linecap="round" stroke-linejoin="round" d="M9.172 16.172a4 4 0 015.656 0M9 10h.01M15 10h.01M21 12a9 9 0 11-18 0 9 9 0 0118 0z" />
        </svg>
    }
}

#[function_component(EyeIcon)]
pub fn eye_icon(props: &IconProps) -> Html {
    html! {
        <svg xmlns="http://www.w3.org/2000/svg" class={props.class()} viewBox="0 0 20 20" fill="currentColor">
            <path d="M10 12a2 2 0 100-4 2 2 0 000 4z" />
            <path fill-rule="evenodd" d="M.458 10C1.732 5.943 5.522 3 10 3s8.268 2.943 9.542 7c-1.274 4.057-5.064 7-9.542 7S1.732 14.057.458 10zM14 10a4 4 0 11-8 0 4 4 0 018 0z" clip-rule="evenodd" />
        </svg>
    }
}

#[function_component(FolderOpenIcon)]
pub fn folder_open_icon(props: &IconProps) -> Html {
    html! {
        <svg xmlns="http://www.w3.org/2000/svg" class={props.class()} viewBox="0 0 20 20" fill="currentColor">
            <path fill-rule="evenodd" d="M2 6a2 2 0 012-2h4l2 2h4a2 2 0 012 2v1H8a3 3 0 00-3 3v1.5a1.5 1.5 0 01-3 0V6z" clip-rule="evenodd" />
            <path d="M6 12a2 2 0 012-2h8a2 2 0 012 2v2a2 2 0 01-2 2H2h2a2 2 0 002-2v-2z" />
        </svg>
    }
}

#[function_component(LightningBoltIcon)]
pub fn lightning_bolt_icon(props: &IconProps) -> Html {
    html! {
        <svg xmlns="http://www.w3.org/2000/svg" class={props.class()} viewBox="0 0 20 20" fill="currentColor">
            <path fill-rule="evenodd" d="M11.3 1.046A1 1 0 0112 2v5h4a1 1 0 01.82 1.573l-7 10A1 1 0 018 18v-5H4a1 1 0 01-.82-1.573l7-10a1 1 0 011.12-.38z" clip-rule="evenodd" />
        </svg>
    }
}

#[function_component(PencilIcon)]
pub fn pencil_icon(props: &IconProps) -> Html {
    html! {
        <svg xmlns="http://www.w3.org/2000/svg" class={props.class()} viewBox="0 0 20 20" fill="currentColor">
            <path d="M13.586 3.586a2 2 0 112.828 2.828l-.793.793-2.828-2.828.793-.793zM11.379 5.793L3 14.172V17h2.828l8.38-8.379-2.83-2.828z" />
        </svg>
    }
}

#[function_component(RefreshIcon)]
pub fn refresh_icon(props: &IconProps) -> Html {
    html! {
        <svg xmlns="http://www.w3.org/2000/svg" class={props.class()} fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
            <path stroke-linecap="round" stroke-linejoin="round" d="M4 4v5h.582m15.356 2A8.001 8.001 0 004.582 9m0 0H9m11 11v-5h-.581m0 0a8.003 8.003 0 01-15.357-2m15.357 2H15" />
        </svg>
    }
}

#[function_component(TrashIcon)]
pub fn trash_icon(props: &IconProps) -> Html {
    html! {
        <svg xmlns="http://www.w3.org/2000/svg" class={props.class()} fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
            <path stroke-linecap="round" stroke-linejoin="round" d="M19 7l-.867 12.142A2 2 0 0116.138 21H7.862a2 2 0 01-1.995-1.858L5 7m5 4v6m4-6v6m1-10V4a1 1 0 00-1-1h-4a1 1 0 00-1 1v3M4 7h16" />
        </svg>
    }
}
