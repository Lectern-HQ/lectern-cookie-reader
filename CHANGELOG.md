# Changelog

All notable changes to this library. The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and versions follow [Semantic Versioning](https://semver.org/) (before 1.0, a minor version may break the API).

## [Unreleased]

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

[0.2.0]: https://github.com/Lectern-HQ/lectern-cookie-reader/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/Lectern-HQ/lectern-cookie-reader/releases/tag/v0.1.0
