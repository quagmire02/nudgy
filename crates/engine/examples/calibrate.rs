//! Prints raw cosine similarities for goal/title pairs (feature `embeddings`).
fn main() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data/models/all-MiniLM-L6-v2");
    let mut m = nudgy_engine::embed::MiniLm::lazy(dir);
    let pairs = [
        ("Study DSA: graphs and dynamic programming", "Dijkstra's shortest path algorithm explained", "R"),
        ("Study DSA: graphs and dynamic programming", "Lecture 12: Dijkstra's Algorithm | Graph Theory", "R"),
        ("Study DSA: graphs and dynamic programming", "Knapsack problem memoization tutorial", "R"),
        ("Thesis chapter 3: literature review", "How to write a literature review", "R"),
        ("Thesis chapter 3: literature review", "APA citation format guide", "R"),
        ("Organic chemistry revision", "Alkene addition reactions mechanism", "R"),
        ("Organic chemistry revision", "SN1 vs SN2 reactions", "R"),
        ("Python assignment: linked lists", "Reverse a linked list recursively", "R"),
        ("Python assignment: linked lists", "Python pointers and references explained", "R"),
        ("Physics lab report", "Writing a lab report: discussion section", "R"),
        ("Physics lab report", "Measurement uncertainty and error propagation", "R"),
        ("Client campaign posts", "Instagram caption ideas for brands", "R"),
        ("Study DSA: graphs and dynamic programming", "Top 10 funny cat fails", "U"),
        ("Study DSA: graphs and dynamic programming", "Funny programming memes", "A"),
        ("Thesis chapter 3: literature review", "Funny cats compilation", "U"),
        ("Organic chemistry revision", "Real Madrid vs Barcelona highlights", "U"),
        ("Python assignment: linked lists", "Minecraft speedrun world record", "U"),
        ("Physics lab report", "Best street food in Dhaka", "U"),
        ("Physics lab report", "Bangladesh vs India cricket highlights", "U"),
        ("Client campaign posts", "BTS new music video reaction", "U"),
        ("Organic chemistry revision", "Cooking chemistry: why onions make you cry", "A"),
        ("Python assignment: linked lists", "Top 10 programming languages in 2026", "A"),
    ];
    for (goal, title, kind) in pairs {
        println!("{kind} {:.3}  {goal} | {title}", m.cosine(goal, title).unwrap());
    }
}
