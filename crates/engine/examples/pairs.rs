//! Prints cosines for goal/title pairs given as arguments: "goal|title" ...
fn main() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data/models/all-MiniLM-L6-v2");
    let mut m = nudgy_engine::embed::MiniLm::lazy(dir);
    for pair in std::env::args().skip(1) {
        let (goal, title) = pair.split_once('|').unwrap();
        println!("{:.3}  {goal} | {title}", m.cosine(goal, title).unwrap());
    }
}
