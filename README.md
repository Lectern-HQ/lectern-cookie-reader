# lectern-cookie-reader

A small Rust library that reads the login cookies for **one website** from a browser on your own Mac, so a desktop app can download something you are already allowed to see, without asking for your password.

It is the part of [Lectern](https://github.com/Lectern-HQ) that touches your browser data, published so anyone can check exactly what it does.

Status: version 0.1.0, Dia on macOS.

## Usage

```toml
[dependencies]
lectern-cookie-reader = { git = "https://github.com/Lectern-HQ/lectern-cookie-reader", tag = "v0.1.0" }
```

```rust
use lectern_cookie_reader::{Browser, CookieFile, Error, read_cookies};

fn download(host: &str) -> Result<(), Error> {
    let profile = Browser::Dia.default_profile()?;
    let cookies = read_cookies(&profile, host)?; // macOS may ask for permission here
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

`host` is a host name such as `mediaserver.unige.ch`, not a link.

### Errors

Every error says what failed, never a cookie value. The ones an app usually handles:

| Error | Meaning | What the app can do |
| --- | --- | --- |
| `KeyAccessDenied` | The user clicked "Deny" in the macOS dialog. | Explain why the key is needed and offer to try again. |
| `KeyNotFound` | The browser never stored its key. | Ask the user to open the browser once. |
| `DatabaseNotFound` | The profile has no cookie database. | Ask which browser or profile the user logs in with. |
| `UnsupportedDatabaseVersion` | The browser was updated to a format this version does not know. | Ask the user to update the app. |
| `HostHashMismatch` | A stored value belongs to another site. Nothing is returned. | Report it; it should never happen in a normal browser. |
| `Unsupported` | Browser or operating system not supported yet. | Offer the `cookies.txt` fallback. |

## What it does

1. Copies the profile's cookie database into a temporary folder only you can open. The browser's own file is never opened or changed.
2. Looks for cookies of the website you asked about, using standard cookie domain matching. This step only reads site names, which browsers store unencrypted. If there are none, it stops here and macOS shows no dialog.
3. For Chromium browsers, asks macOS for the browser's "Safe Storage" key through Apple's Keychain API. macOS shows its own dialog first, and you decide.
4. Decrypts only the cookies that match that website, and refuses any value the browser stored for another site.
5. If asked, writes them to a `cookies.txt` file readable only by you, deleted as soon as the app is done with it.

## What it never does

- Never asks for, sees or stores a password.
- Never reads cookies for other websites.
- Never sends anything over the network.
- Never writes cookie values to logs or error messages.
- Never runs the `security` command line tool. Using Apple's Keychain API means the macOS permission belongs to the app that uses this library, not to a shared system tool.

## Supported browsers

Tested: Dia on macOS. Planned: finding which browser and profile is logged in, other Chromium browsers and the Firefox family on macOS, then Windows, then Safari (needs your explicit permission to read its protected cookie file).

## Development

```bash
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
cargo llvm-cov --fail-under-lines 90   # coverage, needs cargo-llvm-cov
```

Tests use generated databases and test keys only, never real cookies.

## Licence

Apache License 2.0, see [LICENSE](LICENSE).
