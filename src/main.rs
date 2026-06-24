#![cfg_attr(coverage_nightly, feature(coverage_attribute))]

use std::env;
use std::fs::{self, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::process::ExitCode;

use clap::{CommandFactory, Parser, Subcommand};
use clap_complete::{Shell, generate};

#[derive(Parser)]
#[command(name = "jump", about = "Directory bookmark manager")]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,

    /// Bookmark name to jump to (when no subcommand given)
    #[arg(value_name = "NAME")]
    name: Option<String>,
}

#[derive(Subcommand)]
enum Commands {
    /// Add current directory as a bookmark
    Add {
        /// Name for the bookmark
        name: String,
    },
    /// List all bookmarks
    List,
    /// Remove a bookmark
    Rm {
        /// Name of the bookmark to remove
        name: String,
    },
    /// Generate shell completions
    Completions {
        /// Shell to generate completions for
        #[arg(value_enum)]
        shell: Shell,
    },
}

fn bookmarks_path() -> PathBuf {
    let home = env::var("HOME").expect("HOME not set");
    PathBuf::from(home).join(".config/jump/bookmarks")
}

fn read_bookmarks() -> Vec<(String, String)> {
    let path = bookmarks_path();
    if !path.exists() {
        return Vec::new();
    }
    let file = fs::File::open(&path).expect("Failed to open bookmarks");
    BufReader::new(file)
        .lines()
        .filter_map(|line| {
            let line = line.ok()?;
            let mut parts = line.splitn(2, '|');
            let path = parts.next()?.to_string();
            let name = parts.next()?.to_string();
            Some((path, name))
        })
        .collect()
}

fn write_bookmarks(bookmarks: &[(String, String)]) {
    let path = bookmarks_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("Failed to create config directory");
    }
    let mut file = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(&path)
        .expect("Failed to open bookmarks for writing");
    for (path, name) in bookmarks {
        writeln!(file, "{path}|{name}").expect("Failed to write bookmark");
    }
}

fn expand_home(path: &str) -> String {
    if path.starts_with("$HOME") {
        let home = env::var("HOME").expect("HOME not set");
        path.replacen("$HOME", &home, 1)
    } else {
        path.to_string()
    }
}

fn collapse_home(path: &str) -> String {
    let home = env::var("HOME").expect("HOME not set");
    if path.starts_with(&home) {
        path.replacen(&home, "$HOME", 1)
    } else {
        path.to_string()
    }
}

fn jump(name: &str) -> ExitCode {
    let bookmarks = read_bookmarks();
    for (path, bookmark_name) in &bookmarks {
        if bookmark_name == name {
            println!("{}", expand_home(path));
            return ExitCode::SUCCESS;
        }
    }
    eprintln!("Bookmark '{name}' not found");
    ExitCode::FAILURE
}

fn bookmark(name: &str) -> ExitCode {
    let cwd = env::current_dir().expect("Failed to get current directory");
    let cwd_str = collapse_home(cwd.to_str().expect("Invalid path"));

    let mut bookmarks = read_bookmarks();
    if bookmarks.iter().any(|(_, n)| n == name) {
        eprintln!("Bookmark '{name}' already exists");
        return ExitCode::FAILURE;
    }

    bookmarks.push((cwd_str, name.to_string()));
    write_bookmarks(&bookmarks);
    eprintln!("Bookmark '{name}' saved");
    ExitCode::SUCCESS
}

fn showmarks() -> ExitCode {
    let bookmarks = read_bookmarks();
    if bookmarks.is_empty() {
        eprintln!("No bookmarks");
        return ExitCode::SUCCESS;
    }
    let max_name = bookmarks.iter().map(|(_, n)| n.len()).max().unwrap_or(0);
    for (path, name) in &bookmarks {
        println!("{:width$}  {}", name, expand_home(path), width = max_name);
    }
    ExitCode::SUCCESS
}

fn deletemark(name: &str) -> ExitCode {
    let bookmarks = read_bookmarks();
    let new_bookmarks: Vec<_> = bookmarks.into_iter().filter(|(_, n)| n != name).collect();

    if new_bookmarks.len() == read_bookmarks().len() {
        eprintln!("Bookmark '{name}' not found");
        return ExitCode::FAILURE;
    }

    write_bookmarks(&new_bookmarks);
    eprintln!("Bookmark '{name}' deleted");
    ExitCode::SUCCESS
}

#[cfg_attr(coverage_nightly, coverage(off))]
fn print_completions(shell: Shell) {
    let mut cmd = Cli::command();
    generate(shell, &mut cmd, "jump", &mut std::io::stdout());
}

#[cfg_attr(coverage_nightly, coverage(off))]
fn main() -> ExitCode {
    let cli = Cli::parse();

    match cli.command {
        Some(Commands::Add { name }) => bookmark(&name),
        Some(Commands::List) => showmarks(),
        Some(Commands::Rm { name }) => deletemark(&name),
        Some(Commands::Completions { shell }) => {
            print_completions(shell);
            ExitCode::SUCCESS
        }
        None => {
            if let Some(name) = cli.name {
                jump(&name)
            } else {
                Cli::command().print_help().ok();
                ExitCode::FAILURE
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    fn with_home<T>(run: impl FnOnce(&tempfile::TempDir) -> T) -> T {
        let _guard = ENV_LOCK.lock().unwrap();
        let tmp = tempfile::TempDir::new().unwrap();
        let old_home = env::var_os("HOME");
        // SAFETY: tests that mutate HOME hold ENV_LOCK for the full mutation
        // window, so project tests in this crate do not race on process env.
        unsafe {
            env::set_var("HOME", tmp.path());
        }
        let result = run(&tmp);
        unsafe {
            match old_home {
                Some(value) => env::set_var("HOME", value),
                None => env::remove_var("HOME"),
            }
        }
        result
    }

    #[test]
    fn test_expand_home() {
        let home = env::var("HOME").unwrap();
        assert_eq!(expand_home("$HOME/foo"), format!("{home}/foo"));
        assert_eq!(expand_home("/absolute/path"), "/absolute/path");
        assert_eq!(expand_home("relative"), "relative");
    }

    #[test]
    fn test_collapse_home() {
        let home = env::var("HOME").unwrap();
        assert_eq!(collapse_home(&format!("{home}/foo")), "$HOME/foo");
        assert_eq!(collapse_home("/other/path"), "/other/path");
    }

    #[test]
    fn test_parse_bookmark_line() {
        let line = "$HOME/projects|proj";
        let mut parts = line.splitn(2, '|');
        let path = parts.next().unwrap();
        let name = parts.next().unwrap();
        assert_eq!(path, "$HOME/projects");
        assert_eq!(name, "proj");
    }

    #[test]
    fn test_parse_bookmark_with_pipe_in_path() {
        let line = "$HOME/path|with|pipe|name";
        let mut parts = line.splitn(2, '|');
        let path = parts.next().unwrap();
        let name = parts.next().unwrap();
        assert_eq!(path, "$HOME/path");
        assert_eq!(name, "with|pipe|name");
    }

    #[test]
    fn bookmarks_path_uses_home_config_directory() {
        with_home(|home| {
            assert_eq!(bookmarks_path(), home.path().join(".config/jump/bookmarks"));
        });
    }

    #[test]
    fn read_bookmarks_returns_empty_when_file_missing() {
        with_home(|_| {
            assert!(read_bookmarks().is_empty());
        });
    }

    #[test]
    fn write_and_read_bookmarks_round_trip_entries() {
        with_home(|_| {
            write_bookmarks(&[
                ("$HOME/src".to_string(), "src".to_string()),
                ("/tmp/work".to_string(), "work".to_string()),
            ]);

            assert_eq!(
                read_bookmarks(),
                vec![
                    ("$HOME/src".to_string(), "src".to_string()),
                    ("/tmp/work".to_string(), "work".to_string())
                ]
            );
        });
    }

    #[test]
    fn bookmark_adds_current_directory_and_rejects_duplicates() {
        with_home(|home| {
            let project = home.path().join("project");
            fs::create_dir_all(&project).unwrap();
            let old_cwd = env::current_dir().unwrap();
            env::set_current_dir(&project).unwrap();

            assert_eq!(bookmark("proj"), ExitCode::SUCCESS);
            assert_eq!(bookmark("proj"), ExitCode::FAILURE);

            env::set_current_dir(old_cwd).unwrap();
            assert_eq!(
                read_bookmarks(),
                vec![("$HOME/project".to_string(), "proj".to_string())]
            );
        });
    }

    #[test]
    fn jump_returns_success_for_existing_bookmark() {
        with_home(|_| {
            write_bookmarks(&[("$HOME/src".to_string(), "src".to_string())]);

            assert_eq!(jump("src"), ExitCode::SUCCESS);
            assert_eq!(jump("missing"), ExitCode::FAILURE);
        });
    }

    #[test]
    fn showmarks_succeeds_for_empty_and_non_empty_lists() {
        with_home(|_| {
            assert_eq!(showmarks(), ExitCode::SUCCESS);
            write_bookmarks(&[("$HOME/src".to_string(), "src".to_string())]);
            assert_eq!(showmarks(), ExitCode::SUCCESS);
        });
    }

    #[test]
    fn deletemark_removes_existing_bookmark_and_reports_missing() {
        with_home(|_| {
            write_bookmarks(&[
                ("$HOME/src".to_string(), "src".to_string()),
                ("/tmp/work".to_string(), "work".to_string()),
            ]);

            assert_eq!(deletemark("src"), ExitCode::SUCCESS);
            assert_eq!(
                read_bookmarks(),
                vec![("/tmp/work".to_string(), "work".to_string())]
            );
            assert_eq!(deletemark("missing"), ExitCode::FAILURE);
        });
    }
}
