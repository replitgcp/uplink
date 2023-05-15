use common::icons::outline::Shape as Icon;
use common::language::get_local_text;
use dioxus::prelude::*;
use kit::components::context_menu::{ContextItem, ContextMenu};
use warp::constellation::file::File;

use super::storage::functions::thumbnail_to_base64;

#[derive(Props)]
pub struct Props<'a> {
    file: &'a File,
    on_download: EventHandler<'a, ()>,
}

#[allow(non_snake_case)]
pub fn FilePreview<'a>(cx: Scope<'a, Props<'a>>) -> Element<'a> {
    let thumbnail = thumbnail_to_base64(cx.props.file);

    cx.render(rsx!(rsx!(div {
        ContextMenu {
            id: "file-preview-context-menu".into(),
            items: cx.render(rsx!(
                ContextItem {
                    icon: Icon::ArrowDownCircle,
                    aria_label: "files-download-preview".into(),
                    text: get_local_text("files.download"),
                    onpress: move |_| {
                        cx.props.on_download.call(());
                    }
                },)),
            img {
                id: "file_preview_img",
                src: "{thumbnail}",
                position: "absolute",
                top: "50%",
                left: "50%",
                transform: "translate(-50%, -50%)",
                max_height: "80%",
                max_width: "80%",
            },
        },
    })))
}
