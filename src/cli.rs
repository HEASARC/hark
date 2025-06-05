
use anyhow::{Result};
use rusqlite::Connection;
use colored::*;
use crate::commands;

/// Outcome of a command execution
pub enum CommandOutcome {
    Continue,
    Exit,
}

/// Parses the input line and dispatches the command.
pub fn parse_and_dispatch_command(line: &str, conn: &Connection) -> Result<CommandOutcome> {
    let parts: Vec<&str> = line.split_whitespace().collect();
    if parts.is_empty() {
        return Ok(CommandOutcome::Continue);
    }

    let command_name = parts[0].to_lowercase();
    let args = parts[1..].iter().map(|s| s.to_string()).collect::<Vec<String>>();

    // Basic command parsing logic (to be expanded significantly)
    match command_name.as_str() {
        "exit" | "quit" | "q" => Ok(CommandOutcome::Exit),
        "help" | "h" | "?" => {
            commands::handle_help(&args, conn)?;
            Ok(CommandOutcome::Continue)
        }
        "list-tables" => {
            commands::list_tables(&args, conn)?;
            Ok(CommandOutcome::Continue)
        }
        "list-columns" => {
            if args.is_empty() {
                println!();
                println!(
                    "{}: {} {} [{}]",
                    "Usage".blue().underline(), "list-columns".bold(), "table_name".cyan(), "all".yellow().dimmed());
                println!();
                println!("{:>12}: The name of the table for which to list columns. Use {} to see a list of supported tables",
                        "table_name".cyan(), "list-tables".bold());
                println!("{:>12}: List all columns. The default is to list a subset of useful columns.",
                        "all".yellow().dimmed());
                println!();
            } else {
                // Pass the first argument as the table_name
                let mut all = false;
                if args.len() > 1 && args[1] == "all" {
                    all = true;
                }

                commands::list_columns(&args[0], &all, conn)?;
            }
            Ok(CommandOutcome::Continue)
        }
        "query-table" => {
            if args.is_empty() || args.len() < 2 {
                println!();
                println!(
                    "{}: {} {} [{}]",
                    "Usage".blue().underline(), "query-table".bold(), "table_name position".cyan(), "radius".yellow().dimmed());
                println!();
                println!("{:>12}: The name of the table to be queried. Use {} to see a list of supported tables",
                        "table_name".cyan(), "list-tables".bold());
                println!("{:>12}: Search RA and DEC as: ra,dec", "position".cyan());
                println!("{:>12}: Search radius. If not given, the default for the table is used.",
                        "radius".yellow().dimmed());
                println!();
            } else {
                // Pass the first argument as the table_name
                let position = args[1].to_string();
                let mut radius = 0.0;
                if args.len() > 2 {
                    radius = args[2].parse::<f64>().unwrap_or(0.0);
                }
                let mut prod_only = false;
                if args.len() > 3 && args[3] == "products" {
                    prod_only = true;
                }

                commands::query_table(&args[0], &position, &radius, &prod_only, conn)?;
            }
            Ok(CommandOutcome::Continue)
        }
        _ => {
            println!("Unknown command: {}", command_name);
            Ok(CommandOutcome::Continue)
        }
    }
}