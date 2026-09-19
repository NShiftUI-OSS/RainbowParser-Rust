use std::env;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode, Stdio};

use rainbow_parser::RAINBOW_PARSER_VERSION;

use crate::style::Style;

const PACKAGE_NAME: &str = "vscode-rainbow";
const DEFAULT_GITLAB_HOST: &str = "https://gitlab.com";
const DEFAULT_GITLAB_PROJECT: &str = "tcc-nshiftui/global/rainbowparser-rust";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ide {
    Cursor,
    VsCode,
}

impl Ide {
    pub fn parse(name: &str) -> Option<Self> {
        match name.to_ascii_lowercase().as_str() {
            "cursor" => Some(Self::Cursor),
            "vscode" | "code" | "vs-code" => Some(Self::VsCode),
            "both" => None, // handled separately
            _ => None,
        }
    }

    pub fn cli_name(self) -> &'static str {
        match self {
            Self::Cursor => "cursor",
            Self::VsCode => "code",
        }
    }

    pub fn display_name(self) -> &'static str {
        match self {
            Self::Cursor => "Cursor",
            Self::VsCode => "VS Code",
        }
    }

    pub fn install_arg(self) -> &'static str {
        match self {
            Self::Cursor => "cursor",
            Self::VsCode => "vscode",
        }
    }

    pub fn all() -> [Ide; 2] {
        [Ide::Cursor, Ide::VsCode]
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum InstallTarget {
    One(Ide),
    Both,
}

#[derive(Debug)]
struct InstallOptions {
    list_only: bool,
    dry_run: bool,
    write_settings: bool,
    vsix: Option<PathBuf>,
    target: Option<InstallTarget>,
}

impl Default for InstallOptions {
    fn default() -> Self {
        Self {
            list_only: false,
            dry_run: false,
            write_settings: true,
            vsix: None,
            target: None,
        }
    }
}

pub fn command_install(args: &[String]) -> Result<ExitCode, String> {
    if has_flag(args, "--help") || has_flag(args, "-h") {
        print_install_help();
        return Ok(ExitCode::SUCCESS);
    }

    let style = Style::stdout();
    let options = InstallOptions::parse(args)?;

    if options.list_only {
        list_ides(&style);
        return Ok(ExitCode::SUCCESS);
    }

    println!("{}", style.header("Rainbow editor setup"));
    println!();

    let available = detect_available_ides();
    print_detected_editors(&style);

    if available.is_empty() {
        return Err(format!(
            "{}\n\
             \n\
             Install the editor, then enable the shell command:\n\
               Cursor:  Command Palette → \"Shell Command: Install 'cursor' command\"\n\
               VS Code: Command Palette → \"Shell Command: Install 'code' command\"\n\
             \n\
             Then re-run: {}",
            style.fail_line("Neither Cursor nor VS Code CLI was found on PATH."),
            style.cyan("rainbow install")
        ));
    }

    let targets = resolve_targets(options.target, &available, &style)?;
    let vsix = resolve_vsix(options.vsix.as_deref(), &style)?;
    let lsp_path = resolve_lsp_path_hint();

    println!("{}", style.dim("Package"));
    println!(
        "  {}",
        style.ok_line(&style.path(&vsix.display().to_string()))
    );
    println!(
        "  {}",
        style.indent(&format!("{} LSP {}", style.arrow(), style.path(&lsp_path)))
    );
    println!();

    let steps_per_ide = if options.write_settings { 2 } else { 1 };
    let total_steps = targets.len() * steps_per_ide;
    let target_names = targets
        .iter()
        .map(|ide| ide.display_name())
        .collect::<Vec<_>>()
        .join(" + ");
    println!("{} {}", style.dim("Target"), style.bold(&target_names));
    println!();

    let mut step = 1usize;
    for ide in &targets {
        println!(
            "{}",
            style.step(
                step,
                total_steps,
                &format!("Install extension → {}", ide.display_name())
            )
        );
        install_into_ide(*ide, &vsix, options.dry_run, &style)?;
        println!(
            "{}",
            style.step_done(
                step,
                total_steps,
                &format!("{} extension ready", ide.display_name())
            )
        );
        step += 1;

        if options.write_settings {
            println!(
                "{}",
                style.step(
                    step,
                    total_steps,
                    &format!("Write settings → {}", ide.display_name())
                )
            );
            configure_ide_settings(*ide, &lsp_path, options.dry_run, &style)?;
            println!(
                "{}",
                style.step_done(
                    step,
                    total_steps,
                    &format!("lsp path + [rainbow] formatter → {}", style.path(&lsp_path))
                )
            );
            step += 1;
        }
    }

    print_post_install_notes(&style, &lsp_path, options.write_settings, &targets);
    Ok(ExitCode::SUCCESS)
}

impl InstallOptions {
    fn parse(args: &[String]) -> Result<Self, String> {
        let mut options = Self::default();
        let mut positional: Option<String> = None;

        let mut index = 0;
        while index < args.len() {
            let arg = &args[index];
            match arg.as_str() {
                "--list" | "-l" => options.list_only = true,
                "--dry-run" => options.dry_run = true,
                "--no-settings" => options.write_settings = false,
                "--vsix" => {
                    index += 1;
                    let path = args
                        .get(index)
                        .ok_or_else(|| "Missing path after --vsix.".to_string())?;
                    options.vsix = Some(PathBuf::from(path));
                }
                "--help" | "-h" => {}
                _ if arg.starts_with('-') => {
                    return Err(format!("Unknown option '{arg}'.\n\n{}", install_usage()));
                }
                _ => {
                    if positional.replace(arg.clone()).is_some() {
                        return Err(format!(
                            "Only one IDE argument is supported.\n\n{}",
                            install_usage()
                        ));
                    }
                }
            }
            index += 1;
        }

        if let Some(name) = positional {
            options.target = Some(parse_target(&name)?);
        }

        Ok(options)
    }
}

fn parse_target(name: &str) -> Result<InstallTarget, String> {
    match name.to_ascii_lowercase().as_str() {
        "both" => Ok(InstallTarget::Both),
        other => Ide::parse(other).map(InstallTarget::One).ok_or_else(|| {
            format!(
                "Unknown IDE '{other}'. Supported: cursor, vscode, both.\n\n{}",
                install_usage()
            )
        }),
    }
}

fn list_ides(style: &Style) {
    println!("{}", style.header("Compatible IDEs"));
    println!();
    print_detected_editors(style);
    println!();
    println!("{}", style.dim("Also accepted as argument: both"));
}

fn print_detected_editors(style: &Style) {
    println!("{}", style.dim("Detected"));
    for ide in Ide::all() {
        if which(ide.cli_name()).is_some() {
            println!(
                "  {}",
                style.ok_line(&format!(
                    "{:<8}  available ({})",
                    ide.display_name(),
                    style.cyan(ide.cli_name())
                ))
            );
        } else {
            println!(
                "  {}",
                style.skip_line(&format!(
                    "{:<8}  CLI `{}` not on PATH",
                    ide.display_name(),
                    ide.cli_name()
                ))
            );
        }
    }
    println!();
}

pub(crate) fn detect_available_ides() -> Vec<Ide> {
    Ide::all()
        .into_iter()
        .filter(|ide| which(ide.cli_name()).is_some())
        .collect()
}

fn resolve_targets(
    requested: Option<InstallTarget>,
    available: &[Ide],
    style: &Style,
) -> Result<Vec<Ide>, String> {
    match requested {
        Some(InstallTarget::One(ide)) => {
            if !available.contains(&ide) {
                return Err(format!(
                    "{}\nAvailable: {}",
                    style.fail_line(&format!(
                        "{} CLI (`{}`) was not found on PATH.",
                        ide.display_name(),
                        ide.cli_name()
                    )),
                    available_names(available)
                ));
            }
            Ok(vec![ide])
        }
        Some(InstallTarget::Both) => {
            if available.len() < 2 {
                return Err(format!(
                    "{}\nAvailable: {}",
                    style.fail_line("`both` requires Cursor and VS Code CLIs on PATH."),
                    available_names(available)
                ));
            }
            Ok(available.to_vec())
        }
        None => {
            if available.len() == 1 {
                println!(
                    "{}",
                    style.ok_line(&format!(
                        "Using {} (only editor CLI detected)",
                        available[0].display_name()
                    ))
                );
                println!();
                Ok(available.to_vec())
            } else {
                prompt_ide_selection(available, style)
            }
        }
    }
}

fn prompt_ide_selection(available: &[Ide], style: &Style) -> Result<Vec<Ide>, String> {
    println!("{}", style.header("Where should Rainbow install?"));
    println!();
    println!("  {}  Cursor", style.cyan("1)"));
    println!("  {}  VS Code", style.cyan("2)"));
    println!("  {}  Both", style.cyan("3)"));
    print!("\n{} ", style.dim("Selection [1-3]:"));
    io::stdout()
        .flush()
        .map_err(|error| format!("Failed to write prompt: {error}"))?;

    let mut line = String::new();
    io::stdin()
        .read_line(&mut line)
        .map_err(|error| format!("Failed to read selection: {error}"))?;

    match line.trim() {
        "1" => require_available(Ide::Cursor, available).map(|ide| vec![ide]),
        "2" => require_available(Ide::VsCode, available).map(|ide| vec![ide]),
        "3" => Ok(available.to_vec()),
        other => Err(format!(
            "{}",
            style.fail_line(&format!(
                "Invalid selection '{other}'. Expected 1, 2, or 3."
            ))
        )),
    }
}

fn require_available(ide: Ide, available: &[Ide]) -> Result<Ide, String> {
    if available.contains(&ide) {
        Ok(ide)
    } else {
        Err(format!(
            "{} CLI is not available on PATH.",
            ide.display_name()
        ))
    }
}

fn resolve_vsix(explicit: Option<&Path>, style: &Style) -> Result<PathBuf, String> {
    if let Some(path) = explicit {
        if path.is_file() {
            return Ok(path.to_path_buf());
        }
        return Err(format!(
            "{}",
            style.fail_line(&format!("VSIX not found: {}", path.display()))
        ));
    }

    if let Ok(path) = env::var("RAINBOW_VSIX") {
        let path = PathBuf::from(path);
        if path.is_file() {
            return Ok(path);
        }
        return Err(format!(
            "{}",
            style.fail_line(&format!(
                "RAINBOW_VSIX is set but file not found: {}",
                path.display()
            ))
        ));
    }

    if let Some(path) = find_share_vsix() {
        println!("{}", style.dim("Resolved VSIX from Homebrew share/rainbow"));
        return Ok(path);
    }

    if let Some(path) = download_vsix_from_gitlab(style)? {
        println!(
            "{}",
            style.ok_line("Downloaded VSIX from GitLab Package Registry")
        );
        return Ok(path);
    }

    if let Some(path) = build_vsix_from_source(style)? {
        println!(
            "{}",
            style.ok_line("Built VSIX from local extension source")
        );
        return Ok(path);
    }

    Err(format!(
        "{}\n\
         \n\
         Options:\n\
           1) brew reinstall rainbow   # packs VSIX into share/rainbow\n\
           2) rainbow install --vsix /path/to/vscode-rainbow.vsix\n\
           3) Set GITLAB_TOKEN and retry (downloads from private Package Registry)\n\
           4) Set RAINBOW_EXTENSION_DIR to editors/vscode-rainbow and ensure Node is installed",
        style.fail_line("Could not find or build a Rainbow editor extension (.vsix).")
    ))
}

pub(crate) fn find_share_vsix() -> Option<PathBuf> {
    let mut dirs = Vec::new();

    if let Ok(output) = Command::new("brew").args(["--prefix", "rainbow"]).output() {
        if output.status.success() {
            let prefix = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !prefix.is_empty() {
                dirs.push(PathBuf::from(prefix).join("share/rainbow"));
            }
        }
    }

    if let Ok(exe) = env::current_exe() {
        if let Some(bin_dir) = exe.parent() {
            if let Some(prefix) = bin_dir.parent() {
                dirs.push(prefix.join("share/rainbow"));
            }
        }
    }

    for dir in dirs {
        if let Some(path) = newest_vsix_in(&dir) {
            return Some(path);
        }
    }

    None
}

fn newest_vsix_in(dir: &Path) -> Option<PathBuf> {
    let mut files: Vec<PathBuf> = fs::read_dir(dir)
        .ok()?
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .filter(|path| {
            path.extension()
                .and_then(|ext| ext.to_str())
                .is_some_and(|ext| ext.eq_ignore_ascii_case("vsix"))
        })
        .collect();

    files.sort();
    files.pop()
}

fn download_vsix_from_gitlab(style: &Style) -> Result<Option<PathBuf>, String> {
    let token = env::var("RAINBOW_GITLAB_TOKEN")
        .or_else(|_| env::var("GITLAB_TOKEN"))
        .or_else(|_| env::var("PRIVATE_TOKEN"))
        .ok();

    let Some(token) = token else {
        return Ok(None);
    };

    let host = env::var("RAINBOW_GITLAB_HOST").unwrap_or_else(|_| DEFAULT_GITLAB_HOST.to_string());
    let project =
        env::var("RAINBOW_GITLAB_PROJECT").unwrap_or_else(|_| DEFAULT_GITLAB_PROJECT.to_string());
    let version =
        env::var("RAINBOW_VSIX_VERSION").unwrap_or_else(|_| RAINBOW_PARSER_VERSION.to_string());

    let encoded_project = urlencoding_encode(&project);
    let url = format!(
        "{host}/api/v4/projects/{encoded_project}/packages/generic/{PACKAGE_NAME}/{version}/vscode-rainbow.vsix"
    );

    let temp_dir = env::temp_dir().join(format!("rainbow-vsix-{version}"));
    fs::create_dir_all(&temp_dir)
        .map_err(|error| format!("Failed to create temp dir {}: {error}", temp_dir.display()))?;
    let dest = temp_dir.join(format!("{PACKAGE_NAME}-{version}.vsix"));

    println!("{}", style.dim("Downloading VSIX from GitLab…"));
    println!("  {}", style.path(&url));

    let response = ureq::get(&url)
        .set("PRIVATE-TOKEN", &token)
        .call()
        .map_err(|error| format!("GitLab download failed: {error}"))?;

    if !(200..300).contains(&response.status()) {
        return Err(format!(
            "GitLab download failed with HTTP {}.",
            response.status()
        ));
    }

    let mut reader = response.into_reader();
    let mut file = fs::File::create(&dest)
        .map_err(|error| format!("Failed to write {}: {error}", dest.display()))?;
    io::copy(&mut reader, &mut file).map_err(|error| format!("Failed to save VSIX: {error}"))?;

    Ok(Some(dest))
}

fn build_vsix_from_source(style: &Style) -> Result<Option<PathBuf>, String> {
    let Some(extension_dir) = find_extension_source_dir() else {
        return Ok(None);
    };

    if which("npm").is_none() {
        return Err(format!(
            "{}",
            style.fail_line(
                "Found extension source, but `npm` is not on PATH. Install Node 20+ or use a prebuilt VSIX."
            )
        ));
    }

    println!("{}", style.dim("Building extension from source…"));
    println!("  {}", style.path(&extension_dir.display().to_string()));

    run_checked(
        Command::new("npm")
            .arg("ci")
            .current_dir(&extension_dir)
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit()),
        "npm ci",
    )?;

    run_checked(
        Command::new("npm")
            .arg("run")
            .arg("compile")
            .current_dir(&extension_dir)
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit()),
        "npm run compile",
    )?;

    run_checked(
        Command::new("npx")
            .args(["vsce", "package", "--skip-license"])
            .current_dir(&extension_dir)
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit()),
        "vsce package",
    )?;

    newest_vsix_in(&extension_dir)
        .ok_or_else(|| "vsce package finished but no .vsix was produced.".to_string())
        .map(Some)
}

fn find_extension_source_dir() -> Option<PathBuf> {
    if let Ok(dir) = env::var("RAINBOW_EXTENSION_DIR") {
        let path = PathBuf::from(dir);
        if path.join("package.json").is_file() {
            return Some(path);
        }
    }

    let mut candidates = Vec::new();

    if let Ok(cwd) = env::current_dir() {
        candidates.push(cwd.join("editors/vscode-rainbow"));
        candidates.push(cwd);
    }

    if let Ok(exe) = env::current_exe() {
        if let Some(bin) = exe.parent() {
            candidates.push(bin.join("../editors/vscode-rainbow"));
            candidates.push(bin.join("../../editors/vscode-rainbow"));
            candidates.push(bin.join("../../../editors/vscode-rainbow"));
        }
    }

    for candidate in candidates {
        if let Ok(path) = candidate.canonicalize() {
            if path.join("package.json").is_file()
                && path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| name.contains("rainbow"))
            {
                return Some(path);
            }
            // Allow editors/vscode-rainbow even if folder naming differs after canonicalize
            if path.join("package.json").is_file() && path.join("src/extension.ts").is_file() {
                return Some(path);
            }
        }
    }

    None
}

fn install_into_ide(ide: Ide, vsix: &Path, dry_run: bool, style: &Style) -> Result<(), String> {
    let cli = ide.cli_name();

    if dry_run {
        println!(
            "  {}",
            style.dim(&format!(
                "[dry-run] {cli} --install-extension {}",
                vsix.display()
            ))
        );
        return Ok(());
    }

    let output = Command::new(cli)
        .arg("--install-extension")
        .arg(vsix)
        .arg("--force")
        .output()
        .map_err(|error| format!("Failed to run `{cli}`: {error}"))?;

    if !output.status.success() {
        let detail = String::from_utf8_lossy(&output.stderr);
        let detail = detail.trim();
        if detail.is_empty() {
            return Err(format!(
                "{}",
                style.fail_line(&format!(
                    "`{cli} --install-extension` failed with status {}.",
                    output.status
                ))
            ));
        }
        return Err(format!(
            "{}\n{}",
            style.fail_line(&format!(
                "`{cli} --install-extension` failed with status {}.",
                output.status
            )),
            style.indent(detail)
        ));
    }

    Ok(())
}

fn configure_ide_settings(
    ide: Ide,
    lsp_path: &str,
    dry_run: bool,
    style: &Style,
) -> Result<(), String> {
    let settings_path = ide_user_settings_path(ide)?;
    println!(
        "  {}",
        style.indent(&style.path(&settings_path.display().to_string()))
    );

    if dry_run {
        println!(
            "  {}",
            style.dim(&format!(
                "[dry-run] set rainbow.lsp.path = {lsp_path} and [rainbow] defaultFormatter"
            ))
        );
        return Ok(());
    }

    write_rainbow_editor_settings(&settings_path, lsp_path)?;
    Ok(())
}

pub(crate) fn ide_user_settings_path(ide: Ide) -> Result<PathBuf, String> {
    let home = dirs_home_dir().ok_or_else(|| "Could not resolve home directory.".to_string())?;

    if cfg!(target_os = "windows") {
        let appdata = env::var_os("APPDATA")
            .map(PathBuf::from)
            .unwrap_or_else(|| home.join("AppData/Roaming"));
        return Ok(match ide {
            Ide::Cursor => appdata.join("Cursor/User/settings.json"),
            Ide::VsCode => appdata.join("Code/User/settings.json"),
        });
    }

    let relative = if cfg!(target_os = "macos") {
        match ide {
            Ide::Cursor => "Library/Application Support/Cursor/User/settings.json",
            Ide::VsCode => "Library/Application Support/Code/User/settings.json",
        }
    } else {
        match ide {
            Ide::Cursor => ".config/Cursor/User/settings.json",
            Ide::VsCode => ".config/Code/User/settings.json",
        }
    };

    Ok(home.join(relative))
}

fn dirs_home_dir() -> Option<PathBuf> {
    env::var_os("HOME")
        .or_else(|| env::var_os("USERPROFILE"))
        .map(PathBuf::from)
}

/// VS Code / Cursor extension id that owns Format Document for Rainbow.
const RAINBOW_EXTENSION_ID: &str = "tcc-nshiftui.vscode-rainbow";

fn write_rainbow_editor_settings(settings_path: &Path, lsp_path: &str) -> Result<(), String> {
    if let Some(parent) = settings_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            format!(
                "Failed to create settings directory {}: {error}",
                parent.display()
            )
        })?;
    }

    let existing = if settings_path.is_file() {
        fs::read_to_string(settings_path).map_err(|error| {
            format!(
                "Failed to read settings {}: {error}",
                settings_path.display()
            )
        })?
    } else {
        "{}".to_string()
    };

    let updated = upsert_rainbow_settings(&existing, lsp_path)?;
    fs::write(settings_path, updated).map_err(|error| {
        format!(
            "Failed to write settings {}: {error}",
            settings_path.display()
        )
    })?;
    Ok(())
}

fn upsert_rainbow_settings(existing: &str, lsp_path: &str) -> Result<String, String> {
    let trimmed = existing.trim();
    let source = if trimmed.is_empty() { "{}" } else { existing };
    let without_comments = strip_json_comments(source);
    let mut value: serde_json::Value =
        serde_json::from_str(&without_comments).map_err(|error| {
            format!(
                "Could not parse editor settings.json (unsupported comments/syntax): {error}\n\
             Fix the file manually or re-run with --no-settings."
            )
        })?;

    let object = value
        .as_object_mut()
        .ok_or_else(|| "Editor settings.json root must be a JSON object.".to_string())?;
    object.insert(
        "rainbow.lsp.path".to_string(),
        serde_json::Value::String(lsp_path.to_string()),
    );

    // Ensure Format Document / format on save use this extension after install/reinstall.
    let language_key = "[rainbow]";
    let mut language_settings = object
        .get(language_key)
        .and_then(|v| v.as_object())
        .cloned()
        .unwrap_or_default();
    language_settings.insert(
        "editor.defaultFormatter".to_string(),
        serde_json::Value::String(RAINBOW_EXTENSION_ID.to_string()),
    );
    language_settings.insert(
        "editor.formatOnSave".to_string(),
        serde_json::Value::Bool(true),
    );
    object.insert(
        language_key.to_string(),
        serde_json::Value::Object(language_settings),
    );

    let mut serialized = serde_json::to_string_pretty(&value)
        .map_err(|error| format!("Failed to serialize settings: {error}"))?;
    serialized.push('\n');
    Ok(serialized)
}

/// Strip `//` and `/* */` comments outside of JSON strings (JSONC → JSON).
fn strip_json_comments(input: &str) -> String {
    let mut output = String::with_capacity(input.len());
    let chars: Vec<char> = input.chars().collect();
    let mut index = 0;
    let mut in_string = false;
    let mut escaped = false;

    while index < chars.len() {
        let ch = chars[index];

        if in_string {
            output.push(ch);
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == '"' {
                in_string = false;
            }
            index += 1;
            continue;
        }

        if ch == '"' {
            in_string = true;
            output.push(ch);
            index += 1;
            continue;
        }

        if ch == '/' && index + 1 < chars.len() {
            let next = chars[index + 1];
            if next == '/' {
                index += 2;
                while index < chars.len() && chars[index] != '\n' {
                    index += 1;
                }
                continue;
            }
            if next == '*' {
                index += 2;
                while index + 1 < chars.len() && !(chars[index] == '*' && chars[index + 1] == '/') {
                    index += 1;
                }
                index = (index + 2).min(chars.len());
                continue;
            }
        }

        output.push(ch);
        index += 1;
    }

    output
}

fn print_post_install_notes(style: &Style, lsp_path: &str, wrote_settings: bool, targets: &[Ide]) {
    println!();
    println!("{}", style.ok_line(&style.bold("Done")));
    println!();
    println!("{}", style.dim("Next"));
    if wrote_settings {
        let editors = targets
            .iter()
            .map(|ide| ide.display_name())
            .collect::<Vec<_>>()
            .join(" / ");
        println!(
            "  {} + {} formatter configured in {editors}",
            style.cyan("rainbow.lsp.path"),
            style.cyan("[rainbow]")
        );
        println!("  {}", style.path(lsp_path));
    } else {
        println!(
            "  {}",
            style.dim("Settings were not modified (--no-settings). Set manually:")
        );
        println!(
            "  {}",
            style.cyan(&format!("\"rainbow.lsp.path\": \"{lsp_path}\""))
        );
        println!(
            "  {}",
            style.cyan(&format!(
                "\"[rainbow]\": {{ \"editor.defaultFormatter\": \"{RAINBOW_EXTENSION_ID}\" }}"
            ))
        );
    }
    println!(
        "  {}",
        style.dim("Reload Window (Developer: Reload Window), then Format Document on a .rbw file.")
    );
}

pub(crate) fn resolve_lsp_path_hint() -> String {
    if let Ok(output) = Command::new("brew").args(["--prefix", "rainbow"]).output() {
        if output.status.success() {
            let prefix = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !prefix.is_empty() {
                let candidate = PathBuf::from(&prefix).join("bin/rainbow-lsp");
                if let Ok(canonical) = candidate.canonicalize() {
                    return canonical.display().to_string();
                }
                return candidate.display().to_string();
            }
        }
    }

    if let Some(path) = which("rainbow-lsp") {
        if let Ok(canonical) = path.canonicalize() {
            return canonical.display().to_string();
        }
        return path.display().to_string();
    }

    "rainbow-lsp".to_string()
}

fn run_checked(command: &mut Command, label: &str) -> Result<(), String> {
    let status = command
        .status()
        .map_err(|error| format!("Failed to run {label}: {error}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("{label} failed with status {status}."))
    }
}

pub(crate) fn which(binary: &str) -> Option<PathBuf> {
    let path = env::var_os("PATH")?;
    for dir in env::split_paths(&path) {
        let candidate = dir.join(binary);
        if candidate.is_file() {
            return Some(candidate);
        }
        #[cfg(windows)]
        {
            let candidate = dir.join(format!("{binary}.cmd"));
            if candidate.is_file() {
                return Some(candidate);
            }
            let candidate = dir.join(format!("{binary}.exe"));
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }
    None
}

fn available_names(ides: &[Ide]) -> String {
    if ides.is_empty() {
        "none".to_string()
    } else {
        ides.iter()
            .map(|ide| ide.cli_name())
            .collect::<Vec<_>>()
            .join(", ")
    }
}

fn has_flag(args: &[String], flag: &str) -> bool {
    args.iter().any(|arg| arg == flag)
}

fn urlencoding_encode(value: &str) -> String {
    let mut encoded = String::with_capacity(value.len());
    for byte in value.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                encoded.push(byte as char);
            }
            _ => encoded.push_str(&format!("%{byte:02X}")),
        }
    }
    encoded
}

pub fn print_install_help() {
    println!("{}", install_usage());
}

fn install_usage() -> String {
    let style = Style::stdout();
    format!(
        "\
{}

{}
  rainbow install [--list] [--dry-run] [--no-settings] [--vsix PATH] [cursor|vscode|both]

{}
  rainbow install
  rainbow install cursor
  rainbow install vscode
  rainbow install both
  rainbow install --list
  rainbow install --vsix ./vscode-rainbow.vsix cursor
  rainbow install cursor --no-settings

{}
  Writes \"rainbow.lsp.path\" and \"[rainbow].editor.defaultFormatter\" into
  the editor user settings so Format Document works after Reload (skip with --no-settings).

{}
  1. --vsix PATH / RAINBOW_VSIX
  2. Homebrew share ($(brew --prefix rainbow)/share/rainbow)
  3. GitLab Package Registry (requires GITLAB_TOKEN or RAINBOW_GITLAB_TOKEN)
  4. Local source build (RAINBOW_EXTENSION_DIR or ./editors/vscode-rainbow + Node)

{}
  project  {DEFAULT_GITLAB_PROJECT}
  version  {RAINBOW_PARSER_VERSION}
  package  {PACKAGE_NAME}",
        style.header("rainbow install — install the Rainbow editor extension"),
        style.dim("Usage"),
        style.dim("Examples"),
        style.dim("Settings"),
        style.dim("VSIX resolution order"),
        style.dim("Private GitLab defaults"),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_ide_names() {
        assert_eq!(Ide::parse("cursor"), Some(Ide::Cursor));
        assert_eq!(Ide::parse("VSCode"), Some(Ide::VsCode));
        assert_eq!(Ide::parse("code"), Some(Ide::VsCode));
        assert_eq!(Ide::parse("nope"), None);
    }

    #[test]
    fn parses_install_options() {
        let options = InstallOptions::parse(&[
            "--dry-run".into(),
            "--no-settings".into(),
            "--vsix".into(),
            "/tmp/x.vsix".into(),
            "cursor".into(),
        ])
        .unwrap();
        assert!(options.dry_run);
        assert!(!options.write_settings);
        assert_eq!(options.vsix, Some(PathBuf::from("/tmp/x.vsix")));
        assert_eq!(options.target, Some(InstallTarget::One(Ide::Cursor)));
    }

    #[test]
    fn encodes_project_path() {
        assert_eq!(
            urlencoding_encode("tcc-nshiftui/global/rainbowparser-rust"),
            "tcc-nshiftui%2Fglobal%2Frainbowparser-rust"
        );
    }

    #[test]
    fn upserts_lsp_path_and_formatter_in_json_settings() {
        let updated = upsert_rainbow_settings(
            r#"{ "editor.fontSize": 14, "[rainbow]": { "editor.tabSize": 2 } }"#,
            "/opt/homebrew/bin/rainbow-lsp",
        )
        .unwrap();
        let value: serde_json::Value = serde_json::from_str(&updated).unwrap();
        assert_eq!(value["rainbow.lsp.path"], "/opt/homebrew/bin/rainbow-lsp");
        assert_eq!(value["editor.fontSize"], 14);
        assert_eq!(
            value["[rainbow]"]["editor.defaultFormatter"],
            RAINBOW_EXTENSION_ID
        );
        assert_eq!(value["[rainbow]"]["editor.formatOnSave"], true);
        assert_eq!(value["[rainbow]"]["editor.tabSize"], 2);
    }

    #[test]
    fn upserts_rainbow_settings_in_jsonc() {
        let updated = upsert_rainbow_settings(
            "{\n  // comment\n  \"editor.tabSize\": 2\n}\n",
            "/usr/local/bin/rainbow-lsp",
        )
        .unwrap();
        let value: serde_json::Value = serde_json::from_str(&updated).unwrap();
        assert_eq!(value["rainbow.lsp.path"], "/usr/local/bin/rainbow-lsp");
        assert_eq!(value["editor.tabSize"], 2);
        assert_eq!(
            value["[rainbow]"]["editor.defaultFormatter"],
            RAINBOW_EXTENSION_ID
        );
    }
}
