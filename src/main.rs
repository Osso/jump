use std::env;
use std::fs::{self, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::process::ExitCode;

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
    let new_bookmarks: Vec<_> = bookmarks
        .into_iter()
        .filter(|(_, n)| n != name)
        .collect();

    if new_bookmarks.len() == read_bookmarks().len() {
        eprintln!("Bookmark '{name}' not found");
        return ExitCode::FAILURE;
    }

    write_bookmarks(&new_bookmarks);
    eprintln!("Bookmark '{name}' deleted");
    ExitCode::SUCCESS
}

fn usage() {
    eprintln!("Usage:");
    eprintln!("  jump <name>        Jump to bookmark");
    eprintln!("  jump add <name>    Add current directory as bookmark");
    eprintln!("  jump list          List all bookmarks");
    eprintln!("  jump rm <name>     Remove bookmark");
}

fn main() -> ExitCode {
    let args: Vec<String> = env::args().collect();

    match args.get(1).map(|s| s.as_str()) {
        Some("add") => match args.get(2) {
            Some(name) => bookmark(name),
            None => {
                eprintln!("Usage: jump add <name>");
                ExitCode::FAILURE
            }
        },
        Some("list") => showmarks(),
        Some("rm") => match args.get(2) {
            Some(name) => deletemark(name),
            None => {
                eprintln!("Usage: jump rm <name>");
                ExitCode::FAILURE
            }
        },
        Some("--help" | "-h") => {
            usage();
            ExitCode::SUCCESS
        }
        Some(name) => jump(name),
        None => {
            usage();
            ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
