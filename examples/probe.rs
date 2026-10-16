//! Checks this library on a real computer, by hand. It never prints a cookie value.
//!
//! ```text
//! cargo run --example probe -- <host or https link>
//! cargo run --example probe -- <host> --read <browser id> [--profile-dir <folder>] [--cookie-file]
//! ```
//!
//! Without `--read`, it lists the default browser, every profile it checked, the profiles
//! logged in to the site, the browsers it skipped and why, and the suggested profile.
//! Nothing is decrypted and no permission dialog appears.
//!
//! With `--read <browser id>`, it reads the site's cookies from that browser (macOS may
//! ask for the Keychain) and lists their names and attributes. `--profile-dir` reads a
//! given profile folder instead, for example the folder of a Safari cookie file picked
//! by hand. `--cookie-file` also writes the `cookies.txt` for yt-dlp and waits, so its
//! permissions can be checked before it is deleted.

use std::path::PathBuf;
use std::process::ExitCode;
use std::time::{SystemTime, UNIX_EPOCH};

use lectern_cookie_reader::{
    Browser, CookieFile, Profile, Scan, default_browser, find_logins, host_of_link, read_cookies,
    suggest,
};

/// What to check, from the command line.
struct Options {
    /// The site's host name.
    host: String,
    /// The browser to read cookies from, if any.
    read: Option<Browser>,
    /// The profile folder to read instead of the browser's own.
    profile_dir: Option<PathBuf>,
    /// Whether to write and keep the cookie file until Enter is pressed.
    cookie_file: bool,
}

fn main() -> ExitCode {
    if let Err(error) = lectern_cookie_reader::clean_up_on_signals() {
        eprintln!("error: {error}");
        return ExitCode::FAILURE;
    }
    match options(std::env::args().skip(1).collect()) {
        Ok(options) => match run(&options) {
            Ok(()) => ExitCode::SUCCESS,
            Err(error) => {
                eprintln!("error: {error}");
                ExitCode::FAILURE
            }
        },
        Err(usage) => {
            eprintln!("{usage}");
            ExitCode::from(2)
        }
    }
}

/// The options in `args`, or the usage text.
fn options(args: Vec<String>) -> Result<Options, String> {
    let usage = || {
        let ids: Vec<&str> = Browser::all().map(|b| b.id()).collect();
        format!(
            "usage: probe <host or https link> [--read <browser id>] [--profile-dir <folder>] [--cookie-file]\nbrowser ids: {}",
            ids.join(", ")
        )
    };
    let mut args = args.into_iter();
    let target = args.next().ok_or_else(usage)?;
    let host = if target.starts_with("https://") {
        host_of_link(&target).map_err(|e| e.to_string())?
    } else {
        target
    };
    let mut options = Options {
        host,
        read: None,
        profile_dir: None,
        cookie_file: false,
    };
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--read" => {
                let id = args.next().ok_or_else(usage)?;
                options.read = Some(Browser::by_id(&id).ok_or_else(usage)?);
            }
            "--profile-dir" => options.profile_dir = Some(args.next().ok_or_else(usage)?.into()),
            "--cookie-file" => options.cookie_file = true,
            _ => return Err(usage()),
        }
    }
    Ok(options)
}

/// Scans every browser, then reads one if asked.
fn run(options: &Options) -> lectern_cookie_reader::Result<()> {
    let scan = find_logins(&options.host)?;
    print_scan(&options.host, &scan);
    if let Some(browser) = options.read {
        let profile = match &options.profile_dir {
            Some(dir) => Profile {
                browser,
                name: "given folder".into(),
                path: dir.clone(),
            },
            None => scan
                .logins
                .iter()
                .find(|login| login.profile.browser == browser)
                .map(|login| login.profile.clone())
                .map_or_else(|| browser.default_profile(), Ok)?,
        };
        read(&profile, options)?;
    }
    Ok(())
}

/// Prints what `find_logins` found for `host`.
fn print_scan(host: &str, scan: &Scan) {
    let default = default_browser();
    match &default {
        Some(d) => println!(
            "Default browser: {} ({})",
            d.name,
            d.browser.map_or("not in the catalog", |b| b.id())
        ),
        None => println!("Default browser: not known on this system"),
    }
    println!("\nChecked {} profiles:", scan.checked.len());
    for checked in &scan.checked {
        println!(
            "  {} / {}: last active {}  ({})",
            checked.profile.browser.name(),
            checked.profile.name,
            ago(checked.last_active),
            checked.profile.path.display()
        );
    }
    println!("\nLogged in to {host}: {}", scan.logins.len());
    for login in &scan.logins {
        println!(
            "  {} / {}: last used {}",
            login.profile.browser.name(),
            login.profile.name,
            ago(Some(login.last_used))
        );
    }
    println!("\nSkipped: {}", scan.skipped.len());
    for skipped in &scan.skipped {
        println!("  {}: {}", skipped.browser.name(), skipped.error);
    }
    match suggest(scan, default.as_ref()) {
        Some(s) => println!(
            "\nSuggested: {} / {} ({:?})",
            s.profile.browser.name(),
            s.profile.name,
            s.reason
        ),
        None => println!("\nSuggested: none"),
    }
}

/// Reads the site's cookies from `profile` and lists them, without their values.
fn read(profile: &Profile, options: &Options) -> lectern_cookie_reader::Result<()> {
    println!(
        "\nReading {} / {} ({})",
        profile.browser.name(),
        profile.name,
        profile.path.display()
    );
    let cookies = read_cookies(profile, &options.host)?;
    println!("{} cookies for {}:", cookies.len(), options.host);
    for c in &cookies {
        println!(
            "  {} {} path={} secure={} http_only={} expires={}",
            c.domain,
            c.name,
            c.path,
            c.secure,
            c.http_only,
            c.expires.map_or("end of session".into(), |e| e.to_string())
        );
    }
    if options.cookie_file {
        let file = CookieFile::create(&cookies)?;
        println!("\nCookie file: {}", file.path().display());
        println!("Check who can read it now, then press Enter to delete it.");
        let mut line = String::new();
        let _ = std::io::stdin().read_line(&mut line);
        let path = file.path().to_owned();
        drop(file);
        println!("Deleted: {}", if path.exists() { "NO" } else { "yes" });
    }
    Ok(())
}

/// How long ago `unix_secs` was, in words.
fn ago(unix_secs: Option<i64>) -> String {
    let Some(then) = unix_secs else {
        return "never".into();
    };
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs() as i64);
    let minutes = (now - then).max(0) / 60;
    match minutes {
        0 => "just now".into(),
        m if m < 120 => format!("{m} minutes ago"),
        m if m < 48 * 60 => format!("{} hours ago", m / 60),
        m => format!("{} days ago", m / (24 * 60)),
    }
}
