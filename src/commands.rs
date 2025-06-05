use anyhow::Result;
use rusqlite::{params, Connection, Row};
use colored::*;
use chrono::{NaiveDate, Datelike};
use std::error::Error;

use crate::SUPPORTED_TABLES;


pub fn handle_help(args: &[String], _conn: &Connection) -> Result<()> {
    if args.is_empty() {
        println!();
        println!("{}: HEASARC archive offline explorer.", "hark".bold().yellow());
        println!();
        println!("{}", "Commands".cyan().underline());
        println!("{:>15}: List supported tables", "list-tables".cyan());
        println!("{:>15}: List columns of a table", "list-columns".cyan());
        println!("{:>15}: Query a specific table", "query-table".cyan());
        println!("{:>16}", "-----------".cyan().dimmed());
        println!("{:>15}: Show help message (also: h, ?)", "help".cyan());
        println!("{:>15}: Exit (also: quit, q)", "exit".cyan());
        println!();
    } else {
        println!("Help for command '{}': (not yet implemented)", args[0]);
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


pub fn query_table(table: &str, position: &str, radius: &f64, prod_only: &bool, conn: &Connection) -> Result<()> {
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

    // Fetch the comma-separated list of default column names for display
    let default_cols_str_opt = match get_default_columns(&conn, table) {
        Ok(s) if !s.is_empty() => {
            println!("Will display additional default columns: {}", s.cyan());
            Some(s)
        }
        Ok(_) => { // Empty string means no default columns with order
            println!("{}", "No specific ordered default columns found for additional display.".dimmed());
            None
        }
        Err(e) => {
            eprintln!("Warning: Could not fetch default column list for table '{}': {}. No additional columns will be shown.", table.yellow(), e);
            None
        }
    };

    let default_col_names_vec: Vec<String> = default_cols_str_opt
        .as_deref()
        .map_or_else(Vec::new, |s| s.split(',').map(String::from).collect());

    // Prepare the list of additional columns to display in the header,
    // excluding those already covered by the fixed header (id, ra, dec).
    let additional_display_col_names: Vec<String> = default_col_names_vec
        .iter()
        .filter(|&col_name| col_name != "id" && col_name != "ra" && col_name != "dec")
        .cloned()
        .collect();

    // Construct and print the header
    let mut header_line = format!("{:<5} | {:<8} | {:<8} | {:<8}",
                                    "#".bold(), "ra (deg)".bold(), "dec (deg)".bold(), "Offset (')".bold());
    let mut separator_line = format!("{:-<5}-+-{:-<8}-+-{:-<8}-+-{:-<8}",
                                        "".dimmed(), "".dimmed(), "".dimmed(), "".dimmed());
    if ! *prod_only {
        for col_name in &additional_display_col_names {
            header_line.push_str(&format!(" | {:<20}", col_name.bold())); // Assuming 20 char width for additional cols
            separator_line.push_str(&format!("-+-{:-<20}", "".dimmed()));
        }
    }
    header_line.push_str(&format!(" | {:<20}", "link".bold()));
    separator_line.push_str(&format!("-+-{:-<20}", "".dimmed()));

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
        ", default_cols_str_opt.as_deref().unwrap_or("*"), table
    );
    //println!("Executing query: {}", query.dimmed().italic());
    let mut stmt = conn.prepare(&query)?;
    let mut rows = stmt.query([unit_x, unit_y, unit_z, radius_c])?;
    
    let mut found_count = 0;
    while let Some(row) = rows.next()? {
        // Attempt to get ra and dec directly as f64 if they are numeric in DB
        // If they are stored as TEXT, then getting as String and parsing is correct.
        // Assuming they might be TEXT for now, based on current code.
        let row_ra_str_res = row.get::<_, String>("ra");
        let row_dec_str_res = row.get::<_, String>("dec");
        let row_offset = row.get::<_, f64>("_offset");
        let sep = "|".blue();

        found_count += 1;
                            
        // Start building the output line with fixed columns
        let mut output_line = format!("{:<5} | {:<8.6} | {:<8.6} | {:<8.6}",
                    found_count.to_string().cyan(),
                    row_ra_str_res.unwrap().to_string().green(), // Use parsed f64
                    row_dec_str_res.unwrap().to_string().green(), // Use parsed f64
                    (row_offset.unwrap().acos().to_degrees() * 60.0).to_string().yellow()
        );
        if !default_col_names_vec.is_empty() {
            if ! *prod_only {
                for col_name in &default_col_names_vec {
                    // Skip if it's one of the primary columns already printed, to avoid redundancy
                    if col_name == "id" || col_name == "ra" || col_name == "dec" {
                        continue;
                    }
                    // reset obsid and time values
                    match row.get::<_, Option<String>>(col_name.as_str()) {
                        Ok(Some(val)) => {
                            output_line.push_str(&format!(" {} {}", sep, val));
                        }
                        Ok(None) => {
                            output_line.push_str(&format!(" {} {}", sep, "NULL".dimmed()));
                        }
                        Err(_) => {
                            // This column might not be in SELECT * (if get_default_columns is out of sync)
                            // or it's not convertible to Option<String>
                            output_line.push_str(&format!(" {} {}: {}", sep, col_name.red(), "<N/A or Error>".dimmed()));
                        }
                    }
                }
            }
            let product_link = get_product_link(table, row);
            output_line.push_str(&format!(" {} {}{}", sep, "s3://nasa-heasarc/".green(), product_link.green()));
        }
        println!("{}", output_line); // Print the complete line
    }

    if found_count == 0 {
        println!("{}", "No results found with the specified parameters.".italic());
    } else {
        println!("{}", "---------------------------".dimmed());
        println!("Query returned {} entries", found_count.to_string().green().bold());
    }
    println!();
    Ok(())
}

// work out product links
fn get_product_link(table_name: &str, row: &Row) -> String {

    let obsid = match row.get::<_, Option<String>>("obsid") {
        Ok(Some(val)) => val,
        _ => {"No Obsid".to_string()}
    };

    match table_name {
        "nicermastr" => {
            let mjd: f64 = match row.get::<_, Option<String>>("time") {
                Ok(Some(val)) => val,
                _ => {"0.0".to_string()}
            }.parse().unwrap_or(0.0);
            let tzero = NaiveDate::from_ymd_opt(2014, 01, 01).unwrap();
            let mjdref =  56658;
            let days_since_ce = mjd.floor() as i32 - mjdref + tzero.num_days_from_ce();
            let date =     NaiveDate::from_num_days_from_ce_opt(days_since_ce).unwrap();
            let year_month = format!("{:04}_{:02}", date.year(), date.month());
            format!("nicer/data/obs/{year_month}/{obsid}/")
        },
        "xmmmaster" => format!("xmm/data/rev0/{obsid}"),
        "swiftmastr" => {
                        let mjd: f64 = match row.get::<_, Option<String>>("start_time") {
                Ok(Some(val)) => val,
                _ => {"0.0".to_string()}
            }.parse().unwrap_or(0.0);
            let tzero = NaiveDate::from_ymd_opt(2001, 01, 01).unwrap();
            let mjdref =  51910;
            let days_since_ce = mjd.floor() as i32 - mjdref + tzero.num_days_from_ce();
            let date = NaiveDate::from_num_days_from_ce_opt(days_since_ce).unwrap();
            let year_month = format!("{:04}_{:02}", date.year(), date.month());
            format!("swift/data/obs/{year_month}/{obsid}")
        },
        "chanmaster" => {
            //let substr = obsid.chars().last().map(|c| c.to_string()).unwrap_or_default();
            let substr = obsid.chars().last().map(|c| c.to_string()).unwrap_or_default();
            format!("chandra/data/byobsid/{substr}/{obsid}")
        },
        "numaster" => {
            let substr1: String = obsid.chars().skip(1).take(2).collect();
            let substr2: String = obsid.chars().take(1).collect();
            format!("nustar/data/obs/{substr1}/{substr2}/{obsid}")
        },
        "ixmaster" => {
            let substr: String = obsid.chars().take(2).collect();
            format!("ixpe/data/obs/{substr}/{obsid}")
        },
        "xrismmastr" => {
            let substr: String = obsid.chars().take(1).collect();
            format!("xrism/data/obs/{substr}/{obsid}")
        },
        _ => {
            "".to_string()
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