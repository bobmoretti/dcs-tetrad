use mlua::prelude::{LuaResult, LuaTable};
use mlua::Lua;
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::sync::mpsc::Sender;
mod dcs;
pub mod worker;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Config {
    pub write_dir: String,
    pub lua_path: String,
    pub dll_path: String,
    #[serde(default)]
    pub debug: bool,
}

impl<'lua> mlua::FromLua<'lua> for Config {
    fn from_lua(lua_value: mlua::Value<'lua>, lua: &'lua mlua::Lua) -> mlua::Result<Self> {
        use mlua::LuaSerdeExt;
        let config: Config = lua.from_value(lua_value)?;
        Ok(config)
    }
}

struct LibState {
    // time before integer overflow > 1 year @ 120 FPS
    frame_count: i32,
    tx: Sender<worker::Message>,
}

static mut LIB_STATE: Option<LibState> = None;

fn get_lib_state() -> &'static mut LibState {
    unsafe { LIB_STATE.as_mut().expect("msg") }
}

fn increment_frame_count() {
    get_lib_state().frame_count += 1;
}

fn send_message(message: worker::Message) {
    log::trace!("sending message {:?}", message);
    get_lib_state()
        .tx
        .send(message)
        .expect("Should be able to send message");
}

fn init(config: &Config) {
    use log::LevelFilter;
    let level = if config.debug {
        LevelFilter::Debug
    } else {
        LevelFilter::Info
    };
    let writedir = Path::new(config.write_dir.as_str());
    let p = writedir.join("dcs_tetrad.log");
    simple_logging::log_to_file(p, level).unwrap();

    log_panics::init();

    log::info!("Initialization complete!");
}

#[no_mangle]
pub fn start(lua: &Lua, config: Config) -> LuaResult<()> {
    unsafe {
        if LIB_STATE.is_some() {
            log::info!("Library already created!!!");
            return Ok(());
        } else {
            log::info!("Called for the very first time!");
        }
    }

    init(&config);
    log::info!("Creating channel");

    let (tx, rx) = std::sync::mpsc::channel();

    unsafe {
        LIB_STATE = Some(LibState {
            frame_count: 0,
            tx: tx,
        });
    }

    let mission_name = dcs::get_mission_name(lua);
    log::info!("Loaded in mission {}", mission_name);

    std::thread::spawn(|| {
        log::info!("Spawning worker thread");
        worker::entry(config.write_dir, mission_name, rx);
    });

    Ok(())
}

#[no_mangle]
pub fn on_frame_begin(lua: &Lua, _: ()) -> LuaResult<()> {
    log::trace!("Frame {} begun!", get_lib_state().frame_count);
    increment_frame_count();
    let t = dcs::get_model_time(lua);
    let n = get_lib_state().frame_count;
    send_message(worker::Message::NewFrame(n, t));

    let ballistics = dcs::get_ballistics_objects(lua);
    send_message(worker::Message::BallisticsStateUpdate(ballistics));

    let units = dcs::get_unit_objects(lua);
    send_message(worker::Message::UnitStateUpdate(units));
    Ok(())
}

#[no_mangle]
pub fn on_frame_end(_lua: &Lua, _: ()) -> LuaResult<()> {
    Ok(())
}

#[no_mangle]
pub fn stop(_lua: &Lua, _: ()) -> LuaResult<()> {
    send_message(worker::Message::Stop);
    Ok(())
}

#[mlua::lua_module]
pub fn dcs_tetrad(lua: &Lua) -> LuaResult<LuaTable> {
    let exports = lua.create_table()?;
    exports.set("start", lua.create_function(start)?)?;
    exports.set("on_frame_begin", lua.create_function(on_frame_begin)?)?;
    exports.set("on_frame_end", lua.create_function(on_frame_end)?)?;
    exports.set("stop", lua.create_function(stop)?)?;
    Ok(exports)
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        let result = 2 + 2;
        assert_eq!(result, 4);
    }
}
