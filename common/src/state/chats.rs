use std::{
    collections::{HashMap, HashSet, VecDeque},
    time::Instant,
};

use serde::{ser::SerializeStruct, Deserialize, Serialize, Serializer};
use uuid::Uuid;
use warp::{crypto::DID, raygun};

use crate::{warp_runner::ui_adapter, STATIC_ARGS};

// let (p = window_bottom) be an index into Chat.messages
// show messages from (p - window_size) to (p + window_extra)
// scroll up by window_extra (this allows an onmouseout event to trigger)
// pub struct ChatView {
//     // the idx of the message on the bottom of the screen
//     message_idx: usize,
//     // the number of messages to render in the window
//     window_size: usize,
//     // the number of messages to add outside the window, for scrolling purposes
//     window_extra: usize,
// }

// warning: Chat implements Serialize
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct Chat {
    // Warp generated UUID of the chat
    // TODO: This should be wired up to warp conversation id's
    pub id: Uuid,
    // Includes the list of participants within a given chat.
    // these don't need to be stored in state either
    pub participants: HashSet<DID>,
    // Messages should only contain messages we want to render. Do not include the entire message history.
    // don't store the actual message in state
    // warn: Chat has a custom serialize method which skips this field when not using mock data.
    #[serde(default)]
    pub messages: VecDeque<ui_adapter::Message>,
    // Unread count for this chat, should be cleared when we view the chat.
    pub unreads: u32,
    // If a value exists, we will render the message we're replying to above the chatbar
    #[serde(skip)]
    pub replying_to: Option<raygun::Message>,
    // list of users currently typing.
    // (user id, last update time)
    #[serde(skip)]
    pub typing_indicator: HashMap<DID, Instant>,
}

// warning: Chats implements Serialize
#[derive(Clone, Debug, Default, Deserialize)]
pub struct Chats {
    #[serde(skip)]
    pub initialized: bool,
    // All active chats from warp.
    pub all: HashMap<Uuid, Chat>,
    // Chat to display / interact with currently.
    pub active: Option<Uuid>,
    // don't persist a call across restarts
    // the Uuid is the chat associated with the current call
    #[serde(skip)]
    pub active_media: Option<Uuid>, // TODO: in the future, this should probably be a vec of media streams or something
    // Chats to show in the sidebar
    pub in_sidebar: VecDeque<Uuid>,
    // Favorite Chats
    pub favorites: Vec<Uuid>,
}

impl Chats {
    pub fn active_chat_has_unreads(&self) -> bool {
        let id = match self.active {
            Some(c) => c,
            None => return false,
        };

        match self.all.get(&id) {
            Some(c) => c.unreads > 0,
            None => false,
        }
    }
    /// returns the UUID of the message being replied to by the active chat
    pub fn get_replying_to(&self) -> Option<Uuid> {
        self.active.and_then(|id| {
            self.all
                .get(&id)
                .and_then(|chat| chat.replying_to.as_ref().map(|msg| msg.id()))
        })
    }
}

impl Serialize for Chats {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut state = serializer.serialize_struct("Chats", 6)?;

        if STATIC_ARGS.use_mock {
            state.serialize_field("initialized", &self.initialized)?;
        } else {
            state.skip_field("initialized")?;
        }

        state.serialize_field("all", &self.all)?;
        state.serialize_field("active", &self.active)?;
        state.skip_field("active_media")?;
        state.serialize_field("in_sidebar", &self.in_sidebar)?;
        state.serialize_field("favorites", &self.favorites)?;

        state.end()
    }
}

// don't skip messages and participants when using mock data
impl Serialize for Chat {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut state = serializer.serialize_struct("Chat", 5)?;
        state.serialize_field("id", &self.id)?;
        state.serialize_field("participants", &self.participants)?;

        if STATIC_ARGS.use_mock {
            state.serialize_field("messages", &self.messages)?;
        } else {
            state.skip_field("messages")?;
        }

        state.serialize_field("unreads", &self.unreads)?;
        state.skip_field("replying_to")?;
        state.end()
    }
}
