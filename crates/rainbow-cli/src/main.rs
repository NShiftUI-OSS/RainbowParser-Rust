mod doctor;
mod install;
mod style;

use std::env;
use std::fs;
use std::io::{self, Read};
use std::process::ExitCode;

use rainbow_parser::{
    decode, decode_name_registry, encode, expand_and_validate, expand_error_to_json,
    expand_placeholders, expand_success_to_json, format_error_to_json, format_source,
    format_success_to_json, parse_error_to_json, parse_substitution_map, parse_success_to_json,
    validate_concrete_source, validate_error_to_json, validate_source, validate_success_to_json,
    RainbowDiagnostic, RainbowDiagnosticSeverity, RAINBOW_PARSER_VERSION,
};

use style::Style;

fn main() -> ExitCode {
    match run(env::args().skip(1).collect()) {
        Ok(code) => code,
        Err(error) => {
            // Commands may already include styled lines; print as-is.
            eprintln!("{error}");
            ExitCode::from(1)
        }
    }
}

fn run(args: Vec<String>) -> Result<ExitCode, String> {
    if args.is_empty() {
        print_help();
        return Ok(ExitCode::SUCCESS);
    }

    match args[0].as_str() {
        "--help" | "-h" => {
            print_help();
            Ok(ExitCode::SUCCESS)
        }
        "--version" | "-V" => {
            let style = Style::stdout();
            println!(
                "{} {}",
                style.bold("rainbow"),
                style.cyan(RAINBOW_PARSER_VERSION)
            );
            Ok(ExitCode::SUCCESS)
        }
        "parse" => command_parse(&args[1..]),
        "format" => command_format(&args[1..]),
        "validate" => command_validate(&args[1..]),
        "validate-registry" => command_validate_registry(&args[1..]),
        "expand" => command_expand(&args[1..]),
        "install" => install::command_install(&args[1..]),
        "doctor" => doctor::command_doctor(&args[1..]),
        other => Err(format!("Unknown command '{other}'.\n\n{}", usage())),
    }
}

fn command_parse(args: &[String]) -> Result<ExitCode, String> {
    if has_help(args) {
        print_command_help("parse");
        return Ok(ExitCode::SUCCESS);
    }

    let options = CommandOptions::parse(args, "parse")?;
    let label = source_label(options.path.as_deref());
    let source = read_source(options.path.as_deref())?;

    match decode(&source) {
        Ok(document) => {
            println!("{}", parse_success_to_json(&document));
            Ok(ExitCode::SUCCESS)
        }
        Err(error) => {
            if options.json {
                println!("{}", parse_error_to_json(&error.diagnostics));
            } else {
                print_human_diagnostics(&label, &error.diagnostics);
            }
            Ok(ExitCode::from(2))
        }
    }
}

fn command_format(args: &[String]) -> Result<ExitCode, String> {
    if has_help(args) {
        print_command_help("format");
        return Ok(ExitCode::SUCCESS);
    }

    let options = CommandOptions::parse(args, "format")?;
    let label = source_label(options.path.as_deref());
    let source = read_source(options.path.as_deref())?;

    match format_source(&source) {
        Ok(formatted) => {
            if options.json {
                println!("{}", format_success_to_json(&formatted));
                return Ok(ExitCode::SUCCESS);
            }
            if options.check {
                let style = Style::stdout();
                if formatted == source {
                    println!("{}", style.ok_line(&format!("{label} already formatted")));
                    return Ok(ExitCode::SUCCESS);
                }
                println!("{}", style.fail_line(&format!("{label} needs format")));
                return Ok(ExitCode::from(1));
            }
            print!("{formatted}");
            if !formatted.ends_with('\n') {
                println!();
            }
            Ok(ExitCode::SUCCESS)
        }
        Err(error) => {
            if options.json {
                println!("{}", format_error_to_json(&error.diagnostics));
            } else {
                print_human_diagnostics(&label, &error.diagnostics);
            }
            Ok(ExitCode::from(2))
        }
    }
}

fn command_validate(args: &[String]) -> Result<ExitCode, String> {
    if has_help(args) {
        print_command_help("validate");
        return Ok(ExitCode::SUCCESS);
    }

    let options = CommandOptions::parse(args, "validate")?;
    let label = source_label(options.path.as_deref());
    let source = read_source(options.path.as_deref())?;
    let style = Style::stdout();

    let result = if options.concrete {
        validate_concrete_source(&source)
    } else {
        validate_source(&source)
    };

    match result {
        Ok(document) => {
            let _ = encode(&document);
            if options.json {
                println!("{}", validate_success_to_json());
            } else {
                println!("{}", style.ok_line(&style.path(&label)));
                println!("{}", style.indent(&style.dim("OK — no diagnostics")));
            }
            Ok(ExitCode::SUCCESS)
        }
        Err(error) => {
            if options.json {
                println!("{}", validate_error_to_json(&error.diagnostics));
            } else {
                print_human_diagnostics(&label, &error.diagnostics);
            }
            Ok(ExitCode::from(2))
        }
    }
}

fn command_expand(args: &[String]) -> Result<ExitCode, String> {
    if has_help(args) {
        print_command_help("expand");
        return Ok(ExitCode::SUCCESS);
    }

    let options = ExpandOptions::parse(args)?;
    let label = source_label(options.path.as_deref());
    let source = read_source(options.path.as_deref())?;
    let map_json = fs::read_to_string(&options.map_path)
        .map_err(|error| format!("Failed to read map '{}': {error}", options.map_path))?;
    let substitutions = parse_substitution_map(&map_json).map_err(|error| {
        error
            .diagnostics
            .iter()
            .map(|diagnostic| diagnostic.to_string())
            .collect::<Vec<_>>()
            .join("\n")
    })?;

    let result = if options.validate {
        expand_and_validate(&source, &substitutions)
    } else {
        expand_placeholders(&source, &substitutions)
    };

    match result {
        Ok(expanded) => {
            if options.json {
                println!("{}", expand_success_to_json(&expanded));
            } else {
                print!("{expanded}");
                if !expanded.ends_with('\n') {
                    println!();
                }
            }
            Ok(ExitCode::SUCCESS)
        }
        Err(error) => {
            if options.json {
                println!("{}", expand_error_to_json(&error.diagnostics));
            } else {
                print_human_diagnostics(&label, &error.diagnostics);
            }
            Ok(ExitCode::from(2))
        }
    }
}

fn command_validate_registry(args: &[String]) -> Result<ExitCode, String> {
    if has_help(args) {
        print_command_help("validate-registry");
        return Ok(ExitCode::SUCCESS);
    }

    let options = CommandOptions::parse(args, "validate-registry")?;
    let label = source_label(options.path.as_deref());
    let source = read_source(options.path.as_deref())?;
    let style = Style::stdout();

    match decode_name_registry(&source) {
        Ok(_) => {
            if options.json {
                println!("{}", validate_success_to_json());
            } else {
                println!("{}", style.ok_line(&style.path(&label)));
                println!(
                    "{}",
                    style.indent(&style.dim("OK — plugin/event names are disjoint"))
                );
            }
            Ok(ExitCode::SUCCESS)
        }
        Err(error) => {
            if options.json {
                println!("{}", validate_error_to_json(&error.diagnostics));
            } else {
                print_human_diagnostics(&label, &error.diagnostics);
            }
            Ok(ExitCode::from(2))
        }
    }
}

fn print_human_diagnostics(label: &str, diagnostics: &[RainbowDiagnostic]) {
    let style = Style::stderr();
    eprintln!("{}", style.fail_line(&style.path(label)));
    for diagnostic in diagnostics {
        let severity = match diagnostic.severity {
            RainbowDiagnosticSeverity::Error => style.red("error"),
            RainbowDiagnosticSeverity::Warning => style.yellow("warning"),
        };
        eprintln!(
            "{}",
            style.indent(&format!(
                "{}:{}  {}  {}",
                style.cyan(&diagnostic.range.start.line.to_string()),
                style.cyan(&diagnostic.range.start.column.to_string()),
                severity,
                diagnostic.message
            ))
        );
    }
    let count = diagnostics.len();
    let noun = if count == 1 { "issue" } else { "issues" };
    eprintln!();
    eprintln!("{}", style.fail_line(&format!("{count} {noun}")));
}

#[derive(Debug, Default)]
struct CommandOptions {
    json: bool,
    check: bool,
    concrete: bool,
    path: Option<String>,
}

impl CommandOptions {
    fn parse(args: &[String], command: &str) -> Result<Self, String> {
        let mut options = Self::default();

        for arg in args {
            match arg.as_str() {
                "--json" => options.json = true,
                "--check" if command == "format" => options.check = true,
                "--concrete" if command == "validate" => options.concrete = true,
                "--help" | "-h" => return Err(command_usage(command)),
                _ if arg.starts_with('-') && arg != "-" => {
                    return Err(format!(
                        "Unknown option '{arg}'.\n\n{}",
                        command_usage(command)
                    ));
                }
                _ => {
                    if options.path.replace(arg.clone()).is_some() {
                        return Err(format!(
                            "Only one input path is supported.\n\n{}",
                            command_usage(command)
                        ));
                    }
                }
            }
        }

        Ok(options)
    }
}

#[derive(Debug, Default)]
struct ExpandOptions {
    json: bool,
    validate: bool,
    map_path: String,
    path: Option<String>,
}

impl ExpandOptions {
    fn parse(args: &[String]) -> Result<Self, String> {
        let mut options = Self::default();
        let mut index = 0usize;

        while index < args.len() {
            match args[index].as_str() {
                "--json" => options.json = true,
                "--validate" => options.validate = true,
                "--map" => {
                    index += 1;
                    let Some(path) = args.get(index) else {
                        return Err(format!(
                            "Missing path after --map.\n\n{}",
                            command_usage("expand")
                        ));
                    };
                    options.map_path = path.clone();
                }
                "--help" | "-h" => return Err(command_usage("expand")),
                arg if arg.starts_with('-') && arg != "-" => {
                    return Err(format!(
                        "Unknown option '{arg}'.\n\n{}",
                        command_usage("expand")
                    ));
                }
                arg => {
                    if options.path.replace(arg.to_string()).is_some() {
                        return Err(format!(
                            "Only one input path is supported.\n\n{}",
                            command_usage("expand")
                        ));
                    }
                }
            }
            index += 1;
        }

        if options.map_path.is_empty() {
            return Err(format!(
                "Missing required --map <file.json>.\n\n{}",
                command_usage("expand")
            ));
        }

        Ok(options)
    }
}

fn read_source(path: Option<&str>) -> Result<String, String> {
    match path {
        Some("-") | None => {
            let mut source = String::new();
            io::stdin()
                .read_to_string(&mut source)
                .map_err(|error| format!("Failed to read stdin: {error}"))?;
            Ok(source)
        }
        Some(path) => {
            fs::read_to_string(path).map_err(|error| format!("Failed to read '{path}': {error}"))
        }
    }
}

fn source_label(path: Option<&str>) -> String {
    match path {
        Some("-") | None => "<stdin>".to_string(),
        Some(path) => path.to_string(),
    }
}

fn has_help(args: &[String]) -> bool {
    args.iter().any(|arg| arg == "--help" || arg == "-h")
}

fn print_help() {
    println!("{}", usage());
}

fn print_command_help(command: &str) {
    println!("{}", command_usage(command));
}

fn usage() -> String {
    let style = Style::stdout();
    format!(
        "\
{} {}
Rainbow DSL toolkit — parse, format, validate, and set up editors.

{}
  rainbow <command> [options]

{}
  parse              Parse a .rbw file → JSON AST
  format             Print canonical Rainbow source
  validate           Check syntax / naming (templates OK)
  validate-registry  Check plugin/event name registry JSON
  expand             Substitute #{{...}} placeholders (backend)

{}
  install    Install editor extension (Cursor / VS Code)
  doctor     Check CLI, LSP, editors, and settings

{}
  -h, --help       Show help
  -V, --version    Show version

{}
  rainbow validate screen.rbw
  rainbow expand --map vars.json template.rbw
  rainbow validate-registry names.json
  rainbow format home.rbw
  rainbow install cursor
  rainbow doctor

{}
  Colors respect TTY, NO_COLOR, and FORCE_COLOR.
  See `rainbow <command> --help` for details.",
        style.bold("rainbow"),
        style.cyan(RAINBOW_PARSER_VERSION),
        style.dim("Usage"),
        style.dim("Core"),
        style.dim("Setup"),
        style.dim("Flags"),
        style.dim("Examples"),
        style.dim("Notes"),
    )
}

fn command_usage(command: &str) -> String {
    let style = Style::stdout();
    match command {
        "parse" => format!(
            "\
{}

{}
  rainbow parse [--json] [file|-]

Parse Rainbow source and print a JSON response (Go/CLI contract).
On syntax errors without --json, prints human diagnostics to stderr (exit 2).",
            style.header("rainbow parse"),
            style.dim("Usage"),
        ),
        "format" => format!(
            "\
{}

{}
  rainbow format [--json] [--check] [file|-]

Print canonical Rainbow source.
  --check   exit 0 if already formatted, 1 if it would change
  --json    machine-readable response",
            style.header("rainbow format"),
            style.dim("Usage"),
        ),
        "validate" => format!(
            "\
{}

{}
  rainbow validate [--json] [--concrete] [file|-]

Validate Rainbow syntax and document naming rules.
Templates with #{{Name}} are allowed unless --concrete is set.
Fails on non-UpperCamelCase plugin/event/trigger/use names, or if the same
name is used both as a plugin (root/body) and as an event (under On*).
Human output uses ✓ / ✗ feedback; --json keeps the machine contract.",
            style.header("rainbow validate"),
            style.dim("Usage"),
        ),
        "expand" => format!(
            "\
{}

{}
  rainbow expand --map <file.json> [--json] [--validate] [file|-]

Replace every #{{Name}} using a JSON string map for backend pipelines.
  --map        required object of placeholder → Rainbow fragment
  --validate   expand then run concrete validation (no leftover holes)
Same placeholder name is substituted in every occurrence.",
            style.header("rainbow expand"),
            style.dim("Usage"),
        ),
        "validate-registry" => format!(
            "\
{}

{}
  rainbow validate-registry [--json] [file|-]

Validate a plugin/event name registry JSON file.
Fails if the same name appears in both \"plugins\" and \"events\".",
            style.header("rainbow validate-registry"),
            style.dim("Usage"),
        ),
        _ => usage(),
    }
}
