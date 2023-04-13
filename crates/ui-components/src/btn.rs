use yew::prelude::*;

#[allow(dead_code)]
#[derive(Clone, PartialEq, Eq)]
pub enum BtnAlign {
    Left,
    Right,
    Center,
}

impl Default for BtnAlign {
    fn default() -> Self {
        Self::Center
    }
}

#[derive(Clone, PartialEq, Eq)]
pub enum BtnType {
    Default,
    Danger,
    Success,
}

impl Default for BtnType {
    fn default() -> Self {
        Self::Default
    }
}

#[allow(dead_code)]
#[derive(PartialEq, Eq)]
pub enum BtnSize {
    Xs,
    Sm,
    Base,
    Lg,
    Xl,
}

impl Default for BtnSize {
    fn default() -> Self {
        Self::Base
    }
}

#[derive(Properties, PartialEq)]
pub struct DefaultBtnProps {
    #[prop_or_default]
    pub _type: BtnType,
    #[prop_or_default]
    pub size: BtnSize,
    #[prop_or_default]
    pub align: BtnAlign,
    #[prop_or_default]
    pub onclick: Callback<MouseEvent>,
    #[prop_or_default]
    pub disabled: bool,
    #[prop_or_default]
    pub children: Children,
    #[prop_or_default]
    pub href: Option<AttrValue>,
    #[prop_or_default]
    pub classes: Classes,
}

#[function_component(Btn)]
pub fn default_button(props: &DefaultBtnProps) -> Html {
    let mut colors = match props._type {
        BtnType::Default => classes!(
            "border-neutral-600",
            "border",
            "hover:bg-neutral-600",
            "active:bg-neutral-700",
            "text-white",
        ),
        BtnType::Danger => classes!(
            "border",
            "border-red-700",
            "hover:bg-red-700",
            "text-red-500",
            "hover:text-white"
        ),
        BtnType::Success => classes!("bg-green-700", "hover:bg-green-900"),
    };

    if props.disabled {
        colors.push("text-stone-400");
    }

    let sizes = match props.size {
        BtnSize::Xs => classes!("text-xs", "px-2", "py-1"),
        BtnSize::Sm => classes!("text-sm", "px-2", "py-1"),
        BtnSize::Base => classes!("text-base", "px-3", "py-2"),
        BtnSize::Lg => classes!("text-lg", "px-3", "py-2"),
        BtnSize::Xl => classes!("text-xl", "px-4", "py-4"),
    };

    let styles = classes!(
        props.classes.clone(),
        colors,
        sizes,
        "cursor-pointer",
        "flex-row",
        "flex",
        "font-semibold",
        "items-center",
        "leading-5",
        "rounded-md",
    );

    let is_confirmed = use_state(|| false);

    let confirmed_state = is_confirmed.clone();
    let prop_onclick = props.onclick.clone();
    let btn_type = props._type.clone();

    let handle_onclick = Callback::from(move |evt| {
        // Handle confirmation for danger buttons
        if btn_type == BtnType::Danger {
            if *confirmed_state {
                prop_onclick.emit(evt);
            } else {
                confirmed_state.set(true);
            }
        } else {
            prop_onclick.emit(evt);
        }
    });

    let label = if props._type == BtnType::Danger && *is_confirmed {
        Children::new(vec![html! { <>{"⚠️ Click to confirm"}</> }])
    } else {
        props.children.clone()
    };

    let mut label_styles = classes!("flex", "flex-row", "gap-1", "items-center",);

    match &props.align {
        BtnAlign::Left => {}
        BtnAlign::Right => label_styles.push("ml-auto"),
        BtnAlign::Center => label_styles.push("mx-auto"),
    }

    if props.href.is_none() {
        html! {
            <button onclick={handle_onclick} class={styles} disabled={props.disabled}>
                <div class={label_styles}>
                    {label}
                </div>
            </button>
        }
    } else {
        html! {
            <a onclick={handle_onclick} class={styles} href={props.href.clone()} target="blank">
                <div class={label_styles}>
                    {label}
                </div>
            </a>
        }
    }
}
