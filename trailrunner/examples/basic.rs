use std::time::Duration;

use futures::{select, FutureExt};
use futures_timer::Delay;
use log::{info, warn};
use tracing_subscriber::EnvFilter;
use trailrunner::prelude::*;

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub enum MyMessage {
    String(String),
    Something,
}

pub struct App {
    users: UserList<User>,
    message_queue: MessageQueue<User, App, MyMessage>,
}

impl TApp<User> for App {
    type Application = App;
    type Message = MyMessage;

    fn users(&mut self) -> &mut UserList<User> {
        &mut self.users
    }

    fn message_queue(&mut self) -> &mut MessageQueue<User, Self::Application, Self::Message> {
        &mut self.message_queue
    }

    fn receive(&mut self, id: MessageId, from_peer: PeerId, message: &Self::Message) {
        match message {
            MyMessage::String(s) => {
                info!("Received message {} from peer {}: {}", id, from_peer, s);
            }
            _=> {
                warn!("Received message {} from peer {}: but we don't support it here", id, from_peer);
            }
        }
    }

    fn receive_must_ack(&mut self, id: MessageId, from_peer: PeerId, message: &Self::Message) -> Self::Message {
        match message {
            MyMessage::String(s) => {
                info!("Received message {} from peer {}: {}", id, from_peer, s);
            }
            _=> {
                warn!("Received message {} from peer {}: but weren't expecting this type of message here", id, from_peer);
            }
        }

        // Just respond with a "Something" for all messages. This should trigger the ack callback on the other peer.
        Self::Message::Something
    }

    fn tick(&mut self, _delta: Duration) {
        // nothing
    }

    fn post_user_connected(&mut self, peer_id: PeerId) {
        info!("User connected {}... sending them a hello that expects an ack.", peer_id);
        self.message_queue.enqueue(Message::new(
            MyMessage::String("Hello!".to_string())
        )
            .to_peer(peer_id)
            .with_ack_handler(|_app, id, from_peer, message| {
                info!("Received ack for message {} from peer {} {:?}", id, from_peer, message);
            })
        );
    }

    fn post_user_disconnected(&mut self, peer_id: PeerId) {

    }
}

#[derive(Debug, Clone)]
pub struct User {
    peer_id: PeerId,
}

impl TUser for User {
    fn new(peer_id: PeerId) -> Self {
        Self { peer_id }
    }
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env()
            .add_directive(tracing::Level::INFO.into()))
        .init();

    let (socket, loop_fut) = WebRtcSocket::new_reliable("ws://localhost:3536/");

    let loop_fut = loop_fut.fuse();
    futures::pin_mut!(loop_fut);

    let timeout = Delay::new(Duration::from_millis(100));
    futures::pin_mut!(timeout);

    let app = App { 
        users: UserList::new(),
        message_queue: MessageQueue::<User, App, MyMessage>::new(),
    };

    let mut network = NetworkManager::new(socket, app);

    let delta = Duration::from_millis(16);
    
    loop {
        let delta = delta.clone();
        network.tick(delta.clone());
        select! {
            // Run this loop periodically
            _ = (&mut timeout).fuse() => {
                timeout.reset(delta);
            }
            // Or break if the message loop ends (disconnected, closed, etc.)
            _ = &mut loop_fut => {
                break;
            }
        }
    }
}
