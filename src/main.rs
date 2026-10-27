use std::{
    env,
    ffi::OsString,
    fs,
    io::{self, Read, Write},
    path::PathBuf,
    process::ExitCode,
};

const HELP: &str = "ruby2gwbasic — convert a small Ruby subset to GW-BASIC

Usage: ruby2gwbasic [INPUT.rb|-] [-o OUTPUT.bas]

With no input file, or with -, read Ruby from stdin.
Write the numbered BASIC listing to stdout unless -o is supplied.

Options:
  -o, --output FILE   Write to FILE
  -h, --help          Show this help
  -V, --version       Show the version
";

fn run() -> Result<(), String> {
    let mut args = env::args_os().skip(1);
    let (mut input, mut output): (Option<OsString>, Option<PathBuf>) = (None, None);
    let mut positional_only = false;
    while let Some(arg) = args.next() {
        if !positional_only {
            match arg.to_str() {
                Some("-h" | "--help") => {
                    print!("{HELP}");
                    return Ok(());
                }
                Some("-V" | "--version") => {
                    println!("ruby2gwbasic {}", env!("CARGO_PKG_VERSION"));
                    return Ok(());
                }
                Some("--") => {
                    positional_only = true;
                    continue;
                }
                Some("-o" | "--output") => {
                    if output.is_some() {
                        return Err("output file specified more than once".into());
                    }
                    output = Some(args.next().ok_or("expected a file after --output")?.into());
                    continue;
                }
                Some(s) if s.starts_with('-') && s != "-" => {
                    return Err(format!("unknown option '{s}'; see --help"));
                }
                _ => {}
            }
        }
        if input.replace(arg).is_some() {
            return Err("expected at most one input file; see --help".into());
        }
    }
    let stdin = input.as_deref().is_none_or(|path| path == "-");
    let source_name = if stdin {
        "<stdin>".into()
    } else {
        input.as_ref().unwrap().to_string_lossy().into_owned()
    };
    let source = if stdin {
        let mut source = String::new();
        io::stdin()
            .read_to_string(&mut source)
            .map_err(|e| format!("cannot read stdin: {e}"))?;
        source
    } else {
        fs::read_to_string(input.as_ref().unwrap())
            .map_err(|e| format!("cannot read {source_name}: {e}"))?
    };
    let basic = ruby2gwbasic::convert(&source).map_err(|e| format!("{source_name}:{e}"))?;
    if let Some(path) = output {
        // Conversion completes before opening the output, so source errors never truncate it.
        fs::write(&path, basic).map_err(|e| format!("cannot write {}: {e}", path.display()))?;
    } else {
        let result = io::stdout().lock().write_all(basic.as_bytes());
        if let Err(error) = result {
            if error.kind() != io::ErrorKind::BrokenPipe {
                return Err(format!("cannot write stdout: {error}"));
            }
        }
    }
    Ok(())
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("ruby2gwbasic: {error}");
            ExitCode::FAILURE
        }
    }
}
