//! A parser for Safari's `Cookies.binarycookies` format. It is a plain reader of bytes: it
//! never opens files, and every offset is checked against the data before it is used.
//!
//! The layout, as read by yt-dlp (`yt_dlp/cookies.py`):
//!
//! - File: `cook`, the number of pages (big-endian `u32`), each page's size (big-endian
//!   `u32`), then the pages. A checksum and other data follow the pages and are ignored.
//! - Page: `00 00 01 00`, the number of cookies (little-endian `u32`), each cookie's
//!   offset in the page (little-endian `u32`).
//! - Cookie: its size, an unknown field, flags (1 = secure, 4 = HTTP only), an unknown
//!   field, the offsets of its domain, name, path and value inside the cookie, 8 unknown
//!   bytes, then expiry and creation time as little-endian `f64` seconds since 2001-01-01.
//!   The strings follow, each ending with a zero byte.

use crate::{Error, Result};

/// Every file starts with these bytes.
const FILE_SIGNATURE: &[u8] = b"cook";
/// Every page starts with these bytes.
const PAGE_SIGNATURE: &[u8] = &[0, 0, 1, 0];
/// The cookie's flag for "sent over https only".
const FLAG_SECURE: u32 = 0x1;
/// The cookie's flag for "hidden from the site's scripts".
const FLAG_HTTP_ONLY: u32 = 0x4;
/// Bytes from a cookie's start to its expiry time.
const EXPIRY_OFFSET: usize = 40;
/// Bytes from a cookie's start to its creation time.
const CREATION_OFFSET: usize = 48;
/// Seconds from 1970-01-01, where Unix counts time from, to 2001-01-01, where macOS does.
const MAC_EPOCH_OFFSET_SECS: i64 = 978_307_200;

/// One cookie as stored, with its value still as bytes.
#[derive(PartialEq)]
pub(crate) struct Record<'a> {
    /// The cookie's domain, for example `.example.com`.
    pub(crate) domain: &'a str,
    /// The cookie's name.
    pub(crate) name: &'a str,
    /// The path on the site the cookie is sent for.
    pub(crate) path: &'a str,
    /// The value, never printed: see the `Debug` implementation.
    pub(crate) value: &'a [u8],
    /// Whether the cookie is sent over https only.
    pub(crate) secure: bool,
    /// Whether the browser hides the cookie from the site's scripts.
    pub(crate) http_only: bool,
    /// Expiry as Unix seconds.
    pub(crate) expires: i64,
    /// Creation time as Unix seconds.
    pub(crate) created: i64,
}

impl std::fmt::Debug for Record<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Record({} {})", self.domain, self.name)
    }
}

/// Every cookie in the file `data`, in the order stored. Fails on the first part that
/// does not follow the format, without saying more than where.
pub(crate) fn parse(data: &[u8]) -> Result<Vec<Record<'_>>> {
    let mut file = Reader::new(data);
    file.expect(FILE_SIGNATURE, "file signature")?;
    let page_count = file.u32_be()?;
    let page_sizes = (0..page_count)
        .map(|_| file.u32_be())
        .collect::<Result<Vec<u32>>>()?;
    let mut records = Vec::new();
    for size in page_sizes {
        let page = file.take(to_usize(size), "page")?;
        records.extend(parse_page(page)?);
    }
    Ok(records)
}

/// The cookies of one page.
fn parse_page(page: &[u8]) -> Result<Vec<Record<'_>>> {
    let mut reader = Reader::new(page);
    reader.expect(PAGE_SIGNATURE, "page signature")?;
    let count = reader.u32_le()?;
    (0..count)
        .map(|_| {
            let offset = to_usize(reader.u32_le()?);
            parse_record(page.get(offset..).ok_or_else(|| invalid("cookie offset"))?)
        })
        .collect()
}

/// One cookie, from the bytes that start with it.
fn parse_record(data: &[u8]) -> Result<Record<'_>> {
    let size = to_usize(Reader::new(data).u32_le()?);
    let record = data.get(..size).ok_or_else(|| invalid("cookie size"))?;
    let mut header = Reader::new(record);
    let _size = header.u32_le()?;
    let _unknown = header.u32_le()?;
    let flags = header.u32_le()?;
    let _unknown = header.u32_le()?;
    let mut offsets = [0usize; 4];
    for offset in &mut offsets {
        *offset = to_usize(header.u32_le()?);
    }
    let [domain, name, path, value] = offsets;
    Ok(Record {
        domain: text(record, domain, "cookie domain")?,
        name: text(record, name, "cookie name")?,
        path: text(record, path, "cookie path")?,
        value: zero_terminated(record, value, "cookie value")?,
        secure: flags & FLAG_SECURE != 0,
        http_only: flags & FLAG_HTTP_ONLY != 0,
        expires: mac_time(record, EXPIRY_OFFSET)?,
        created: mac_time(record, CREATION_OFFSET)?,
    })
}

/// The zero-terminated bytes at `offset` in `record`, without the zero.
fn zero_terminated<'a>(record: &'a [u8], offset: usize, what: &str) -> Result<&'a [u8]> {
    let rest = record.get(offset..).ok_or_else(|| invalid(what))?;
    let end = rest
        .iter()
        .position(|&b| b == 0)
        .ok_or_else(|| invalid(what))?;
    Ok(&rest[..end])
}

/// The zero-terminated UTF-8 text at `offset` in `record`.
fn text<'a>(record: &'a [u8], offset: usize, what: &str) -> Result<&'a str> {
    std::str::from_utf8(zero_terminated(record, offset, what)?).map_err(|_| invalid(what))
}

/// The time at `offset` in `record`, stored as `f64` seconds since 2001, as Unix seconds.
fn mac_time(record: &[u8], offset: usize) -> Result<i64> {
    let bytes = record
        .get(offset..offset + 8)
        .and_then(|b| <[u8; 8]>::try_from(b).ok())
        .ok_or_else(|| invalid("cookie time"))?;
    let seconds = f64::from_le_bytes(bytes);
    if !seconds.is_finite() {
        return Err(invalid("cookie time"));
    }
    // `as` saturates for times beyond i64, which is what an expiry that far away means.
    Ok((seconds as i64).saturating_add(MAC_EPOCH_OFFSET_SECS))
}

/// A `u32` from the file as an index. Every target this crate supports has 32-bit or
/// wider pointers.
fn to_usize(value: u32) -> usize {
    usize::try_from(value).unwrap_or(usize::MAX)
}

/// The error for a file that does not follow the format at `what`.
fn invalid(what: &str) -> Error {
    Error::DatabaseQuery(format!("not a Safari cookie file: unexpected {what}"))
}

/// Reads numbers and byte runs from the front of a slice, refusing to read past its end.
struct Reader<'a> {
    /// The bytes not read yet.
    rest: &'a [u8],
}

impl<'a> Reader<'a> {
    /// A reader at the start of `data`.
    fn new(data: &'a [u8]) -> Self {
        Self { rest: data }
    }

    /// The next `len` bytes; `what` names them in the error.
    fn take(&mut self, len: usize, what: &str) -> Result<&'a [u8]> {
        if len > self.rest.len() {
            return Err(invalid(what));
        }
        let (taken, rest) = self.rest.split_at(len);
        self.rest = rest;
        Ok(taken)
    }

    /// Checks that the next bytes are `expected`.
    fn expect(&mut self, expected: &[u8], what: &str) -> Result<()> {
        if self.take(expected.len(), what)? == expected {
            Ok(())
        } else {
            Err(invalid(what))
        }
    }

    /// The next four bytes as a big-endian `u32`.
    fn u32_be(&mut self) -> Result<u32> {
        Ok(u32::from_be_bytes(self.four()?))
    }

    /// The next four bytes as a little-endian `u32`.
    fn u32_le(&mut self) -> Result<u32> {
        Ok(u32::from_le_bytes(self.four()?))
    }

    /// The next four bytes.
    fn four(&mut self) -> Result<[u8; 4]> {
        let bytes = self.take(4, "number")?;
        <[u8; 4]>::try_from(bytes).map_err(|_| invalid("number"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::safari::test_data::{Generated, NOW, file, page};

    fn generated(domain: &'static str, name: &'static str) -> Generated {
        Generated::new(domain, name, NOW + 60)
    }

    #[test]
    fn reads_every_cookie_of_every_page() {
        let data = file(&[
            page(&[
                generated(".example.com", "a").flags(FLAG_SECURE | FLAG_HTTP_ONLY),
                generated("www.example.com", "b"),
            ]),
            page(&[generated(".other.com", "c")]),
        ]);
        let records = parse(&data).unwrap();
        let names: Vec<_> = records.iter().map(|r| (r.domain, r.name)).collect();
        assert_eq!(
            names,
            [
                (".example.com", "a"),
                ("www.example.com", "b"),
                (".other.com", "c")
            ]
        );
        let a = &records[0];
        assert!(a.secure && a.http_only);
        assert!(!records[1].secure && !records[1].http_only);
        assert_eq!(a.value, b"value-of-a");
        assert_eq!(a.path, "/");
        assert_eq!(a.expires, NOW + 60);
        assert_eq!(a.created, NOW - 10);
    }

    #[test]
    fn the_checksum_after_the_pages_is_ignored() {
        let mut data = file(&[page(&[generated(".example.com", "a")])]);
        data.extend_from_slice(&[1, 2, 3, 4, 0x07, 0x17, 0x20, 0x05, 0, 0, 0, 0x28]);
        assert_eq!(parse(&data).unwrap().len(), 1);
    }

    #[test]
    fn an_empty_file_and_an_empty_page_have_no_cookies() {
        assert!(parse(&file(&[])).unwrap().is_empty());
        assert!(parse(&file(&[page(&[])])).unwrap().is_empty());
    }

    #[test]
    fn data_that_is_not_a_cookie_file_is_refused() {
        assert!(parse(b"").is_err());
        assert!(parse(b"bplist00").is_err());
        let mut wrong_page = file(&[page(&[generated(".a.com", "a")])]);
        wrong_page[12] = 9; // The page signature starts after `cook`, the count and one size.
        assert!(parse(&wrong_page).is_err());
    }

    #[test]
    fn every_cut_short_file_is_refused_without_a_panic() {
        let data = file(&[page(&[generated(".example.com", "a")])]);
        for len in 0..data.len() {
            assert!(parse(&data[..len]).is_err(), "cut at {len}");
        }
    }

    #[test]
    fn offsets_outside_the_cookie_are_refused() {
        let data = file(&[page(
            &[generated(".example.com", "a").domain_offset(10_000)],
        )]);
        let err = parse(&data).unwrap_err();
        assert!(err.to_string().contains("cookie domain"), "{err}");
    }

    #[test]
    fn a_string_without_its_end_is_refused() {
        let data = file(&[page(&[generated(".example.com", "a").without_final_zero()])]);
        assert!(parse(&data).is_err());
    }

    #[test]
    fn a_time_that_is_not_a_number_is_refused() {
        let data = file(&[page(
            &[generated(".example.com", "a").expiry_bits(f64::NAN)],
        )]);
        assert!(parse(&data).is_err());
    }

    #[test]
    fn debug_never_shows_the_value() {
        let data = file(&[page(&[generated(".example.com", "a")])]);
        let records = parse(&data).unwrap();
        let printed = format!("{:?}", records[0]);
        assert_eq!(printed, "Record(.example.com a)");
    }
}
