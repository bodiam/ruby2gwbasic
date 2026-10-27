use std::{
    io::Write,
    process::{Command, Stdio},
};

fn binary() -> Command {
    Command::new(env!("CARGO_BIN_EXE_ruby2gwbasic"))
}

#[test]
fn reads_stdin_and_writes_a_listing() {
    let mut child = binary()
        .arg("-")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(b"puts 'Hello'\n")
        .unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(output.status.success());
    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        "10 PRINT \"Hello\"\n20 END\n"
    );
}

#[test]
fn reports_source_locations_without_partial_output() {
    let mut child = binary()
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(b"puts 'valid'\nputs missing\n")
        .unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert!(
        String::from_utf8(output.stderr)
            .unwrap()
            .contains("<stdin>:2:6:")
    );
}

#[test]
fn file_output_and_failed_conversion_preserve_existing_files() {
    let dir = std::env::temp_dir().join(format!("ruby2gwbasic-cli-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let input = dir.join("input.rb");
    let output = dir.join("output.bas");
    std::fs::write(&input, "puts 'file'\n").unwrap();
    let result = binary()
        .arg(&input)
        .arg("-o")
        .arg(&output)
        .output()
        .unwrap();
    assert!(result.status.success());
    let expected = "10 PRINT \"file\"\n20 END\n";
    assert_eq!(std::fs::read_to_string(&output).unwrap(), expected);
    std::fs::write(&input, "def unsupported\nend\n").unwrap();
    let result = binary()
        .arg(&input)
        .arg("-o")
        .arg(&output)
        .output()
        .unwrap();
    assert!(!result.status.success());
    assert_eq!(std::fs::read_to_string(&output).unwrap(), expected);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn help_and_invalid_options() {
    assert!(binary().arg("--help").output().unwrap().status.success());
    for args in [vec!["--unknown"], vec!["-o"], vec!["first.rb", "second.rb"]] {
        assert!(!binary().args(args).output().unwrap().status.success());
    }
}
