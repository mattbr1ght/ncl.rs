use console::style;
use grep_regex::RegexMatcher;
use grep_searcher::{SearcherBuilder, sinks::UTF8};
use regex::Regex;
use std::collections::HashMap;
use std::path::Path;
use terminal_size::{Width, terminal_size};
use walkdir::{DirEntry, WalkDir};

const IGNORED_DIRS: &[&str] = &[
    "node_modules",
    ".git",
    "target",
    ".idea",
    ".vscode",
    ".next",
    ".data",
    "vendor",
];

const DEFAULT_QUERY: &str = "TODO";

fn is_ignored(entry: &DirEntry) -> bool {
    entry.file_type().is_dir()
        && IGNORED_DIRS.contains(&entry.file_name().to_string_lossy().as_ref())
}

fn get_terminal_width() -> usize {
    terminal_size()
        .map(|(Width(w), _)| w as usize)
        .unwrap_or(80)
}

fn truncate_line(line: &str, max_width: usize) -> String {
    line.chars()
        .take(max_width)
        .chain(if line.chars().count() > max_width {
            Some('…')
        } else {
            None
        })
        .collect()
}

fn search_file(
    path: &Path,
    matcher: &RegexMatcher,
    searcher: &mut grep_searcher::Searcher,
    max_width: usize,
) -> std::io::Result<Vec<(u64, String)>> {
    let mut matches = Vec::new();

    searcher.search_path(
        matcher,
        path,
        UTF8(|line_num, line| {
            let truncated = truncate_line(line, max_width);
            matches.push((line_num, truncated));
            Ok(true)
        }),
    )?;

    Ok(matches)
}

fn highlight_matches(text: &str, pattern: &Regex) -> String {
    pattern
        .replace_all(text, |caps: &regex::Captures| {
            style(&caps[0]).red().bold().to_string()
        })
        .to_string()
}

fn collect_matches(
    root: &Path,
    query: &str,
) -> std::io::Result<HashMap<String, Vec<(u64, String)>>> {
    let matcher = RegexMatcher::new(query).map_err(|e| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!("Invalid regex pattern: {}", e),
        )
    })?;

    let mut searcher = SearcherBuilder::new()
        .binary_detection(grep_searcher::BinaryDetection::quit(b'\x00'))
        .build();

    let max_width = get_terminal_width().saturating_sub(10);
    let mut all_matches = HashMap::new();

    for entry in WalkDir::new(root)
        .into_iter()
        .filter_entry(|e| !is_ignored(e))
        .filter_map(Result::ok)
        .filter(|e| e.file_type().is_file())
    {
        if let Ok(file_matches) = search_file(entry.path(), &matcher, &mut searcher, max_width) {
            if !file_matches.is_empty() {
                all_matches.insert(entry.path().to_string_lossy().to_string(), file_matches);
            }
        }
    }

    Ok(all_matches)
}

fn display_matches(matches: HashMap<String, Vec<(u64, String)>>, query: &str) {
    let pattern = Regex::new(query).unwrap();

    for (path, lines) in matches {
        println!("{}", style(path).cyan());
        for (line_num, text) in lines {
            let highlighted = highlight_matches(&text, &pattern);
            println!(
                "{}:{} {}",
                style(line_num).green(),
                style(":").dim(),
                highlighted
            );
        }
        println!();
    }
}

use anyhow::Result;

pub fn run() -> Result<()> {
    let root = Path::new(".");
    let matches = collect_matches(root, DEFAULT_QUERY)?;
    display_matches(matches, DEFAULT_QUERY);
    Ok(())
}
