//! This was made for the chatbar but it turns out that a contenteditable div is needed to render markdown. This is a temporary solution.
//! this could be merged with kit/src/elements/input and make the input element use a textarea based on a property.
//! that might helpful if a textarea needed to perform input validation.

use dioxus::prelude::*;
use dioxus_html::input_data::keyboard_types::{Code, Modifiers};

#[derive(Clone, Copy)]
pub enum Size {
    Small,
    Normal,
}

impl Size {
    fn get_height(&self) -> &str {
        match self {
            Size::Small => "0",
            _ => "",
        }
    }
}

#[derive(Props)]
pub struct Props<'a> {
    #[props(default = "".to_owned())]
    id: String,
    #[props(default = false)]
    focus: bool,
    #[props(default = false)]
    loading: bool,
    #[props(default = "".to_owned())]
    placeholder: String,
    #[props(default = 1024)]
    max_length: i32,
    #[props(default = Size::Normal)]
    size: Size,
    #[props(default = "".to_owned())]
    default_text: String,
    #[props(default = "".to_owned())]
    aria_label: String,
    onchange: EventHandler<'a, (String, bool)>,
    onreturn: EventHandler<'a, (String, bool, Code)>,
    #[props(!optional)]
    reset: Option<UseState<bool>>,
}

#[allow(non_snake_case)]
pub fn Input<'a>(cx: Scope<'a, Props<'a>>) -> Element<'a> {
    let val = use_ref(cx, || cx.props.default_text.clone());

    if let Some(hook) = &cx.props.reset {
        let should_reset = hook.get();
        if *should_reset {
            val.write().clear();
            hook.set(false);
        }
    }

    let element_id = &cx.props.id;
    let element_label = &cx.props.aria_label;
    let loading = cx.props.loading;
    let element_max_length = cx.props.max_length;
    let element_placeholder = &cx.props.placeholder;

    let eval = dioxus_desktop::use_eval(cx);
    // only run this after the component has been mounted and when the id of the input changes
    use_effect(cx, (&cx.props.id,), move |(id,)| {
        to_owned![eval];
        async move {
            let height_script = include_str!("./update_input_height.js");
            eval(height_script.to_string());
            let script = include_str!("./script.js")
                .replace("UUID", &id)
                .replace("$MULTI_LINE", "true");
            eval(script);
            let focus_script = include_str!("./focus.js").replace("UUID", &id);
            eval(focus_script);
        }
    });

    cx.render(rsx! (
        div {
            class: format_args!("input-group {}", if cx.props.loading { "disabled" } else { " " }),
            div {
                class: "input",
                height: cx.props.size.get_height(),
                textarea {
                    key: "{element_id}",
                    class: "input_textarea",
                    id: "{element_id}",
                    // todo: troubleshoot this. it isn't working
                    autofocus: cx.props.focus,
                    aria_label: "{element_label}",
                    disabled: "{loading}",
                    value: "{val.read()}",
                    maxlength: "{element_max_length}",
                    placeholder: "{element_placeholder}",
                    onblur: move |_| {
                        cx.props.onreturn.call((val.read().to_string(), false, Code::Escape));
                    },
                    oninput: move |evt| {
                        let current_val = evt.value.clone();
                        *val.write_silent() = current_val;
                        if !val.read().trim().is_empty() {
                            cx.props.onchange.call((val.read().to_string(), true));
                        }
                    },
                    onkeyup: move |evt| {
                        let is_valid = !val.read().trim().is_empty();
                        if evt.code() == Code::Enter && !evt.data.modifiers().contains(Modifiers::SHIFT) {
                            cx.props.onreturn.call((val.read().to_string(), is_valid, evt.code()));
                        }
                    }
                }
            },
        }
    ))
}
