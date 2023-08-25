# DCS Tetrad

Real-time server monitor and data logger.

## Installation
1. Extract the .zip archive onto a permanent location as it will be referenced by the script.
2. Copy the contents of the `lua/DCS` directory into your DCS saved games directory (so that `/lua/DCS/Scripts` matches the `Scripts` and `/lua/DCS/Config` matches the `Config` directory in your DCS Server Saved Games Directory, etc.).

## Configuration

Modify the contents of `<saved games>/DCS[.openbeta_server]/Config/tetrad-config.lua` to suit your preferences.

**You will need to modify the dll and lua paths to match the location where you extracted the build zip file. 
(Important to not miss any \ or the path will be interepreted incorrectly)**

```lua
dll_path = [[C:\projects\dcs_tetrad\target\release\]]  -> Location of Folder that contains `dcs_tetrad.dll` as per Step 1 of the Installation Guide
lua_path = [[C:\projects\dcs_tetrad\lua\]] -> Location of Folder that contains `hook.lua` as per Step 1 of the Installation Guide
debug = true 
enable_object_log = false -> Object Log will log (Location,Vector, Name, etc) of all objects on the server and results in very large files. 
```

## Export
Once installation and configuration is complete. DCS Tetrad logger will run automatically upon mission start and will present a live grapher with data. 

Upon mission completion Tetrad will export at `Saved Games\DCS.openbeta_server\Logs\Tetrad`. Tetard will export a Log File and CSV files in `Saved Games\DCS.openbeta_server\Logs\Tetrad\frames` and `Saved Games\DCS.openbeta_server\Logs\Tetrad\objects` (Objects CSV will only be logged if enable_object_log is set to True in the configuration file).

Note: The CSV files are compressed using .zstd format. To decompress .zstd files you must use programs simillar to: https://github.com/mcmilk/7-Zip-zstd.

**Interpreting Raw Data**
The frame excels will export the following variables:
1. t_game: Ingame Frame Time (Note: The time is added to the last frame time with each tick)
2. t_real: Real Frame Time (Note: The time is added to the last frame time with each tick)
3. units: Number of Units Simulated by the Server during the tick
4. ballistics: Number of Ballistic Objects (Missiles, Gun Rounds, Bombs, etc) simulated by the server during the tick.
5. SYS_CPU, SYS_WALL, PROC_CPU are WIN32 CPU Performacne Metrics 

## For developers

### Building

Just run

```
cargo build --release
```

You will need to point the lua config file at the `target/release/` directory.
