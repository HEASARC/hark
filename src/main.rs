use rustyline::error::ReadlineError;
use rustyline::DefaultEditor;
use std::env;
use std::error::Error;
use std::io::Write; 
use rusqlite::{Connection};
use tempfile::NamedTempFile;
use colored::*;
use flate2::read::GzDecoder;
use std::io::Read;
use tokio;


mod cli;
mod commands;

/// A static list of officially supported table names.
pub const SUPPORTED_TABLES: &[&str] = &[
    "nicermastr", "xmmmaster", "swiftmastr", "chanmaster",
    "numaster", "ixmaster", "xrismmastr"
];


/// Embed the SQLite database bytes at compile time.
/// Assumes 'hea.db.gz' is in the project root (alongside Cargo.toml).
const HEA_DB_GZ_BYTES: &[u8] = include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/hea.db.gz"));

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    // Check for command-line arguments like -v or --version
    let args: Vec<String> = env::args().collect();
    if args.len() > 1 {
        match args[1].as_str() {
            "-v" | "--version" => {
                println!("hark version {}", env!("CARGO_PKG_VERSION"));
                return Ok(());
            }
            _ => {} // Handle other potential command-line args or ignore
        }
    }
    // 1. Decompress the embedded gzipped database
    let mut decoder = GzDecoder::new(HEA_DB_GZ_BYTES);
    let mut decompressed_db_bytes = Vec::new();
    decoder.read_to_end(&mut decompressed_db_bytes)?;

    // 2. Write the decompressed database to a temporary file
    let mut temp_db_file = NamedTempFile::new()?;
    temp_db_file.write_all(&decompressed_db_bytes)?;
    let temp_db_path = temp_db_file.path(); // Path to the temporary DB file

    // 3. Open a connection to the temporary database
    let conn = Connection::open(temp_db_path)?;
    println!("{} Archive Interactive Browser", "HEASARC".bold().cyan());
    println!("Type {} for help, or {} to quit.", "?/help".cyan(), "exit/quit".cyan());
    println!("{}", "------------------------------------------------------".dimmed());
    println!();

    // Initialize rustyline editor
    let mut rl = DefaultEditor::new()?;

    // Load history if it exists
    let history_file = "history.txt";
    if rl.load_history(history_file).is_err() {
        // No history file found or error loading, not critical, just print a small message
        println!("No previous history.");
    }


    loop {
        let readline = rl.readline("hark> ");
        match readline {
            Ok(line) => {
                let trimmed_line = line.trim();
                if trimmed_line.is_empty() {
                    continue; // Skip empty lines
                }

                // Add command to history
                rl.add_history_entry(trimmed_line)?;

                // Parse and dispatch the command
                match cli::parse_and_dispatch_command(trimmed_line, &conn).await {
                    Ok(cli::CommandOutcome::Exit) => {
                        println!("Exiting hark.");
                        break;
                    }
                    Ok(cli::CommandOutcome::Continue) => {
                        // Continue to the next command
                        // The debug print below can be removed if no longer needed
                        // println!("Debug: Spectra count after command: {:?}", app_state.spectra.len());
                    }
                    Err(e) => {
                        eprintln!("Error: {}", e);
                    }
                }
            }
            Err(ReadlineError::Interrupted) => {
                println!("Interrupted (Ctrl-C). Type 'exit' or 'quit' to exit.");
            }
            Err(ReadlineError::Eof) => {
                println!("Exiting rspec (Ctrl-D).");
                break;
            }
            Err(err) => {
                eprintln!("Readline error: {:?}", err);
                break;
            }
        }
    }

    // Save history before exiting
    rl.save_history(history_file)?;

    Ok(())
}