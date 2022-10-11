local function writeLog(level, message)
    log.write("[tetrad-hook]", level, message)
end

-- Register Callbacks in DCS World GUI environment

local tetradCallbacks = {}
TETRAD = {}
local function onMissionLoadEnd()
    writeLog(log.INFO, "On Mission load end!")
    -- Let DCS know where to find the DLLs
    if not string.find(package.cpath, tetrad_config.dll_path) then
        package.cpath = package.cpath .. [[;]] .. tetrad_config.dll_path .. [[?.dll;]]
    else
        writeLog(log.INFO, "dll path already in cpath.")
    end

    tetrad_config = {}
    _G.tetrad_config = tetrad_config

    local file, err = io.open(lfs.writedir() .. [[Config\tetrad-config.lua]], "r")
    if file then
        local f = assert(loadstring(file:read("*all")))
        setfenv(f, tetrad_config)
        f()
        writeLog(log.INFO, "`Config/tetrad-config.lua` successfully read")
    else
        writeLog(log.INFO, "`Config/tetrad-config.lua` not found (" .. tostring(err) .. ")")
    end
    tetrad_config.write_dir = lfs.writedir()
    writeLog(log.INFO, "Tetrad config follows: ")
    for k, v in pairs(tetrad_config) do
        writeLog(log.INFO, k .. " = " .. tostring(v))
    end
    writeLog(log.INFO, "End of Tetrad config")

    local tetrad_lib = require("dcs_tetrad")
    if tetrad_lib then
        writeLog(log.INFO, "Loaded tetrad library from hook")
        tetrad_lib.start(tetrad_config)
        writeLog(log.INFO, "Started tetrad library from hook.")
        TETRAD['lib'] = tetrad_lib
    else
        writeLog(log.ERROR, "Failed to load tetrad library from hook")
    end
end

do
    function tetradCallbacks.onMissionLoadEnd()
        local status, err = pcall(onMissionLoadEnd)
        if not status then
            writeLog(log.INFO, "error starting library: " .. tostring(err))
        end
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
