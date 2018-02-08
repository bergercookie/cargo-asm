//! Handles how to build the project.

use options::Options;
use process;

#[derive(Copy, Clone, Debug)]
pub enum Type {
    Debug,
    Release,
}

impl ::std::str::FromStr for Type {
    type Err = String;
    fn from_str(s: &str) -> ::std::result::Result<Self, Self::Err> {
        match s {
            "debug" => Ok(Type::Debug),
            "release" => Ok(Type::Release),
            v => Err(format!("\"{}\" is not a valid build type. Try \"debug\" or \"releaes\"", v))
        }
    }
}

/// Builds the project according to the CLI options and returns a list of
/// assembly files generated.
pub fn project(opt: &Options) -> Vec<::std::path::PathBuf> {
    use std::process::Command;
    // Read the RUSTFLAGS environment variable
    let rustflags = ::std::env::var_os("RUSTFLAGS")
        .unwrap_or_default()
        .into_string()
        .expect("RUSTFLAGS are not valid UTF-8");

    // Runs `cargo clean` before generating assembly code.
    // TODO: figure out if this is really necessary
    if opt.clean {
        let mut cargo_clean = Command::new("cargo");
        cargo_clean.arg("clean");
        let error_msg = "cargo clean failed";
        process::exec(&mut cargo_clean, error_msg, opt.verbose)
            .expect(error_msg);
    }

    // Compile project generating assembly output:
    let mut cargo_build = Command::new("cargo");
    // TODO: unclear if `cargo build` + `RUSTFLAGS` should be used,
    // or instead one should use `cargo rustc -- --emit asm`
    cargo_build.arg("build");
    if opt.color {
        cargo_build.arg("--color=always");
        cargo_build.env("LS_COLORS", "rs=0:di=38;5;27:mh=44;38;5;15");
    }
    match opt.build_type {
        Type::Release => cargo_build.arg("--release"),
        Type::Debug => cargo_build.arg("--debug"),
    };
    cargo_build.arg("--verbose");
    let asm_syntax = match opt.asm_style {
        ::asm::Style::Intel => {
            "-Z asm-comments -C llvm-args=-x86-asm-syntax=intel"
        }
        ::asm::Style::ATT => "",
    };
    cargo_build.env(
        "RUSTFLAGS",
        format!("{} --emit asm -g {}", rustflags, asm_syntax),
    );

    let build_start = ::std::time::SystemTime::now();
    let error_msg = "cargo build failed";
    let (_stdout, stderr) =
        process::exec(&mut cargo_build, error_msg, opt.verbose)
            .expect(error_msg);

    // Find output directories:
    // TODO: is this really necessary? Assembly output "should" be in
    // ${working_dir}/targets/{release,debug}/build/*.s
    let mut output_directories = Vec::<String>::new();
    for l in stderr.lines() {
        // This goes through the Running "rustc ... " invokations printed to
        // stderr looking for --out-dir and collects the directories into a
        // Vec:
        l.trim()
            .split_whitespace()
            .skip_while(|s| s != &"--out-dir")
            .skip(1)
            .take(1)
            .for_each(|v| output_directories.push(v.to_string()));
    }

    // Scan the output directories for assembly files ".s" that have been
    // generated after the build start.
    let mut output_files = Vec::new();
    for dir in output_directories {
        for entry in ::walkdir::WalkDir::new(dir) {
            let e = entry.unwrap();
            let p = e.path();
            let modified_after_build_start =
                ::std::fs::metadata(p).unwrap().modified().unwrap()
                    >= build_start;
            let is_assembly_file =
                p.extension().map_or("", |v| v.to_str().unwrap_or("")) == "s";
            if modified_after_build_start && is_assembly_file {
                output_files.push(p.to_path_buf());
            }
        }
    }

    // Sort the files, remove duplicates, and done:
    output_files.sort_unstable();
    output_files.dedup();
    output_files
}
