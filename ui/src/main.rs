//#![deny(elided_lifetimes_in_paths)]

use ::extensions::ExtensionProxy;
use clap::Parser;
use common::icons::outline::Shape as Icon;
use common::icons::Icon as IconElement;
use common::language::{change_language, get_local_text};
use common::{state, warp_runner, LogProfile, STATIC_ARGS, WARP_CMD_CH, WARP_EVENT_CH};
use dioxus::prelude::*;
use dioxus_desktop::tao::dpi::LogicalSize;
use dioxus_desktop::tao::event::WindowEvent;
use dioxus_desktop::tao::menu::AboutMetadata;
use dioxus_desktop::Config;
use dioxus_desktop::{tao, use_window};
use fs_extra::dir::*;
use futures::channel::oneshot;
use futures::StreamExt;
use kit::components::nav::Route as UIRoute;
use kit::elements::button::Button;
use kit::elements::Appearance;
use notify::{RecommendedWatcher, RecursiveMode, Watcher};
use once_cell::sync::Lazy;
use overlay::{make_config, OverlayDom};
use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::time::Instant;
use std::{fs, io};
use uuid::Uuid;
use warp::multipass;
use warp::multipass::identity::Platform;

use std::sync::Arc;
use tao::menu::{MenuBar as Menu, MenuItem};
use tao::window::WindowBuilder;
use tokio::sync::Mutex;
use tokio::time::{sleep, Duration};
use warp::logging::tracing::log::{self, LevelFilter};

use dioxus_desktop::use_wry_event_handler;
use dioxus_desktop::wry::application::event::Event as WryEvent;

use crate::components::debug_logger::DebugLogger;
use crate::components::toast::Toast;
use crate::extensions::AvailableExtensions;
use crate::layouts::create_account::CreateAccountLayout;
use crate::layouts::friends::FriendsLayout;
use crate::layouts::settings::SettingsLayout;
use crate::layouts::storage::{FilesLayout, DRAG_EVENT};
use crate::layouts::unlock::UnlockLayout;

use crate::window_manager::WindowManagerCmdChannels;
use crate::{components::chat::RouteInfo, layouts::chat::ChatLayout};
use common::{
    state::{friends, storage, ui::WindowMeta, Action, State},
    warp_runner::{ConstellationCmd, MultiPassCmd, RayGunCmd, WarpCmd},
};
use dioxus_router::*;

use kit::STYLE as UIKIT_STYLES;
pub const APP_STYLE: &str = include_str!("./compiled_styles.css");
mod components;
mod extensions;
mod layouts;
mod logger;
mod overlay;
mod utils;
mod window_manager;

// used to close the popout player, among other things
pub static WINDOW_CMD_CH: Lazy<WindowManagerCmdChannels> = Lazy::new(|| {
    let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
    WindowManagerCmdChannels {
        tx,
        rx: Arc::new(Mutex::new(rx)),
    }
});

pub struct UplinkRoutes<'a> {
    pub chat: &'a str,
    pub friends: &'a str,
    pub files: &'a str,
    pub settings: &'a str,
}

pub static UPLINK_ROUTES: UplinkRoutes = UplinkRoutes {
    chat: "/",
    friends: "/friends",
    files: "/files",
    settings: "/settings",
};

// serve as a sort of router while the user logs in]
#[allow(clippy::large_enum_variant)]
#[derive(PartialEq, Eq)]
pub enum AuthPages {
    Unlock,
    CreateAccount,
    Success(multipass::identity::Identity),
}

fn copy_assets() {
    let themes_dest = &STATIC_ARGS.themes_path;
    let themes_src = Path::new("ui").join("extra").join("themes");

    match create_all(themes_dest.clone(), false) {
        Ok(_) => {
            let mut options = CopyOptions::new();
            options.skip_exist = true;
            options.copy_inside = true;

            if let Err(error) = copy(themes_src, themes_dest, &options) {
                log::error!("Error on copy themes {error}");
            }
        }
        Err(error) => log::error!("Error on create themes folder: {error}"),
    };
}

fn main() {
    // Attempts to increase the file desc limit on unix-like systems
    // Note: Will be changed out in the future
    if fdlimit::raise_fd_limit().is_none() {}
    // configure logging
    let args = common::Args::parse();
    let max_log_level = if let Some(profile) = args.profile {
        match profile {
            LogProfile::Debug => {
                logger::set_write_to_stdout(true);
                LevelFilter::Debug
            }
            LogProfile::Trace => {
                logger::set_display_trace(true);
                logger::set_write_to_stdout(true);
                LevelFilter::Trace
            }
            LogProfile::Trace2 => {
                logger::set_display_warp(true);
                logger::set_display_trace(true);
                logger::set_write_to_stdout(true);
                LevelFilter::Trace
            }
            _ => LevelFilter::Debug,
        }
    } else {
        LevelFilter::Debug
    };
    logger::init_with_level(max_log_level).expect("failed to init logger");

    // Initializes the cache dir if needed
    std::fs::create_dir_all(STATIC_ARGS.uplink_path.clone())
        .expect("Error creating Uplink directory");
    std::fs::create_dir_all(STATIC_ARGS.warp_path.clone()).expect("Error creating Warp directory");

    copy_assets();

    let mut main_menu = Menu::new();
    let mut app_menu = Menu::new();
    let mut edit_menu = Menu::new();
    let mut window_menu = Menu::new();

    app_menu.add_native_item(MenuItem::About(
        String::from("Uplink"),
        AboutMetadata::default(),
    ));
    app_menu.add_native_item(MenuItem::Quit);
    // add native shortcuts to `edit_menu` menu
    // in macOS native item are required to get keyboard shortcut
    // to works correctly
    edit_menu.add_native_item(MenuItem::Undo);
    edit_menu.add_native_item(MenuItem::Redo);
    edit_menu.add_native_item(MenuItem::Separator);
    edit_menu.add_native_item(MenuItem::Cut);
    edit_menu.add_native_item(MenuItem::Copy);
    edit_menu.add_native_item(MenuItem::Paste);
    edit_menu.add_native_item(MenuItem::SelectAll);

    window_menu.add_native_item(MenuItem::Minimize);
    window_menu.add_native_item(MenuItem::Zoom);
    window_menu.add_native_item(MenuItem::Separator);
    window_menu.add_native_item(MenuItem::ShowAll);
    window_menu.add_native_item(MenuItem::EnterFullScreen);
    window_menu.add_native_item(MenuItem::Separator);
    window_menu.add_native_item(MenuItem::CloseWindow);

    main_menu.add_submenu("Uplink", true, app_menu);
    main_menu.add_submenu("Edit", true, edit_menu);
    main_menu.add_submenu("Window", true, window_menu);

    let title = get_local_text("uplink");

    #[allow(unused_mut)]
    let mut window = WindowBuilder::new()
        .with_title(title)
        .with_resizable(true)
        .with_inner_size(LogicalSize::new(950.0, 600.0))
        // We start the min inner size smaller because the prelude pages like unlock can be rendered much smaller.
        .with_min_inner_size(LogicalSize::new(300.0, 350.0));

    #[cfg(target_os = "macos")]
    {
        use dioxus_desktop::tao::platform::macos::WindowBuilderExtMacOS;

        window = window
            .with_has_shadow(true)
            .with_transparent(true)
            .with_fullsize_content_view(true)
            .with_menu(main_menu)
            .with_titlebar_transparent(true);
        // .with_movable_by_window_background(true)
    }

    #[cfg(not(target_os = "macos"))]
    {
        window = window.with_decorations(false).with_transparent(true);
    }

    let config = Config::default();

    dioxus_desktop::launch_cfg(
        bootstrap,
        config
            .with_window(window)
            .with_custom_index(
                r#"
    <!doctype html>
    <html>
    <script src="https://cdn.jsdelivr.net/npm/interactjs/dist/interact.min.js"></script>
    <body style="background-color:rgba(0,0,0,0);"><div id="main"></div></body>
    </html>"#
                    .to_string(),
            )
            .with_file_drop_handler(|_w, drag_event| {
                log::info!("Drag Event: {:?}", drag_event);
                *DRAG_EVENT.write() = drag_event;
                true
            }),
    )
}

// start warp_runner and ensure the user is logged in
fn bootstrap(cx: Scope) -> Element {
    log::trace!("rendering bootstrap");

    // warp_runner must be started from within a tokio reactor
    // store in a use_ref to make it not get dropped
    let warp_runner = use_ref(cx, warp_runner::WarpRunner::new);
    warp_runner.write_silent().run();

    // make the window smaller while the user authenticates
    let desktop = use_window(cx);
    desktop.set_inner_size(LogicalSize {
        width: 500.0,
        height: 350.0,
    });

    cx.render(rsx!(crate::auth_page_manager {}))
}

// Uplink's Router depends on State, which can't be loaded until the user logs in.
// don't see a way to replace the router
// so instead use a Prop to determine which page to render
// after the user logs in, app_bootstrap loads Uplink as normal.
fn auth_page_manager(cx: Scope) -> Element {
    let page = use_state(cx, || AuthPages::Unlock);
    let pin = use_ref(cx, String::new);
    cx.render(rsx!(match &*page.current() {
        AuthPages::Success(ident) => rsx!(app_bootstrap {
            identity: ident.clone()
        }),
        _ => rsx!(auth_wrapper {
            page: page.clone(),
            pin: pin.clone()
        }),
    }))
}

#[allow(unused_assignments)]
#[inline_props]
fn auth_wrapper(cx: Scope, page: UseState<AuthPages>, pin: UseRef<String>) -> Element {
    log::trace!("rendering auth wrapper");
    let desktop = use_window(cx);
    let theme = "";

    #[allow(unused_mut)]
    let mut controls: Option<VNode> = None;

    #[cfg(not(target_os = "macos"))]
    {
        controls = cx.render(rsx!(
            div {
                class: "controls",
                Button {
                    aria_label: "minimize-button".into(),
                    icon: Icon::Minus,
                    appearance: Appearance::Transparent,
                    onpress: move |_| {
                        desktop.set_minimized(true);
                    }
                },
                Button {
                    aria_label: "square-button".into(),
                    icon: Icon::Square2Stack,
                    appearance: Appearance::Transparent,
                    onpress: move |_| {
                        desktop.set_maximized(!desktop.is_maximized());
                    }
                },
                Button {
                    aria_label: "close-button".into(),
                    icon: Icon::XMark,
                    appearance: Appearance::Transparent,
                    onpress: move |_| {
                        desktop.close();
                    }
                },
            }
        ))
    }

    cx.render(rsx! (
        style { "{UIKIT_STYLES} {APP_STYLE} {theme}" },
        div {
            id: "app-wrap",
            div {
                id: "titlebar",
                onmousedown: move |_| { desktop.drag(); },
                controls,
            },
            match *page.current() {
                AuthPages::Unlock => rsx!(UnlockLayout { page: page.clone(), pin: pin.clone() }),
                AuthPages::CreateAccount => rsx!(CreateAccountLayout { page: page.clone(), pin: pin.clone() }),
                _ => panic!("invalid page")
            }
        }
    ))
}

fn get_extensions() -> Result<HashMap<String, ExtensionProxy>, io::Error> {
    // load any extensions, we currently don't care to store the result of located extensions since they are stored by the librarian.
    // We should however ensure we use the same librarian across the app so they should probably live in a globally accessible place
    // that updates when they have new info, i.e. state.
    fs::create_dir_all(&STATIC_ARGS.extensions_path)?;
    let paths = fs::read_dir(&STATIC_ARGS.extensions_path)?;
    let mut extensions_library = AvailableExtensions::new();

    for entry in paths {
        let path = entry?.path();
        if path.extension().unwrap_or_default() == ::extensions::FILE_EXT {
            log::debug!("Found extension: {:?}", path);
            unsafe {
                let loader = extensions_library.load(&path);
                match loader {
                    Ok(_) => {
                        log::debug!("Loaded extension: {:?}", &path);
                    }
                    Err(e) => {
                        log::error!("Error loading extension: {:?}", e);
                    }
                }
            }
        }
    }
    Ok(extensions_library.extensions)
}

// called at the end of the auth flow
#[inline_props]
pub fn app_bootstrap(cx: Scope, identity: multipass::identity::Identity) -> Element {
    log::trace!("rendering app_bootstrap");
    let mut state = State::load();

    if STATIC_ARGS.use_mock {
        assert!(state.friends().initialized);
        assert!(state.chats().initialized);
    } else {
        state.set_own_identity(identity.clone().into());
    }

    // set the window to the normal size.
    // todo: perhaps when the user resizes the window, store that in State, and load that here
    let desktop = use_window(cx);
    // Here we set the size larger, and bump up the min size in preparation for rendering the main app.
    desktop.set_inner_size(LogicalSize::new(950.0, 600.0));
    desktop.set_min_inner_size(Some(LogicalSize::new(300.0, 500.0)));

    // todo: delete this. it is just an example
    if state.configuration.general.enable_overlay {
        let overlay_test = VirtualDom::new(OverlayDom);
        let window = desktop.new_window(overlay_test, make_config());
        state.ui.overlays.push(window);
    }

    let size = desktop.webview.inner_size();
    // Update the window metadata now that we've created a window
    let window_meta = WindowMeta {
        focused: desktop.is_focused(),
        maximized: desktop.is_maximized(),
        minimized: desktop.is_minimized(),
        width: size.width,
        height: size.height,
        minimal_view: size.width < 1200, // todo: why is it that on Linux, checking if desktop.inner_size().width < 600 is true?
    };
    state.ui.metadata = window_meta;

    match get_extensions() {
        Ok(ext) => state.ui.extensions = ext,
        Err(e) => {
            log::error!("failed to get extensions: {e}");
        }
    }
    log::debug!("Loaded {} extensions.", state.ui.extensions.keys().len());

    use_shared_state_provider(cx, || state);

    cx.render(rsx!(crate::app {}))
}

fn app(cx: Scope) -> Element {
    log::trace!("rendering app");
    let desktop = use_window(cx);
    let state = use_shared_state::<State>(cx)?;

    // don't fetch friends and conversations from warp when using mock data
    let friends_init = use_ref(cx, || STATIC_ARGS.use_mock);
    let items_init = use_ref(cx, || STATIC_ARGS.use_mock);
    let chats_init = use_ref(cx, || STATIC_ARGS.use_mock);
    let needs_update = use_state(cx, || false);

    // this gets rendered at the bottom. this way you don't have to scroll past all the use_futures to see what this function renders
    let main_element = {
        // render the Uplink app
        let user_lang_saved = state.read().settings.language.clone();
        change_language(user_lang_saved);

        let theme = state
            .read()
            .ui
            .theme
            .as_ref()
            .map(|theme| theme.styles.clone())
            .unwrap_or_default();

        rsx! (
            style { "{UIKIT_STYLES} {APP_STYLE} {theme}" },
            div {
                id: "app-wrap",
                get_titlebar(cx),
                get_toasts(cx),
                get_call_dialog(cx),
                get_pre_release_message(cx),
                get_router(cx),
                get_logger(cx)
            }
        )
    };

    // `use_future`s
    // all of Uplinks periodic tasks are located here. it's a lot to read but
    // it's better to have them in one place. this makes it a lot easier to find them.
    // there are 2 categories of tasks: warp tasks and UI tasks
    //
    // warp tasks
    // handle warp events
    // initialize friends: load from warp and store in State
    // initialize conversations: same
    //
    // UI tasks
    // clear toasts
    // update message timestamps
    // control child windows
    // clear typing indicator
    //
    // misc
    // when a task requires the UI be updated, `needs_update` is set.
    // when mock data is used, friends and conversations are generated randomly,
    //     not loaded from Warp. however, warp_runner continues to operate normally.
    //

    // yes, double render. sry.
    if *needs_update.get() {
        needs_update.set(false);
        state.write();
    }

    // There is currently an issue in Tauri/Wry where the window size is not reported properly.
    // Thus we bind to the resize event itself and update the size from the webview.
    let webview = desktop.webview.clone();
    let inner = state.inner();
    use_wry_event_handler(cx, {
        to_owned![needs_update, desktop];
        move |event, _| match event {
            WryEvent::WindowEvent {
                event: WindowEvent::Focused(focused),
                ..
            } => {
                //log::trace!("FOCUS CHANGED {:?}", *focused);
                match inner.try_borrow_mut() {
                    Ok(state) => {
                        state.write().ui.metadata.focused = *focused;
                        //crate::utils::sounds::Play(Sounds::Notification);
                        //needs_update.set(true);
                    }
                    Err(e) => {
                        log::error!("{e}");
                    }
                }
            }
            WryEvent::WindowEvent {
                event: WindowEvent::CloseRequested,
                ..
            } => match inner.try_borrow_mut() {
                Ok(state) => {
                    state
                        .write()
                        .mutate(Action::ClearAllPopoutWindows(desktop.clone()));
                }
                Err(e) => log::error!("{e}"),
            },
            WryEvent::WindowEvent {
                event: WindowEvent::Resized(_),
                ..
            } => {
                let size = webview.inner_size();
                //log::trace!(
                //    "Resized - PhysicalSize: {:?}, Minimal: {:?}",
                //    size,
                //    size.width < 1200
                //);
                match inner.try_borrow_mut() {
                    Ok(state) => {
                        let metadata = state.read().ui.metadata.clone();
                        let new_metadata = WindowMeta {
                            height: size.height,
                            width: size.width,
                            minimal_view: size.width < 600,
                            ..metadata
                        };
                        if metadata != new_metadata {
                            state.write().ui.sidebar_hidden = new_metadata.minimal_view;
                            state.write().ui.metadata = new_metadata;
                            needs_update.set(true);
                        }
                    }
                    Err(e) => {
                        log::error!("{e}");
                    }
                }
            }
            _ => {}
        }
    });

    // update state in response to warp events
    let inner = state.inner();
    use_future(cx, (), |_| {
        to_owned![needs_update, friends_init, chats_init];
        async move {
            // don't process warp events until friends and chats have been loaded
            while !(*friends_init.read() && *chats_init.read()) {
                tokio::time::sleep(std::time::Duration::from_millis(10)).await;
            }
            let warp_event_rx = WARP_EVENT_CH.rx.clone();
            log::trace!("starting warp_runner use_future");
            // it should be sufficient to lock once at the start of the use_future. this is the only place the channel should be read from. in the off change that
            // the future restarts (it shouldn't), the lock should be dropped and this wouldn't block.
            let mut ch = warp_event_rx.lock().await;
            while let Some(evt) = ch.recv().await {
                match inner.try_borrow_mut() {
                    Ok(state) => {
                        state.write().process_warp_event(evt);
                        needs_update.set(true);
                    }
                    Err(e) => {
                        log::error!("{e}");
                    }
                }
            }
        }
    });

    // clear toasts
    let inner = state.inner();
    use_future(cx, (), |_| {
        to_owned![needs_update];
        async move {
            loop {
                sleep(Duration::from_secs(1)).await;
                match inner.try_borrow_mut() {
                    Ok(state) => {
                        if !state.read().has_toasts() {
                            continue;
                        }
                        if state.write().decrement_toasts() {
                            needs_update.set(true);
                        }
                    }
                    Err(e) => {
                        log::error!("{e}");
                    }
                }
            }
        }
    });

    // clear typing indicator
    let inner = state.inner();
    use_future(cx, (), |_| {
        to_owned![needs_update];
        async move {
            loop {
                sleep(Duration::from_secs(STATIC_ARGS.typing_indicator_timeout)).await;
                match inner.try_borrow_mut() {
                    Ok(state) => {
                        let now = Instant::now();
                        if state.write().clear_typing_indicator(now) {
                            needs_update.set(true);
                        }
                    }
                    Err(e) => {
                        log::error!("{e}");
                    }
                }
            }
        }
    });

    // periodically refresh message timestamps and friend's status messages
    use_future(cx, (), |_| {
        to_owned![needs_update];
        async move {
            loop {
                // simply triggering an update will refresh the message timestamps
                sleep(Duration::from_secs(60)).await;
                needs_update.set(true);
            }
        }
    });

    // control child windows
    let inner = state.inner();
    use_future(cx, (), |_| {
        to_owned![needs_update, desktop];
        async move {
            let window_cmd_rx = WINDOW_CMD_CH.rx.clone();
            let mut ch = window_cmd_rx.lock().await;
            while let Some(cmd) = ch.recv().await {
                window_manager::handle_cmd(inner.clone(), cmd, desktop.clone()).await;
                needs_update.set(true);
            }
        }
    });

    // initialize friends
    let inner = state.inner();
    use_future(cx, (), |_| {
        to_owned![friends_init, needs_update];
        async move {
            if *friends_init.read() {
                return;
            }
            let warp_cmd_tx = WARP_CMD_CH.tx.clone();
            let (tx, rx) = oneshot::channel::<
                Result<(friends::Friends, HashSet<state::Identity>), warp::error::Error>,
            >();
            if let Err(e) = warp_cmd_tx.send(WarpCmd::MultiPass(MultiPassCmd::InitializeFriends {
                rsp: tx,
            })) {
                log::error!("failed to initialize Friends {}", e);
                tokio::time::sleep(Duration::from_secs(1)).await;
                return;
            }

            let res = rx.await.expect("failed to get response from warp_runner");

            log::trace!("init friends");
            let friends = match res {
                Ok(friends) => friends,
                Err(e) => {
                    log::error!("init friends failed: {}", e);
                    return;
                }
            };

            match inner.try_borrow_mut() {
                Ok(state) => {
                    state.write().set_friends(friends.0, friends.1);
                    needs_update.set(true);
                }
                Err(e) => {
                    log::error!("{e}");
                }
            }

            *friends_init.write_silent() = true;
            needs_update.set(true);
        }
    });

    // initialize files
    let inner = state.inner();
    use_future(cx, (), |_| {
        to_owned![items_init, needs_update];
        async move {
            if *items_init.read() {
                return;
            }
            let warp_cmd_tx = WARP_CMD_CH.tx.clone();
            let (tx, rx) = oneshot::channel::<Result<storage::Storage, warp::error::Error>>();

            if let Err(e) = warp_cmd_tx.send(WarpCmd::Constellation(
                ConstellationCmd::GetItemsFromCurrentDirectory { rsp: tx },
            )) {
                log::error!("failed to initialize Files {}", e);
                return;
            }

            let res = rx.await.expect("failed to get response from warp_runner");

            log::trace!("init items");
            match res {
                Ok(storage) => match inner.try_borrow_mut() {
                    Ok(state) => {
                        state.write().storage = storage;

                        needs_update.set(true);
                    }
                    Err(e) => {
                        log::error!("{e}");
                    }
                },
                Err(e) => {
                    log::error!("init items failed: {}", e);
                }
            }

            *items_init.write_silent() = true;
            needs_update.set(true);
        }
    });

    // initialize conversations
    let inner = state.inner();
    use_future(cx, (), |_| {
        to_owned![chats_init, needs_update];
        async move {
            if *chats_init.read() {
                return;
            }
            let warp_cmd_tx = WARP_CMD_CH.tx.clone();
            let res = loop {
                let (tx, rx) = oneshot::channel::<
                    Result<
                        (HashMap<Uuid, state::Chat>, HashSet<state::Identity>),
                        warp::error::Error,
                    >,
                >();
                if let Err(e) =
                    warp_cmd_tx.send(WarpCmd::RayGun(RayGunCmd::InitializeConversations {
                        rsp: tx,
                    }))
                {
                    log::error!("failed to init RayGun: {}", e);
                    tokio::time::sleep(Duration::from_secs(1)).await;
                    continue;
                }

                match rx.await {
                    Ok(r) => break r,
                    Err(e) => {
                        log::error!("command canceled: {}", e);
                        tokio::time::sleep(std::time::Duration::from_secs(1)).await
                    }
                }
            };

            log::trace!("init chats");
            let chats = match res {
                Ok(r) => r,
                Err(e) => {
                    log::error!("failed to initialize chats: {}", e);
                    return;
                }
            };

            match inner.try_borrow_mut() {
                Ok(state) => {
                    state.write().set_chats(chats.0, chats.1);
                    needs_update.set(true);
                }
                Err(e) => {
                    log::error!("{e}");
                }
            }

            *chats_init.write_silent() = true;
            needs_update.set(true);
        }
    });

    // Automatically select the best implementation for your platform.
    let inner = state.inner();
    use_future(cx, (), |_| async move {
        let (tx, mut rx) = futures::channel::mpsc::unbounded();
        let mut watcher = match RecommendedWatcher::new(
            move |res| {
                let _ = tx.unbounded_send(res);
            },
            notify::Config::default().with_poll_interval(Duration::from_secs(1)),
        ) {
            Ok(watcher) => watcher,
            Err(e) => {
                log::error!("{e}");
                return;
            }
        };

        // Add a path to be watched. All files and directories at that path and
        // below will be monitored for changes.
        if let Err(e) = watcher.watch(
            STATIC_ARGS.extensions_path.as_path(),
            RecursiveMode::Recursive,
        ) {
            log::error!("{e}");
            return;
        }

        while let Some(event) = rx.next().await {
            let event = match event {
                Ok(event) => event,
                Err(e) => {
                    log::error!("{e}");
                    continue;
                }
            };

            log::debug!("{event:?}");
            match inner.try_borrow_mut() {
                Ok(state) => match get_extensions() {
                    Ok(ext) => {
                        state.write().mutate(Action::RegisterExtensions(ext));
                    }
                    Err(e) => {
                        log::error!("failed to get extensions: {e}");
                    }
                },
                Err(e) => {
                    log::error!("{e}");
                }
            }
        }
    });

    cx.render(main_element)
}

fn get_pre_release_message(cx: Scope) -> Element {
    let pre_release_text = get_local_text("uplink.pre-release");
    cx.render(rsx!(
        div {
            id: "pre-release",
            aria_label: "pre-release",
            IconElement {
                icon: Icon::Beaker,
            },
            p {
                "{pre_release_text}",
            }
        },
    ))
}

fn get_logger(cx: Scope) -> Element {
    let state = use_shared_state::<State>(cx)?;

    cx.render(rsx!(state
        .read()
        .configuration
        .developer
        .developer_mode
        .then(|| rsx!(DebugLogger {}))))
}

fn get_toasts(cx: Scope) -> Element {
    let state = use_shared_state::<State>(cx)?;
    cx.render(rsx!(state.read().ui.toast_notifications.iter().map(
        |(id, toast)| {
            rsx!(Toast {
                id: *id,
                with_title: toast.title.clone(),
                with_content: toast.content.clone(),
                icon: toast.icon.unwrap_or(Icon::InformationCircle),
                appearance: Appearance::Secondary,
            },)
        }
    )))
}

#[allow(unused_assignments)]
fn get_titlebar(cx: Scope) -> Element {
    let desktop = use_window(cx);
    let state = use_shared_state::<State>(cx)?;
    let config = state.read().configuration.clone();

    #[allow(unused_mut)]
    let mut controls: Option<VNode> = None;

    #[cfg(not(target_os = "macos"))]
    {
        controls = cx.render(rsx!(
            div {
                class: "controls",
                Button {
                    aria_label: "minimize-button".into(),
                    icon: Icon::Minus,
                    appearance: Appearance::Transparent,
                    onpress: move |_| {
                        desktop.set_minimized(true);
                    }
                },
                Button {
                    aria_label: "square-button".into(),
                    icon: Icon::Square2Stack,
                    appearance: Appearance::Transparent,
                    onpress: move |_| {
                        desktop.set_maximized(!desktop.is_maximized());
                    }
                },
                Button {
                    aria_label: "close-button".into(),
                    icon: Icon::XMark,
                    appearance: Appearance::Transparent,
                    onpress: move |_| {
                        desktop.close();
                    }
                },
            }
        ))
    }

    cx.render(rsx!(
        div {
            id: "titlebar",
            onmousedown: move |_| { desktop.drag(); },
            // Only display this if developer mode is enabled.
            (config.developer.developer_mode).then(|| rsx!(
                Button {
                    aria_label: "device-phone-mobile-button".into(),
                    icon: Icon::DevicePhoneMobile,
                    appearance: Appearance::Transparent,
                    onpress: move |_| {
                        desktop.set_inner_size(LogicalSize::new(300.0, 534.0));
                        let meta = state.read().ui.metadata.clone();
                        state.write().mutate(Action::SetMeta(WindowMeta {
                            width: 300,
                            height: 534,
                            minimal_view: true,
                            ..meta
                        }));
                        state.write().mutate(Action::SidebarHidden(true));
                        state.write().mock_own_platform(Platform::Mobile);
                    }
                },
                Button {
                    aria_label: "device-tablet-button".into(),
                    icon: Icon::DeviceTablet,
                    appearance: Appearance::Transparent,
                    onpress: move |_| {
                        desktop.set_inner_size(LogicalSize::new(600.0, 534.0));
                        let meta = state.read().ui.metadata.clone();
                        state.write().mutate(Action::SetMeta(WindowMeta {
                            width: 600,
                            height: 534,
                            minimal_view: false,
                            ..meta
                        }));
                        state.write().mutate(Action::SidebarHidden(false));
                        state.write().mock_own_platform(Platform::Web);
                    }
                },
                Button {
                    aria_label: "computer-desktop-button".into(),
                    icon: Icon::ComputerDesktop,
                    appearance: Appearance::Transparent,
                    onpress: move |_| {
                        desktop.set_inner_size(LogicalSize::new(950.0, 600.0));
                        let meta = state.read().ui.metadata.clone();
                        state.write().mutate(Action::SetMeta(WindowMeta {
                            width: 950,
                            height: 600,
                            minimal_view: false,
                            ..meta
                        }));
                        state.write().mutate(Action::SidebarHidden(false));
                        state.write().mock_own_platform(Platform::Desktop);
                    }
                },
                Button {
                    aria_label: "command-line-button".into(),
                    icon: Icon::CommandLine,
                    appearance: Appearance::Transparent,
                    onpress: |_| {
                        desktop.devtool();
                    }
                }
            )),

            controls,

        },
    ))
}

fn get_call_dialog(_cx: Scope) -> Element {
    // CallDialog {
    //     caller: cx.render(rsx!(UserImage {
    //         platform: Platform::Mobile,
    //         status: Status::Online
    //     })),
    //     callee: cx.render(rsx!(UserImage {
    //         platform: Platform::Mobile,
    //         status: Status::Online
    //     })),
    //     description: "Call Description".into(),
    //     // with_accept_btn: cx.render(rsx! (
    //     //     Button {
    //     //         icon: Icon::Phone,
    //     //         appearance: Appearance::Success,
    //     //     }
    //     // )),
    //     with_deny_btn: cx.render(rsx! (
    //         Button {
    //             icon: Icon::PhoneXMark,
    //             appearance: Appearance::Danger,
    //             text: "End".into(),
    //         }
    //     )),
    // }
    None
}

fn get_router(cx: Scope) -> Element {
    let state = use_shared_state::<State>(cx)?;
    let pending_friends = state.read().friends().incoming_requests.len();

    let chat_route = UIRoute {
        to: UPLINK_ROUTES.chat,
        name: get_local_text("uplink.chats"),
        icon: Icon::ChatBubbleBottomCenterText,
        ..UIRoute::default()
    };
    let settings_route = UIRoute {
        to: UPLINK_ROUTES.settings,
        name: get_local_text("settings.settings"),
        icon: Icon::Cog6Tooth,
        ..UIRoute::default()
    };
    let friends_route = UIRoute {
        to: UPLINK_ROUTES.friends,
        name: get_local_text("friends.friends"),
        icon: Icon::Users,
        with_badge: if pending_friends > 0 {
            Some(pending_friends.to_string())
        } else {
            None
        },
        loading: None,
    };
    let files_route = UIRoute {
        to: UPLINK_ROUTES.files,
        name: get_local_text("files.files"),
        icon: Icon::Folder,
        ..UIRoute::default()
    };
    let routes = vec![
        chat_route.clone(),
        files_route.clone(),
        friends_route.clone(),
        settings_route.clone(),
    ];

    cx.render(rsx!(
        Router {
            Route {
                to: UPLINK_ROUTES.chat,
                ChatLayout {
                    route_info: RouteInfo {
                        routes: routes.clone(),
                        active: chat_route.clone(),
                    }
                }
            },
            Route {
                to: UPLINK_ROUTES.settings,
                SettingsLayout {
                    route_info: RouteInfo {
                        routes: routes.clone(),
                        active: settings_route.clone(),
                    }
                }
            },
            Route {
                to: UPLINK_ROUTES.friends,
                FriendsLayout {
                    route_info: RouteInfo {
                        routes: routes.clone(),
                        active: friends_route.clone(),
                    }
                }
            },
            Route {
                to: UPLINK_ROUTES.files,
                FilesLayout {
                    route_info: RouteInfo {
                        routes: routes.clone(),
                        active: files_route,
                    }
                }
            },
        }
    ))
}
