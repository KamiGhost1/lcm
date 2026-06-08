//! Minimal `os-release(5)` parser.
//!
//! We only need a handful of keys (`ID`, `ID_LIKE`), so this is a small,
//! dependency-free reader rather than a full shell-style parser.

use std::collections::HashMap;
use std::path::Path;

use crate::error::Result;

/// Parse the contents of an `os-release` file into key/value pairs.
///
/// Surrounding single or double quotes are stripped from values; comment and
/// blank lines are ignored. This is intentionally lenient.
pub fn parse(content: &str) -> HashMap<String, String> {
    let mut map = HashMap::new();
    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some((key, value)) = line.split_once('=') {
            let value = value.trim().trim_matches(|c| c == '"' || c == '\'');
            map.insert(key.trim().to_string(), value.to_string());
        }
    }
    map
}

/// Read `/etc/os-release` (falling back to `/usr/lib/os-release`).
///
/// Returns an empty map if neither file exists (e.g. on non-Linux hosts), so
/// callers can treat "no data" as "unsupported distro".
pub fn read() -> Result<HashMap<String, String>> {
    for path in ["/etc/os-release", "/usr/lib/os-release"] {
        if Path::new(path).exists() {
            return Ok(parse(&std::fs::read_to_string(path)?));
        }
    }
    Ok(HashMap::new())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_quoted_and_unquoted() {
        let content = r#"
# a comment
NAME="Ubuntu"
ID=ubuntu
ID_LIKE=debian
VERSION_ID="24.04"
PRETTY_NAME='Ubuntu 24.04 LTS'
"#;
        let m = parse(content);
        assert_eq!(m.get("ID").unwrap(), "ubuntu");
        assert_eq!(m.get("ID_LIKE").unwrap(), "debian");
        assert_eq!(m.get("NAME").unwrap(), "Ubuntu");
        assert_eq!(m.get("PRETTY_NAME").unwrap(), "Ubuntu 24.04 LTS");
        assert!(!m.contains_key("# a comment"));
    }
}
