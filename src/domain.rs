use unicode_segmentation::UnicodeSegmentation;

pub struct NewSubscriber {
    pub email: String,
    pub name: SubscriberName,
}

pub struct SubscriberName(String);

impl SubscriberName {
    pub fn parse(s: String) -> SubscriberName {
        let is_empty = s.trim().is_empty();
        let is_too_long = s.graphemes(true).count() > 256;

        let forbidden_chars = ['/', '(', ')', '"', '<', '>', '\\', '{', '}'];
        let contains_forbidden_chars = s.chars().any(|g| forbidden_chars.contains(&g));

        if is_empty || is_too_long || contains_forbidden_chars {
            panic!("{} is not a valid name", s)
        } else {
            SubscriberName(s)
        }
    }
}

impl AsRef<str> for SubscriberName {
    fn as_ref(&self) -> &str {
        &self.0
    }
}
