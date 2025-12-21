//! Legacy permission prompts (replaced by policy.rs)
//! Kept for reference but no longer used.

#![allow(dead_code)]

use std::io::{self, Write};

pub fn prompt_write(path: &str, print_mode: bool, auto_yes: bool) -> bool {
    prompt("Write", path, "create/overwrite file", print_mode, auto_yes)
}

pub fn prompt_edit(path: &str, summary: &str, print_mode: bool, auto_yes: bool) -> bool {
    if print_mode && !auto_yes {
        eprintln!("Permission denied: Edit({}) - use --yes in -p mode", path);
        return false;
    }
    if auto_yes {
        return true;
    }
    println!("Permission required: Edit(\"{}\")", path);
    println!("Summary: {}", summary);
    print!("Allow? [y/N]: ");
    io::stdout().flush().ok();
    read_yes()
}

fn prompt(tool: &str, path: &str, action: &str, print_mode: bool, auto_yes: bool) -> bool {
    if print_mode && !auto_yes {
        eprintln!(
            "Permission denied: {}({}) - use --yes in -p mode",
            tool, path
        );
        return false;
    }
    if auto_yes {
        return true;
    }
    println!("Permission required: {}(\"{}\")", tool, path);
    println!("Action: {}", action);
    print!("Allow? [y/N]: ");
    io::stdout().flush().ok();
    read_yes()
}

fn read_yes() -> bool {
    let mut input = String::new();
    if io::stdin().read_line(&mut input).is_ok() {
        let input = input.trim().to_lowercase();
        input == "y" || input == "yes"
    } else {
        false
    }
}
