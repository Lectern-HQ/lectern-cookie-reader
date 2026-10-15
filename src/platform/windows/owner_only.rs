//! Folders only the current Windows user can open: created with an access list that has one
//! entry, full control for the current user, and inherits nothing from its parent. Files
//! created inside inherit that entry, so they are private too.

use std::ffi::OsStr;
use std::io;
use std::os::windows::ffi::OsStrExt;
use std::path::Path;

use windows_sys::Win32::Foundation::{CloseHandle, HANDLE, LocalFree};
use windows_sys::Win32::Security::Authorization::{
    ConvertSidToStringSidW, ConvertStringSecurityDescriptorToSecurityDescriptorW, SDDL_REVISION_1,
};
use windows_sys::Win32::Security::{
    GetTokenInformation, PSECURITY_DESCRIPTOR, SECURITY_ATTRIBUTES, TOKEN_QUERY, TOKEN_USER,
    TokenUser,
};
use windows_sys::Win32::Storage::FileSystem::CreateDirectoryW;
use windows_sys::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};
use windows_sys::core::PWSTR;

/// Creates the folder `path` with an access list that lets only the current user in.
/// Fails with [`io::ErrorKind::AlreadyExists`] if `path` exists, so a folder prepared by
/// someone else is never reused.
pub(crate) fn create_owner_only_dir(path: &Path) -> io::Result<()> {
    // P: protected, inherits nothing. OICI: files and folders inside inherit the entry.
    // FA: full access, for the current user's SID only.
    let sddl = wide(OsStr::new(&format!(
        "D:P(A;OICI;FA;;;{})",
        current_user_sid()?
    )));
    let mut descriptor: PSECURITY_DESCRIPTOR = std::ptr::null_mut();
    // SAFETY: `sddl` is a valid, zero-terminated wide string that outlives the call. On
    // success Windows allocates `descriptor` with LocalAlloc; it is freed once below.
    let ok = unsafe {
        ConvertStringSecurityDescriptorToSecurityDescriptorW(
            sddl.as_ptr(),
            SDDL_REVISION_1,
            &mut descriptor,
            std::ptr::null_mut(),
        )
    };
    if ok == 0 {
        return Err(io::Error::last_os_error());
    }
    let attributes = SECURITY_ATTRIBUTES {
        nLength: size_of::<SECURITY_ATTRIBUTES>() as u32,
        lpSecurityDescriptor: descriptor,
        bInheritHandle: 0,
    };
    let path = wide(path.as_os_str());
    // SAFETY: `path` is zero-terminated and `attributes` points to the descriptor made
    // above; both outlive the call. The descriptor is freed once, after the call.
    unsafe {
        let created = CreateDirectoryW(path.as_ptr(), &attributes);
        let error = io::Error::last_os_error();
        LocalFree(descriptor);
        if created == 0 { Err(error) } else { Ok(()) }
    }
}

/// The current user's security identifier as text, for example `S-1-5-21-...-1001`.
fn current_user_sid() -> io::Result<String> {
    let mut token: HANDLE = std::ptr::null_mut();
    // SAFETY: opens the current process's own token for reading; closed once below.
    if unsafe { OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) } == 0 {
        return Err(io::Error::last_os_error());
    }
    let sid = token_user_sid(token);
    // SAFETY: `token` was opened above and is closed exactly once.
    unsafe { CloseHandle(token) };
    sid
}

/// The user SID in the access token `token`, as text.
fn token_user_sid(token: HANDLE) -> io::Result<String> {
    let mut len = 0u32;
    // SAFETY: asks only for the size; Windows writes it to `len`.
    unsafe { GetTokenInformation(token, TokenUser, std::ptr::null_mut(), 0, &mut len) };
    // `u64` elements keep the buffer aligned for `TOKEN_USER`.
    let mut buffer = vec![0u64; (len as usize).div_ceil(size_of::<u64>())];
    // SAFETY: `buffer` holds at least `len` bytes and outlives the call.
    let ok =
        unsafe { GetTokenInformation(token, TokenUser, buffer.as_mut_ptr().cast(), len, &mut len) };
    if ok == 0 {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: on success the buffer starts with a `TOKEN_USER`, whose SID points into the
    // buffer, which lives until the end of this function.
    let sid = unsafe { (*buffer.as_ptr().cast::<TOKEN_USER>()).User.Sid };
    let mut text: PWSTR = std::ptr::null_mut();
    // SAFETY: `sid` is valid (see above). On success Windows allocates `text` with
    // LocalAlloc; it is read, then freed once.
    unsafe {
        if ConvertSidToStringSidW(sid, &mut text) == 0 {
            return Err(io::Error::last_os_error());
        }
        let result = from_wide(text);
        LocalFree(text.cast());
        Ok(result)
    }
}

/// `text` as a zero-terminated wide string for Windows.
pub(crate) fn wide(text: &OsStr) -> Vec<u16> {
    text.encode_wide().chain(std::iter::once(0)).collect()
}

/// A zero-terminated wide string from Windows as a `String`.
///
/// # Safety
/// `text` must point to a zero-terminated wide string.
pub(crate) unsafe fn from_wide(text: PWSTR) -> String {
    // SAFETY: the caller guarantees a zero terminator, so every read is inside the string.
    unsafe {
        let len = (0..).take_while(|&i| *text.add(i) != 0).count();
        String::from_utf16_lossy(std::slice::from_raw_parts(text, len))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use windows_sys::Win32::Security::Authorization::{
        ConvertSecurityDescriptorToStringSecurityDescriptorW, GetNamedSecurityInfoW, SE_FILE_OBJECT,
    };
    use windows_sys::Win32::Security::DACL_SECURITY_INFORMATION;

    /// The access list of the file or folder at `path`, as SDDL text.
    fn access_list(path: &Path) -> String {
        let name = wide(path.as_os_str());
        let mut descriptor: PSECURITY_DESCRIPTOR = std::ptr::null_mut();
        let mut text: PWSTR = std::ptr::null_mut();
        // SAFETY: test helper; every pointer Windows returns is freed once.
        unsafe {
            assert_eq!(
                GetNamedSecurityInfoW(
                    name.as_ptr(),
                    SE_FILE_OBJECT,
                    DACL_SECURITY_INFORMATION,
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                    &mut descriptor,
                ),
                0
            );
            assert_ne!(
                ConvertSecurityDescriptorToStringSecurityDescriptorW(
                    descriptor,
                    SDDL_REVISION_1,
                    DACL_SECURITY_INFORMATION,
                    &mut text,
                    std::ptr::null_mut(),
                ),
                0
            );
            let sddl = from_wide(text);
            LocalFree(text.cast());
            LocalFree(descriptor);
            sddl
        }
    }

    #[test]
    fn only_the_current_user_can_open_the_folder_and_its_files() {
        let parent = std::env::temp_dir().join(format!("lectern-acl-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&parent);
        create_owner_only_dir(&parent).unwrap();
        let file = parent.join("cookies.txt");
        std::fs::write(&file, "x").unwrap();
        let sid = current_user_sid().unwrap();
        let folder_acl = access_list(&parent);
        let file_acl = access_list(&file);
        let _ = std::fs::remove_dir_all(&parent);

        assert!(sid.starts_with("S-1-"), "{sid}");
        // The folder inherits nothing (P); the file inherits the folder's one entry (ID).
        assert!(folder_acl.starts_with("D:P"), "{folder_acl}");
        assert!(file_acl.contains(";ID;"), "{file_acl}");
        for acl in [&folder_acl, &file_acl] {
            assert_eq!(acl.matches('(').count(), 1, "one entry only: {acl}");
            assert!(acl.contains(&format!(";FA;;;{sid})")), "{acl}");
        }
    }

    #[test]
    fn an_existing_folder_is_never_reused() {
        let path = std::env::temp_dir().join(format!("lectern-acl-exists-{}", std::process::id()));
        std::fs::create_dir_all(&path).unwrap();
        let err = create_owner_only_dir(&path).unwrap_err();
        let _ = std::fs::remove_dir_all(&path);
        assert_eq!(err.kind(), io::ErrorKind::AlreadyExists);
    }
}
