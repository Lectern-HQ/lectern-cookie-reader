//! The work folders are removed on the way out: by the app's exit handler, or by a signal.
//! Each case runs in a child process (this test binary again), so that removing every
//! folder of the process never touches the folders of other tests.

use std::process::{Command, ExitStatus, Stdio};

use lectern_cookie_reader::{CookieFile, clean_up_on_signals, remove_work_folders};

/// The environment variable that tells [`child`] which case to run.
const CASE: &str = "LECTERN_COOKIE_READER_CHILD_CASE";

/// What [`child`] prints before the path of its work folder.
const FOLDER_MARK: &str = "FOLDER=";

/// Runs only in a child started by a test below; does nothing otherwise.
#[test]
fn child() {
    let Some(case) = std::env::var_os(CASE) else {
        return;
    };
    let file = CookieFile::create(&[]).unwrap();
    let folder = file.path().parent().unwrap().to_owned();
    if case == "exit" {
        assert_eq!(remove_work_folders(), 1);
        assert!(!folder.exists());
        assert_eq!(remove_work_folders(), 0);
        drop(file);
        return;
    }
    clean_up_on_signals().unwrap();
    clean_up_on_signals().unwrap();
    println!("{FOLDER_MARK}{}", folder.display());
    std::thread::sleep(std::time::Duration::from_secs(60));
}

/// Starts [`child`] in a new process for `case`, with its output piped.
fn start(case: &str) -> std::process::Child {
    Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "child", "--nocapture", "--test-threads=1"])
        .env(CASE, case)
        .stdout(Stdio::piped())
        .spawn()
        .unwrap()
}

#[test]
fn the_exit_handler_removes_every_work_folder() {
    let status: ExitStatus = start("exit").wait().unwrap();
    assert!(status.success());
}

#[cfg(target_os = "macos")]
#[test]
fn a_termination_signal_removes_the_folders_and_still_ends_the_process() {
    use std::io::{BufRead, BufReader};
    use std::os::unix::process::ExitStatusExt;
    use std::path::PathBuf;

    let mut child = start("signals");
    // libtest prints `test child ... ` before the line on the same line.
    let lines = BufReader::new(child.stdout.take().unwrap()).lines();
    let folder = lines
        .map_while(Result::ok)
        .find_map(|line| {
            line.split_once(FOLDER_MARK)
                .map(|(_, path)| PathBuf::from(path))
        })
        .unwrap();
    assert!(folder.exists());
    let pid = child.id().to_string();
    assert!(
        Command::new("kill")
            .args(["-TERM", &pid])
            .status()
            .unwrap()
            .success()
    );
    let status = child.wait().unwrap();
    assert_eq!(status.signal(), Some(signal_hook::consts::SIGTERM));
    assert!(!folder.exists());
}
