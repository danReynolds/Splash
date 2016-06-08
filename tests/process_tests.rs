extern crate nix;
extern crate tempfile;
extern crate libc;

use nix::sys::signal;
use std::env;
use std::io::{Read, Write};
use std::process::{Command, Output};
use std::os::unix::process::ExitStatusExt;
use std::thread;
use std::time::Duration;
use tempfile::NamedTempFile;

#[allow(dead_code)]
/// Useful for debugging, only needs to be called in that situation
fn print_extra_info(output: &Output) {
    println!("stderr: {}", String::from_utf8(output.stderr.clone()).unwrap());
}

fn set_env(cmd: &mut Command) -> &mut Command {
    let build_dir = env::var("SPLASH_BUILD_DIR").unwrap_or(String::from("target/debug"));
    cmd.env("PATH", format!("/bin:/usr/bin:{}", build_dir))
}

fn run_in_splash(shell_cmd: &str) -> (i32, String) {
    let mut cmd = Command::new("sh");
    let _ = cmd.arg("-c")
        .arg(format!("echo \"{}\" | splash", shell_cmd));

    set_env(&mut cmd);

    let output = cmd.output()
        .unwrap_or_else(|e| panic!("failed to execute process: {}", e));

    let stdout = String::from_utf8(output.stdout).unwrap();
    (output.status.code().unwrap(), stdout)
}

#[test]
fn run_builtin() {
    let (ecode, output) = run_in_splash("echo hello");
    assert_eq!(ecode, 0);
    assert_eq!(output, "hello\n");
}

#[test]
fn run_external() {
    let (ecode, output) = run_in_splash("expr 1 + 1");
    assert_eq!(ecode, 0);
    assert_eq!(output, "2\n");
}

#[test]
fn pipe_from_builtin_to_ext() {
    let (ecode, output) = run_in_splash("echo hello | sed 's/$/ world/'");
    assert_eq!(ecode, 0);
    assert_eq!(output, "hello world\n");
}

#[test]
fn pipe_from_ext_to_ext() {
    let (ecode, output) = run_in_splash("expr 1 + 1 | sed 's/.*/& &/'");
    assert_eq!(ecode, 0);
    assert_eq!(output, "2 2\n");
}

#[test]
fn pipe_from_ext_to_builtin() {
    let (ecode, output) = run_in_splash("expr 1 + 1 | echo 'hello world'");
    assert_eq!(ecode, 0);
    assert_eq!(output, "hello world\n");
}

#[test]
fn continue_pipe_with_failure() {
    let (ecode, output) = run_in_splash("not_a_program | echo hello");
    assert_eq!(ecode, 0);
    assert_eq!(output, "hello\n");
}

#[test]
fn continue_pipe_with_exit_nonzero() {
    let (ecode, output) = run_in_splash("false | echo hello");
    assert_eq!(ecode, 0);
    assert_eq!(output, "hello\n");
}

#[test]
fn command_not_found() {
    let (ecode, output) = run_in_splash("not_a_program");
    assert_eq!(ecode, 127);
    assert_eq!(output, "");
}

#[test]
fn redirect_out() {
    let mut f = NamedTempFile::new().unwrap();

    let (ecode, output) = run_in_splash(&format!("echo 'foo' > {}", f.path().display()));

    assert_eq!(ecode, 0);
    assert_eq!(output, "");

    let mut contents = String::new();
    f.read_to_string(&mut contents).unwrap();
    assert_eq!(contents, "foo\n");
}

#[test]
fn redirect_in() {
    let mut f = NamedTempFile::new().unwrap();
    f.write(b"hello\n").unwrap();
    f.flush().unwrap();

    let (ecode, output) = run_in_splash(&format!("cat < {}", f.path().display()));

    assert_eq!(ecode, 0);
    assert_eq!(output, "hello\n");
}

#[test]
fn redirect_overrides_pipe() {
    let mut f = NamedTempFile::new().unwrap();
    f.write(b"hello\n").unwrap();
    f.flush().unwrap();

    let (ecode, output) = run_in_splash(&format!("echo 'hi' | cat < {}", f.path().display()));

    assert_eq!(output, "hello\n");
    assert_eq!(ecode, 0);
}

#[test]
fn redirect_in_other_fd() {
    let mut f = NamedTempFile::new().unwrap();
    f.write(b"hello\n").unwrap();
    f.flush().unwrap();

    let (ecode, output) = run_in_splash(&format!("cat /dev/fd/3 3< {}", f.path().display()));

    assert_eq!(output, "hello\n");
    assert_eq!(ecode, 0);
}

#[test]
fn redirect_out_other_fd() {
    let mut f = NamedTempFile::new().unwrap();

    let (ecode, output) = run_in_splash(&format!("echo 'hello' | cat 3> {} >&3", f.path().display()));

    assert_eq!(output, "");
    assert_eq!(ecode, 0);

    let mut contents = String::new();
    f.read_to_string(&mut contents).unwrap();
    assert_eq!(contents, "hello\n");
}

#[test]
fn heredoc_in() {
    let (ecode, output) = run_in_splash(&"cat <<EOF\nabc\nEOF");

    assert_eq!(output, "abc\n");
    assert_eq!(ecode, 0);
}

#[test]
fn heredoc_in_no_whitespace() {
    let (ecode, output) = run_in_splash(&"cat <<-EOF\n   abc\n  EOF");

    assert_eq!(output, "abc\n");
    assert_eq!(ecode, 0);
}

#[test]
fn no_stop_with_sigtstp() {
    let mut splash_cmd = Command::new("splash");
    let mut splash = set_env(&mut splash_cmd).spawn().unwrap();

    let pid = splash.id() as i32;

    signal::kill(pid, libc::SIGTSTP).unwrap();
    thread::sleep(Duration::from_millis(100));
    splash.kill().unwrap();

    let status = splash.wait().unwrap();
    assert_eq!(status.signal().unwrap(), 9);
}
