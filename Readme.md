# DCS Tetrad

Real-time server monitor and data logger.

## Installation

##

Extract the .zip archive onto your disk.

Copy the contents of the `lua/DCS` directory into your DCS saved games directory (so that `/lua/DCS/Scripts` matches the `Scripts` directory in your DCS saved games directory, etc.).

## Configuration

Modify the contents of `<saved games>/DCS[.openbeta]/Config/tetrad-config.lua` to suit your preferences.

**You will need to modify the dll and lua paths to match the location where you extracted the build zip file.**

```lua
dll_path = [[C:\projects\dcs_tetrad\target\release\]]
lua_path = [[C:\projects\dcs_tetrad\lua\]]
debug = true
enable_object_log = false
```

## For developers

### Building

Just run

```
cargo build --release
```

You will need to point the lua config file at the `target/release/` directory.
