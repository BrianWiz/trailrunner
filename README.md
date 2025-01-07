# trailrunner
An experimental, opinionated convenience wrapper around [matchbox](https://github.com/johanhelsing/matchbox)

## Goal
Trailrunner aims to be a convenience wrapper, it currently supports the following features:
- Application Level Acknowledgement (only works if connection is set to reliable).
  - ```rust
    fn on_post_user_connected(&mut self, peer_id: PeerId) {
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
  ```
- Broadcast to all peers:
  - You can broadcast to all peers by simply not calling `.to_peer()`. If it expects an ack, it will fire the response for each peer only after all peers have acked
- User:
  - With Trailrunner, you define a User struct by implementing `TUser`. Users are available via `get_user_list()` on the Application where you can fetch a user via peer id
- Application:
  - With Trailrunner, you define an Application struct by implementing `TApp`. This is where you send messages, and where hooks fire off such as:
    - post user connect
    - post user disconnect
    - receive message
    - receive message that expects a response
    - tick
    - etc

## Examples
Please check out the [basic example](https://github.com/BrianWiz/trailrunner/blob/main/trailrunner/examples/basic.rs)
