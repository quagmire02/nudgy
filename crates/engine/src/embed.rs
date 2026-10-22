//! Title-vs-goal similarity with all-MiniLM-L6-v2 (Apache-2.0) on ONNX
//! Runtime. Loads lazily on first use; `scripts/fetch-model.mjs` downloads
//! the model into `data/models/all-MiniLM-L6-v2/`.
//!
//! Tokenisation is BERT-uncased WordPiece, written out here (lower-case,
//! split on spaces and punctuation, greedy longest-match subwords) rather
//! than pulling in the full `tokenizers` crate.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use ort::session::Session;
use ort::value::Tensor;

use crate::similarity::Similarity;

const MAX_TOKENS: usize = 64;

/// Cosine similarity mapped to the classifier's 0..1 scale: cosine 0.24 is a
/// goal match (0.5), 0.13 ambiguous (0.25). Calibrated with
/// `examples/calibrate.rs`: related student titles scored 0.24-0.59,
/// borderline ones ("funny programming memes") 0.16-0.22, unrelated under 0.10.
fn relatedness(cosine: f32) -> f32 {
    ((cosine - 0.02) / 0.44).clamp(0.0, 1.0)
}

pub struct MiniLm {
    dir: PathBuf,
    loaded: Option<Loaded>,
    failed: bool,
    cache: HashMap<String, Vec<f32>>,
}

struct Loaded {
    session: Session,
    vocab: HashMap<String, i64>,
}

impl MiniLm {
    /// Nothing is read until the first `score`.
    pub fn lazy(dir: impl Into<PathBuf>) -> Self {
        MiniLm { dir: dir.into(), loaded: None, failed: false, cache: HashMap::new() }
    }

    pub fn available(dir: &Path) -> bool {
        dir.join("model.onnx").is_file() && dir.join("vocab.txt").is_file()
    }

    fn load(&mut self) -> Option<&mut Loaded> {
        if self.loaded.is_none() && !self.failed {
            match Self::open(&self.dir) {
                Ok(loaded) => self.loaded = Some(loaded),
                Err(e) => {
                    // Without the model the lexical matcher still works.
                    eprintln!("embedding model unavailable: {e}");
                    self.failed = true;
                }
            }
        }
        self.loaded.as_mut()
    }

    fn open(dir: &Path) -> Result<Loaded, String> {
        let vocab = std::fs::read_to_string(dir.join("vocab.txt")).map_err(|e| e.to_string())?;
        let vocab = vocab.lines().enumerate().map(|(i, t)| (t.to_string(), i as i64)).collect();
        let env = ort::init().with_name("nudgy").build().map_err(|e| e.to_string())?;
        // One thread: a title every few seconds needs no more, and idle CPU stays low.
        let mut builder = Session::builder(&env)
            .map_err(|e| e.to_string())?
            .with_intra_threads(1)
            .map_err(|e| e.to_string())?;
        let session = builder.commit_from_file(dir.join("model.onnx")).map_err(|e| e.to_string())?;
        Ok(Loaded { session, vocab })
    }

    pub fn embed(&mut self, text: &str) -> Option<Vec<f32>> {
        if let Some(v) = self.cache.get(text) {
            return Some(v.clone());
        }
        let mut mean = {
            let loaded = self.load()?;
            let ids = tokenize(text, &loaded.vocab);
            let n = ids.len();
            let input = |values: Vec<i64>| Tensor::from_array(([1usize, n], values)).ok();
            let outputs = loaded
                .session
                .run(ort::inputs![
                    "input_ids" => input(ids)?,
                    "attention_mask" => input(vec![1; n])?,
                    "token_type_ids" => input(vec![0; n])?,
                ])
                .ok()?;
            let (shape, data) = outputs["last_hidden_state"].try_extract_tensor::<f32>().ok()?;
            let dim = *shape.last()? as usize;
            // Mean over tokens.
            let mut mean = vec![0f32; dim];
            for token in data.chunks(dim) {
                for (m, x) in mean.iter_mut().zip(token) {
                    *m += x / n as f32;
                }
            }
            mean
        };
        let norm = mean.iter().map(|x| x * x).sum::<f32>().sqrt().max(1e-9);
        mean.iter_mut().for_each(|x| *x /= norm);
        if self.cache.len() > 4_000 {
            self.cache.clear();
        }
        self.cache.insert(text.to_string(), mean.clone());
        Some(mean)
    }

    pub fn cosine(&mut self, a: &str, b: &str) -> Option<f32> {
        let (a, b) = (self.embed(a)?, self.embed(b)?);
        Some(a.iter().zip(&b).map(|(x, y)| x * y).sum())
    }
}

impl Similarity for MiniLm {
    fn score(&mut self, goal: &str, title: &str) -> Option<f32> {
        if goal.trim().is_empty() || title.trim().is_empty() {
            return None;
        }
        self.cosine(goal, title).map(relatedness)
    }
}

/// `[CLS] wordpieces… [SEP]`, at most `MAX_TOKENS`.
fn tokenize(text: &str, vocab: &HashMap<String, i64>) -> Vec<i64> {
    let id = |t: &str| vocab.get(t).copied();
    let unk = id("[UNK]").unwrap_or(100);
    let mut ids = vec![id("[CLS]").unwrap_or(101)];
    'words: for word in basic_tokens(text) {
        let chars: Vec<char> = word.chars().collect();
        if chars.len() > 100 {
            ids.push(unk);
            continue;
        }
        let mut pieces = Vec::new();
        let mut start = 0;
        while start < chars.len() {
            let mut end = chars.len();
            let mut found = None;
            while start < end {
                let piece: String = chars[start..end].iter().collect();
                let piece = if start > 0 { format!("##{piece}") } else { piece };
                if let Some(i) = id(&piece) {
                    found = Some(i);
                    break;
                }
                end -= 1;
            }
            match found {
                Some(i) => pieces.push(i),
                None => {
                    ids.push(unk);
                    continue 'words;
                }
            }
            start = end;
        }
        ids.extend(pieces);
        if ids.len() >= MAX_TOKENS - 1 {
            ids.truncate(MAX_TOKENS - 1);
            break;
        }
    }
    ids.push(id("[SEP]").unwrap_or(102));
    ids
}

/// Lower-case; split on whitespace; every punctuation mark is its own token.
fn basic_tokens(text: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    for word in text.to_lowercase().split_whitespace() {
        let mut current = String::new();
        for c in word.chars() {
            if c.is_ascii_punctuation() || (!c.is_alphanumeric() && !c.is_whitespace()) {
                if !current.is_empty() {
                    tokens.push(std::mem::take(&mut current));
                }
                tokens.push(c.to_string());
            } else {
                current.push(c);
            }
        }
        if !current.is_empty() {
            tokens.push(current);
        }
    }
    tokens
}

#[cfg(test)]
mod tests {
    use super::*;

    fn model() -> Option<MiniLm> {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data/models/all-MiniLM-L6-v2");
        if !MiniLm::available(&dir) {
            eprintln!("skipping: run `node scripts/fetch-model.mjs` first");
            return None;
        }
        Some(MiniLm::lazy(dir))
    }

    #[test]
    fn tokenizes_like_bert_uncased() {
        let vocab: HashMap<String, i64> =
            ["[CLS]", "[SEP]", "[UNK]", "dynamic", "program", "##ming", ":", "lecture"]
                .iter()
                .enumerate()
                .map(|(i, t)| (t.to_string(), i as i64))
                .collect();
        assert_eq!(tokenize("Lecture: Dynamic Programming", &vocab), [0, 7, 6, 3, 4, 5, 1]);
        assert_eq!(tokenize("zzz", &vocab), [0, 2, 1]);
    }

    #[test]
    fn related_titles_beat_unrelated_ones() {
        let Some(mut m) = model() else { return };
        let goal = "Study DSA: graphs and dynamic programming";
        let related = m.score(goal, "Dijkstra's shortest path algorithm explained").unwrap();
        let unrelated = m.score(goal, "Top 10 funny cat fails").unwrap();
        println!("related {related:.2}, unrelated {unrelated:.2}");
        assert!(related >= 0.5, "related {related}");
        assert!(unrelated < 0.25, "unrelated {unrelated}");
    }

    #[test]
    fn student_titles_land_on_the_right_side() {
        let Some(mut m) = model() else { return };
        let cases = [
            ("Thesis chapter 3: literature review", "How to write a literature review", true),
            ("Organic chemistry revision", "Alkene addition reactions mechanism", true),
            ("Python assignment: linked lists", "Reverse a linked list recursively", true),
            ("Physics lab report", "Writing a lab report: discussion section", true),
            ("Thesis chapter 3: literature review", "Funny cats compilation", false),
            ("Organic chemistry revision", "Real Madrid vs Barcelona highlights", false),
            ("Python assignment: linked lists", "Minecraft speedrun world record", false),
            ("Physics lab report", "Best street food in Dhaka", false),
        ];
        for (goal, title, related) in cases {
            let s = m.score(goal, title).unwrap();
            println!("{s:.2}  {goal} | {title}");
            if related {
                assert!(s >= 0.5, "{goal} / {title}: {s}");
            } else {
                assert!(s < 0.25, "{goal} / {title}: {s}");
            }
        }
    }
}
