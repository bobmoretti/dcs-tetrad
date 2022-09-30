local function init()
    log.write("[tetrad-export]", log.INFO, "Initializing ...")

    -- settings
    tetrad_config = {
        dll_path = [[F:\projects\dcs\tetrad\target\release\]],
        lua_path = [[F:\projects\dcs\tetrad\lua\]],
        debug = false
    }

    dofile(tetrad_config.lua_path .. [[export.lua]])

    log.write("[tetrad-export]", log.INFO, "Initialized...")
end

local ok, err = pcall(init)
if not ok then
    log.write("[tetrad-export]", log.ERROR, "Failed to Initialize: " .. tostring(err))
end
