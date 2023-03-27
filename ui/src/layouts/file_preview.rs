use std::io::Cursor;

use common::icons::outline::Shape as Icon;
use common::{
    language::get_local_text, state::State, DOC_EXTENSIONS, IMAGE_EXTENSIONS, STATIC_ARGS,
    VIDEO_FILE_EXTENSIONS,
};
use dioxus::prelude::*;
use dioxus_desktop::{use_window, DesktopContext, LogicalSize};
use image::io::Reader as ImageReader;
use kit::components::context_menu::{ContextItem, ContextMenu};
use kit::elements::file::get_file_extension;
use notify::{Config, RecommendedWatcher, RecursiveMode, Watcher};
use std::sync::mpsc::channel;
use warp::constellation::file::File;

use crate::utils::WindowDropHandler;

const CSS_STYLE: &str = include_str!("./style.scss");

#[derive(Clone, PartialEq, Eq)]
pub enum FileFormat {
    Video,
    Image,
    Document,
    Other,
}

pub fn get_file_format(file_name: String) -> FileFormat {
    let file_extension = get_file_extension(file_name);

    let image_formats = IMAGE_EXTENSIONS.to_vec();
    if image_formats.iter().any(|f| f == &file_extension) {
        return FileFormat::Image;
    }

    let video_formats = VIDEO_FILE_EXTENSIONS.to_vec();

    if video_formats.iter().any(|f| f == &file_extension) {
        return FileFormat::Video;
    }

    let doc_formats = DOC_EXTENSIONS.to_vec();

    if doc_formats.iter().any(|f| f == &file_extension) {
        return FileFormat::Document;
    }
    FileFormat::Other
}

#[inline_props]
#[allow(non_snake_case)]
pub fn FilePreview(cx: Scope, file: File, _drop_handler: WindowDropHandler) -> Element {
    let file_format = get_file_format(file.name());
    let file_name = file.name();
    let thumbnail = file.thumbnail();
    let has_thumbnail = !file.thumbnail().is_empty();
    let desktop = use_window(cx);
    let mut css_style = update_theme_colors();
    let update_state: &UseRef<Option<()>> = use_ref(cx, || Some(()));

    if update_state.read().is_some() {
        css_style = update_theme_colors();
        *update_state.write_silent() = None;
    }

    let first_render = use_state(cx, || true);

    resize_window(
        has_thumbnail,
        *first_render.get(),
        desktop,
        &thumbnail,
        file.clone(),
        &file_format,
    );

    if *first_render.get() {
        first_render.set(false);
    }

    use_future(cx, (), |_| {
        to_owned![update_state];
        async move {
            let (tx, rx) = channel();
            let fs_event_watcher_result = RecommendedWatcher::new(tx, Config::default());
            if let Ok(fs_event_watcher) = fs_event_watcher_result {
                let mut watcher: RecommendedWatcher = fs_event_watcher;
                if watcher
                    .watch(
                        STATIC_ARGS.cache_path.clone().as_path(),
                        RecursiveMode::NonRecursive,
                    )
                    .is_ok()
                {
                    loop {
                        let mut event_processed = false;
                        tokio::time::sleep(std::time::Duration::from_millis(300)).await;
                        while rx.try_recv().is_ok() {
                            if update_state.read().is_none() && !event_processed {
                                update_state.with_mut(|i| *i = Some(()));
                                event_processed = true;
                            }
                        }
                    }
                };
            }
        }
    });

    cx.render(rsx! (
        style { css_style },
        style { CSS_STYLE },
        div {
            id: "video-poped-out",
            class: "file-preview",
            div {
                class: "wrap",
                {
                if file_format != FileFormat::Other && has_thumbnail {
                    rsx!{
                        ContextMenu {
                            key: "favorite",
                            id: "test".to_string(),
                            items: cx.render(rsx!(
                                ContextItem {
                                    icon: Icon::Config,
                                    text: "Copy to clipboard".to_string(),
                                    onpress: move |_| {
                                        let mut buffer = Vec::new();
                                        file.read_to_end(&mut buffer)?;

                                        // Initialize the clipboard context
                                        let mut ctx: ClipboardContext = ClipboardProvider::new()?;

                                        // Copy the buffer to the clipboard
                                        ctx.set_contents(buffer)?;


                                        println!("Copy to clipboard");
                                    }
                                }
                            )),
                            div {
                                img {
                                    src: "{thumbnail}",
                                    width: "100%",
                            },
                                p {
                                    class: "thumbnail-text thumb-text",
                                    format!("{}", match file_format {
                                        FileFormat::Video => get_local_text("files.video-thumb"),
                                        FileFormat::Image => get_local_text("files.image-thumb"),
                                        FileFormat::Document => get_local_text("files.doc-thumb"),
                                        _ => String::from("Thumb"),
                                    }),
                                }
                            }
                        }
                        }
                    } else {
                        rsx!(div{
                            h3 {
                                class: "thumb-text",
                                " {file_name}"}
                            p {
                                class: "thumb-text",
                                get_local_text("files.no-thumbnail-preview")}

                        })
                    }
                }
            },
        },
    ))
}

fn resize_window(
    has_thumbnail: bool,
    first_render: bool,
    desktop: &DesktopContext,
    thumbnail: &str,
    file: File,
    file_format: &FileFormat,
) -> Option<()> {
    if has_thumbnail && first_render {
        let base64_string = &thumbnail[thumbnail.find(',')? + 1..];
        let thumbnail_bytes = base64::decode(base64_string).ok()?;
        let cursor = Cursor::new(thumbnail_bytes);
        let img_format = if file.name().contains(".png") {
            image::ImageFormat::Png
        } else {
            image::ImageFormat::Jpeg
        };
        let image_reader = ImageReader::with_format(cursor, img_format);
        if let Ok(image) = image_reader.decode() {
            let (mut width, mut height) = (image.width() as f64, image.height() as f64);
            if height > 800.0 || width > 800.0 {
                let scale_factor = desktop.scale_factor() + 0.5;
                width /= scale_factor;
                height /= scale_factor;
            }
            desktop.set_inner_size(LogicalSize::new(width, height));
        }
    } else if first_render && file_format != &FileFormat::Other {
        let scale_factor = desktop.scale_factor() + 0.5;
        desktop.set_inner_size(LogicalSize::new(600.0 / scale_factor, 300.0 / scale_factor));
    }
    Some(())
}

fn update_theme_colors() -> String {
    let state = State::load();
    let mut css_style = state
        .ui
        .theme
        .as_ref()
        .map(|t| t.styles.clone())
        .unwrap_or_default();
    let background_style = if css_style.contains("--background") {
        "background: var(--background);"
    } else {
        "background: #000000;"
    };
    css_style.push_str(&format!(
        "
             html, body {{
                 {}
             }}
        ",
        background_style
    ));
    css_style
}
