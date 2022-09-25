local function writeLog(level, message)
    log.write("[tetrad-export]", level, message)
end

local TETRAD = {}

local function onExportStart()

    -- Let DCS know where to find the DLLs
    if not string.find(package.cpath, tetrad_config.dll_path) then
        package.cpath = package.cpath .. [[;]] .. tetrad_config.dll_path .. [[?.dll;]]
    end

    writeLog(log.INFO, "CPATH is: " .. tostring(package.cpath))

    local tetrad_lib = require("dcs_tetrad")
    if tetrad_lib then
        writeLog(log.INFO, "Loaded library.")
        tetrad_lib.start({
            write_dir = lfs.writedir(),
            lua_path = tetrad_config.lua_path,
            dll_path = tetrad_config.dll_path,
            debug = tetrad_config.debug
        })
        writeLog(log.INFO, "Started library.")
        TETRAD['lib'] = tetrad_lib
    else
        writeLog(log.INFO, "Failed to load library")
    end
end

do
    local function writeLog(level, msg)
        log.write("[tetrad-export]", level, msg)
    end

    writeLog(log.INFO, "doing export")
    -- (Hook) Called once right before mission start.
    do
        local PrevLuaExportStart = LuaExportStart
        LuaExportStart = function()
            writeLog(log.INFO, "On LuaExportStart")
            onExportStart()
            if PrevLuaExportStart then
                PrevLuaExportStart()
            end
        end
    end

    -- (Hook) Called right after every simulation frame.
    do
        local PrevLuaExportAfterNextFrame = LuaExportAfterNextFrame
        LuaExportAfterNextFrame = function()
            TETRAD.lib.on_frame_begin()
            if PrevLuaExportAfterNextFrame then
                PrevLuaExportAfterNextFrame()
            end
        end
    end

    -- (Hook) Called right after mission end.
    do
        local PrevLuaExportStop = LuaExportStop
        LuaExportStop = function()
            TETRAD.lib = nil
            TETRAD = nil

            if PrevLuaExportStop then
                PrevLuaExportStop()
            end
        end
    end

end
