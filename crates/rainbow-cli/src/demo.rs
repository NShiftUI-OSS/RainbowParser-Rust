use std::env;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};
use std::time::Duration;

use rainbow_parser::RAINBOW_PARSER_VERSION;

use crate::style::Style;

pub const DEMO_OWNER: &str = "NShiftUI-OSS";
pub const DEMO_REPO: &str = "NShiftUI-Demo";
pub const DEMO_TAG: &str = "v1.0.0";

const DEMO_FOLDERS: [&str; 4] = ["web", "back", "ios", "android"];
const TAG_FILE: &str = ".rainbow-demo-tag";

pub fn command_demo(args: &[String]) -> Result<ExitCode, String> {
    if args.iter().any(|arg| arg == "--help" || arg == "-h") {
        println!("{}", demo_usage());
        return Ok(ExitCode::SUCCESS);
    }

    let force = parse_force(args)?;
    let style = Style::stdout();
    let destination = demo_directory();

    println!("{}", style.header("Rainbow demo"));
    println!();

    if !force && demo_is_installed(&destination) {
        println!(
            "{}",
            style.ok_line(&format!("{DEMO_TAG} is already downloaded"))
        );
        print_location(&style, &destination);
        println!();
        println!(
            "{}",
            style.dim("Run `rainbow demo --force` to download it again.")
        );
        return Ok(ExitCode::SUCCESS);
    }

    let url = demo_archive_url();
    println!("{}", style.dim("Downloading"));
    println!("  {}", style.path(&url));
    println!();

    let work = env::temp_dir().join(format!("rainbow-demo-{}-{}", DEMO_TAG, std::process::id()));
    if work.exists() {
        fs::remove_dir_all(&work)
            .map_err(|error| format!("Failed to clear {}: {error}", work.display()))?;
    }
    fs::create_dir_all(&work)
        .map_err(|error| format!("Failed to create {}: {error}", work.display()))?;

    let archive = work.join("demo.tar.gz");
    let extracted = download_and_unpack(&url, &archive, &work)?;
    install_tree(&extracted, &destination)?;
    let _ = fs::remove_dir_all(&work);

    println!("{}", style.ok_line(&format!("Downloaded {DEMO_TAG}")));
    print_location(&style, &destination);
    Ok(ExitCode::SUCCESS)
}

pub fn demo_archive_url() -> String {
    format!("https://github.com/{DEMO_OWNER}/{DEMO_REPO}/archive/refs/tags/{DEMO_TAG}.tar.gz")
}

pub fn demo_directory() -> PathBuf {
    demo_directory_from(
        env::var("RAINBOW_DEMO_DIR").ok(),
        env::var("HOME").ok(),
        env::consts::OS,
    )
}

fn demo_directory_from(override_dir: Option<String>, home: Option<String>, os: &str) -> PathBuf {
    if let Some(dir) = override_dir {
        let trimmed = dir.trim();
        if !trimmed.is_empty() {
            return PathBuf::from(trimmed);
        }
    }

    let home = home.unwrap_or_else(|| ".".to_string());
    if os == "macos" {
        PathBuf::from(home).join("Library/Application Support/rainbow/demo")
    } else {
        PathBuf::from(home).join(".rainbow/demo")
    }
}

fn parse_force(args: &[String]) -> Result<bool, String> {
    let mut force = false;
    for arg in args {
        match arg.as_str() {
            "--force" => force = true,
            other => {
                return Err(format!(
                    "Unexpected argument '{other}'.\n\n{}",
                    demo_usage()
                ));
            }
        }
    }
    Ok(force)
}

fn demo_is_installed(destination: &Path) -> bool {
    let tag_path = destination.join(TAG_FILE);
    let tag = fs::read_to_string(&tag_path)
        .ok()
        .map(|value| value.trim().to_string());
    tag.as_deref() == Some(DEMO_TAG) && demo_folders_present(destination)
}

fn demo_folders_present(root: &Path) -> bool {
    DEMO_FOLDERS.iter().all(|name| root.join(name).is_dir())
}

fn download_and_unpack(url: &str, archive: &Path, work: &Path) -> Result<PathBuf, String> {
    download(url, archive)?;
    let extract_dir = work.join("extract");
    fs::create_dir_all(&extract_dir)
        .map_err(|error| format!("Failed to create {}: {error}", extract_dir.display()))?;
    unpack_archive(archive, &extract_dir)?;
    find_demo_root(&extract_dir)
}

fn download(url: &str, archive: &Path) -> Result<(), String> {
    let agent = ureq::AgentBuilder::new()
        .timeout(Duration::from_secs(300))
        .build();
    let response = agent
        .get(url)
        .set(
            "User-Agent",
            &format!("rainbow-cli/{RAINBOW_PARSER_VERSION}"),
        )
        .call()
        .map_err(|error| format!("Demo download failed: {error}"))?;

    if !(200..300).contains(&response.status()) {
        return Err(format!(
            "Demo download failed with HTTP {}.",
            response.status()
        ));
    }

    let mut reader = response.into_reader();
    let mut file = fs::File::create(archive)
        .map_err(|error| format!("Failed to write {}: {error}", archive.display()))?;
    io::copy(&mut reader, &mut file)
        .map_err(|error| format!("Failed to save the demo: {error}"))?;
    Ok(())
}

fn unpack_archive(archive: &Path, extract_dir: &Path) -> Result<(), String> {
    let status = Command::new("tar")
        .arg("-xzf")
        .arg(archive)
        .arg("-C")
        .arg(extract_dir)
        .status()
        .map_err(|error| format!("`tar` is required to unpack the demo: {error}"))?;
    if status.success() {
        Ok(())
    } else {
        Err("tar failed to unpack the demo archive.".to_string())
    }
}

fn find_demo_root(extract_dir: &Path) -> Result<PathBuf, String> {
    if demo_folders_present(extract_dir) {
        return Ok(extract_dir.to_path_buf());
    }

    let entries = fs::read_dir(extract_dir)
        .map_err(|error| format!("Failed to read {}: {error}", extract_dir.display()))?;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() && demo_folders_present(&path) {
            return Ok(path);
        }
    }

    Err("Demo archive does not contain web, back, ios, and android.".to_string())
}

fn install_tree(source: &Path, destination: &Path) -> Result<(), String> {
    if !demo_folders_present(source) {
        return Err("Demo archive does not contain web, back, ios, and android.".to_string());
    }

    fs::write(source.join(TAG_FILE), format!("{DEMO_TAG}\n"))
        .map_err(|error| format!("Failed to record the demo tag: {error}"))?;

    let parent = destination.parent().ok_or_else(|| {
        format!(
            "Demo destination {} has no parent directory.",
            destination.display()
        )
    })?;
    fs::create_dir_all(parent)
        .map_err(|error| format!("Failed to create {}: {error}", parent.display()))?;

    if !destination.exists() {
        return fs::rename(source, destination).map_err(|error| {
            format!(
                "Failed to move the demo to {}: {error}",
                destination.display()
            )
        });
    }

    let backup = parent.join(".rainbow-demo-previous");
    if backup.exists() {
        fs::remove_dir_all(&backup)
            .map_err(|error| format!("Failed to clear {}: {error}", backup.display()))?;
    }
    fs::rename(destination, &backup)
        .map_err(|error| format!("Failed to move the current demo aside: {error}"))?;
    if let Err(error) = fs::rename(source, destination) {
        let _ = fs::rename(&backup, destination);
        return Err(format!(
            "Failed to move the demo to {}: {error}",
            destination.display()
        ));
    }
    let _ = fs::remove_dir_all(&backup);
    Ok(())
}

fn print_location(style: &Style, destination: &Path) {
    println!("  {}", style.path(&destination.display().to_string()));
    println!();
    println!("  {}  editor", style.bold("web"));
    println!("  {}  API", style.bold("back"));
    println!("  {}  iPhone app", style.bold("ios"));
    println!("  {}  Android app", style.bold("android"));
}

fn demo_usage() -> String {
    let style = Style::stdout();
    format!(
        "\
{}

{}
  rainbow demo [--force]

{}
  Downloads tag {DEMO_TAG} of {DEMO_OWNER}/{DEMO_REPO}.
  The archive is unpacked into the local cache. It is not part of `brew install rainbow`.

{}
  macOS   ~/Library/Application Support/rainbow/demo
  other   ~/.rainbow/demo

{}
  RAINBOW_DEMO_DIR   download into this directory instead

{}
  web      CMS editor
  back     CMS API
  ios      iOS app
  android  Android app",
        style.header("rainbow demo — download the NShiftUI demonstration"),
        style.dim("Usage"),
        style.dim("Source"),
        style.dim("Cache"),
        style.dim("Environment"),
        style.dim("Folders"),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn archive_url_pins_the_public_tag() {
        assert_eq!(
            demo_archive_url(),
            "https://github.com/NShiftUI-OSS/NShiftUI-Demo/archive/refs/tags/v1.0.0.tar.gz"
        );
    }

    #[test]
    fn directory_uses_the_override_then_the_platform_default() {
        assert_eq!(
            demo_directory_from(Some("  /tmp/nshift-demo  ".into()), None, "linux"),
            PathBuf::from("/tmp/nshift-demo")
        );
        assert_eq!(
            demo_directory_from(Some("   ".into()), Some("/Users/ada".into()), "macos"),
            PathBuf::from("/Users/ada/Library/Application Support/rainbow/demo")
        );
        assert_eq!(
            demo_directory_from(None, Some("/home/ada".into()), "linux"),
            PathBuf::from("/home/ada/.rainbow/demo")
        );
    }

    #[test]
    fn installs_an_archive_that_contains_the_four_folders() {
        let work = env::temp_dir().join(format!("rainbow-demo-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&work);
        let packed = work.join("packed");
        let root_name = format!("{DEMO_REPO}-{DEMO_TAG}");
        let root = packed.join(&root_name);
        for folder in DEMO_FOLDERS {
            fs::create_dir_all(root.join(folder)).unwrap();
            fs::write(root.join(folder).join("marker.txt"), folder).unwrap();
        }
        let archive = work.join("demo.tar.gz");
        let status = Command::new("tar")
            .arg("-czf")
            .arg(&archive)
            .arg("-C")
            .arg(&packed)
            .arg(&root_name)
            .status()
            .unwrap();
        assert!(status.success());

        let extracted = download_and_unpack_local(&archive, &work);
        let destination = work.join("installed");
        install_tree(&extracted, &destination).unwrap();

        assert!(demo_is_installed(&destination));
        assert_eq!(
            fs::read_to_string(destination.join("ios").join("marker.txt")).unwrap(),
            "ios"
        );
        let _ = fs::remove_dir_all(&work);
    }

    fn download_and_unpack_local(archive: &Path, work: &Path) -> PathBuf {
        let extract_dir = work.join("extract");
        fs::create_dir_all(&extract_dir).unwrap();
        unpack_archive(archive, &extract_dir).unwrap();
        find_demo_root(&extract_dir).unwrap()
    }
}
