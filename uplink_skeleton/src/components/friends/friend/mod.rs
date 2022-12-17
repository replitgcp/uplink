use dioxus::prelude::*;
use fluent_templates::Loader;
use ui_kit::{
    components::{
        context_menu::ContextItem,
        context_menu::ContextMenu,
        indicator::{Platform, Status},
        user_image::UserImage,
    },
    elements::{
        button::Button,
        label::Label,
        tooltip::{ArrowPosition, Tooltip},
        Appearance,
    },
    icons::Icon,
};

use crate::{
    state::{Action, State},
    LOCALES, US_ENGLISH,
};

#[derive(Props)]
pub struct Props<'a> {
    // The username of the friend request sender
    username: String,
    // A suffix to the username, typically a unique identifier
    suffix: String,
    // The user image element to display
    user_image: Element<'a>,
    // An optional event handler for the "onchat" event
    #[props(optional)]
    onchat: Option<EventHandler<'a>>,
    // An optional event handler for the "onremove" event
    #[props(optional)]
    onremove: Option<EventHandler<'a>>,
    #[props(optional)]
    onaccept: Option<EventHandler<'a>>,
    // An optional event handler for the "onblock" event
    #[props(optional)]
    _onblock: Option<EventHandler<'a>>,
}

#[allow(non_snake_case)]
pub fn Friend<'a>(cx: Scope<'a, Props<'a>>) -> Element<'a> {
    let chat_text = LOCALES
        .lookup(&US_ENGLISH, "uplink.chat")
        .unwrap_or_default();
    let more_text = LOCALES
        .lookup(&US_ENGLISH, "uplink.more")
        .unwrap_or_default();
    let remove_text = LOCALES
        .lookup(&US_ENGLISH, "friends.remove")
        .unwrap_or_default();
    let deny_text = LOCALES
        .lookup(&US_ENGLISH, "friends.deny")
        .unwrap_or_default();
    let accept_text = LOCALES
        .lookup(&US_ENGLISH, "friends.accept")
        .unwrap_or_default();

    cx.render(rsx!(
        div {
            class: "friend",
            &cx.props.user_image,
            div {
                class: "request-info",
                p {
                    "{cx.props.username}",
                    span {
                        "#{cx.props.suffix}"
                    }
                },
                Label {
                    // TODO: this is stubbed for now, wire up to the actual request time
                    text: "Requested 4 days ago.".into()
                }
            },
            div {
                class: "request-controls",
                cx.props.onaccept.is_some().then(|| rsx!(
                    Button {
                        icon: Icon::Check,
                        text: accept_text,
                        onpress: move |_| match &cx.props.onaccept {
                            Some(f) => f.call(()),
                            None    => {},
                        }
                    }
                )),
                cx.props.onchat.is_some().then(|| rsx! (
                    Button {
                        icon: Icon::ChatBubbleBottomCenterText,
                        text: chat_text,
                        onpress: move |_| match &cx.props.onchat {
                            Some(f) => f.call(()),
                            None    => {},
                        }
                    }
                )),
                Button {
                    icon: Icon::XMark,
                    appearance: Appearance::Secondary,
                    onpress: move |_| match &cx.props.onremove {
                        Some(f) => f.call(()),
                        None    => {},
                    }
                    tooltip: cx.render(rsx!(
                        Tooltip {
                            arrow_position: ArrowPosition::Right,
                            text: if cx.props.onaccept.is_none() { remove_text } else { deny_text }
                        }
                    )),
                },
                cx.props.onchat.is_some().then(|| rsx!(
                    Button {
                        icon: Icon::EllipsisVertical,
                        appearance: Appearance::Secondary,
                        onpress: move |_| {},
                        tooltip: cx.render(rsx!(
                            Tooltip {
                                arrow_position: ArrowPosition::Right,
                                text: more_text
                            }
                        )),
                    }
                ))
            }
        }
    ))
}

#[allow(non_snake_case)]
pub fn Friends(cx: Scope) -> Element {
    let state: UseSharedState<State> = use_context::<State>(&cx).unwrap();
    let friends_list = state.read().friends.all.clone();
    let friends = State::get_friends_by_first_letter(friends_list);

    let friends_text = LOCALES.lookup(&US_ENGLISH, "friends").unwrap_or_default();

    cx.render(rsx! (
        div {
            class: "friends-list",
            Label {
                text: friends_text,
            },
            friends.into_iter().map(|(letter, sorted_friends)| {
                let group_letter = letter.to_string();
                rsx!(
                    div {
                        key: "friend-group-{group_letter}",
                        Label {
                            text: letter.into(),
                        },
                        sorted_friends.into_iter().map(|friend| {
                            let did = friend.did_key().clone();
                            let did_suffix: String = did.to_string().chars().rev().take(6).collect();
                            let chat_with_friend = state.read().get_chat_with_friend(&friend.clone());
                            let chat_with_friend_context = state.read().get_chat_with_friend(&friend.clone());
                            let remove_friend = friend.clone();
                            let remove_friend_2 = remove_friend.clone();

                            let call_text = LOCALES
                                .lookup(&US_ENGLISH, "uplink.call")
                                .unwrap_or_default();
                            let chat_text = LOCALES
                                .lookup(&US_ENGLISH, "uplink.chat")
                                .unwrap_or_default();
                            let favorite_text = LOCALES
                                .lookup(&US_ENGLISH, "favorites")
                                .unwrap_or_default();
                            let remove_text = LOCALES
                                .lookup(&US_ENGLISH, "uplink.remove")
                                .unwrap_or_default();
                            let block_test = LOCALES
                                .lookup(&US_ENGLISH, "friends.block")
                                .unwrap_or_default();

                            rsx!(
                                ContextMenu {
                                    id: format!("{}-friend-listing", did),
                                    key: "{did}-friend-listing",
                                    items: cx.render(rsx!(
                                        ContextItem {
                                            icon: Icon::ChatBubbleBottomCenterText,
                                            text: chat_text,
                                            onpress: move |_| {
                                                let _ = &state.write().mutate(Action::ChatWith(chat_with_friend_context.clone()));
                                                use_router(&cx).replace_route("/", None, None);
                                            }
                                        },
                                        ContextItem {
                                            icon: Icon::PhoneArrowUpRight,
                                            text: call_text,
                                            // TODO: Wire this up to state
                                        },
                                        ContextItem {
                                            icon: Icon::Heart,
                                            text: favorite_text,
                                            // TODO: Wire this up to state
                                        },
                                        hr{}
                                        ContextItem {
                                            danger: true,
                                            icon: Icon::XMark,
                                            text: remove_text,
                                            onpress: move |_| {
                                                let _ = &state.write().mutate(Action::RemoveFriend(remove_friend.clone()));
                                            }
                                        },
                                        ContextItem {
                                            danger: true,
                                            icon: Icon::NoSymbol,
                                            text: block_test,
                                            // TODO: Wire this up to state
                                        },
                                    )),
                                    Friend {
                                        username: friend.username(),
                                        suffix: did_suffix,
                                        user_image: cx.render(rsx! (
                                            UserImage {
                                                platform: Platform::Desktop,
                                                status: Status::Online,
                                                image: friend.graphics().profile_picture()
                                            }
                                        )),
                                        onchat: move |_| {
                                            let _ = &state.write().mutate(Action::ChatWith(chat_with_friend.clone()));
                                            use_router(&cx).replace_route("/", None, None);
                                        },
                                        onremove: move |_| {
                                            let _ = &state.write().mutate(Action::RemoveFriend(remove_friend_2.clone()));
                                        }
                                    }
                                }
                            )
                        })
                    }
                )
            })
        }
    ))
}

#[allow(non_snake_case)]
pub fn PendingFriends(cx: Scope) -> Element {
    let state: UseSharedState<State> = use_context::<State>(&cx).unwrap();
    let friends_list = state.read().friends.incoming_requests.clone();

    let requests_text = LOCALES
        .lookup(&US_ENGLISH, "friends.incoming_requests")
        .unwrap_or_default();

    cx.render(rsx! (
            div {
                class: "friends-list",
                Label {
                    text: requests_text,
                },
                friends_list.into_iter().map(|friend| {
                    let did = friend.did_key().clone();
                    let did_suffix: String = did.to_string().chars().rev().take(6).collect();

                    let deny_text = LOCALES
                        .lookup(&US_ENGLISH, "friends.deny")
                        .unwrap_or_default();

                    rsx!(
                        ContextMenu {
                            id: format!("{}-friend-listing", did),
                            key: "{did}-friend-listing",
                            items: cx.render(rsx!(
                                ContextItem {
                                    danger: true,
                                    icon: Icon::XMark,
                                    text: deny_text,
                                    onpress: move |_| {} // TODO:
                                },
                            )),
                            Friend {
                                username: friend.username(),
                                suffix: did_suffix,
                                user_image: cx.render(rsx! (
                                    UserImage {
                                        platform: Platform::Desktop,
                                        status: Status::Online,
                                        image: friend.graphics().profile_picture()
                                    }
                                )),
                                onaccept: move |_| {
    // TODO:
                                },
                                onremove: move |_| {
    // TODO::
                                }
                            }
                        }
                    )
                })
            }
        ))
}

#[allow(non_snake_case)]
pub fn OutgoingRequests(cx: Scope) -> Element {
    let state: UseSharedState<State> = use_context::<State>(&cx).unwrap();
    let friends_list = state.read().friends.outgoing_requests.clone();

    let requests_text = LOCALES
        .lookup(&US_ENGLISH, "friends.outgoing_requests")
        .unwrap_or_default();

    cx.render(rsx! (
        div {
            class: "friends-list",
            Label {
                text: requests_text,
            },
            friends_list.into_iter().map(|friend| {
                let did = friend.did_key().clone();
                let did_suffix: String = did.to_string().chars().rev().take(6).collect();

                let cancel_text = LOCALES
                    .lookup(&US_ENGLISH, "friends.cancel")
                    .unwrap_or_default();
                rsx!(
                    ContextMenu {
                        id: format!("{}-friend-listing", did),
                        key: "{did}-friend-listing",
                        items: cx.render(rsx!(
                            ContextItem {
                                danger: true,
                                icon: Icon::XMark,
                                text: cancel_text,
                                onpress: move |_| {} // TODO:
                            },
                        )),
                        Friend {
                            username: friend.username(),
                            suffix: did_suffix,
                            user_image: cx.render(rsx! (
                                UserImage {
                                    platform: Platform::Desktop,
                                    status: Status::Online,
                                    image: friend.graphics().profile_picture()
                                }
                            )),
                            onremove: move |_| {} // TODO:
                        }
                    }
                )
            })
        }
    ))
}

#[allow(non_snake_case)]
pub fn BlockedUsers(cx: Scope) -> Element {
    let state: UseSharedState<State> = use_context::<State>(&cx).unwrap();
    let block_list = state.read().friends.blocked.clone();

    let blocked_text = LOCALES
        .lookup(&US_ENGLISH, "friends.blocked")
        .unwrap_or_default();

    cx.render(rsx! (
        div {
            class: "friends-list",
            Label {
                text: blocked_text,
            },
            block_list.into_iter().map(|blocked_user| {
                let did = blocked_user.did_key().clone();
                let did_suffix: String = did.to_string().chars().rev().take(6).collect();

                let unblock_text = LOCALES
                    .lookup(&US_ENGLISH, "friends.unblock")
                    .unwrap_or_default();
                rsx!(
                    ContextMenu {
                        id: format!("{}-friend-listing", did),
                        key: "{did}-friend-listing",
                        items: cx.render(rsx!(
                            ContextItem {
                                danger: true,
                                icon: Icon::XMark,
                                text: unblock_text,
                                onpress: move |_| {} // TODO:
                            },
                        )),
                        Friend {
                            username: blocked_user.username(),
                            suffix: did_suffix,
                            user_image: cx.render(rsx! (
                                UserImage {
                                    platform: Platform::Desktop,
                                    status: Status::Online,
                                    image: blocked_user.graphics().profile_picture()
                                }
                            )),
                            onremove: move |_| {} // TODO:
                        }
                    }
                )
            })
        }
    ))
}