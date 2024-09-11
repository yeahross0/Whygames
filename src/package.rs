//use crate::nav::Link;
use std::collections::HashMap;

use serde_json::Value;

fn main() {
    if let Some(arg) = std::env::args().nth(1) {
        if arg == "package" {
            let mut memfs = HashMap::new();
            println!("Packaging games");
            {
                let paths = std::fs::read_dir("collections/Green/").unwrap();
                for path in paths {
                    let p = path.unwrap().path();
                    let s = p.to_str().unwrap();

                    // To minimise
                    let cartridge: Value =
                        serde_json::from_str(&std::fs::read_to_string(&p).unwrap()).unwrap();

                    memfs.insert(s.to_string(), serde_json::to_string(&cartridge).unwrap());
                }
            }
            let s = serde_json::to_string(&memfs).unwrap();
            std::fs::write("fs.json", s).unwrap_or_else(|e| println!("{}", e));
            std::process::exit(0);
        }
    }

    println!("HEY");
}
