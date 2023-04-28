use std::path::PathBuf;

use common::icons::outline::Shape;
use dioxus::prelude::*;

use kit::elements::button::Button;

use crate::utils::auto_updater::get_download_dest;

#[inline_props]
pub fn get_download_modal<'a>(
    cx: Scope<'a>,
    on_submit: EventHandler<'a, PathBuf>,
    on_dismiss: EventHandler<'a, ()>,
) -> Element<'a> {
    let download_location: &UseState<Option<PathBuf>> = use_state(cx, || None);

    let dl = download_location.current().clone();
    let disp_download_location = dl
        .as_ref()
        .clone()
        .map(|x| x.to_string_lossy().to_string())
        .unwrap_or_default();
    cx.render(rsx!(
        div {
            class: "modal-wrapper",
            onclick: move |_| on_dismiss.call(()),
            div {
                class: "modal",
                onclick: move |evt| {
                    evt.stop_propagation();
                },
                div {
                    class: "controlls",
                    Button {
                        onpress: move |_| {
                            on_dismiss.call(());
                        },
                        icon: Shape::XMark
                    },
                },
                div {
                    class: "content",
                    div {
                        class: "",
                        Button {
                            text: "pick location to download installer ".into(),
                            onpress: move |_| {
                                let dest = get_download_dest();
                                download_location.set(dest);
                            },
                        } ,
                        p {
                            disp_download_location
                        }
                    },
                    dl.as_ref().clone().map(|dest| rsx!(
                        Button {
                            text: "download installer".into(),
                            onpress: move |_| {
                               on_submit.call(dest.clone());
                            }
                        }
                    ))
                }
            }
        }
    ))
}
