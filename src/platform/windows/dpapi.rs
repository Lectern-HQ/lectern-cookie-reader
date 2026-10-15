//! Windows DPAPI: unprotects data that Windows protected for the current user, without any
//! dialog. Chromium protects its cookie key this way.

use windows_sys::Win32::Foundation::{GetLastError, LocalFree};
use windows_sys::Win32::Security::Cryptography::{
    CRYPT_INTEGER_BLOB, CRYPTPROTECT_UI_FORBIDDEN, CryptUnprotectData,
};
use zeroize::{Zeroize, Zeroizing};

use crate::{Error, Result};

/// The data `protected` holds, for the current user only. Fails with Windows' error code
/// if the data was protected for another user or is damaged.
pub(crate) fn unprotect(protected: &[u8]) -> Result<Zeroizing<Vec<u8>>> {
    let input = CRYPT_INTEGER_BLOB {
        cbData: u32::try_from(protected.len()).map_err(|_| Error::KeyUnavailable(-1))?,
        pbData: protected.as_ptr().cast_mut(),
    };
    let mut output = CRYPT_INTEGER_BLOB {
        cbData: 0,
        pbData: std::ptr::null_mut(),
    };
    // SAFETY: `input` points to `protected`, which outlives the call and is only read.
    // On success Windows allocates `output` with LocalAlloc; it is copied, cleared and
    // freed once below.
    let ok = unsafe {
        CryptUnprotectData(
            &input,
            std::ptr::null_mut(),
            std::ptr::null(),
            std::ptr::null(),
            std::ptr::null(),
            CRYPTPROTECT_UI_FORBIDDEN,
            &mut output,
        )
    };
    if ok == 0 {
        // SAFETY: reads the calling thread's last error code, set by the failed call.
        let code = unsafe { GetLastError() };
        return Err(Error::KeyUnavailable(i32::from_ne_bytes(
            code.to_ne_bytes(),
        )));
    }
    // SAFETY: on success `output` holds `cbData` bytes at `pbData`, owned by us.
    let data = unsafe {
        let bytes = std::slice::from_raw_parts_mut(output.pbData, output.cbData as usize);
        let copy = Zeroizing::new(bytes.to_vec());
        bytes.zeroize();
        LocalFree(output.pbData.cast());
        copy
    };
    Ok(data)
}

#[cfg(test)]
mod tests {
    use super::*;
    use windows_sys::Win32::Security::Cryptography::CryptProtectData;

    /// Protects `data` for the current user, as Chromium does with its key.
    fn protect(data: &[u8]) -> Vec<u8> {
        let input = CRYPT_INTEGER_BLOB {
            cbData: data.len() as u32,
            pbData: data.as_ptr().cast_mut(),
        };
        let mut output = CRYPT_INTEGER_BLOB {
            cbData: 0,
            pbData: std::ptr::null_mut(),
        };
        // SAFETY: as in `unprotect`; the test frees the output once.
        unsafe {
            assert_ne!(
                CryptProtectData(
                    &input,
                    std::ptr::null(),
                    std::ptr::null(),
                    std::ptr::null(),
                    std::ptr::null(),
                    CRYPTPROTECT_UI_FORBIDDEN,
                    &mut output,
                ),
                0
            );
            let protected =
                std::slice::from_raw_parts(output.pbData, output.cbData as usize).to_vec();
            LocalFree(output.pbData.cast());
            protected
        }
    }

    #[test]
    fn protected_data_comes_back_for_the_same_user() {
        let key: Vec<u8> = (0..32).collect();
        let protected = protect(&key);
        assert_ne!(protected, key);
        assert_eq!(unprotect(&protected).unwrap().as_slice(), key.as_slice());
    }

    #[test]
    fn damaged_data_is_an_error_with_the_windows_code() {
        let mut protected = protect(b"secret");
        let last = protected.len() - 1;
        protected[last] ^= 0xff;
        assert!(matches!(
            unprotect(&protected),
            Err(Error::KeyUnavailable(_))
        ));
    }
}
