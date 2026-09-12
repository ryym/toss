use std::ffi::OsString;
use std::io::{self, Write};
use std::path::Path;

use pretty_assertions::assert_eq;

use super::mock_screen::MockScreen;
use crate::AppError;
use crate::run::{RunConfig, run_with};
use crate::screen::ScreenSize;

/// Run the real entry point with a stdout that is not a terminal.
fn run_passthrough<W: Write>(
    args: &[&str],
    input: &[u8],
    stdin_is_terminal: bool,
    stdout: W,
) -> Result<(), AppError> {
    let mut cli_args: Vec<OsString> = vec!["toss".into()];
    cli_args.extend(args.iter().map(OsString::from));

    run_with(RunConfig {
        args: cli_args,
        get_terminal_size: || -> Result<ScreenSize, AppError> {
            panic!("the terminal size must not be read without a terminal")
        },
        shell_lines: 1,
        instant_scroll: true,
        stdin: io::Cursor::new(input.to_vec()),
        stdin_is_terminal,
        stdout,
        stdout_is_terminal: false,
        make_screen: |_w: W| -> Result<MockScreen<W>, AppError> {
            panic!("the pager must not start without a terminal")
        },
        wait_for_all_input: true,
    })
}

/// Input that the line-based document representation cannot reproduce byte for byte:
/// a CRLF line, invalid UTF-8, and no trailing newline.
const RAW_INPUT: &[u8] = b"line 1\r\nli\xffne 2\nline 3";

#[test]
fn stdin_is_relayed_byte_for_byte() {
    let mut out: Vec<u8> = Vec::new();
    run_passthrough(&[], RAW_INPUT, false, &mut out).unwrap();
    assert_eq!(out, RAW_INPUT);
}

#[test]
fn file_is_relayed_byte_for_byte() {
    let dir = Path::new(".local/test");
    std::fs::create_dir_all(dir).unwrap();
    let path = dir.join("test_passthrough.txt");
    std::fs::write(&path, RAW_INPUT).unwrap();

    let mut out: Vec<u8> = Vec::new();
    run_passthrough(&[path.to_str().unwrap()], b"", false, &mut out).unwrap();
    assert_eq!(out, RAW_INPUT);

    std::fs::remove_file(&path).unwrap();
}

/// -F only decides whether to skip the pager on a terminal. Without one there is
/// nothing to decide, so the input is relayed however long it is.
#[test]
fn quit_if_one_screen_still_relays_a_long_input() {
    let input = "line\n".repeat(1000);
    let mut out: Vec<u8> = Vec::new();
    run_passthrough(&["-F"], input.as_bytes(), false, &mut out).unwrap();
    assert_eq!(out, input.as_bytes());
}

#[test]
fn missing_input_is_still_a_usage_error() {
    let mut out: Vec<u8> = Vec::new();
    // No file to read and a terminal stdin: there is nothing to relay.
    let err = run_passthrough(&[], b"", true, &mut out).unwrap_err();
    assert_eq!(err.message, "Usage: toss <file> OR command | toss");
}
