//! Which stored cookie domains apply to a host, following the cookie rules browsers use
//! (RFC 6265): a host-only cookie (`example.com`) applies to that exact host, a domain
//! cookie (`.example.com`) applies to that domain and all its subdomains.

use crate::{Error, Result};

/// Checks that `host` is a plain lower-case host name (no scheme, port, path or spaces)
/// and returns it in lower case.
pub(crate) fn normalize_host(host: &str) -> Result<String> {
    let host = host.trim().trim_end_matches('.').to_ascii_lowercase();
    let valid = !host.is_empty()
        && host.len() <= 253
        && host.split('.').all(|label| {
            !label.is_empty()
                && label.len() <= 63
                && label
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'-')
        });
    if valid {
        Ok(host)
    } else {
        Err(Error::InvalidHost(host))
    }
}

/// Whether a cookie stored under `domain`, expiring at `expires` (Unix seconds, `None` for
/// a session cookie), is sent to a host whose candidate domains are `domains` at `now`:
/// the one rule for readers that filter cookies themselves instead of in SQL.
pub(crate) fn is_sent(domain: &str, expires: Option<i64>, domains: &[String], now: i64) -> bool {
    expires.is_none_or(|at| at > now) && domains.iter().any(|d| d == domain)
}

/// Every stored domain value that can apply to `host`, for an exact database lookup.
///
/// For `www.example.com` this is `www.example.com`, `.www.example.com`, `.example.com`.
/// A domain cookie on a bare top-level domain (`.com`) is never valid, so it is left out.
pub(crate) fn candidate_domains(host: &str) -> Vec<String> {
    let labels: Vec<&str> = host.split('.').collect();
    let mut candidates = vec![host.to_owned()];
    for start in 0..labels.len().saturating_sub(1) {
        candidates.push(format!(".{}", labels[start..].join(".")));
    }
    candidates
}

/// The cookie domains that apply to `host`, after checking it is a plain host name.
pub(crate) fn domains_for(host: &str) -> Result<Vec<String>> {
    Ok(candidate_domains(&normalize_host(host)?))
}

/// The host of an `https` link, checked and in lower case, for the functions of this
/// crate that take a host: `https://MediaServer.unige.ch/play/1?x` gives
/// `mediaserver.unige.ch`. The error never repeats the link, which may be a signed link.
pub fn host_of_link(link: &str) -> Result<String> {
    let invalid = |reason: &str| Error::InvalidLink(reason.to_owned());
    let link = link.trim();
    let rest = link
        .get(..8)
        .filter(|scheme| scheme.eq_ignore_ascii_case("https://"))
        .map(|_| &link[8..])
        .ok_or_else(|| invalid("only https links are accepted"))?;
    let authority = rest.split(['/', '?', '#']).next().unwrap_or_default();
    if authority.contains('@') {
        return Err(invalid("links with a user name are not accepted"));
    }
    let host = authority.split(':').next().unwrap_or_default();
    normalize_host(host).map_err(|_| invalid("the link has no valid host name"))
}

#[cfg(test)]
mod tests {
    #[test]
    fn a_cookie_is_sent_while_unexpired_and_for_one_of_the_domains() {
        let domains = candidate_domains("www.unige.ch");
        assert!(is_sent(".unige.ch", Some(20), &domains, 10));
        assert!(
            is_sent("www.unige.ch", None, &domains, 10),
            "a session cookie"
        );
        assert!(!is_sent(".unige.ch", Some(10), &domains, 10), "expired");
        assert!(
            !is_sent("unige.ch", Some(20), &domains, 10),
            "host-only for another host"
        );
    }

    use super::*;

    #[test]
    fn candidates_cover_the_host_and_its_parent_domains() {
        assert_eq!(
            candidate_domains("mediaserver.unige.ch"),
            vec!["mediaserver.unige.ch", ".mediaserver.unige.ch", ".unige.ch"]
        );
    }

    #[test]
    fn a_two_label_host_never_matches_a_top_level_domain() {
        assert_eq!(
            candidate_domains("example.com"),
            vec!["example.com", ".example.com"]
        );
    }

    #[test]
    fn other_sites_are_never_candidates() {
        let candidates = candidate_domains("unige.ch");
        for other in [
            "notunige.ch",
            ".notunige.ch",
            "unige.ch.example.com",
            ".ch",
            "mediaserver.unige.ch",
        ] {
            assert!(!candidates.contains(&other.to_owned()), "{other}");
        }
    }

    #[test]
    fn the_host_of_an_https_link_is_checked_and_lower_case() {
        assert_eq!(
            host_of_link(" HTTPS://MediaServer.unige.ch:443/play/311754?sig=x ").unwrap(),
            "mediaserver.unige.ch"
        );
        assert_eq!(
            host_of_link("https://example.com#top").unwrap(),
            "example.com"
        );
    }

    #[test]
    fn other_links_are_refused_without_repeating_them() {
        for bad in [
            "http://example.com/",
            "example.com/play",
            "https:///play",
            "https://user@example.com/",
            "https://exa mple.com/",
            "https://",
            "",
        ] {
            let err = host_of_link(bad).unwrap_err();
            assert!(matches!(err, Error::InvalidLink(_)), "{bad}");
        }
        let err = host_of_link("http://cdn.example/v.mp4?sig=SECRET").unwrap_err();
        assert!(!err.to_string().contains("SECRET"));
    }

    #[test]
    fn domains_for_checks_the_host_first() {
        assert_eq!(
            domains_for("Example.com").unwrap(),
            ["example.com", ".example.com"]
        );
        assert!(matches!(domains_for("a b"), Err(Error::InvalidHost(_))));
    }

    #[test]
    fn hosts_are_normalized() {
        assert_eq!(
            normalize_host(" MediaServer.UNIGE.ch. ").unwrap(),
            "mediaserver.unige.ch"
        );
    }

    #[test]
    fn urls_and_odd_strings_are_not_hosts() {
        for bad in [
            "",
            "https://unige.ch",
            "unige.ch/play",
            "unige.ch:443",
            "a..b",
            "x' OR '1'='1",
        ] {
            assert!(
                matches!(normalize_host(bad), Err(Error::InvalidHost(_))),
                "{bad}"
            );
        }
    }
}
