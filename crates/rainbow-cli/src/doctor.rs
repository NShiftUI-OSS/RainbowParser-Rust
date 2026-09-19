use std::fs;
use std::process::ExitCode;

use rainbow_parser::RAINBOW_PARSER_VERSION;

use crate::install::{
    detect_available_ides, find_share_vsix, ide_user_settings_path, resolve_lsp_path_hint, which,
    Ide,
};
use crate::style::Style;

pub fn command_doctor(args: &[String]) -> Result<ExitCode, String> {
    if args.iter().any(|arg| arg == "--help" || arg == "-h") {
        println!("{}", doctor_usage());
        return Ok(ExitCode::SUCCESS);
    }

    let style = Style::stdout();
    println!("{}", style.header("Rainbow doctor"));
    println!();

    let mut issues = 0usize;

    // CLI
    let cli_path = std::env::current_exe()
        .ok()
        .map(|path| path.display().to_string())
        .or_else(|| which("rainbow").map(|path| path.display().to_string()))
        .unwrap_or_else(|| "rainbow".to_string());
    println!(
        "{}",
        style.ok_line(&format!(
            "CLI        {}  ({})",
            style.bold(RAINBOW_PARSER_VERSION),
            style.path(&cli_path)
        ))
    );

    // LSP
    let lsp_path = resolve_lsp_path_hint();
    let lsp_file_ok = lsp_path != "rainbow-lsp" && std::path::Path::new(&lsp_path).is_file();
    let lsp_ok = lsp_file_ok || which("rainbow-lsp").is_some();
    if lsp_ok {
        let display = if lsp_file_ok {
            lsp_path.clone()
        } else {
            which("rainbow-lsp")
                .map(|path| path.display().to_string())
                .unwrap_or(lsp_path.clone())
        };
        println!(
            "{}",
            style.ok_line(&format!(
                "LSP        rainbow-lsp  ({})",
                style.path(&display)
            ))
        );
    } else {
        issues += 1;
        println!(
            "{}",
            style.fail_line("LSP        rainbow-lsp not found (brew install rainbow?)")
        );
    }

    // VSIX
    match find_share_vsix() {
        Some(path) => println!(
            "{}",
            style.ok_line(&format!(
                "VSIX       {}",
                style.path(&path.display().to_string())
            ))
        ),
        None => {
            issues += 1;
            println!(
                "{}",
                style.fail_line("VSIX       not found in Homebrew share/rainbow")
            );
        }
    }

    println!();
    println!("{}", style.dim("Editors"));

    for ide in Ide::all() {
        let cli_available = which(ide.cli_name()).is_some();
        if !cli_available {
            println!(
                "  {}",
                style.skip_line(&format!(
                    "{:<8}  CLI `{}` not on PATH",
                    ide.display_name(),
                    ide.cli_name()
                ))
            );
            continue;
        }

        println!(
            "  {}",
            style.ok_line(&format!(
                "{:<8}  CLI `{}` available",
                ide.display_name(),
                ide.cli_name()
            ))
        );

        match ide_user_settings_path(ide) {
            Ok(settings_path) => {
                if settings_path.is_file() {
                    match fs::read_to_string(&settings_path) {
                        Ok(contents) => {
                            let has_lsp = contents.contains("rainbow.lsp.path");
                            let has_formatter = contents.contains("tcc-nshiftui.vscode-rainbow")
                                && contents.contains("defaultFormatter");
                            if has_lsp && has_formatter {
                                println!(
                                    "  {}",
                                    style.ok_line(&format!(
                                        "{:<8}  lsp path + formatter set in {}",
                                        "",
                                        style.path(&settings_path.display().to_string())
                                    ))
                                );
                            } else {
                                issues += 1;
                                let missing = match (has_lsp, has_formatter) {
                                    (false, false) => "rainbow.lsp.path and defaultFormatter",
                                    (false, true) => "rainbow.lsp.path",
                                    (true, false) => "[rainbow] defaultFormatter",
                                    (true, true) => unreachable!(),
                                };
                                println!(
                                    "  {}",
                                    style.warn_line(&format!(
                                        "         settings found but {missing} missing — run `rainbow install {}`",
                                        ide.install_arg()
                                    ))
                                );
                            }
                        }
                        Err(_) => {
                            issues += 1;
                            println!(
                                "  {}",
                                style.fail_line(&format!(
                                    "         could not read {}",
                                    settings_path.display()
                                ))
                            );
                        }
                    }
                } else {
                    issues += 1;
                    println!(
                        "  {}",
                        style.warn_line(&format!(
                            "         no user settings yet — run `rainbow install {}`",
                            ide.install_arg()
                        ))
                    );
                }
            }
            Err(error) => {
                issues += 1;
                println!(
                    "  {}",
                    style.fail_line(&format!("{:<8}  {error}", ide.display_name()))
                );
            }
        }
    }

    let available = detect_available_ides();
    println!();
    if issues == 0 && !available.is_empty() {
        println!("{}", style.ok_line("All checks passed."));
        println!(
            "{}",
            style.indent(&style.dim("Reload the editor window and open a .rbw file."))
        );
        Ok(ExitCode::SUCCESS)
    } else if available.is_empty() {
        println!(
            "{}",
            style.fail_line("No editor CLIs detected. Install `cursor` or `code` on PATH.")
        );
        Ok(ExitCode::from(1))
    } else {
        println!(
            "{}",
            style.warn_line(&format!(
                "{issues} issue(s) found. Try `rainbow install` after fixing the items above."
            ))
        );
        Ok(ExitCode::from(1))
    }
}

fn doctor_usage() -> String {
    "\
rainbow doctor — check CLI, LSP, VSIX, and editor setup

Usage:
  rainbow doctor

Exit codes:
  0  all relevant checks passed
  1  one or more issues found"
        .to_string()
}
