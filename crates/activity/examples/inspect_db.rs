//! Diagnostic: can this Windows user's key open a Nudgy database?
//!
//!     cargo run -p nudgy-activity --example inspect_db -- <data dir> <db file>

use std::path::PathBuf;

use nudgy_activity::{Store, keys};

fn main() {
    let mut args = std::env::args().skip(1);
    let dir = PathBuf::from(args.next().expect("data dir"));
    let db = PathBuf::from(args.next().expect("db file"));
    let key = keys::db_key(&dir).expect("key");
    println!("key fingerprint: {:02x}{:02x}{:02x}{:02x}…", key[0], key[1], key[2], key[3]);
    match Store::open(&db, &key) {
        Ok(store) => println!("opens: {} events", store.recent(1_000_000).map(|e| e.len()).unwrap_or(0)),
        Err(e) => println!("fails: {e}"),
    }
}
