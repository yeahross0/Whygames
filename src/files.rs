use std::collections::HashMap;

pub struct FileSystem {
    pub memfs: HashMap<String, String>,
}
