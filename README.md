# lectern-cookie-reader

A small Rust library that reads the login cookies for **one website** from a browser on your own Mac, so a desktop app can download something you are already allowed to see, without asking for your password.

It is the part of [Lectern](https://github.com/Lectern-HQ) that touches your browser data, published so anyone can check exactly what it does.

Status: not released yet. The code is being cleaned up from a working prototype.

## What it does

1. Finds which installed browser has a login for the website you asked about. This step only reads site names and cookie names, which browsers store unencrypted. Nothing is decrypted.
2. For Chromium browsers (Chrome, Arc, Dia, Brave, Edge, ...), asks macOS for the browser's "Safe Storage" key through Apple's Keychain API. macOS shows its own dialog first, and you decide.
3. Decrypts only the cookies that match that website, using standard cookie domain matching.

## What it never does

- Never asks for, sees or stores a password.
- Never reads cookies for other websites.
- Never sends anything over the network.
- Never writes cookie values to logs or error messages.
- Never runs the `security` command line tool. Using Apple's Keychain API means the macOS permission belongs to the app that uses this library, not to a shared system tool.

## Supported browsers

Tested: Dia on macOS. Planned: other Chromium browsers and the Firefox family on macOS, then Safari (needs your explicit permission to read its protected cookie file).

## Licence

Apache License 2.0, see [LICENSE](LICENSE).
