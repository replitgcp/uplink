use std::{
    collections::{HashMap, HashSet},
    ffi::OsStr,
    path::PathBuf,
    rc::Rc,
};

use dioxus::prelude::{EventHandler, *};

use futures::StreamExt;

use kit::components::{
    context_menu::{ContextItem, ContextMenu},
    indicator::Status,
    message::{Message, Order, ReactionAdapter},
    message_group::{MessageGroup, MessageGroupSkeletal},
    message_reply::MessageReply,
    user_image::UserImage,
};

use common::{
    icons::outline::Shape as Icon,
    icons::Icon as IconElement,
    language::get_local_text_args_builder,
    state::{group_messages, GroupedMessage, MessageGroup},
    warp_runner::ui_adapter::{self},
};
use common::{
    state::{Action, Identity, State},
    warp_runner::{RayGunCmd, WarpCmd},
    WARP_CMD_CH,
};

use common::language::get_local_text;
use dioxus_desktop::use_eval;
use rfd::FileDialog;

use uuid::Uuid;
use warp::{
    crypto::DID,
    logging::tracing::log,
    multipass::identity::IdentityStatus,
    raygun::{self, ReactionState},
};

use crate::utils::format_timestamp::format_timestamp_timeago;

const SETUP_CONTEXT_PARENT: &str = r#"
    const right_clickable = document.getElementsByClassName("has-context-handler")
    console.log("E", right_clickable)
    for (var i = 0; i < right_clickable.length; i++) {
        //Disable default right click actions (opening the inspect element dropdown)
        right_clickable.item(i).addEventListener("contextmenu",
        function (ev) {
        ev.preventDefault()
        })
    }
"#;

#[allow(clippy::large_enum_variant)]
enum MessagesCommand {
    React((DID, raygun::Message, String)),
    DeleteMessage {
        conv_id: Uuid,
        msg_id: Uuid,
    },
    DownloadAttachment {
        conv_id: Uuid,
        msg_id: Uuid,
        file: warp::constellation::file::File,
        file_path_to_download: PathBuf,
    },
    EditMessage {
        conv_id: Uuid,
        msg_id: Uuid,
        msg: Vec<String>,
    },
    FetchMore {
        conv_id: Uuid,
        new_len: usize,
        current_len: usize,
    },
}

type DownloadTracker = HashMap<Uuid, HashSet<warp::constellation::file::File>>;

/// Lazy loading scheme:
/// load DEFAULT_NUM_TO_TAKE messages to start.
/// tell group_messages to flag the first X messages.
/// if onmouseout triggers over any of those messages, load Y more.
const DEFAULT_NUM_TO_TAKE: usize = 20;
#[inline_props]
pub fn get_messages(cx: Scope, data: Rc<super::ComposeData>) -> Element {
    log::trace!("get_messages");
    use_shared_state_provider(cx, || -> DownloadTracker { HashMap::new() });
    let state = use_shared_state::<State>(cx)?;
    let pending_downloads = use_shared_state::<DownloadTracker>(cx)?;
    let eval = use_eval(cx);

    let num_to_take = use_state(cx, || DEFAULT_NUM_TO_TAKE);
    let prev_chat_id = use_ref(cx, || data.active_chat.id);
    let newely_fetched_messages: &UseRef<Option<(Uuid, Vec<ui_adapter::Message>)>> =
        use_ref(cx, || None);

    let quick_profile_uuid = &*cx.use_hook(|| Uuid::new_v4().to_string());
    let identity_profile = use_state(cx, Identity::default);
    let update_script = use_state(cx, String::new);
    let edit_msg: &UseState<Option<Uuid>> = use_state(cx, || None);
    let reacting_to: &UseState<Option<Uuid>> = use_state(cx, || None);

    let reactions = ["❤️", "😂", "😍", "💯", "👍", "😮", "😢", "😡", "🤔", "😎"];
    let own_did = state.read().did_key();
    let focus_script = r#"
            var message_reactions_container = document.getElementById('add-message-reaction');
            message_reactions_container.focus();
        "#;

    if let Some((id, m)) = newely_fetched_messages.write_silent().take() {
        if m.is_empty() {
            log::debug!("finished loading chat: {id}");
            state.write().finished_loading_chat(id);
        } else {
            num_to_take.with_mut(|x| *x = x.saturating_add(m.len()));
            state.write().prepend_messages_to_chat(id, m);
        }
    }

    // this needs to be a hook so it can change inside of the use_future.
    // it could be passed in as a dependency but then the wait would reset every time a message comes in.
    let max_to_take = use_ref(cx, || data.active_chat.messages.len());
    if *max_to_take.read() != data.active_chat.messages.len() {
        *max_to_take.write_silent() = data.active_chat.messages.len();
    }

    // don't scroll to the bottom again if new messages come in while the user is scrolling up. only scroll
    // to the bottom when the user selects the active chat
    // also must reset num_to_take when the active_chat changes
    let active_chat = use_ref(cx, || None);
    let currently_active = Some(data.active_chat.id);
    let eval = use_eval(cx);
    if *active_chat.read() != currently_active {
        *active_chat.write_silent() = currently_active;
        num_to_take.set(DEFAULT_NUM_TO_TAKE);
    }

    use_effect(cx, &data.active_chat.id, |id| {
        to_owned![eval, prev_chat_id];
        async move {
            // yes, this check seems like some nonsense. but it eliminates a jitter and if
            // switching out of the chats view ever gets fixed, it would let you scroll up in the active chat,
            // switch to settings or whatnot, then come back to the chats view and not lose your place.
            if *prev_chat_id.read() != id {
                *prev_chat_id.write_silent() = id;
                let script = include_str!("../scroll_to_bottom.js");
                eval(script.to_string());
            }
            eval(SETUP_CONTEXT_PARENT.to_string());
        }
    });

    let ch = use_coroutine(cx, |mut rx: UnboundedReceiver<MessagesCommand>| {
        to_owned![newely_fetched_messages, pending_downloads];
        async move {
            let warp_cmd_tx = WARP_CMD_CH.tx.clone();
            while let Some(cmd) = rx.next().await {
                match cmd {
                    MessagesCommand::React((user, message, emoji)) => {
                        let (tx, rx) = futures::channel::oneshot::channel();
                        let reaction_state =
                            match message.reactions().iter().find(|x| x.emoji() == emoji) {
                                Some(reaction) if reaction.users().contains(&user) => {
                                    ReactionState::Remove
                                }
                                _ => ReactionState::Add,
                            };
                        if let Err(e) = warp_cmd_tx.send(WarpCmd::RayGun(RayGunCmd::React {
                            conversation_id: message.conversation_id(),
                            message_id: message.id(),
                            reaction_state,
                            emoji,
                            rsp: tx,
                        })) {
                            log::error!("failed to send warp command: {}", e);
                            continue;
                        }

                        let res = rx.await.expect("command canceled");
                        if res.is_err() {
                            // failed to add/remove reaction
                        }
                    }
                    MessagesCommand::DeleteMessage { conv_id, msg_id } => {
                        let (tx, rx) = futures::channel::oneshot::channel();
                        if let Err(e) =
                            warp_cmd_tx.send(WarpCmd::RayGun(RayGunCmd::DeleteMessage {
                                conv_id,
                                msg_id,
                                rsp: tx,
                            }))
                        {
                            log::error!("failed to send warp command: {}", e);
                            continue;
                        }

                        let res = rx.await.expect("command canceled");
                        if let Err(e) = res {
                            log::error!("failed to delete message: {}", e);
                        }
                    }
                    MessagesCommand::DownloadAttachment {
                        conv_id,
                        msg_id,
                        file,
                        file_path_to_download,
                    } => {
                        let (tx, rx) = futures::channel::oneshot::channel();
                        if let Err(e) =
                            warp_cmd_tx.send(WarpCmd::RayGun(RayGunCmd::DownloadAttachment {
                                conv_id,
                                msg_id,
                                file_name: file.name(),
                                file_path_to_download,
                                rsp: tx,
                            }))
                        {
                            log::error!("failed to send warp command: {}", e);
                            if let Some(conv) = pending_downloads.write().get_mut(&conv_id) {
                                conv.remove(&file);
                            }
                            continue;
                        }

                        let res = rx.await.expect("command canceled");
                        match res {
                            Ok(mut stream) => {
                                while let Some(p) = stream.next().await {
                                    log::debug!("{p:?}");
                                }
                            }
                            Err(e) => {
                                log::error!("failed to download attachment: {}", e);
                            }
                        }
                        if let Some(conv) = pending_downloads.write().get_mut(&conv_id) {
                            conv.remove(&file);
                        }
                    }
                    MessagesCommand::EditMessage {
                        conv_id,
                        msg_id,
                        msg,
                    } => {
                        let (tx, rx) = futures::channel::oneshot::channel();
                        if let Err(e) = warp_cmd_tx.send(WarpCmd::RayGun(RayGunCmd::EditMessage {
                            conv_id,
                            msg_id,
                            msg,
                            rsp: tx,
                        })) {
                            log::error!("failed to send warp command: {}", e);
                            continue;
                        }

                        let res = rx.await.expect("command canceled");
                        if let Err(e) = res {
                            log::error!("failed to edit message: {}", e);
                        }
                    }
                    MessagesCommand::FetchMore {
                        conv_id,
                        new_len,
                        current_len,
                    } => {
                        let (tx, rx) = futures::channel::oneshot::channel();
                        if let Err(e) =
                            warp_cmd_tx.send(WarpCmd::RayGun(RayGunCmd::FetchMessages {
                                conv_id,
                                new_len,
                                current_len,
                                rsp: tx,
                            }))
                        {
                            log::error!("failed to send warp command: {}", e);
                            continue;
                        }

                        match rx.await.expect("command canceled") {
                            Ok(m) => {
                                newely_fetched_messages.set(Some((conv_id, m)));
                            }
                            Err(e) => {
                                log::error!("failed to fetch more message: {}", e);
                            }
                        }
                    }
                }
            }
        }
    });

    let msg_container_end = if data.active_chat.has_more_messages {
        rsx!(MessageGroupSkeletal {}, MessageGroupSkeletal { alt: true })
    } else {
        rsx!(
            div {
                // key: "encrypted-notification-0001",
                class: "msg-container-end",
                aria_label: "messages-secured-alert",
                IconElement {
                    icon:  Icon::LockClosed,
                },
                p {
                    get_local_text("messages.msg-banner")
                }
            }
        )
    };

    let mut remaining_messages = DEFAULT_NUM_TO_TAKE;
    let to_skip = data
        .active_chat
        .messages
        .len()
        .saturating_sub(*num_to_take.get());
    let iter = data.active_chat.messages.iter().skip(to_skip);

    let messages = cx.render(rsx!(iter.map(|message| {
        let is_remote = message.inner.sender() != own_did;
        let is_editing = edit_msg
            .get()
            .map(|id| id == message.inner.id() && !is_remote)
            .unwrap_or(false);
        let message_key = format!("{}-msg-key", message.key);
        let message_key2 = message_key.clone();
        let context_key = format!("context-key-{}-{}", &message.key, is_editing);
        let should_fetch_more = remaining_messages == 0;
        remaining_messages = remaining_messages.saturating_sub(1);

        let reactions_list: Vec<ReactionAdapter> = message
            .inner
            .reactions()
            .iter()
            .map(|x| {
                let users = x.users();
                let user_names: Vec<String> = users
                    .iter()
                    .filter_map(|id| state.read().get_identity(id).map(|x| x.username()))
                    .collect();
                ReactionAdapter {
                    emoji: x.emoji(),
                    reaction_count: users.len(),
                    self_reacted: users.iter().any(|x| x == &own_did),
                    alt: user_names.join(", "),
                }
            })
            .collect();
        
        let is_remote =  message.inner.sender() != own_did; 
        let remote_class = if !is_remote { "" } else { "remote" };
        let reactions_class = format!("message-reactions-container {remote_class}");
        let reaction_picker =  cx.render(rsx!(
            (*reacting_to.current() == Some(message.inner.id())).then(|| {
                rsx!(
                    div {
                        id: "add-message-reaction",
                        class: "{reactions_class} pointer",
                        tabindex: "0",
                        onmouseleave: |_| {
                            #[cfg(not(target_os = "macos"))] 
                            {
                                eval(focus_script.to_string());
                            }
                        },
                        onblur: move |_| {
                            state.write().ui.ignore_focus = false;
                            reacting_to.set(None);
                        },
                        reactions.iter().cloned().map(|reaction| {
                            rsx!(
                                div {
                                    onclick: move |_|  {
                                        reacting_to.set(None);
                                        state.write().ui.ignore_focus = false;
                                        ch.send(MessagesCommand::React((state.read().did_key(), message.inner.clone(), reaction.to_string())));
                                    },
                                    "{reaction}"
                                }
                            )
                        })
                    },
                    script { focus_script },
                )
            })
        ));
        let rendered_message = rsx!(Message {
            id: message_key.clone(),
            key: "{message_key}",
            editing: is_editing,
            remote: is_remote,
            with_text: message.inner.value().join("\n"),
            reactions: reactions_list,
            // todo: calculate order on the fly
            order: Order::Middle,
            attachments: message.inner.attachments(),
            attachments_pending_download: pending_downloads
                .read()
                .get(&message.inner.conversation_id())
                .cloned(),
            on_click_reaction: move |emoji: String| {
                ch.send(MessagesCommand::React((
                    state.read().did_key(),
                    message.inner.clone(),
                    emoji,
                )));
            },
            parse_markdown: true,
            on_download: move |file: warp::constellation::file::File| {
                let file_name = file.name();
                let file_extension = std::path::Path::new(&file_name)
                    .extension()
                    .and_then(OsStr::to_str)
                    .map(|s| s.to_string())
                    .unwrap_or_default();
                let file_stem = PathBuf::from(&file_name)
                    .file_stem()
                    .and_then(OsStr::to_str)
                    .map(str::to_string)
                    .unwrap_or_default();
                if let Some(file_path_to_download) = FileDialog::new()
                    .set_directory(dirs::download_dir().unwrap_or_default())
                    .set_file_name(&file_stem)
                    .add_filter("", &[&file_extension])
                    .save_file()
                {
                    let conv_id = message.inner.conversation_id();
                    if !pending_downloads.read().contains_key(&conv_id) {
                        pending_downloads.write().insert(conv_id, HashSet::new());
                    }
                    pending_downloads
                        .write()
                        .get_mut(&conv_id)
                        .map(|conv| conv.insert(file.clone()));

                    ch.send(MessagesCommand::DownloadAttachment {
                        conv_id,
                        msg_id: message.inner.id(),
                        file,
                        file_path_to_download,
                    })
                }
            },
            on_edit: move |update: String| {
                edit_msg.set(None);
                let msg = update
                    .split('\n')
                    .map(|x| x.to_string())
                    .collect::<Vec<String>>();
                if message.inner.value() == msg || !msg.iter().any(|x| !x.trim().is_empty()) {
                    return;
                }
                ch.send(MessagesCommand::EditMessage {
                    conv_id: message.inner.conversation_id(),
                    msg_id: message.inner.id(),
                    msg,
                })
            }
        },); // end of rsx!(Message

        let own_did2 = own_did.clone();
        let wrapped_message = rsx!(
            reaction_picker,
            div {
                class: "msg-wrapper",
                message.in_reply_to.as_ref().map(|(other_msg, other_msg_attachments, sender_did)| rsx!(
                    MessageReply {
                            key: "reply-{message_key2}",
                            with_text: other_msg.to_string(),
                            with_attachments: other_msg_attachments.clone(),
                            remote: is_remote,
                            remote_message: is_remote,
                            sender_did: sender_did.clone(),
                            replier_did: own_did2,
                        }
                    )),
                rendered_message,
            }
        );

        rsx!(ContextMenu {
            key: "{context_key}",
            id: context_key,
            on_mouseenter: move |_| {
                if should_fetch_more {
                    let new_num_to_take = num_to_take.get().saturating_add(DEFAULT_NUM_TO_TAKE * 2);
                    // lazily render
                    if new_num_to_take < data.active_chat.messages.len() {
                        num_to_take.set(new_num_to_take);
                    } else if data.active_chat.messages.len() > *num_to_take.get() {
                        // lazily add more messages to conversation, then render
                        ch.send(MessagesCommand::FetchMore {
                            conv_id: data.active_chat.id,
                            new_len: new_num_to_take,
                            current_len: *num_to_take.get(),
                        })
                    }
                }
            },
            children: cx.render(rsx!(wrapped_message)),
            items: cx.render(rsx!(
                ContextItem {
                    icon: Icon::ArrowLongLeft,
                    text: get_local_text("messages.reply"),
                    onpress: move |_| {
                        state
                            .write()
                            .mutate(Action::StartReplying(&data.active_chat.id, message));
                    }
                },
                ContextItem {
                    icon: Icon::FaceSmile,
                    text: get_local_text("messages.react"),
                    onpress: move |_| {
                        //reacting_to.set(Some(_msg_uuid));
                    }
                },
                ContextItem {
                    icon: Icon::Pencil,
                    text: get_local_text("messages.edit"),
                    should_render: !is_remote
                        && edit_msg
                            .get()
                            .map(|id| id != message.inner.id())
                            .unwrap_or(true),
                    onpress: move |_| {
                        edit_msg.set(Some(message.inner.id()));
                    }
                },
                ContextItem {
                    icon: Icon::Pencil,
                    text: get_local_text("messages.cancel-edit"),
                    should_render: !is_remote
                        && edit_msg
                            .get()
                            .map(|id| id == message.inner.id())
                            .unwrap_or(false),
                    onpress: move |_| {
                        edit_msg.set(None);
                    }
                },
                ContextItem {
                    icon: Icon::Trash,
                    danger: true,
                    text: get_local_text("uplink.delete"),
                    should_render: !is_remote,
                    onpress: move |_| {
                        ch.send(MessagesCommand::DeleteMessage {
                            conv_id: message.inner.conversation_id(),
                            msg_id: message.inner.id(),
                        });
                    }
                },
            )) // end of context menu items
        }) // end context menu
    }))); // end of cx.render(rsx!(iter.map(|message| {

    cx.render(rsx!(
        div {
            id: "messages",
            span {
                rsx!(
                    msg_container_end,
                    messages,
                )
            },
            script {
                r#"
                (() => {{
                    Prism.highlightAll();
                }})();
                "#
            }
        },
        super::quick_profile::QuickProfileContext{
            id: quick_profile_uuid,
            update_script: update_script,
            identity: identity_profile
        }
    ))
}
