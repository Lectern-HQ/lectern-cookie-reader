//! The Firefox family's `profiles.ini`: which profiles exist, where they are, and which one
//! is the default.
//!
//! ```ini
//! [Install4F96D1932A9F858E]
//! Default=Profiles/abc123.default-release
//!
//! [Profile0]
//! Name=default-release
//! IsRelative=1
//! Path=Profiles/abc123.default-release
//! ```
//!
//! The default is the profile an `[Install...]` section names, as current Firefox does,
//! else the profile marked `Default=1`, as older versions did.

use std::path::{Path, PathBuf};

/// The file's name in the browser's data folder.
pub(crate) const FILE_NAME: &str = "profiles.ini";

/// One profile listed in `profiles.ini`.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct IniProfile {
    /// The name the browser shows, for example `default-release`.
    pub(crate) name: String,
    /// The profile's folder.
    pub(crate) path: PathBuf,
    /// Whether this is the profile the browser opens by default.
    pub(crate) is_default: bool,
}

/// The keys of one `[Profile...]` section that this crate reads.
#[derive(Default)]
struct Section {
    /// `Name=`.
    name: Option<String>,
    /// `Path=`, as written.
    path: Option<String>,
    /// `IsRelative=`: whether `Path=` is relative to the data folder (the default).
    is_relative: bool,
    /// `Default=1`.
    is_default: bool,
}

/// The profiles listed in `text`, the content of the `profiles.ini` in `data_dir`, the
/// default profile first, then by name. Sections without a path are left out.
pub(crate) fn parse(text: &str, data_dir: &Path) -> Vec<IniProfile> {
    let (sections, install_defaults) = read_sections(text);
    let default_path: Option<String> = install_defaults.into_iter().next().or_else(|| {
        sections
            .iter()
            .find(|s| s.is_default)
            .and_then(|s| s.path.clone())
    });
    let mut profiles: Vec<IniProfile> = sections
        .into_iter()
        .filter_map(|section| {
            let written = section.path?;
            Some(IniProfile {
                is_default: default_path.as_deref() == Some(written.as_str()),
                path: resolve(data_dir, &written, section.is_relative),
                name: section.name.unwrap_or_else(|| written.clone()),
            })
        })
        .collect();
    profiles.sort_by(|a, b| (!a.is_default, &a.name).cmp(&(!b.is_default, &b.name)));
    profiles
}

/// The `[Profile...]` sections of `text`, in order, and the `Default=` paths of its
/// `[Install...]` sections (the profile each Firefox installation uses).
fn read_sections(text: &str) -> (Vec<Section>, Vec<String>) {
    let mut sections: Vec<Section> = Vec::new();
    let mut install_defaults: Vec<String> = Vec::new();
    let mut current = Current::Other;
    for line in text.lines().map(str::trim) {
        if let Some(header) = line.strip_prefix('[').and_then(|l| l.strip_suffix(']')) {
            current = Current::of(header);
            if current == Current::Profile {
                sections.push(Section {
                    is_relative: true,
                    ..Section::default()
                });
            }
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let (key, value) = (key.trim(), value.trim());
        match (current, sections.last_mut()) {
            (Current::Install, _) if key == "Default" => install_defaults.push(value.to_owned()),
            (Current::Profile, Some(section)) => section.set(key, value),
            _ => {}
        }
    }
    (sections, install_defaults)
}

impl Section {
    /// Records one `key=value` line of a `[Profile...]` section; other keys are ignored.
    fn set(&mut self, key: &str, value: &str) {
        match key {
            "Name" => self.name = Some(value.to_owned()),
            "Path" => self.path = Some(value.to_owned()),
            "IsRelative" => self.is_relative = value != "0",
            "Default" => self.is_default = value == "1",
            _ => {}
        }
    }
}

/// The kind of section the parser is in.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Current {
    /// A `[Profile...]` section.
    Profile,
    /// An `[Install...]` section.
    Install,
    /// Any other section, such as `[General]`.
    Other,
}

impl Current {
    /// The kind of the section named `header` (the text between the brackets).
    fn of(header: &str) -> Self {
        if header.starts_with("Profile") {
            Current::Profile
        } else if header.starts_with("Install") {
            Current::Install
        } else {
            Current::Other
        }
    }
}

/// The folder a `Path=` value points to. Relative paths use `/` on every system.
fn resolve(data_dir: &Path, written: &str, is_relative: bool) -> PathBuf {
    if is_relative {
        written
            .split(['/', '\\'])
            .filter(|part| !part.is_empty())
            .fold(data_dir.to_owned(), |path, part| path.join(part))
    } else {
        PathBuf::from(written)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DATA: &str = "/data";

    fn names(profiles: &[IniProfile]) -> Vec<(&str, bool)> {
        profiles
            .iter()
            .map(|p| (p.name.as_str(), p.is_default))
            .collect()
    }

    #[test]
    fn the_install_section_names_the_default_profile() {
        let ini = "\
[Profile1]
Name=default
IsRelative=1
Path=Profiles/aaa.default
Default=1

[Profile0]
Name=default-release
IsRelative=1
Path=Profiles/bbb.default-release

[General]
StartWithLastProfile=1
Version=2

[Install4F96D1932A9F858E]
Default=Profiles/bbb.default-release
Locked=1
";
        let profiles = parse(ini, Path::new(DATA));
        assert_eq!(
            names(&profiles),
            [("default-release", true), ("default", false)]
        );
        assert_eq!(
            profiles[0].path,
            Path::new(DATA).join("Profiles").join("bbb.default-release")
        );
    }

    #[test]
    fn without_an_install_section_the_marked_profile_is_the_default() {
        let ini = "[Profile0]\nName=work\nPath=Profiles/w\n\n[Profile1]\nName=home\nPath=Profiles/h\nDefault=1\n";
        assert_eq!(
            names(&parse(ini, Path::new(DATA))),
            [("home", true), ("work", false)]
        );
    }

    #[test]
    fn absolute_paths_are_kept_and_sections_without_a_path_left_out() {
        let ini = "[Profile0]\r\nName=elsewhere\r\nIsRelative=0\r\nPath=/Volumes/Disk/ff\r\n[Profile1]\r\nName=broken\r\n";
        let profiles = parse(ini, Path::new(DATA));
        assert_eq!(names(&profiles), [("elsewhere", false)]);
        assert_eq!(profiles[0].path, PathBuf::from("/Volumes/Disk/ff"));
    }

    #[test]
    fn a_profile_without_a_name_is_named_after_its_path() {
        let profiles = parse("[Profile0]\nPath=Profiles/x\n", Path::new(DATA));
        assert_eq!(profiles[0].name, "Profiles/x");
    }

    #[test]
    fn text_that_is_not_ini_gives_no_profiles() {
        assert!(parse("garbage\n= =\n[", Path::new(DATA)).is_empty());
        assert!(parse("", Path::new(DATA)).is_empty());
    }
}
