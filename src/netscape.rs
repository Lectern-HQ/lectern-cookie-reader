//! The Netscape cookie file format that yt-dlp and curl read: plain functions, tested
//! without touching the disk.

use zeroize::Zeroizing;

use crate::{Cookie, Error, Result};

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
