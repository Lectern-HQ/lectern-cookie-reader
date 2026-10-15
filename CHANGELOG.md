# Changelog

All notable changes to this library. The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and versions follow [Semantic Versioning](https://semver.org/) (before 1.0, a minor version may break the API).

## [Unreleased]

### Added
- Safari on macOS (`safari` in the catalog, `Engine::Safari`): a parser for `Cookies.binarycookies`, and `read_cookies` for a Safari profile. macOS protects the file, so the app asks the user to pick it in a file dialog; `find_logins` never reads it and lists Safari in `Scan::skipped` with the new `Error::PermissionNeeded`. `default_browser()` now matches Safari. Not tested on a real Mac yet.
- The Firefox family on macOS: Firefox, Floorp, LibreWolf, Waterfox and Zen in the catalog. Profiles are found through `profiles.ini`, and `read_cookies`, `find_logins` and `suggest` work as for Chromium browsers. Values are stored without encryption, so there is no permission dialog. Only cookies of normal tabs are read, not those of containers or partitioned third-party cookies. Database schema versions 12 to 17 are accepted (17 is current Firefox). Not tested on a real Mac yet.

- Windows (x64): each browser's data folder is in the catalog (Chromium browsers under `%LOCALAPPDATA%`, Opera and the Firefox family under `%APPDATA%`), so `profiles()`, `default_profile()` and `find_logins` work there. The Chromium database is read from `Network\Cookies` where it exists. A database the running browser keeps locked is the new `Error::DatabaseLocked`. Arc, Dia and Safari are not read on Windows.
- Chromium cookies on Windows: the key is read from `Local State` and unprotected with DPAPI for the current user (no dialog), and `v10` values are decrypted with AES-256-GCM, with the same host hash check as on macOS.
- App-bound encryption (`v20` values, Chrome and Edge on Windows since version 127) is detected before the key is requested and reported as the new `Error::AppBoundEncryption`, so the app can offer the `cookies.txt` fallback. It is never bypassed.
- `CookieFile` on Windows: its folder is created with an access list that lets only the current user in and inherits nothing, and the file inherits it. Every private work folder of the crate uses the same list.

### Changed
- **Breaking:** `Engine` is `non_exhaustive`, so a `match` on it needs a wildcard arm. New engines can then be added without another breaking change.

## [0.3.0] - 2026-10-15

### Added
- `host_of_link(link)`: the checked, lower-case host of an `https` link, so apps do not write their own link parsing. Its error never repeats the link.

### Changed
- **Breaking:** browsers are entries in one catalog (`src/catalog.rs`) instead of an enum. Use `Browser::by_id("dia")` instead of `Browser::Dia`, and `Browser::all()` instead of `Browser::ALL`. `Browser::id()` gives a stable id to save a user's choice. Adding a browser is one catalog entry.

## [0.2.0] - 2026-10-14

### Added
- Table of Chromium browsers on macOS: Arc, Brave, Google Chrome, Chromium, Dia, Microsoft Edge, Opera, Vivaldi (`Browser::ALL`). Only Dia has been tested on a real Mac; the other folders and Keychain names follow yt-dlp's table.
- `Browser::profiles()`: the profiles of a browser that have a cookie database.
- `find_logins(host)`: finds which profiles have an unexpired cookie for a site, most recently used first, without decrypting anything and without a permission dialog. A browser or profile that cannot be read is listed in `Scan::skipped`.
- `default_browser()`: the system's default browser (macOS), with the matching `Browser` if this library can read it.
- `suggest(&scan, default)`: one profile to show already selected: logged in to the site, else the default browser, else the most recently used profile, with the reason.

### Changed
- **Breaking:** `Browser` is now `non_exhaustive`, so a `match` on it needs a wildcard arm. New browsers can then be added without another breaking change.

## [0.1.0] - 2026-10-14

### Added
- `read_cookies(&profile, host)`: reads the unexpired cookies of one site from a copy of a Chromium profile's cookie database. Asks the macOS Keychain for the browser's key only when a matching cookie exists.
- Chromium `v10` decryption with a database version check and the host hash check of database version 24.
- Standard cookie domain matching and parameterised queries.
- `CookieFile`: a `cookies.txt` for downloaders such as yt-dlp, readable only by the user and deleted when dropped.
- Errors that never contain a cookie value; a denied Keychain dialog is its own error.
- Dia on macOS.

[Unreleased]: https://github.com/Lectern-HQ/lectern-cookie-reader/compare/v0.3.0...HEAD
[0.3.0]: https://github.com/Lectern-HQ/lectern-cookie-reader/compare/v0.2.0...v0.3.0
[0.2.0]: https://github.com/Lectern-HQ/lectern-cookie-reader/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/Lectern-HQ/lectern-cookie-reader/releases/tag/v0.1.0
