use serde::{Deserialize, Serialize};

#[derive(Debug, Default, Clone)]
pub struct GameQueue {
    pub index: usize,
    pub links: Vec<Link>,
}

impl GameQueue {
    pub fn new(initial_link: Link) -> GameQueue {
        GameQueue {
            index: 0,
            links: vec![initial_link],
        }
    }
}
#[derive(Debug)]
pub struct Navigation {
    pub queue: GameQueue,
    pub next_game: Option<Link>,
}

impl Navigation {
    pub fn new(initial_link: Link) -> Navigation {
        Navigation {
            queue: GameQueue::new(initial_link),
            next_game: None,
        }
    }
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct Link {
    pub collection: String,
    pub game: String,
}

impl Link {
    pub fn new(collection: String, game: String) -> Link {
        Link { collection, game }
    }

    pub fn to_filename(&self) -> String {
        format!("collections/{}/{}.json", self.collection, self.game)
    }
}
