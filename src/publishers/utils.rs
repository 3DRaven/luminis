/// Trim text to at most `max_chars` characters, appending an ellipsis if trimmed.
/// Uses char-aware slicing to avoid breaking UTF-8 sequences.
pub fn trim_with_ellipsis(text: &str, max_chars: usize) -> String {
    if max_chars == 0 { return String::new(); }
    let count = text.chars().count();
    if count <= max_chars { return text.to_string(); }
    if max_chars == 1 { return "…".to_string(); }
    let take_chars = max_chars.saturating_sub(1);
    let mut s: String = text.chars().take(take_chars).collect();
    s.push('…');
    s
}

#[cfg(test)]
use std::sync::Mutex;
#[cfg(test)]
pub static CONSOLE_TEST_SINK: once_cell::sync::Lazy<Mutex<Vec<String>>> = once_cell::sync::Lazy::new(|| Mutex::new(Vec::new()));

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn trims_with_ellipsis_basic() {
        let s = "абвгд";
        assert_eq!(trim_with_ellipsis(s, 0), "");
        assert_eq!(trim_with_ellipsis(s, 1), "…");
        assert_eq!(trim_with_ellipsis(s, 2), "а…");
        assert_eq!(trim_with_ellipsis(s, 3), "аб…");
        assert_eq!(trim_with_ellipsis(s, 5), "абвгд");
        assert_eq!(trim_with_ellipsis(s, 10), "абвгд");
    }
}
