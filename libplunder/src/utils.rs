use std::str::Chars;

/// Simple Wrapper
pub struct W<T>(pub T);

pub fn string_match(haystack: &[char], start: usize, needle: Chars, _regex: bool) -> Option<usize> {
    let mut last = None;
    for (i, nc) in needle.enumerate() {
        if haystack.get(start + i).is_none_or(|hc| *hc != nc) {
            return None;
        }
        last = last.or(Some(start));
        last = Some(last.unwrap() + 1);
    }
    Some(last.unwrap() - 1)
}
