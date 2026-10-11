// This directory's extension (hand-written; ADR-0031). The generated mod.rs (lib.rs for
// src/) pulls it in with include!, so its lines resolve against this directory.

//! The extension of `event_validator.rs` (hand-written; ADR-0031).

#[allow(unused_imports)]
use super::*;

use backbone_core::FieldRule;
/// An event URL is absolute, or absent.
///
/// The database carries the same rule as a CHECK, which is where the guarantee
/// lives. This rule exists so a caller that sends `example.org/party` is told
/// which field is wrong and why, instead of receiving whatever a violated
/// constraint turns into on the way out.
struct EventUrlIsAbsolute;
impl FieldRule<Event> for EventUrlIsAbsolute {
    fn validate(&self, entity: &Event) -> Vec<ValidationError> {
        match entity.event_url.as_deref() {
            Some(url) if !is_absolute_url(url) => vec![ValidationError::new(
                "event_url",
                "must be an absolute address carrying a scheme and a host, \
                 for example https://example.org/summit",
            )
            .with_code("pattern")],
            _ => vec![],
        }
    }
}
/// Scheme + authority, the same shape the database CHECK accepts. The scheme
/// is not narrowed to http(s): the field holds whatever address the officer
/// publishes, it only has to be one a browser can follow on its own.
fn is_absolute_url(url: &str) -> bool {
    let Some((scheme, rest)) = url.split_once("://") else {
        return false;
    };
    let mut scheme_chars = scheme.chars();
    let scheme_ok = scheme_chars.next().is_some_and(|c| c.is_ascii_alphabetic())
        && scheme_chars.all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '.' | '-'));
    let host = rest
        .split(['/', '?', '#'])
        .next()
        .unwrap_or_default();
    scheme_ok && !host.is_empty() && !host.chars().any(char::is_whitespace)
}
#[cfg(test)]
mod event_url_tests {
    use super::is_absolute_url;
    #[test]
    fn absolute_addresses_pass_and_bare_hosts_do_not() {
        for ok in [
            "https://example.org/summit",
            "http://example.org",
            "webcal://calendar.example.org/feed.ics",
        ] {
            assert!(is_absolute_url(ok), "{ok} should be accepted");
        }
        for bad in [
            "example.org/summit",
            "://example.org",
            "https://",
            "https:// example.org",
            "9https://example.org",
        ] {
            assert!(!is_absolute_url(bad), "{bad} should be refused");
        }
    }
}

/// The rules the schema cannot express, added to the generated validator.
pub(crate) fn extend(validator: EventValidator) -> EventValidator {
    validator
        .rule(EventUrlIsAbsolute)
}
