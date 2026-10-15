//! Generated Safari cookie files for tests, never real cookies.

pub(crate) use crate::chromium::test_vectors::NOW;

/// Seconds from 1970 to 2001, where Safari counts time from.
const MAC_EPOCH: i64 = 978_307_200;
/// Bytes in a cookie's header, before its strings.
const HEADER_LEN: u32 = 56;

/// A cookie to generate, with the value `value-of-<name>`, created 10 seconds before `NOW`.
pub(crate) struct Generated {
    domain: &'static str,
    name: &'static str,
    expires: i64,
    created: i64,
    flags: u32,
    domain_offset: Option<u32>,
    final_zero: bool,
    expiry_bits: Option<f64>,
}

impl Generated {
    pub(crate) fn new(domain: &'static str, name: &'static str, expires: i64) -> Self {
        Self {
            domain,
            name,
            expires,
            created: NOW - 10,
            flags: 0,
            domain_offset: None,
            final_zero: true,
            expiry_bits: None,
        }
    }

    pub(crate) fn flags(mut self, flags: u32) -> Self {
        self.flags = flags;
        self
    }

    pub(crate) fn created(mut self, unix_secs: i64) -> Self {
        self.created = unix_secs;
        self
    }

    pub(crate) fn domain_offset(mut self, offset: u32) -> Self {
        self.domain_offset = Some(offset);
        self
    }

    pub(crate) fn without_final_zero(mut self) -> Self {
        self.final_zero = false;
        self
    }

    pub(crate) fn expiry_bits(mut self, seconds_since_2001: f64) -> Self {
        self.expiry_bits = Some(seconds_since_2001);
        self
    }

    /// The cookie's bytes.
    fn bytes(&self) -> Vec<u8> {
        let value = format!("value-of-{}", self.name);
        let strings = [self.domain, self.name, "/", value.as_str()];
        let mut offsets = Vec::new();
        let mut body = Vec::new();
        for s in strings {
            offsets.push(HEADER_LEN + body.len() as u32);
            body.extend_from_slice(s.as_bytes());
            body.push(0);
        }
        if !self.final_zero {
            body.pop();
        }
        if let Some(offset) = self.domain_offset {
            offsets[0] = offset;
        }
        let size = HEADER_LEN + body.len() as u32;
        let mut out = Vec::new();
        for n in [size, 0, self.flags, 0] {
            out.extend_from_slice(&n.to_le_bytes());
        }
        for n in offsets {
            out.extend_from_slice(&n.to_le_bytes());
        }
        out.extend_from_slice(&[0; 8]);
        let expiry = self
            .expiry_bits
            .unwrap_or((self.expires - MAC_EPOCH) as f64);
        out.extend_from_slice(&expiry.to_le_bytes());
        out.extend_from_slice(&((self.created - MAC_EPOCH) as f64).to_le_bytes());
        out.extend_from_slice(&body);
        out
    }
}

/// A page holding `cookies`.
pub(crate) fn page(cookies: &[Generated]) -> Vec<u8> {
    let records: Vec<Vec<u8>> = cookies.iter().map(Generated::bytes).collect();
    let header_len = 4 + 4 + 4 * records.len() + 4;
    let mut out = vec![0, 0, 1, 0];
    out.extend_from_slice(&(records.len() as u32).to_le_bytes());
    let mut offset = header_len;
    for record in &records {
        out.extend_from_slice(&(offset as u32).to_le_bytes());
        offset += record.len();
    }
    out.extend_from_slice(&[0; 4]);
    for record in records {
        out.extend_from_slice(&record);
    }
    out
}

/// A cookie file holding `pages`, without the checksum Safari writes after them.
pub(crate) fn file(pages: &[Vec<u8>]) -> Vec<u8> {
    let mut out = b"cook".to_vec();
    out.extend_from_slice(&(pages.len() as u32).to_be_bytes());
    for page in pages {
        out.extend_from_slice(&(page.len() as u32).to_be_bytes());
    }
    for page in pages {
        out.extend_from_slice(page);
    }
    out
}
