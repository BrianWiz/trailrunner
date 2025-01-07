use std::time::Duration;
use matchbox_socket::PeerId;
use crate::prelude::*;

/// The trait that the application must implement to be used with the NetworkManager.
///
/// For example:
///
/// ```rust
/// use std::collections::HashMap;
/// use std::time::Duration;
/// use log::{error, info};
/// use tokio::main;
/// use trailrunner::prelude::*;
///
/// pub struct App {
///     users: UserList<User>,
///     message_queue: MessageQueue<User, App, MyMessage>,
/// }
///
/// impl TApp<User> for App {
///     type Application = App;
///     type Message = MyMessage;
///
///     fn get_users(&mut self) -> &mut UserList<User> {
///         &mut self.users
///     }
///
///     fn get_message_queue(&mut self) -> &mut MessageQueue<User, Self::Application, Self::Message> {
///         &mut self.message_queue
///     }
///
///     fn receive(&mut self, id: MessageId, from_peer: PeerId, message: &Self::Message) {
///         info!("Received message {} from peer {}: {}", id, from_peer, message.data);
///     }
///
///     fn receive_must_ack(&mut self, id: MessageId, from_peer: PeerId, message: &Self::Message) -> Self::Message {
///         info!("Received message {} from peer {}: {}", id, from_peer, message.data.clone());
///         Self::Message {
///             data: "Ok!".to_string(),
///         }
///     }
///
///     fn tick(&mut self, delta: Duration) {
///
///     }
/// }
///
/// #[derive(Debug, Clone)]
/// pub struct User {
///     name: String,
/// }
///
/// impl TUser for User {
///     fn new(name: String) -> Self {
///         Self { name }
///     }
/// }
///
/// #[derive(serde::Serialize, serde::Deserialize, Clone)]
/// pub struct MyMessage {
///    data: String,
/// }
/// ```
pub trait TApp<U: TUser> {
    type Application: TApp<U>;
    type Message: TSerializableMessage;

    // User must implement these

    fn get_users(&mut self) -> &mut UserList<U>;
    fn get_message_queue(&mut self) -> &mut MessageQueue<U, Self::Application, Self::Message>;

    /// Called when a message is received. The id is the id of the message, from_peer is the peer that sent the message, and message is the message itself.
    fn receive(&mut self, id: MessageId, from_peer: PeerId, message: &Self::Message);

    /// Called when a message is received that must be acknowledged. The id is the id of the message, from_peer is the peer that sent the message, and message is the message itself. The return value is the message that will be sent back to the sender as an acknowledgment.
    fn receive_must_ack(&mut self, id: MessageId, from_peer: PeerId, message: &Self::Message) -> Self::Message;

    /// Called every frame. The delta is the duration of time passed since the last frame.
    fn tick(&mut self, delta: Duration);

    // No need to implement these

    fn on_post_user_connected(&mut self, _peer_id: PeerId) {}
    fn on_post_user_disconnected(&mut self, _peer_id: PeerId) {}

    fn get_users_mut(&mut self) -> &mut UserList<U> {
        self.get_users()
    }
}
