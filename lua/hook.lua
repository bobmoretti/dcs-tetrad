local function writeLog(level, message)
    log.write("[tetrad-hook]", level, message)
end

-- Register Callbacks in DCS World GUI environment

local tetradCallbacks = {}
TETRAD = {}

local function onMissionLoadEnd()
    -- Let DCS know where to find the DLLs
    if not string.find(package.cpath, tetrad_config.dll_path) then
        package.cpath = package.cpath .. [[;]] .. tetrad_config.dll_path .. [[?.dll;]]
    else
        writeLog(log.INFO, "dll path already in cpath.")
    end
    local tetrad_lib = require("dcs_tetrad")
    if tetrad_lib then
        writeLog(log.INFO, "Loaded tetrad library from hook")
        tetrad_lib.start({
            write_dir = lfs.writedir(),
            lua_path = tetrad_config.lua_path,
            dll_path = tetrad_config.dll_path,
            debug = tetrad_config.debug
        })
        writeLog(log.INFO, "Started tetrad library from hook.")
        TETRAD['lib'] = tetrad_lib
    else
        writeLog(log.ERROR, "Failed to load tetrad library from hook")
    end
end

do
    function tetradCallbacks.onMissionLoadEnd()
        onMissionLoadEnd()
    end

    function tetradCallbacks.onSimulationStop()
        TETRAD.lib.stop()
        TETRAD.lib = nil
        TETRAD = nil
        package.loaded['dcs_tetrad'] = nil
    end

    function tetradCallbacks.onSimulationFrame()
        TETRAD.lib.on_frame_begin()
    end

    function tetradCallbacks.onPlayerConnect(id)
    end

    function tetradCallbacks.onPlayerDisconnect(id, err_code)
    end

    DCS.setUserCallbacks(tetradCallbacks)
    writeLog(log.INFO, "Set up Tetrad hook callbacks.")
end
