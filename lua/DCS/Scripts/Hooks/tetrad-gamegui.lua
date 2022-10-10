local function init()
    log.write("[tetrad-hook]", log.INFO, "Initializing ...")

    -- settings
    _G.tetrad_config = {
        dll_path = [[F:\projects\dcs\tetrad\target\release\]],
        lua_path = [[F:\projects\dcs\tetrad\lua\]],
        debug = true
    }

    dofile(tetrad_config.lua_path .. [[hook.lua]])
    log.write("[tetrad-hook]", log.INFO, "Initialized...")
end

local ok, err = pcall(init)
if not ok then
    log.write("[tetrad-hook]", log.ERROR, "Failed to Initialize: " .. tostring(err))
end
