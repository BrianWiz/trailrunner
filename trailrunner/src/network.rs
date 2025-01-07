use std::collections::HashMap;
use std::marker::PhantomData;
use std::time::Duration;
use log::{info, warn};
use matchbox_socket::{PeerState, WebRtcSocket};
use crate::prelude::*;

pub const CHANNEL_ID: usize = 0;
pub type MessageId = usize;
pub type FromPeerId = PeerId;

/// The message queue are messages that will be sent to other peers. The messages are sent in the order they are added to the queue.
pub struct MessageQueue<U: TUser, A: TApp<U>, M: TSerializableMessage> {
    messages: Vec<Message<U, A, M>>,
    _phantom_data: PhantomData<(U, M)>,
}

impl<U: TUser, A: TApp<U>, M: TSerializableMessage> MessageQueue<U, A, M> {
    pub fn new() -> Self {
        Self {
            messages: Vec::new(),
            _phantom_data: PhantomData
        }
    }

    pub fn enqueue(&mut self, message: Message<U, A, M>) {
        self.messages.push(message);
    }

    pub(crate) fn drain(&mut self, range: std::ops::RangeFull) -> Vec<Message<U, A, M>> {
        self.messages.drain(range).collect()
    }
}

pub trait TSerializableMessage: serde::Serialize + for<'de> serde::Deserialize<'de> + Clone + Send + 'static {}

// Blanket implementation for any type that meets the requirements
impl<M> TSerializableMessage for M
where
    M: serde::Serialize + for<'de> serde::Deserialize<'de> + Clone + Send + 'static
{}

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(bound = "M: TSerializableMessage")]
struct PackedMessage<M: TSerializableMessage> {
    id: MessageId,
    is_ack: bool,
    must_ack: bool,
    data: M,
}

impl<T: TSerializableMessage> PackedMessage<T> {
}

pub struct Message<U: TUser, T: TApp<U>, M: TSerializableMessage> {
    id: MessageId,
    to_peer: Option<PeerId>,
    data: M,
    ack_handler: Option<Box<dyn FnMut(&mut T::Application, MessageId, FromPeerId, &M)>>,
    _phantom_data: PhantomData<U>,
}

impl<U: TUser, T: TApp<U>, M: TSerializableMessage> Message<U, T, M> {

    /// Create a new message. Creating this doesn't send by itself, you must have access to a `MessageQueue` to send it.
    /// If you leave `to_peer` as `None`, the message will be broad-casted to all peers.
    ///
    /// Example usage:
    /// ```rust
    /// use trailrunner::prelude::*;
    /// use matchbox_socket::PeerId;
    ///
    /// let message = Message::new("hello world".as_bytes().to_vec())
    ///     // Optional: specify a peer to send the message to, otherwise it broadcasts to all peers.
    ///     .to_peer(PeerId::from("some-peer-id"))
    ///     // Optional: subscribe to a callback that peers must respond to.
    ///     .with_ack_handler(|app, from_peer, data| {
    ///         // Handle incoming messages here
    ///     });
    /// // move `message` into an `enqueue` call on a `MessageQueue` to send the message
    /// ```
    ///
    pub fn new(data: M) -> Self {
        Self {
            id: 0, // gets set by the `NetworkManager` before sending
            to_peer: None,
            data,
            ack_handler: None,
            _phantom_data: PhantomData
        }
    }

    pub fn to_peer(mut self, to_peer: PeerId) -> Self {
        self.to_peer = Some(to_peer);
        self
    }

    /// Subscribes to a callback that peers must respond to.
    ///
    /// Peers will be expected to respond back with a message unless they disconnect between the time your
    /// client sent the message and the time the other peer(s) would have received it.
    ///
    /// If it was a broadcast, we wait until all have responded before calling the handlers on each.
    /// That way, if this is called on a broadcast, you can be sure that all peers have received the message.
    pub fn with_ack_handler(
        mut self,
        handler: impl FnMut(&mut T::Application, MessageId, FromPeerId, &M) + 'static
    ) -> Self {
        self.ack_handler = Some(Box::new(handler));
        self
    }
}

pub struct MessageWaitingForAck<U: TUser, T: TApp<U>, M: TSerializableMessage> {
    message: Message<U, T::Application, M>,
    peers_that_have_acked: Vec<PeerId>,
}

impl<U: TUser, T: TApp<U>, M: TSerializableMessage> MessageWaitingForAck<U, T, M> {
    pub fn was_broadcast(&self) -> bool {
        self.message.to_peer.is_none()
    }

    pub fn have_all_acked(&self, connected_peers: &Vec<PeerId>) -> bool {
        if self.was_broadcast() {
            // Check that all currently connected peers have acked
            connected_peers.iter().all(|peer| self.peers_that_have_acked.contains(peer))
        } else {
            let intended_recipient = self.message.to_peer.unwrap(); // SAFETY: safe to unwrap here because we are NOT a broadcast, which means this had to have been set.
            self.peers_that_have_acked.contains(&intended_recipient)
        }
    }
}

pub struct NetworkManager<U: TUser, T: TApp<U>, M: TSerializableMessage> {
    socket: WebRtcSocket,
    app: T,
    messages_waiting_for_ack: HashMap<MessageId, MessageWaitingForAck<U, T, M>>,
    next_message_id: MessageId,
    _phantom_data: PhantomData<(U, M)>,
}

impl<U: TUser, T: TApp<U>, M> NetworkManager<U, T, M>
where
    T: TApp<U, Application = T, Message = M>,
    U: TUser,
    M: TSerializableMessage
{
    pub fn new(socket: WebRtcSocket, app: T) -> Self {
        Self {
            socket,
            app,
            messages_waiting_for_ack: HashMap::new(),
            next_message_id: 0,
            _phantom_data: PhantomData,
        }
    }

    pub fn tick(&mut self, delta: Duration) {
        for (peer_id, state) in self.socket.update_peers() {
            match state {
                PeerState::Connected => {
                    let users = self.app.get_users_mut();
                    let user = U::new(peer_id);
                    users.insert(peer_id, user);
                    self.app.post_user_connected(peer_id);
                    info!("Peer connected: {peer_id}");
                }
                PeerState::Disconnected => {
                    info!("Peer disconnected: {peer_id}");
                    match self.app.get_users_mut().remove(&peer_id){
                        Some(_) => self.app.post_user_disconnected(peer_id),
                        None => warn!("Peer disconnected but no user found"),
                    }
                }
            }
        }

        let connected_peers: Vec<_> = self.socket.connected_peers().collect();

        // Accept any messages incoming
        for (from_peer, packet) in self.socket.channel_mut(CHANNEL_ID).receive() {

            let incoming_message: PackedMessage<M> = match bincode::deserialize_from(&packet[..]) {
                Ok(packet) => packet,
                Err(e) => {
                    warn!("Failed to deserialize packet: {e}");
                    continue;
                }
            };

            // Is this message an ack?
            if incoming_message.is_ack {
                if let Some(unacked) = self.messages_waiting_for_ack.get_mut(&incoming_message.id) {
                    unacked.peers_that_have_acked.push(from_peer);

                    // If all peers have acked, call the handler(s)
                    if unacked.have_all_acked(&connected_peers) {
                        // For broadcasted messages, call the handler on all peers
                        if unacked.was_broadcast() {
                            for peer in connected_peers.iter() {
                                if let Some(handler) = unacked.message.ack_handler.as_mut() {
                                    handler(&mut self.app, incoming_message.id, *peer, &incoming_message.data);
                                }
                            }
                        } else {
                            if let Some(handler) = unacked.message.ack_handler.as_mut() {
                                handler(&mut self.app, incoming_message.id, from_peer, &incoming_message.data);
                            }
                        }

                        // Clean up after handling
                        self.messages_waiting_for_ack.remove(&incoming_message.id);
                    }
                }
            } else {
                if incoming_message.must_ack {
                    let response = self.app.receive_must_ack(incoming_message.id, from_peer, &incoming_message.data);

                    // send the response
                    let packet = match bincode::serialize(&PackedMessage {
                        id: incoming_message.id,
                        data: response,
                        is_ack: true,
                        must_ack: false,
                    }) {
                        Ok(packet) => packet,
                        Err(e) => {
                            warn!("Failed to serialize packet: {e}");
                            continue;
                        }
                    }.into_boxed_slice();

                    self.socket.channel_mut(CHANNEL_ID).send(packet, from_peer);
                }
                else {
                    self.app.receive(incoming_message.id, from_peer, &incoming_message.data);
                }
            }
        }

        // Send any messages waiting to be sent
        for mut message in self.app.message_queue().drain(..) {

            message.id = self.next_message_id;

            let packet = match bincode::serialize(&PackedMessage {
                id: message.id,
                data: message.data.clone(),
                is_ack: false,
                must_ack: message.ack_handler.is_some(),
            }) {
                Ok(packet) => packet,
                Err(e) => {
                    warn!("Failed to serialize packet: {e}");
                    continue;
                }
            }.into_boxed_slice();

            match message.to_peer {
                Some(to_peer) => {
                    self.socket.channel_mut(CHANNEL_ID).send(packet, to_peer);
                }
                None => {
                    // Broadcast to all connected peers
                    for &peer in &connected_peers {
                        self.socket.channel_mut(CHANNEL_ID).send(packet.clone(), peer);
                    }
                }
            }

            let id = self.next_message_id;
            self.next_message_id += 1;

            if message.ack_handler.is_some() {
                self.messages_waiting_for_ack.insert(id,MessageWaitingForAck {
                    message,
                    peers_that_have_acked: Vec::new(),
                });
            }
        }

        self.app.tick(delta);
    }
}
