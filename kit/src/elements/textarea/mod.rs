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
}

#[allow(non_snake_case)]
pub fn Input<'a>(cx: Scope<'a, Props<'a>>) -> Element<'a> {
    let Props {
        id,
        focus,
        loading,
        placeholder,
        max_length,
        size,
        default_text,
        aria_label,
        onchange,
        onreturn,
    } = &cx.props;
    let val = use_ref(cx, || default_text.clone());

    render_input(
        cx,
        id,
        *focus,
        *loading,
        placeholder,
        *max_length,
        *size,
        aria_label,
        onchange,
        onreturn,
        val.read().as_str(),
    )
}

#[derive(Props)]
pub struct ControlledInputProps<'a> {
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
    aria_label: String,
    onchange: EventHandler<'a, (String, bool)>,
    onreturn: EventHandler<'a, (String, bool, Code)>,
    value: String,
}

#[allow(non_snake_case)]
pub fn ControlledInput<'a>(cx: Scope<'a, ControlledInputProps<'a>>) -> Element<'a> {
    let ControlledInputProps {
        id,
        focus,
        loading,
        placeholder,
        max_length,
        size,
        aria_label,
        onchange,
        onreturn,
        value,
    } = &cx.props;

    render_input(
        cx,
        id,
        *focus,
        *loading,
        placeholder,
        *max_length,
        *size,
        aria_label,
        onchange,
        onreturn,
        value.as_str(),
    )
}

#[allow(clippy::too_many_arguments)]
fn render_input<'a>(
    cx: &'a ScopeState,
    id: &String,
    focus: bool,
    loading: bool,
    placeholder: &String,
    max_length: i32,
    size: Size,
    aria_label: &String,
    onchange: &'a EventHandler<'a, (String, bool)>,
    onreturn: &'a EventHandler<'a, (String, bool, Code)>,
    value: &str,
) -> Element<'a> {
    let height_script = include_str!("./update_input_height.js");
    let focus_script = include_str!("./focus.js").replace("UUID", id);
    dioxus_desktop::use_eval(cx)(height_script.to_string());
    dioxus_desktop::use_eval(cx)(focus_script.to_string());

    let script = include_str!("./script.js")
        .replace("UUID", id)
        .replace("$MULTI_LINE", &format!("{}", true));
    let current_val = value.to_string();

    cx.render(rsx! (
        div {
            class: format_args!("input-group {}", if loading { "disabled" } else { " " }),
            div {
                class: "input",
                height: "{size.get_height()}",
                script { "{script}" },
                textarea {
                    key: "{element_id}",
                    class: "input_textarea",
                    id: "{id}",
                    // todo: troubleshoot this. it isn't working
                    autofocus: focus,
                    aria_label: "{aria_label}",
                    disabled: "{loading}",
                    value: "{value}",
                    maxlength: "{max_length}",
                    placeholder: "{placeholder}",
                    oninput: move |evt| {
                        let current_val = evt.value.clone();
                        if !current_val.trim().is_empty() {
                            onchange.call((current_val, true));
                        }
                    },
                    onkeyup: move |evt| {
                        let is_valid = !current_val.trim().is_empty();
                        if evt.code() == Code::Enter && !evt.data.modifiers().contains(Modifiers::SHIFT) {
                            onreturn.call((current_val.clone(), is_valid, evt.code()));
                        }
                    }
                }
            },
        }
        script { focus_script }
    ))
}
