//! First-pass categories from `data/categories.json`, compiled into the binary.

use std::collections::HashMap;

use nudgy_shared::{Category, Foreground, Label};
use serde::Deserialize;

const DATA: &str = include_str!("../../../data/categories.json");

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct File {
    apps: HashMap<String, Category>,
    domains: HashMap<String, Category>,
    suffixes: HashMap<String, Category>,
    prefixes: HashMap<String, Category>,
    browser_titles: HashMap<String, Category>,
}

pub struct Categories {
    file: File,
}

/// What a category is, before anything is known about the goal.
pub fn default_label(category: Category) -> Label {
    use Category::*;
    match category {
        Ide | Docs | Reading | Research | Learning | Design | AiTools | Productivity => Label::Focused,
        Social | Video | Games | Shopping | News | Messaging => Label::Distracting,
        Communication | Search | System | Private | Music | Unknown => Label::Neutral,
    }
}

/// Categories where the title can change the verdict: a YouTube lecture on
/// the goal's topic is work; a cat video isn't.
pub fn title_matters(category: Category) -> bool {
    use Category::*;
    matches!(category, Video | Social | News | Search | Messaging | Unknown | Music)
}

const BROWSER_EXES: [&str; 5] = ["chrome.exe", "msedge.exe", "firefox.exe", "brave.exe", "opera.exe"];

impl Categories {
    pub fn load() -> Self {
        let file: File = serde_json::from_str(DATA).expect("data/categories.json is valid");
        Categories { file }
    }

    pub fn len(&self) -> usize {
        self.file.apps.len() + self.file.domains.len() + self.file.suffixes.len() + self.file.prefixes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn of(&self, foreground: &Foreground) -> Category {
        match foreground {
            Foreground::App { app_id, title, .. } => {
                if BROWSER_EXES.contains(&app_id.as_str()) {
                    // Browser without the extension: guess from the window title.
                    return title.as_deref().and_then(|t| self.browser_title(t)).unwrap_or(Category::Unknown);
                }
                self.file.apps.get(app_id).copied().unwrap_or(Category::Unknown)
            }
            Foreground::Web { domain, .. } => self.domain(domain),
            Foreground::Private => Category::Private,
            Foreground::None => Category::System,
        }
    }

    /// The most specific entry wins: `music.youtube.com` before `youtube.com`.
    pub fn domain(&self, domain: &str) -> Category {
        let domain = domain.trim_start_matches("www.").to_ascii_lowercase();
        let mut candidate = domain.as_str();
        loop {
            if let Some(c) = self.file.domains.get(candidate) {
                return *c;
            }
            match candidate.split_once('.') {
                Some((_, rest)) if rest.contains('.') => candidate = rest,
                _ => break,
            }
        }
        if let Some((_, c)) = self.file.suffixes.iter().find(|(s, _)| domain.ends_with(s.as_str())) {
            return *c;
        }
        if let Some((_, c)) = self.file.prefixes.iter().find(|(p, _)| domain.starts_with(p.as_str())) {
            return *c;
        }
        Category::Unknown
    }

    fn browser_title(&self, title: &str) -> Option<Category> {
        self.file
            .browser_titles
            .iter()
            .filter(|(hint, _)| title.contains(hint.as_str()))
            .max_by_key(|(hint, _)| hint.len())
            .map(|(_, c)| *c)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn web(domain: &str) -> Foreground {
        Foreground::Web { browser: "chrome.exe".into(), domain: domain.into(), title: None }
    }

    fn app(exe: &str, title: &str) -> Foreground {
        Foreground::App { app_id: exe.into(), app_name: exe.into(), title: Some(title.into()) }
    }

    #[test]
    fn the_list_is_big_enough() {
        assert!(Categories::load().len() >= 400);
    }

    #[test]
    fn domains_match_subdomains_most_specific_first() {
        let c = Categories::load();
        assert_eq!(c.of(&web("youtube.com")), Category::Video);
        assert_eq!(c.of(&web("m.youtube.com")), Category::Video);
        assert_eq!(c.of(&web("music.youtube.com")), Category::Music);
        assert_eq!(c.of(&web("aws.amazon.com")), Category::Ide);
        assert_eq!(c.of(&web("amazon.com")), Category::Shopping);
        assert_eq!(c.of(&web("en.wikipedia.org")), Category::Research);
        assert_eq!(c.of(&web("daraz.com.bd")), Category::Shopping);
    }

    #[test]
    fn universities_and_lms_hosts_are_learning() {
        let c = Categories::load();
        assert_eq!(c.of(&web("cse.buet.ac.bd")), Category::Learning);
        assert_eq!(c.of(&web("moodle.nsu.edu.bd")), Category::Learning);
        assert_eq!(c.of(&web("moodle.example.org")), Category::Learning);
    }

    #[test]
    fn apps_and_browsers_without_the_extension() {
        let c = Categories::load();
        assert_eq!(c.of(&app("code.exe", "main.rs")), Category::Ide);
        assert_eq!(c.of(&app("whatsapp.exe", "Chats")), Category::Messaging);
        assert_eq!(c.of(&app("chrome.exe", "Funny cats - YouTube - Google Chrome")), Category::Video);
        assert_eq!(c.of(&app("msedge.exe", "Thesis - Google Docs - Microsoft Edge")), Category::Docs);
        assert_eq!(c.of(&app("chrome.exe", "Some page - Google Chrome")), Category::Unknown);
        assert_eq!(c.of(&app("totally-new-app.exe", "x")), Category::Unknown);
    }

    #[test]
    fn defaults_follow_the_spec() {
        assert_eq!(default_label(Category::Ide), Label::Focused);
        assert_eq!(default_label(Category::Social), Label::Distracting);
        assert_eq!(default_label(Category::Unknown), Label::Neutral);
        assert_eq!(default_label(Category::Private), Label::Neutral);
    }
}
