# lectern-cookie-reader

A small Rust library that reads the login cookies for **one website** from a browser on your own computer (macOS on Apple Silicon, or Windows x64), so a desktop app can download something you are already allowed to see, without asking for your password.

It is the part of [Lectern](https://github.com/Lectern-HQ) that touches your browser data, published so anyone can check exactly what it does.

Status: version 0.3.0 is released; the next version adds the Firefox family, Safari and Windows (see [CHANGELOG.md](CHANGELOG.md)). Reading cookies is tested on macOS with Arc, Dia, Google Chrome, Firefox and Safari; the other browsers and Windows are in the library but not tested on a real computer yet.

## Usage

```toml
[dependencies]
lectern-cookie-reader = { git = "https://github.com/Lectern-HQ/lectern-cookie-reader", tag = "v0.3.0" }
```

```rust
use lectern_cookie_reader::{Browser, CookieFile, Error, read_cookies};

fn download(host: &str) -> Result<(), Error> {
    let profile = Browser::by_id("dia").expect("Dia is in the catalog").default_profile()?;
    let cookies = read_cookies(&profile, host)?; // macOS may ask for permission here; Windows does not
    if cookies.is_empty() {
        // Not logged in to this site in this profile.
        return Ok(());
    }
    let file = CookieFile::create(&cookies)?;
    // Run the downloader, for example: yt-dlp --cookies <file.path()> <link>
    drop(file); // The file and its folder are deleted here, also on error or panic.
    Ok(())
}
```

`host` is a host name such as `mediaserver.unige.ch`, not a link. To get it from a lecture link, use `host_of_link(link)`, which checks the link with the same rules as the rest of the crate.

To suggest which browser to use, without decrypting anything and without any permission dialog:

```rust
use lectern_cookie_reader::{Error, Reason, default_browser, find_logins, suggest};

fn which_browser(host: &str) -> Result<(), Error> {
    let scan = find_logins(host)?;
    match suggest(&scan, default_browser().as_ref()) {
        Some(s) if s.reason == Reason::LoggedIn => {
            println!("Using your {} login", s.profile.browser.name())
        }
        Some(s) => println!("No login for {host}. Try {}?", s.profile.browser.name()),
        None => println!("No browser found"),
    }
    for skipped in &scan.skipped {
        println!("Could not check {}: {}", skipped.browser.name(), skipped.error);
    }
    Ok(())
}
```

The suggestion is a profile logged in to the site (most recently used first), else the
system's default browser, else the most recently used profile. Show it for the user to
confirm: the default browser is often not the one used for lectures.

### Errors

Every error says what failed, never a cookie value. The ones an app usually handles:

| Error | Meaning | What the app can do |
| --- | --- | --- |
| `KeyAccessDenied` | The user clicked "Deny" in the macOS dialog. | Explain why the key is needed and offer to try again. |
| `KeyNotFound` | The browser never stored its key. | Ask the user to open the browser once. |
| `DatabaseNotFound` | The profile has no cookie database. | Ask which browser or profile the user logs in with. |
| `UnsupportedDatabaseVersion` | The browser was updated to a format this version does not know. | Ask the user to update the app. |
| `PermissionNeeded` | macOS protects the file (Safari) and the user has not allowed access. | Ask the user to pick the file, as above. |
| `AppBoundEncryption` | Chrome or Edge on Windows protects the site's cookies so that only the browser can read them. This library never gets around that. | Offer the `cookies.txt` fallback. |
| `DatabaseLocked` | The browser is running and keeps its cookie database locked (Windows). | Ask the user to close the browser, then try again. |
| `HostHashMismatch` | A stored value belongs to another site. Nothing is returned. | Report it; it should never happen in a normal browser. |
| `Unsupported` | Browser or operating system not supported yet. | Offer the `cookies.txt` fallback. |

## What it does

1. Copies the profile's cookie database into a temporary folder only you can open. The browser's own file is never opened or changed. On Windows, Chromium browsers lock that file while they run, so the browser has to be closed first (`DatabaseLocked`).
2. Looks for cookies of the website you asked about, using standard cookie domain matching. This step only reads site names, which browsers store unencrypted. If there are none, it stops here and macOS shows no dialog.
3. For Chromium browsers, gets the browser's key. On macOS it asks for the "Safe Storage" key through Apple's Keychain API: macOS shows its own dialog first, and you decide. On Windows it reads the key from the browser's `Local State` file and unprotects it with DPAPI, which works only for your Windows account and shows no dialog. If the site's cookies use Chrome's app-bound encryption, it stops here with `AppBoundEncryption`.
4. Decrypts only the cookies that match that website, and refuses any value the browser stored for another site. The Firefox family and Safari store values without encryption, so steps 3 and 4 do not apply; only the cookies of normal tabs are read, not those of Firefox containers. Safari's file is read into memory that is cleared afterwards, not copied, and only after the user allowed it.
5. If asked, writes them to a `cookies.txt` file readable only by you, deleted as soon as the app is done with it.

## What it never does

- Never asks for, sees or stores a password.
- Never reads cookies for other websites.
- Never sends anything over the network.
- Never writes cookie values to logs or error messages.
- Never gets around app-bound encryption, and never needs administrator rights.
- Never runs the `security` command line tool. Using Apple's Keychain API means the macOS permission belongs to the app that uses this library, not to a shared system tool.

## Supported browsers

| Browser | macOS | Windows |
| --- | --- | --- |
| Dia | Tested | Not available |
| Google Chrome | Tested; needs Full Disk Access on macOS 27 | In the library, not tested yet ([#20](https://github.com/Lectern-HQ/lectern-cookie-reader/issues/20)); cookies with app-bound encryption are reported, not read |
| Brave, Chromium, Microsoft Edge, Opera, Vivaldi | In the catalog, not tested yet ([#54](https://github.com/Lectern-HQ/lectern-cookie-reader/issues/54)) | In the library, not tested yet ([#20](https://github.com/Lectern-HQ/lectern-cookie-reader/issues/20)); Chrome and Edge cookies with app-bound encryption are reported, not read |
| Arc | Tested | Not supported |
| Firefox | Tested; needs Full Disk Access on macOS 27 | In the library, not tested yet ([#20](https://github.com/Lectern-HQ/lectern-cookie-reader/issues/20)) |
| Floorp, LibreWolf, Waterfox, Zen | In the catalog, not tested yet ([#6](https://github.com/Lectern-HQ/lectern-cookie-reader/issues/6)) | In the library, not tested yet ([#20](https://github.com/Lectern-HQ/lectern-cookie-reader/issues/20)) |
| Safari | Tested with Full Disk Access; the file dialog route is not tested yet ([#7](https://github.com/Lectern-HQ/lectern-cookie-reader/issues/7)) | Not available |

Finding which browser and profile is logged in (`find_logins`, `suggest`) covers every browser in the catalog except Safari. macOS protects Safari's cookie file, so `find_logins` never reads it and lists Safari in `skipped` with `PermissionNeeded` instead of causing a dialog.

To read Safari, let the user pick `Cookies.binarycookies` in a file dialog opened at `Browser::by_id("safari")?.default_profile()?.path`. Then call `read_cookies` with a `Profile` whose `path` is the folder of the picked file. If macOS still refuses, the only other route is Full Disk Access for the app, which gives it access to all of the user's files: explain that before asking. Full Disk Access was checked on a real Mac and works; the file dialog route was not checked yet.

On macOS 27, Google Chrome's and Firefox's data folders are protected too: without Full Disk Access, the system refuses them without a dialog, and `find_logins` lists them in `skipped` with `PermissionNeeded`. Arc's and Dia's are not protected.

## Development

```bash
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
cargo llvm-cov --fail-under-lines 90   # coverage, needs cargo-llvm-cov
```

To check a real computer by hand, without ever printing a cookie value:

```bash
cargo run --example probe -- mediaserver.unige.ch                     # every browser: profiles, logins, skipped, suggestion
cargo run --example probe -- mediaserver.unige.ch --read dia          # cookie names of one browser (may ask for the Keychain)
cargo run --example probe -- mediaserver.unige.ch --read safari --profile-dir <folder of a picked Cookies.binarycookies>
cargo run --example probe -- mediaserver.unige.ch --read chrome --cookie-file   # also writes cookies.txt and waits
```

Tests use generated databases and test keys only, never real cookies.

## Licence

Apache License 2.0, see [LICENSE](LICENSE).
