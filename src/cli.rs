
use anyhow::{Result};
use rusqlite::Connection;
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
                commands::handle_help(&["list-columns".to_string()], conn)?;
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
                commands::handle_help(&["query-table".to_string()], conn)?;
            } else {
                // Pass the first argument as the table_name
                let table_name = &args[0];
                let position_str = args[1].to_string();
                let mut radius  = 0.0;
                let mut columns_arg: Option<String> = None;
                let mut products_only_flag = false;

                let mut current_arg_idx = 2; // Index for optional arguments

                // Try to parse radius
                if current_arg_idx < args.len() {
                    if let Ok(r_val) = args[current_arg_idx].parse::<f64>() {
                        radius = r_val;
                        current_arg_idx += 1;
                    }
                    // If not a float, assume it's not radius, and current_arg_idx remains for columns/products
                }

                // Try to parse columns specifier
                if current_arg_idx < args.len() && args[current_arg_idx].to_lowercase() != "products" {
                    columns_arg = Some(args[current_arg_idx].clone());
                    current_arg_idx += 1;
                }

                // Try to parse "products" flag
                if current_arg_idx < args.len() && args[current_arg_idx].to_lowercase() == "products" {
                    products_only_flag = true;
                }

                commands::query_table(table_name, &position_str, &radius, &columns_arg, &products_only_flag, conn)?;
            }
            Ok(CommandOutcome::Continue)
        }
        _ => {
            println!("Unknown command: {}", command_name);
            Ok(CommandOutcome::Continue)
        }
    }
}