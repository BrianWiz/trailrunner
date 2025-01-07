use std::collections::HashMap;
use std::fmt::Debug;
use matchbox_socket::PeerId;

pub struct UserList<T: TUser> {
    users: HashMap<PeerId, T>,
}

impl <T: TUser> UserList<T> {
    pub fn new() -> Self {
        Self { users: HashMap::new() }
    }

    pub(crate) fn insert(&mut self, peer_id: PeerId, user: T) {
        self.users.insert(peer_id, user);
    }

    pub(crate) fn remove(&mut self, peer_id: &PeerId) -> Option<T> {
        self.users.remove(peer_id)
    }
    
    pub fn get(&self, peer_id: &PeerId) -> Option<&T> {
        self.users.get(peer_id)
    }
    
    pub fn get_mut(&mut self, peer_id: &PeerId) -> Option<&mut T> {
        self.users.get_mut(peer_id)
    }
}

pub trait TUser: Debug + Clone {
    fn new(peer_id: PeerId) -> Self;
}
