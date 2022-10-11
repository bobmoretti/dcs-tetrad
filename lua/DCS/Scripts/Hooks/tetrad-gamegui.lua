local function init()
    log.write("[tetrad-hook]", log.INFO, "Initializing ...")
    -- load settings from `Saved Games/DCS/Config/dcs-grpc.lua`

    if not tetrad_config then
        _G.tetrad_config = {
            dll_path = [[F:\projects\dcs\tetrad\target\release\]],
            lua_path = [[F:\projects\dcs\tetrad\lua\]],
            debug = true
        }
    end

    do
        log.write("[tetrad-hook]", log.INFO, "Checking optional config at `Config/tetrad-config.lua` ...")
        local file, err = io.open(lfs.writedir() .. [[Config\tetrad-config.lua]], "r")
        if file then
            local f = assert(loadstring(file:read("*all")))
            setfenv(f, tetrad_config)
            f()
            log.write("[tetrad-hook]", log.INFO, "`Config/tetrad-config.lua` successfully read")
        else
            log.write("[tetrad-hook]", log.INFO, "`Config/tetrad-config.lua` not found (" .. tostring(err) .. ")")
        end
    end

    dofile(tetrad_config.lua_path .. [[hook.lua]])
    log.write("[tetrad-hook]", log.INFO, "Initialized...")
end

local ok, err = pcall(init)
if not ok then
    log.write("[tetrad-hook]", log.ERROR, "Failed to Initialize: " .. tostring(err))
end
