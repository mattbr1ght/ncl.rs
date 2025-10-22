use grep_regex::RegexMatcher;
use grep_searcher::{sinks::UTF8, SearcherBuilder};
use regex::Regex;
use walkdir::{DirEntry, WalkDir};
use std::collections::HashMap;
use std::path::Path;
use terminal_size::{terminal_size, Width};
use console::style;

fn is_ignored(entry: &DirEntry) -> bool {
    let name = entry.file_name().to_string_lossy();
    entry.file_type().is_dir() && matches!(
        name.as_ref(),
        "node_modules" | ".git" | "target" | ".idea" | ".vscode" | ".next" | ".data"
    )
}

fn get_terminal_width() -> usize {
    if let Some((Width(w), _)) = terminal_size() {
        w as usize
    } else {
        80
    }
}

fn truncate_line_to_terminal(line: &str) -> String {
    let max_len = get_terminal_width().saturating_sub(10);
    let mut result = String::with_capacity(max_len + 3);
    for (i, c) in line.chars().enumerate() {
        if i >= max_len {
            result.push('…');
            result.push('\n');
            break;
        }
        result.push(c);
    }
    result
}

fn search_dir(root: &Path, query: &str) -> std::io::Result<()> {
    let matcher = RegexMatcher::new(query).expect("Should be a valid regex pattern");
    let mut searcher = SearcherBuilder::new()
        .binary_detection(grep_searcher::BinaryDetection::quit(b'\x00'))
        .build();

    let mut matches: HashMap<String, Vec<(u64, String)>> = HashMap::new();

    for entry in WalkDir::new(root)
        .into_iter()
        .filter_entry(|e| !is_ignored(e))
        .filter_map(Result::ok)
        .filter(|e| e.file_type().is_file())
    {
        let path = entry.path().to_string_lossy().to_string();
        let mut file_matches = Vec::new();

        let _ = searcher.search_path(
            &matcher,
            entry.path(),
            UTF8(|line_num, line| {
                file_matches.push((line_num, truncate_line_to_terminal(line)));
                Ok(true)
            }),
        );

        if !file_matches.is_empty() {
            matches.insert(path, file_matches);
        }
    }

    for (path, lines) in matches {
        println!("{}", style(path).cyan());
        for (line_num, text) in lines {
            let ln = style(line_num).green();
            let re = Regex::new(query).unwrap();
            let highlighted = re.replace_all(&text, |caps: &regex::Captures| {
                style(&caps[0]).red().bold().to_string()
            });
            print!("{}:{} {}", ln, style(":").dim(), highlighted);
        }
        println!();
    }

    Ok(())
}

pub fn run() -> std::io::Result<()> {
    let root = Path::new(".");
    // let query = "(?i)TODO";
    let query = "TODO";

    search_dir(root, query)
}
