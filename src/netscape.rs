//! The Netscape cookie file format that yt-dlp and curl read and write: plain functions,
//! tested without touching the disk.

use zeroize::Zeroizing;

use crate::clock::now_unix;
use crate::domain::domains_for;
use crate::{Cookie, Error, Result, SecretValue};

/// The first line of a Netscape cookie file, which readers check for.
const HEADER: &str = "# Netscape HTTP Cookie File\n";

/// Netscape format: one line per cookie with seven tab-separated fields.
///
/// The `#HttpOnly_` prefix that some tools write is left out: Python's standard cookie
/// reader treats such lines as comments, and the cookies work without it.
pub(crate) fn to_netscape(cookies: &[Cookie]) -> Result<Zeroizing<String>> {
    let mut out = Zeroizing::new(String::from(HEADER));
    for cookie in cookies {
        check_fields(cookie)?;
        let line = Zeroizing::new(format!(
            "{}\t{}\t{}\t{}\t{}\t{}\t{}\n",
            cookie.domain,
            flag(cookie.domain.starts_with('.')),
            cookie.path,
            flag(cookie.secure),
            cookie.expires.unwrap_or(0),
            cookie.name,
            cookie.value.expose(),
        ));
        out.push_str(&line);
    }
    Ok(out)
}

/// The prefix some tools (yt-dlp, curl) put before the domain of an `HttpOnly` cookie.
const HTTP_ONLY: &str = "#HttpOnly_";

/// Reads a Netscape cookie file (a `cookies.txt` as browser extensions, yt-dlp and curl
/// write it) and returns its unexpired cookies that apply to `host`, with the same domain
/// rules as reading a browser. Comment lines, the `#HttpOnly_` prefix, CRLF line ends and
/// a byte order mark are accepted. Cookies of other sites are skipped and never returned.
///
/// # Errors
///
/// [`Error::InvalidHost`] for a host that is not a plain host name, and
/// [`Error::InvalidCookieFile`] for a line that is not seven tab-separated fields. The
/// error names the line number only, never its content, which may be another site's
/// cookie.
pub fn cookies_from_netscape(text: &str, host: &str) -> Result<Vec<Cookie>> {
    from_netscape_at(text, host, now_unix())
}

/// [`cookies_from_netscape`] with `now` as the current time, in Unix seconds.
fn from_netscape_at(text: &str, host: &str, now: i64) -> Result<Vec<Cookie>> {
    let domains = domains_for(host)?;
    let mut cookies = Vec::new();
    for (index, line) in text.trim_start_matches('\u{feff}').lines().enumerate() {
        let line = line.trim_end_matches('\r');
        let (line, http_only) = match line.strip_prefix(HTTP_ONLY) {
            Some(rest) => (rest, true),
            None if line.trim().is_empty() || line.starts_with('#') => continue,
            None => (line, false),
        };
        let cookie =
            parse_line(line, http_only).ok_or(Error::InvalidCookieFile { line: index + 1 })?;
        let unexpired = cookie.expires.is_none_or(|expires| expires > now);
        if unexpired && domains.contains(&cookie.domain) {
            cookies.push(cookie);
        }
    }
    Ok(cookies)
}

/// One cookie line: domain, include-subdomains flag, path, secure flag, expiry (0 for a
/// session cookie), name, value. `None` unless there are exactly seven fields, the flags
/// are `TRUE` or `FALSE` and the expiry is a number. A domain marked for subdomains gets
/// its leading dot, as browsers store it.
fn parse_line(line: &str, http_only: bool) -> Option<Cookie> {
    let fields: Vec<&str> = line.split('\t').collect();
    let [domain, subdomains, path, secure, expires, name, value] = fields[..] else {
        return None;
    };
    let domain = domain.to_ascii_lowercase();
    let domain = if read_flag(subdomains)? && !domain.starts_with('.') {
        format!(".{domain}")
    } else {
        domain
    };
    let expires: i64 = expires.parse().ok()?;
    Some(Cookie {
        domain,
        name: name.to_owned(),
        value: SecretValue::new(value.to_owned()),
        path: path.to_owned(),
        secure: read_flag(secure)?,
        http_only,
        expires: (expires != 0).then_some(expires),
    })
}

/// `TRUE` or `FALSE` (in any case) as a flag.
fn read_flag(field: &str) -> Option<bool> {
    match field.to_ascii_uppercase().as_str() {
        "TRUE" => Some(true),
        "FALSE" => Some(false),
        _ => None,
    }
}

/// A tab or line break inside a field would shift the columns or start a new line, so a
/// cookie for another site could be slipped into the file.
fn check_fields(cookie: &Cookie) -> Result<()> {
    let fields = [
        cookie.domain.as_str(),
        &cookie.path,
        &cookie.name,
        cookie.value.expose(),
    ];
    if fields
        .iter()
        .any(|field| field.contains(['\t', '\n', '\r']))
    {
        return Err(Error::InvalidCookie {
            cookie_name: cookie.name.escape_debug().to_string(),
        });
    }
    Ok(())
}

/// `TRUE` or `FALSE`, as the Netscape format writes them.
fn flag(value: bool) -> &'static str {
    if value { "TRUE" } else { "FALSE" }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::SecretValue;

    fn cookie(domain: &str, name: &str, value: &str) -> Cookie {
        Cookie {
            domain: domain.into(),
            name: name.into(),
            value: SecretValue::new(value.into()),
            path: "/".into(),
            secure: true,
            http_only: true,
            expires: Some(1_900_000_000),
        }
    }

    #[test]
    fn writes_one_line_per_cookie_in_netscape_format() {
        let mut session = cookie("mediaserver.unige.ch", "csrftoken", "b");
        session.secure = false;
        session.expires = None;
        let text = to_netscape(&[cookie(".unige.ch", "sessionid", "a"), session]).unwrap();
        assert_eq!(
            text.as_str(),
            "# Netscape HTTP Cookie File\n\
             .unige.ch\tTRUE\t/\tTRUE\t1900000000\tsessionid\ta\n\
             mediaserver.unige.ch\tFALSE\t/\tFALSE\t0\tcsrftoken\tb\n"
        );
    }

    use crate::clock::tests::NOW;

    /// A file with cookies for the lecture's site, its parent domain and another site.
    const FILE: &str = "\u{feff}# Netscape HTTP Cookie File\r\n\
        # a comment\r\n\
        \r\n\
        #HttpOnly_mediaserver.unige.ch\tFALSE\t/\tTRUE\t1900000000\tmediaserver_session\tabc\r\n\
        unige.ch\tTRUE\t/\tFALSE\t0\t_shibsession\tdef\r\n\
        .unige.ch\tTRUE\t/\tFALSE\t1000\told\tgone\r\n\
        bank.example\tFALSE\t/\tTRUE\t1900000000\tsecret\tdo-not-return\r\n";

    #[test]
    fn reads_only_the_sites_unexpired_cookies() {
        let cookies = from_netscape_at(FILE, "mediaserver.unige.ch", NOW).unwrap();
        let names: Vec<&str> = cookies.iter().map(|c| c.name.as_str()).collect();
        assert_eq!(names, ["mediaserver_session", "_shibsession"]);
        assert!(cookies[0].http_only && cookies[0].secure);
        assert_eq!(cookies[0].expires, Some(1_900_000_000));
        assert_eq!(cookies[0].value.expose(), "abc");
        assert_eq!(
            cookies[1].domain, ".unige.ch",
            "a domain cookie gets its dot"
        );
        assert_eq!(cookies[1].expires, None, "0 is a session cookie");
        assert!(!cookies[1].http_only && !cookies[1].secure);
    }

    #[test]
    fn a_file_without_the_site_gives_no_cookies() {
        assert!(
            from_netscape_at(FILE, "moodle.unige.org", NOW)
                .unwrap()
                .is_empty()
        );
        assert!(from_netscape_at("", "a.ch", NOW).unwrap().is_empty());
        assert!(matches!(
            from_netscape_at(FILE, "not a host", NOW),
            Err(Error::InvalidHost(_))
        ));
    }

    #[test]
    fn a_line_that_is_not_netscape_names_its_number_only() {
        for bad in [
            "{\"cookies\": \"secret-value\"}",
            "a.ch\tMAYBE\t/\tTRUE\t0\tn\tsecret-value",
            "a.ch\tTRUE\t/\tTRUE\tsoon\tn\tsecret-value",
            "a.ch\tTRUE\t/\tTRUE\t0\tn\tsecret-value\textra",
        ] {
            let text = format!("# Netscape HTTP Cookie File\n{bad}\n");
            let err = from_netscape_at(&text, "a.ch", NOW).unwrap_err();
            assert!(matches!(err, Error::InvalidCookieFile { line: 2 }), "{bad}");
            assert!(!err.to_string().contains("secret"), "{err}");
        }
    }

    #[test]
    fn what_this_crate_writes_it_reads_back() {
        let written = to_netscape(&[cookie(".unige.ch", "sessionid", "a")]).unwrap();
        let read = cookies_from_netscape(&written, "www.unige.ch").unwrap();
        assert_eq!(read[0].value.expose(), "a");
        assert_eq!(read[0].domain, ".unige.ch");
    }

    #[test]
    fn no_cookies_gives_only_the_header() {
        assert_eq!(to_netscape(&[]).unwrap().as_str(), HEADER);
    }

    #[test]
    fn a_line_break_in_a_value_is_refused_without_showing_the_value() {
        let err = to_netscape(&[cookie(
            ".unige.ch",
            "sessionid",
            "x\n.evil.com\tTRUE\t/\tTRUE\t0\tsteal\tme",
        )])
        .unwrap_err();
        assert!(matches!(&err, Error::InvalidCookie { cookie_name } if cookie_name == "sessionid"));
        assert!(!err.to_string().contains("evil"));
    }

    #[test]
    fn a_tab_in_any_field_is_refused() {
        for bad in [
            cookie("a\tb", "n", "v"),
            cookie("a", "n\t", "v"),
            cookie("a", "n", "v\r"),
        ] {
            assert!(matches!(
                to_netscape(&[bad]),
                Err(Error::InvalidCookie { .. })
            ));
        }
        let mut bad_path = cookie("a", "n", "v");
        bad_path.path = "/\t".into();
        assert!(to_netscape(&[bad_path]).is_err());
    }
}
