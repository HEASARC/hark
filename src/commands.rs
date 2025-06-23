// Copyright 2025, University of Maryland, All Rights Reserved

use anyhow::{Context, Result};
use rusqlite::{params, Connection, Row};
use std::collections::HashSet;
use colored::*;
use chrono::{NaiveDate, Datelike};
use std::error::Error;
use std::path::{Path, PathBuf};

use aws_config;
use aws_sdk_s3::{Client as S3Client};
use tokio::fs::{self, File};
use tokio::io::AsyncWriteExt;

use self_update::{self, cargo_crate_version};
 
use crate::SUPPORTED_TABLES;

const INTERNAL_COLS_ARRAY: [&str; 3] = ["__x_ra_dec", "__y_ra_dec", "__z_ra_dec"];

pub fn handle_help(args: &[String], _conn: &Connection) -> Result<()> {
    if args.is_empty() {
        println!();
        println!("{}: HEASARC archive offline explorer.", "hark".bold().yellow());
        println!();
        println!("{}", "Commands".cyan().underline());
        println!("{:>16}: List supported tables", "list-tables".cyan());
        println!("{:>16}: List columns of a table", "list-columns".cyan());
        println!("{:>16}: Query a specific table", "query-table".cyan());
        println!("{:>16}: Download data from AWS", "aws-download".cyan());
        println!("{:>17}", "-----------".cyan().dimmed());
        println!("{:>16}: Show help message (also: h, ?).\n{:>18}Use {} for command help",
                        "help".cyan(), "", "help command-name".cyan());
        println!("{:>16}: Update hark to the latest version!", "self-update".cyan());
        println!("{:>16}: About hark!", "about".cyan());
        println!("{:>16}: Exit (also: quit, q)", "exit".cyan());
        println!();
    } else if args[0] == "self-update" {
        println!("{}", "self-update:".green().bold());
        println!("{:>17}", "-------------------".cyan().dimmed());
        println!("Update hark to get the most recent feasures and table updates.");

    } else if args[0] == "list-tables" {
        println!("{}", "list-tables:".green().bold());
        println!("{:>17}", "-------------------".cyan().dimmed());
        println!("Call {} to see a list of the available table.", "list-tables".cyan());
    } else if args[0] == "list-columns" {

        println!();
        println!("List the columns of the given table.");
        println!(
            "{}: {} {} [{}]",
            "Usage".yellow().underline(), "list-columns".bold(), "table_name".cyan(), "all".yellow().dimmed());
        println!();
        println!("{:>12}: The name of the table for which to list columns. Use {} to see a list of supported tables",
                "table_name".cyan(), "list-tables".bold());
        println!("{:>12}: List all columns. The default is to list a subset of useful columns.",
                "all".yellow().dimmed());
        println!();

        println!("{}", "Examples:".yellow().underline());
        println!("  - List the default columns for the NICER master catalog:\n     {}\n",
                "list-columns nicermastr".cyan());
        println!("  - List all columns for the SWIFT master catalog:\n    {}\n",
                "list-columns swiftmastr all".cyan());

    } else if args[0] == "query-table" {

        println!();
        println!("Query a given table around an RA,DEC position and get related data products.");
        println!(
            "{}: {} {} [{}] [{}] [{}]",
            "Usage".yellow().underline(), "query-table".bold(), "table_name position".cyan(), 
            "radius".yellow().dimmed(),
            "columns".yellow().dimmed(),
            "products".yellow().dimmed(),
        );
        println!();
        println!("{:>12}: The name of the table to be queried. Use {} to see a list of supported tables",
                "table_name".cyan(), "list-tables".bold());
        println!("{:>12}: Search RA and DEC as: ra,dec in {}", "position".cyan(), "degrees".yellow());
        println!("{:>12}: Search radius in {}. If not given or 0, the default for the table is used.",
                "radius".yellow().dimmed(), "arcmin".yellow());
        println!("{:>12}: Columns to be printed. Use */all for all columns. If not given or \"\", 
        the default is used. Use list-columns to see available columns",
                "columns".yellow().dimmed());
        println!("{:>12}: Print product links. If given, add product links to the table.
            e.g. {}.
            Pass 0 for radius to use the default",
                "products".yellow().dimmed(), "query-table xrismmastr 16,-72 products".cyan());
        println!();
        
        println!("{}", "Examples:".yellow().underline());
        println!("  - Query nicermastr around position 182.6,39.4 using the default radius:\n    {}\n", 
                "query-table nicermastr 182.6,39.4".cyan());
        println!("  - Query numaster around position 182.6,39.4 and radius 40 arcmin:\n    {}\n", 
                "query-table numaster 182.6,39.4 40".cyan());
        println!("  - Query numaster around position 182.6,39.4 for specific columns:\n    {}\n", 
                "query-table numaster 182.6,39.4 ra,dec,name".cyan());
        println!("  - Query numaster around position 182.6,39.4 for specific columns and add product links:\n    {}\n", 
                "query-table numaster 182.6,39.4 ra,dec,name products".cyan());
        println!("  - Query xmmmaster around position 182.6,39.4 for default columns and add product links:\n     {}\n", 
                "query-table xmmmaster 182.6,39.4 products".cyan());
    
    } else if args[0] == "aws-download" {

        println!();
        println!("Download data from the AWS cloud.");
        println!(
            "{}: {} {}",
            "Usage".yellow().underline(), "aws-download".bold(), "s3_uri".cyan());
        println!();
        println!("{:>12}: The s3 URI return in query-table.",
                "s3_uri".cyan());
        println!();

        println!("{}", "Examples:".yellow().underline());
        println!("  - Download SWIFT obsid 000037258040:\n     {}\n",
                "aws-download s3://nasa-heasarc/swift/data/obs/2015_12/00037258040".cyan());

    } else {
        println!("No Help for command '{}'", args[0]);
    }
    Ok(())
}

pub fn handle_about() -> Result<()> {
    println!();
    println!("{}: HEASARC archive offline explorer.", "hark".bold().yellow());
    println!();
    println!("{}", "Developed by Abdu Zoghbi for the HEASARC".dimmed());
    println!("{}", "Copyright (c) 2025 University of Maryland. All rights reserved.".dimmed());
    println!("{}", "See LICENSE file at https://github.com/HEASARC/hark".dimmed());
    println!("{}", "The material is based upon work supported by NASA under award number 80GSFC24M0006".dimmed());
    println!();
    Ok(())
}

/// Handles the self-update command, checking for and applying new releases.
pub async fn handle_self_update() -> Result<()> {
    println!("{}", "Checking for updates...".yellow());
    let current_version = cargo_crate_version!();

    let status = tokio::task::spawn_blocking(|| {
        self_update::backends::github::Update::configure()
            .repo_owner("heasarc")
            .repo_name("hark")   
            .bin_name("hark")      
            .show_download_progress(true)
            .target(&get_target())
            .current_version(current_version) 
            .build().context("Failed to build self-update configuration")?
            .update().context("Self-update operation failed")
    })
    .await
    .context("Failed to join blocking task for self-update")??; // Two '?' because of nested Result

    println!();

    match status {
        self_update::Status::UpToDate(current_version) => {
            println!("{} {}!", "hark is already up to date, verion: ".green(), current_version.green().bold())
        },
        self_update::Status::Updated(v) => {
            println!("{} {}!\n {}", 
            "Successfully updated hark to version:".green(), v.green().bold(),
            "you may need to restart hark to pick up the updates".green());
        }
    }
    Ok(())
}

pub fn list_tables(_args: &[String], conn: &Connection) -> Result<()> {
    println!();
    println!("{}", "List of supported tables".bold().underline());
    let mut stmt = conn.prepare(
        r#"
        SELECT DISTINCT name, value as description FROM metainfo
        WHERE type='table' AND relation='description'
        AND name LIKE '%mast%' AND NAME NOT LIKE 'master_table%'
        "#
    )?;
    let mission_iter = stmt.query_map([], |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)))?;
    for mission_result in mission_iter {
        match mission_result {
            Ok((name, description)) => {
                if SUPPORTED_TABLES.contains(&name.as_str()) {
                    println!(
                        "{:>14}: {}",
                        name.to_lowercase().cyan(), // Color the table name
                        description
                    );
                }
            }
            Err(e) => eprintln!("Error fetching mission: {}", e),
        }
    }
    println!();
    Ok(())
}

pub fn list_columns(table_name: &str, all: &bool, conn: &Connection) -> Result<()> {
    // Check if the provided table_name is in the list of supported tables
    if !SUPPORTED_TABLES.contains(&table_name) {
        eprintln!("Error: Table '{}' is not a supported table.", table_name.red().bold());
        eprintln!("Supported tables are: {}", SUPPORTED_TABLES.join(", ").cyan());
        return Ok(()); // Exit the function early
    }

    println!("Columns for table {}:", table_name.yellow().bold());
    let mut having_clause = ""; // Initialize as an empty string
    if *all {
        println!("{}", "(Showing all columns)".italic());
    } else {
        println!("{}", "(Showing columns with a defined order)".italic());
        // This clause will filter out groups where the calculated order is NULL
        having_clause = "HAVING MAX(CASE WHEN relation = 'order' THEN CAST(value AS INTEGER) END) IS NOT NULL";
    }

    // Dynamically construct the query string to include the conditional HAVING clause
    let query_string = format!(r#"
        SELECT
            substr(name, instr(name, '.') + 1) AS column_name,
            MAX(CASE WHEN relation = 'description' THEN value END) AS description,
            MAX(CASE WHEN relation = 'unit' THEN value END) AS unit
        FROM metainfo
        WHERE type='parameter' AND name LIKE ?1 || '.%'
        GROUP BY substr(name, instr(name, '.') + 1)
        {} 
        ORDER BY
            CASE
                WHEN MAX(CASE WHEN relation = 'order' THEN CAST(value AS INTEGER) END) IS NULL THEN 1
                ELSE 0
            END,
            MAX(CASE WHEN relation = 'order' THEN CAST(value AS INTEGER) END);
    "#, having_clause); 

    let mut stmt = conn.prepare(&query_string)?;
    let columns_iter = stmt.query_map(
        params![table_name],
        |row| Ok((
            row.get::<_, String>(0)?, // column_name
            row.get::<_, Option<String>>(1)?, // description (can be NULL)
            row.get::<_, Option<String>>(2)?  // unit (can be NULL)
    )))?;
    println!();
    for column in columns_iter {
        match column {
            Ok((column_name_val, description_opt, unit_opt)) => {
                let description_str = description_opt.as_deref().unwrap_or("");
                let unit_str = unit_opt.as_deref().filter(|s| !s.is_empty()).map_or(String::new(), |u| format!(" ({})", u));
                
                println!("{:>20}| {}{}",
                    column_name_val.to_lowercase().green(), // Color the column name
                    description_str.blue(), // Color the description
                    unit_str
                );
            }
            Err(e) => eprintln!("Error fetching mission: {}", e),
        }
    }
    println!();
    Ok(())
}


pub async fn aws_download(s3_uri: &str, _conn: &Connection) -> Result<()> {
    println!("\nAttempting to download from: {}", s3_uri.cyan());

    if !s3_uri.starts_with("s3://") {
        eprintln!("{}", "Error: S3 URI must start with s3://".red());
        return Ok(());
    }

    let s3_path = s3_uri.strip_prefix("s3://").unwrap(); // Safe due to check above
    let (bucket, prefix) = match s3_path.split_once('/') {
        Some((b, p)) => (b.to_string(), p.to_string()),
        None => {
            // This means URI was like "s3://bucketname" with no key part
            (s3_path.to_string(), "".to_string())
        }
    };

    if bucket.is_empty() {
        eprintln!("{}", "Error: Bucket name cannot be empty.".red());
        return Ok(());
    }
    if bucket != "nasa-heasarc" {
        eprintln!("{}", "Error: Bucket name is not nasa-heasarc.".red());
        return Ok(());
    }

    // Determine local target directory: named after the last component of the S3 prefix,
    // or the bucket name if the prefix is empty.
    let local_target_dir_name = if prefix.is_empty() {
        bucket.clone()
    } else {
        Path::new(&prefix)
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .filter(|name| !name.is_empty() && name != "/") // Ensure it's a valid dir name
            .unwrap_or_else(|| prefix.replace("/", "_")) // Fallback for complex prefixes
    };
    
    let local_base_path = PathBuf::from(&local_target_dir_name);

    if !local_base_path.exists() {
        println!("Creating local directory: {}", local_base_path.display());
        fs::create_dir_all(&local_base_path).await
            .with_context(|| format!("Failed to create directory {}", local_base_path.display()))?;
    } else if !local_base_path.is_dir() {
        eprintln!("Error: A file exists at the target path '{}', cannot create directory.", local_base_path.display());
        return Ok(());
    }

    println!("Target S3 Bucket: {}", bucket.green());
    println!("Target S3 Prefix: {}", if prefix.is_empty() { " (root)".dimmed().to_string() } else { prefix.green().to_string() });
    println!("Local download directory: {}", local_base_path.display());


    let config = aws_config::defaults(aws_config::BehaviorVersion::latest())
        .no_credentials()
        .region("us-east-1")
        .load()
        .await;
    let client = S3Client::new(&config);

    let mut objects_stream = client
        .list_objects_v2()
        .bucket(bucket.clone())
        .prefix(prefix.clone()) // list_objects_v2 handles empty prefix correctly
        .into_paginator()
        .send();

    let mut downloaded_count = 0;
    let mut total_size_bytes: u64 = 0;

    while let Some(result) = objects_stream.next().await {
        match result {
            Ok(output) => {
                for object in output.contents() {
                    let object_key = object.key().unwrap_or_default();
                    if object_key.ends_with('/') { // Skip S3 "directory" markers
                        continue;
                    }

                    let object_key_path = Path::new(object_key);
                    //println!("++ {:?}", object_key_path);
                    let final_local_path: PathBuf;

                    if object_key == prefix { // Handles downloading a single file specified by its full key
                        let filename = object_key_path.file_name().ok_or_else(|| anyhow::anyhow!("Object key {} has no filename", object_key))?;
                        final_local_path = local_base_path.join(filename);
                    } else {
                        let s3_prefix_to_strip = if prefix.is_empty() || prefix.ends_with('/') {
                            prefix.clone()
                        } else {
                             // If prefix is "foo" and object_key is "foo/bar.txt", strip "foo/"
                            if object_key.starts_with(&format!("{}/", prefix)) {
                                format!("{}/", prefix)
                            } else {
                                prefix.clone() // Should only match if object_key == prefix (handled above) or not at all
                            }
                        };
                        let path_suffix = object_key.strip_prefix(&s3_prefix_to_strip)
                            .ok_or_else(|| anyhow::anyhow!("Logic error: Failed to strip prefix '{}' from object key '{}'", s3_prefix_to_strip, object_key))?;
                        final_local_path = local_base_path.join(path_suffix);
                    }

                    if let Some(parent_dir) = final_local_path.parent() {
                        if !parent_dir.exists() {
                            fs::create_dir_all(parent_dir).await.with_context(|| format!("Failed to create parent directory {}", parent_dir.display()))?;
                        }
                    }
                    
                    println!("  Downloading {} ...", object_key.yellow());
                    match client.get_object().bucket(bucket.clone()).key(object_key.to_string()).send().await {
                        Ok(get_obj_output) => {
                            let mut file = File::create(&final_local_path).await.with_context(|| format!("Failed to create file {}", final_local_path.display()))?;
                            let mut byte_stream = get_obj_output.body;
                            let mut file_size_bytes: u64 = 0;
                            while let Some(bytes) = byte_stream.try_next().await.with_context(|| format!("Failed to read bytes from S3 stream for {}", object_key))? {
                                file.write_all(&bytes).await.with_context(|| format!("Failed to write to file {}", final_local_path.display()))?;
                                file_size_bytes += bytes.len() as u64;
                            }
                            total_size_bytes += file_size_bytes;
                            downloaded_count += 1;
                            println!("    {} Downloaded ({} MB)", "Success:".green(), file_size_bytes / 1024 / 1024);
                        }
                        Err(e) => eprintln!("    {} Failed to download {}: {}", "Error:".red(), object_key.yellow(), e),
                    }
                }
            }
            Err(e) => {
                eprintln!("{} Failed to list objects from S3: {}", "Error:".red(), e);
                break; 
            }
        }
    }

    if downloaded_count > 0 {
        println!("\n{} Downloaded {} files, total size {} MB to {}.", "Finished:".bold().green(),
                    downloaded_count, total_size_bytes / 1024 / 1024, local_base_path.display());
    } else {
        println!("\n{} No files found to download for S3 prefix '{}' in bucket '{}', or an error occurred during listing.", "Info:".yellow(), prefix, bucket);
    }
    Ok(())
}

pub fn query_table(table: &str, position: &str, radius: &f64, columns_specifier: &Option<String>, add_prods: &bool, conn: &Connection) -> Result<()> {

    // Check if the provided table_name is in the list of supported tables
    if !SUPPORTED_TABLES.contains(&table) {
        eprintln!("Error: Table '{}' is not a supported table.", table.red().bold());
        eprintln!("Supported tables are: {}", SUPPORTED_TABLES.join(", ").cyan().bold());
        return Ok(()); // Exit the function early
    }
    
    // figure out the requested ra,dec
    let mut ra_opt: Option<f64> = None;
    let mut dec_opt: Option<f64> = None;

    // Try splitting by comma first
    let parts_comma: Vec<&str> = position.split(',').map(|s| s.trim()).filter(|s| !s.is_empty()).collect();
    if parts_comma.len() == 2 {
        if let (Ok(ra_val), Ok(dec_val)) = (parts_comma[0].parse::<f64>(), parts_comma[1].parse::<f64>()) {
            ra_opt = Some(ra_val);
            dec_opt = Some(dec_val);
        }
    } else {
        // If comma split didn't work or yielded wrong number of parts, try splitting by whitespace
        let parts_space: Vec<&str> = position.split_whitespace().map(|s| s.trim()).filter(|s| !s.is_empty()).collect();
        if parts_space.len() == 2 {
                if let (Ok(ra_val), Ok(dec_val)) = (parts_space[0].parse::<f64>(), parts_space[1].parse::<f64>()) {
                ra_opt = Some(ra_val);
                dec_opt = Some(dec_val);
            }
        }
    }

    let (user_ra, user_dec) = match (ra_opt, dec_opt) {
        (Some(ra_val), Some(dec_val)) => (ra_val, dec_val),
        _ => {
            eprintln!("Error: Invalid position format '{}'. Expected 'ra,dec' in degrees with valid numbers.", position.red().bold());
            return Ok(());
        }
    };

    // Determine the search radius to use
    let search_radius = if *radius == 0.0 {
        // If user provided 0.0, try to get the default radius from metainfo
        match get_default_radius(&conn, &table) {
            Ok(default_val) => {
                println!("{}", "(Using default radius from metainfo)".italic());
                default_val
            },
            Err(e) => {
                eprintln!("Error fetching default radius for table '{}': {}", table.red().bold(), e);
                eprintln!("{}", "Please provide a radius explicitly using --radius.".italic());
                return Ok(()); // Exit if default radius lookup fails when needed
            }
        }
    } else {
        *radius // Use the user-provided radius
    };

    println!("Searching table {} around RA: {}, Dec: {} within radius: {} arcmin",
        table.magenta(),
        user_ra.to_string().green(),
        user_dec.to_string().green(),
        search_radius.to_string().yellow()
    );

    let display_columns_vec: Vec<String>;
    let select_clause_for_sql: String;

    match columns_specifier {
        Some(spec) => {
            if spec == "*" || spec.to_lowercase() == "all" {
                select_clause_for_sql = "*".to_string();
                display_columns_vec = get_all_table_columns(conn, table)?
                    .into_iter()
                    //.filter(|c| !INTERNAL_COLS_ARRAY.contains(&c.as_str()))
                    .collect();
            } else {
                // User provided a comma-separated list
                display_columns_vec = spec.split(',')
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                    .collect();
                
                let mut sql_select_set: HashSet<String> = display_columns_vec.iter().cloned().collect();
                // Ensure we have columns needed to get the products
                sql_select_set.insert("obsid".to_string());
                if table == "nicermastr" {
                    sql_select_set.insert("time".to_string());
                } else if table == "swiftmastr" {
                    sql_select_set.insert("start_time".to_string());
                }
                select_clause_for_sql = sql_select_set.into_iter().collect::<Vec<String>>().join(", ");
            }
        }
        None => { // Default columns
            match get_default_columns(conn, table) { // get_default_columns returns comma-separated string
                Ok(default_cols_str) if !default_cols_str.is_empty() => {
                    display_columns_vec = default_cols_str.split(',').map(String::from).collect();
                    let sql_select_set: HashSet<String> = display_columns_vec.iter().cloned().collect();
                    select_clause_for_sql = sql_select_set.into_iter().collect::<Vec<String>>().join(", ");
                }
                _ => { // Error or no default columns defined, fallback to all
                    println!("{}", "(No default display columns defined or error fetching, showing all. Specify columns or use 'list-columns' to see available.)".italic());
                    select_clause_for_sql = "*".to_string();
                    display_columns_vec = get_all_table_columns(conn, table)?
                        .into_iter()
                        //.filter(|c| !INTERNAL_COLS_ARRAY.contains(&c.as_str()))
                        .collect();
                }
            }
        }
    }

    // Construct and print the header
    let mut header_line = format!("{:<5} | {:<10}",
                                    "#".bold(), "Offset (')".bold());
    for col_name in &display_columns_vec {
        //if col_name != "ra" && col_name != "dec" && col_name != "id" { // Assuming 'id' is special and ra/dec handled
            header_line.push_str(&format!(" | {:<12}", col_name.bold()));
        //}
    }
    if *add_prods {
        header_line.push_str(&format!(" | {:<20}", "link".bold()));
    }

    println!("\n{}", header_line);
    //println!("{}", separator_line);

    let ra_rad = user_ra.to_radians();
    let dec_rad = user_dec.to_radians();
    let unit_x = ra_rad.sin() * dec_rad.cos();
    let unit_y = ra_rad.cos() * dec_rad.cos();
    let unit_z = dec_rad.sin();
    let radius_c = (search_radius / 60.0).to_radians().cos();


    let query = format!("
        SELECT {}, (__x_ra_dec * ?1 + __y_ra_dec * ?2 + __z_ra_dec * ?3) as _offset FROM {}
        WHERE _offset >= ?4
        ", select_clause_for_sql, table
    );
    //println!("Executing query: {}", query.dimmed().italic());
    let mut stmt = conn.prepare(&query)?;
    let mut rows = stmt.query([unit_x, unit_y, unit_z, radius_c])?;
    let mut last_prod = "".to_string(); // use for printing help at the end
    
    let mut found_count = 0;
    while let Some(row) = rows.next()? {
        let row_offset = row.get::<_, f64>("_offset")?; // Use ? to propagate error
        let sep = "|".blue();

        found_count += 1;
                            
        // Start building the output line with fixed columns
        let mut output_line = format!("{:<5} | {:<10.6}",
                    found_count.to_string().cyan(),
                    (row_offset.acos().to_degrees() * 60.0).to_string().yellow()
        );
        for col_name in &display_columns_vec {
            // Try to get the column value as Option<String>
            match row.get::<_, Option<String>>(col_name.as_str()) {
                Ok(Some(val)) => {
                    output_line.push_str(&format!(" {} {:<12}", sep, val));
                }
                Ok(None) => {
                    output_line.push_str(&format!(" {} {:<12}", sep, "NULL".dimmed()));
                }
                Err(_) => {
                    // This column might not be in SELECT (if select_clause_for_sql is out of sync with display_columns_vec)
                    // or it's not convertible to Option<String>
                    output_line.push_str(&format!(" {} {}: {}", sep, col_name.red(), "<N/A>".dimmed()));
                }
            }
        }
        if *add_prods {
            // Always add product link at the end
            let product_link = get_product_link(table, row);
            last_prod = format!("{}{}", "s3://nasa-heasarc/".green(), product_link.green());
            output_line.push_str(&format!(" {} {}", sep, last_prod));
        }
        
        println!("{}", output_line); // Print the complete line
    }

    if found_count == 0 {
        println!("{}", "No results found with the specified parameters.".italic());
    } else {
        println!("{}", "---------------------------".dimmed());
        println!("Query returned {} entries", found_count.to_string().green().bold());
        println!("{}", "---------------------------".dimmed());
        if !last_prod.contains("No Product Link") && !last_prod.is_empty() { // Check if a valid product link was generated
            println!("To retrieve a product, use the {} command. For example:\n{} {}",
                "aws-download".yellow(),
                "aws-download".green(),
                last_prod
            );
        }
    }
    println!();
    Ok(())
}

// Helper function to get all column names for a table
fn get_all_table_columns(conn: &Connection, table_name: &str) -> Result<Vec<String>, rusqlite::Error> {
    let mut stmt_info = conn.prepare(&format!("PRAGMA table_info('{}')", table_name))?;
    let mut rows_info = stmt_info.query([])?;
    let mut column_names = Vec::new();
    while let Some(row_info) = rows_info.next()? {
        let col_name: String = row_info.get(1)?; // Column name is at index 1
        if !INTERNAL_COLS_ARRAY.contains(&col_name.as_str()) { // Exclude internal columns
             column_names.push(col_name);
        }
    }
    Ok(column_names)
}

// work out product links
fn get_product_link(table_name: &str, row: &Row) -> String {
    let obsid_res = row.get::<_, Option<String>>("obsid");
    let obsid = match obsid_res {
        Ok(Some(val)) => val,
        _ => return "No Product Link (obsid missing)".to_string(), // Early return if obsid is not found or error
    };

    match table_name {
        "nicermastr" => {
            let time_str_res = row.get::<_, Option<String>>("time");
            let mjd_str = match time_str_res {
                Ok(Some(val)) => val,
                _ => return "No Product Link (time missing)".to_string(),
            };
            let mjd: f64 = mjd_str.parse().unwrap_or(0.0);
            if mjd == 0.0 { return "No Product Link (invalid time)".to_string(); }

            let tzero = NaiveDate::from_ymd_opt(2014, 1, 1).unwrap();
            let mjdref = 56658; // MJD for 2014-01-01
            let days_since_ce = mjd.floor() as i32 - mjdref + tzero.num_days_from_ce();
            let date = NaiveDate::from_num_days_from_ce_opt(days_since_ce).unwrap_or(tzero); // Fallback to tzero on error
            let year_month = format!("{:04}_{:02}", date.year(), date.month());
            format!("nicer/data/obs/{year_month}/{obsid}")
        },
        "xmmmaster" => format!("xmm/data/rev0/{obsid}"),
        "swiftmastr" => {
            let start_time_str_res = row.get::<_, Option<String>>("start_time");
            let mjd_str = match start_time_str_res {
                Ok(Some(val)) => val,
                _ => return "No Product Link (start_time missing)".to_string(),
            };
            let mjd: f64 = mjd_str.parse().unwrap_or(0.0);
            if mjd == 0.0 { return "No Product Link (invalid start_time)".to_string(); }

            let tzero = NaiveDate::from_ymd_opt(2001, 1, 1).unwrap();
            let mjdref = 51910; // MJD for 2001-01-01
            let days_since_ce = mjd.floor() as i32 - mjdref + tzero.num_days_from_ce();
            let date = NaiveDate::from_num_days_from_ce_opt(days_since_ce).unwrap_or(tzero);
            let year_month = format!("{:04}_{:02}", date.year(), date.month());
            format!("swift/data/obs/{year_month}/{obsid}")
        },
        "chanmaster" => {
            let substr = obsid.chars().last().map_or("?".to_string(), |c| c.to_string()); // Handle empty obsid
            format!("chandra/data/byobsid/{substr}/{obsid}")
        },
        "numaster" => {
            if obsid.len() < 3 { return "No Product Link (obsid too short)".to_string(); }
            let substr1: String = obsid.chars().skip(1).take(2).collect();
            let substr2: String = obsid.chars().take(1).collect();
            format!("nustar/data/obs/{substr1}/{substr2}/{obsid}")
        },
        "ixmaster" => { // Assuming ixpemaster was a typo for ixmaster
            if obsid.is_empty() { return "No Product Link (obsid empty)".to_string(); }
            let substr: String = obsid.chars().take(2).collect();
            format!("ixpe/data/obs/{substr}/{obsid}")
        },
        _ => {
            "No Product Link (unknown table)".to_string()
        }
    }
}

// Function to query metainfo and get a comma-separated list of default column names
fn get_default_columns(conn: &Connection, table_name: &str) -> Result<String, rusqlite::Error> {
    let query = r#"
        SELECT
            substr(name, instr(name, '.') + 1) AS column_name
        FROM metainfo
        WHERE type='parameter' AND name LIKE ?1 || '.%'
        GROUP BY substr(name, instr(name, '.') + 1)
        HAVING MAX(CASE WHEN relation = 'order' THEN CAST(value AS INTEGER) END) IS NOT NULL
        ORDER BY
            CASE
                WHEN MAX(CASE WHEN relation = 'order' THEN CAST(value AS INTEGER) END) IS NULL THEN 1
                ELSE 0
            END,
            MAX(CASE WHEN relation = 'order' THEN CAST(value AS INTEGER) END);
    "#;

    let mut stmt = conn.prepare(query)?;
    let column_names_iter = stmt.query_map(params![table_name], |row| row.get::<_, String>(0))?;

    let column_names: Result<Vec<String>, _> = column_names_iter.collect();
    let column_names = column_names?; // Propagate any error from fetching rows

    Ok(column_names.join(","))
}

// Function to query metainfo for default radius from a table
fn get_default_radius(conn: &Connection, table_name: &str) -> Result<f64, Box<dyn Error>> {
    let query = r#"
        SELECT value
        FROM metainfo
        WHERE name = ?1 AND relation = 'defaultSearchRadius' LIMIT 1
    "#;

    let mut stmt = conn.prepare(query)?;
    // The value in the database is likely stored as TEXT, so we get it as String and then parse.
    // If it's stored as REAL, you could use row.get::<_, f64>(0) directly.
    let radius_str = stmt.query_row(params![table_name], |row| row.get::<_, String>(0))?;
    
    let radius = radius_str.parse::<f64>()?;

    Ok(radius)
}

/// Returns a normalized target string for the current platform
fn get_target() -> &'static str {
    let system_target = self_update::get_target();
    
    match system_target {
        // Linux x86_64
        target if target.contains("x86_64") && target.contains("linux") => "linux-amd64",
        // macOS x86_64 (Intel)
        target if target.contains("x86_64") && target.contains("apple") => "macos-amd64",
        // macOS aarch64 (Apple Silicon)
        target if target.contains("aarch64") && target.contains("apple") => "macos-arm64",
        // Default fallback - you might want to handle this differently
        _ => {
            eprintln!("Warning: Unsupported target '{}', defaulting to linux-amd64", system_target);
            "linux-amd64"
        }
    }
}