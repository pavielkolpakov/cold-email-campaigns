use std::collections::BTreeMap;

/// The values a template can draw on: mapped lead fields plus every custom
/// column from the import, keyed by the header the CSV used.
#[derive(Debug, Default, Clone)]
pub struct MergeValues(BTreeMap<String, String>);

impl MergeValues {
    pub fn insert(&mut self, key: impl Into<String>, value: impl Into<String>) {
        self.0.insert(key.into(), value.into());
    }

    fn get(&self, key: &str) -> Option<&str> {
        self.0
            .get(key)
            .map(String::as_str)
            .filter(|value| !value.is_empty())
    }
}

impl<K: Into<String>, V: Into<String>, const N: usize> From<[(K, V); N]> for MergeValues {
    fn from(pairs: [(K, V); N]) -> Self {
        let mut values = Self::default();
        for (key, value) in pairs {
            values.insert(key, value);
        }
        values
    }
}

/// Renders a template, or reports every tag that had neither a value nor a
/// fallback. A half-filled cold email is worse than one that never goes out,
/// so this refuses rather than leaving a gap.
pub fn render(template: &str, values: &MergeValues) -> Result<String, Vec<String>> {
    let mut out = String::with_capacity(template.len());
    let mut unfilled = Vec::new();
    let mut rest = template;

    while let Some(start) = rest.find("{{") {
        out.push_str(&rest[..start]);
        let after = &rest[start + 2..];
        let Some(end) = after.find("}}") else {
            break;
        };
        let (tag, fallback) = split_tag(&after[..end]);
        match values.get(tag) {
            Some(value) => out.push_str(value),
            None if !fallback.is_empty() => out.push_str(fallback),
            None => unfilled.push(tag.to_string()),
        }
        rest = &after[end + 2..];
    }

    out.push_str(rest);

    if unfilled.is_empty() {
        Ok(out)
    } else {
        Err(unfilled)
    }
}

/// `{{first_name|there}}` — everything after the first pipe is the fallback.
fn split_tag(raw: &str) -> (&str, &str) {
    match raw.split_once('|') {
        Some((tag, fallback)) => (tag.trim(), fallback.trim()),
        None => (raw.trim(), ""),
    }
}
