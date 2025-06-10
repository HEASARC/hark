# HEASARC offline databse browser
This is an application that browses the HEASARC archive offline and generates access urls to the data in the cloud.

# Installation

```sh
curl -sSL https://raw.githubusercontent.com/HEASARC/hark/gh/install.sh | sh
```
This will install the `hardk` binary file in `~/.local/bin/`. If you want to install it
in another location, run:
```sh
curl -sSL https://raw.githubusercontent.com/HEASARC/hark/gh/install.sh | sh -s -- --install-dir /my/custom/location/bin
```

Make sure the install location is in you `$PATH` variable. Then run: `hark`.

Type `?` to print the help message:
```
hark> ?

hark: HEASARC archive offline explorer.

Commands
     list-tables: List supported tables
    list-columns: List columns of a table
     query-table: Query a specific table
    aws-download: Download data from AWS
      -----------
            help: Show help message (also: h, ?).
                  Use help command-name for command help
            exit: Exit (also: quit, q)
```