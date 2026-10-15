//! The key a Chromium browser on Windows keeps in its `Local State` file: base64 text at
//! `os_crypt.encrypted_key`, which decodes to `DPAPI` followed by the key protected with
//! Windows DPAPI for the current user.

use std::path::Path;

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use zeroize::Zeroizing;

use crate::files::{existing, unreadable};
use crate::{Error, Result};

/// The file's name in the browser's data folder.
const FILE_NAME: &str = "Local State";
/// What the decoded key starts with before the DPAPI data.
const DPAPI_PREFIX: &[u8] = b"DPAPI";

/// The DPAPI-protected key in the `Local State` file of the browser data folder
/// `data_dir`. A browser that never ran has no such file: [`Error::KeyNotFound`].
pub(crate) fn read_protected_key(data_dir: &Path) -> Result<Zeroizing<Vec<u8>>> {
    let path = data_dir.join(FILE_NAME);
    let text = existing(std::fs::read_to_string(&path), &path)?.ok_or(Error::KeyNotFound)?;
    protected_key(&Zeroizing::new(text), &path)
}

/// The DPAPI-protected key in `text`, the content of the `Local State` file at `path`.
/// `path` only names the file in errors.
fn protected_key(text: &str, path: &Path) -> Result<Zeroizing<Vec<u8>>> {
    let unreadable = |reason: &str| unreadable(path, reason);
    let state: serde_json::Value =
        serde_json::from_str(text).map_err(|_| unreadable("not valid JSON"))?;
    let encoded = state
        .pointer("/os_crypt/encrypted_key")
        .and_then(serde_json::Value::as_str)
        .ok_or(Error::KeyNotFound)?;
    let decoded = Zeroizing::new(
        STANDARD
            .decode(encoded)
            .map_err(|_| unreadable("the key is not base64"))?,
    );
    let key = decoded
        .strip_prefix(DPAPI_PREFIX)
        .ok_or_else(|| unreadable("the key is not protected with DPAPI"))?;
    Ok(Zeroizing::new(key.to_vec()))
}

#[cfg(test)]
mod tests {
    use super::*;

    const PATH: &str = "Local State";

    fn state_with_key(encoded: &str) -> String {
        format!(r#"{{"browser":{{}},"os_crypt":{{"encrypted_key":"{encoded}"}}}}"#)
    }

    #[test]
    fn the_protected_key_follows_the_dpapi_prefix() {
        let encoded = STANDARD.encode(b"DPAPI\x01\x02\x03");
        let key = protected_key(&state_with_key(&encoded), Path::new(PATH)).unwrap();
        assert_eq!(key.as_slice(), [1, 2, 3]);
    }

    #[test]
    fn the_key_is_read_from_the_data_folder() {
        let data = crate::private_dir::PrivateDir::new().unwrap();
        assert!(matches!(
            read_protected_key(data.path()),
            Err(Error::KeyNotFound)
        ));
        let encoded = STANDARD.encode(b"DPAPI\x07");
        std::fs::write(data.path().join(FILE_NAME), state_with_key(&encoded)).unwrap();
        assert_eq!(read_protected_key(data.path()).unwrap().as_slice(), [7]);
        let folder = data.path().join("folder");
        std::fs::create_dir_all(folder.join(FILE_NAME)).unwrap();
        // A folder is unreadable as a file on macOS, and access is denied on Windows.
        assert!(matches!(
            read_protected_key(&folder),
            Err(Error::DatabaseUnreadable { .. } | Error::PermissionNeeded(_))
        ));
    }

    #[test]
    fn a_state_without_a_key_is_key_not_found() {
        let err = protected_key(r#"{"os_crypt":{}}"#, Path::new(PATH)).unwrap_err();
        assert!(matches!(err, Error::KeyNotFound));
    }

    #[test]
    fn a_broken_state_names_the_file_but_not_its_content() {
        let cases = [
            "not json".to_owned(),
            state_with_key("!!!"),
            state_with_key(&STANDARD.encode(b"PLAIN-secret")),
        ];
        for text in cases {
            let err = protected_key(&text, Path::new(PATH)).unwrap_err();
            assert!(
                matches!(&err, Error::DatabaseUnreadable { path, .. } if path == Path::new(PATH))
            );
            assert!(!err.to_string().contains("secret"));
        }
    }
}
