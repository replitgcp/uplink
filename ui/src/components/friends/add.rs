use arboard::Clipboard;
use std::str::FromStr;

use common::language::get_local_text;
use dioxus::prelude::*;
use futures::{channel::oneshot, StreamExt};
use kit::elements::{
    button::Button,
    input::{Input, Options, Validation},
    label::Label,
};
use warp::error::Error;
use warp::{crypto::DID, logging::tracing::log};

use common::icons::outline::Shape as Icon;
use common::{
    state::{Action, Identity, State, ToastNotification},
    warp_runner::{MultiPassCmd, WarpCmd},
    STATIC_ARGS, WARP_CMD_CH,
};

#[allow(non_snake_case)]
pub fn AddFriend(cx: Scope) -> Element {
    let state = use_shared_state::<State>(cx)?;
    let clear_input = use_state(cx, || false);
    let friend_input = use_state(cx, String::new);
    let friend_input_valid = use_state(cx, || false);
    let request_sent = use_state(cx, || false);
    let errors_toast = use_state(cx, || Error::CannotSendSelfFriendRequest);
    // used when copying the user's id to the clipboard
    let my_id: &UseState<Option<String>> = use_state(cx, || None);
    // Set up validation options for the input field
    let friend_validation = Validation {
        max_length: Some(56),
        min_length: Some(56),
        alpha_numeric_only: true,
        no_whitespace: true,
        ignore_colons: true,
        special_chars: None,
    };

    if *request_sent.get() {
        state
            .write()
            .mutate(Action::AddToastNotification(ToastNotification::init(
                "".into(),
                get_local_text("friends.request-sent"),
                None,
                5,
            )));
        request_sent.set(false);
    }

    if let Some(id) = my_id.get().clone() {
        let mut clipboard = Clipboard::new().unwrap();
        clipboard.set_text(id).unwrap();
        state
            .write()
            .mutate(Action::AddToastNotification(ToastNotification::init(
                "".into(),
                get_local_text("friends.copied-did"),
                None,
                5,
            )));
        my_id.set(None);
    }

    let gettoast = move || {
        state
            .write()
            .mutate(Action::AddToastNotification(ToastNotification::init(
                "".into(),
                get_local_text("friends.copied-did"),
                None,
                5,
            )));
    };

    let ch = use_coroutine(cx, |mut rx: UnboundedReceiver<DID>| {
        to_owned![request_sent];
        async move {
            let warp_cmd_tx = WARP_CMD_CH.tx.clone();
            while let Some(did) = rx.next().await {
                let (tx, rx) = oneshot::channel::<Result<(), warp::error::Error>>();
                if let Err(e) = warp_cmd_tx.send(WarpCmd::MultiPass(MultiPassCmd::RequestFriend {
                    did,
                    rsp: tx,
                })) {
                    log::error!("failed to send warp command: {}", e);
                    continue;
                }

                let res = rx.await.expect("failed to get response from warp_runner");
                match res {
                    Ok(_) | Err(Error::FriendRequestExist) => {
                        request_sent.set(true);
                    }
                    Err(e) => match e {
                        Error::CannotSendFriendRequest
                        | Error::IdentityDoesntExist
                        | Error::BlockedByUser
                        | Error::InvalidIdentifierCondition
                        | Error::CannotSendSelfFriendRequest => {
                            log::warn!("add cannot add self: {}", e);
                        }
                        Error::PublicKeyIsBlocked => {
                            log::warn!("add friend failed: {}", e);
                        }
                        _ => {
                            log::error!("add friend failed: {}", e);
                        }
                    },
                }
            }
        }
    });

    let id_ch = use_coroutine(cx, |mut rx: UnboundedReceiver<()>| {
        to_owned![my_id];
        async move {
            let warp_cmd_tx = WARP_CMD_CH.tx.clone();
            while rx.next().await.is_some() {
                let (tx, rx) = oneshot::channel::<Result<DID, warp::error::Error>>();
                if let Err(e) =
                    warp_cmd_tx.send(WarpCmd::MultiPass(MultiPassCmd::GetOwnDid { rsp: tx }))
                {
                    log::error!("failed to send warp command: {}", e);
                    continue;
                }

                let res = rx.await.expect("failed to get response from warp_runner");
                match res {
                    Ok(did) => my_id.set(Some(did.to_string())),
                    Err(e) => log::error!("get own did failed: {}", e),
                }
            }
        }
    });

    // let error_ch = use_future(cx, (), |_| {
    //     to_owned![errors_toast];
    //     // loop {
    //     println!("{}", errors_toast);

    //     // }
    // });

    cx.render(rsx!(
        div {
            class: "add-friend",
            Label {
                text: get_local_text("friends.add"),
            },
            div {
                class: "body",
                Input {
                    placeholder: get_local_text("friends.placeholder"),
                    icon: Icon::MagnifyingGlass,
                    options: Options {
                        with_validation: Some(friend_validation),
                        // Do not replace spaces with underscores
                        replace_spaces_underscore: false,
                        // Show a clear button inside the input field
                        with_clear_btn: true,
                        // Use the default options for the remaining fields
                        ..Options::default()
                    },
                    reset: clear_input.clone(),
                    onreturn: move |_| {
                        match DID::from_str(friend_input.get()) {
                            Ok(did) => {
                                if STATIC_ARGS.use_mock {
                                    let mut ident = Identity::default();
                                    ident.set_did_key(did);
                                    state.write().mutate(Action::SendRequest(ident));
                                } else {
                                    ch.send(did);
                                }
                            },
                            Err(e) => {
                                println!("herer");
                                state
                                .write()
                                .mutate(Action::AddToastNotification(ToastNotification::init(
                                    "".into(),
                                    get_local_text("friends.copied-did"),
                                    None,
                                    5,
                                )));
                                log::error!("could not get did from str: {}", e);
                            }
                        }
                    },
                    onchange: |(s, is_valid)| {
                        friend_input.set(s);
                        friend_input_valid.set(is_valid);
                    },
                    aria_label: "Add Someone Input".into()
                },
                Button {
                    icon: Icon::Plus,
                    text: get_local_text("uplink.add"),
                    disabled: !friend_input_valid.get(),
                    onpress: move |_| {
                        match DID::from_str(friend_input.get()) {
                            Ok(did) => {
                                if STATIC_ARGS.use_mock {
                                    let mut ident = Identity::default();
                                    ident.set_did_key(did);
                                    state.write().mutate(Action::SendRequest(ident));
                                } else {
                                    ch.send(did);
                                }
                            },
                            Err(e) => {
                                println!("herer");
                                // if e == Error::CannotSendSelfFriendRequest {
                                    state.write().mutate(Action::AddToastNotification(
                                        ToastNotification::init(
                                            "".into(),
                                            get_local_text("friends.copied-did"),
                                            None,
                                            5,
                                        ),
                                    ));
                                // }
                                log::error!("could not get did from str: {}", e);
                            }
                        }
                        clear_input.set(true);
                    },
                    aria_label: "Add Someone Button".into()
                },
                Button {
                    aria_label: "Copy ID".into()
                    icon: Icon::ClipboardDocument,
                    onpress: move |_| {
                        id_ch.send(());
                    }
                }
            }
        }
    ))
}
