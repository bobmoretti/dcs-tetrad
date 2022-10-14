use mlua::prelude::{LuaResult, LuaTable};
use mlua::Lua;
use std::path::Path;
use std::sync::mpsc::Sender;
mod config;
mod dcs;
mod gui;
pub mod worker;

struct LibState {
    // time before integer overflow > 1 year @ 120 FPS
    worker_tx: Sender<worker::Message>,
    gui_tx: Sender<gui::Message>,
}

impl<'lua> mlua::FromLua<'lua> for config::Config {
    fn from_lua(lua_value: mlua::Value<'lua>, lua: &'lua mlua::Lua) -> mlua::Result<Self> {
        use mlua::LuaSerdeExt;
        let config: config::Config = lua.from_value(lua_value)?;
        Ok(config)
    }
}

static mut LIB_STATE: Option<LibState> = None;

fn get_lib_state() -> &'static mut LibState {
    unsafe { LIB_STATE.as_mut().expect("msg") }
}

fn send_worker_message(message: worker::Message) {
    log::trace!("sending message {:?} to worker", message);
    get_lib_state()
        .worker_tx
        .send(message)
        .expect("Should be able to send message");
}

fn send_gui_message(message: gui::Message) {
    log::trace!("sending message to gui: {:?}", message);
    get_lib_state().gui_tx.send(message).unwrap();
}

fn setup_logging(config: &config::Config) {
    use log::LevelFilter;
    let level = if config.debug {
        LevelFilter::Debug
    } else {
        LevelFilter::Info
    };

    let logdir = Path::new(config.write_dir.as_str())
        .join("Logs")
        .join("Tetrad");

    std::fs::create_dir_all(&logdir).unwrap();
    let p = logdir.join("dcs_tetrad.log");
    simple_logging::log_to_file(p, level).unwrap();

    log_panics::init();
}

fn init(config: &config::Config) {
    static mut FIRST_TIME: bool = true;
    unsafe {
        if FIRST_TIME {
            setup_logging(config);
            FIRST_TIME = false;
        }
    }
    log::info!("Initialization complete!");
}

#[no_mangle]
pub fn start(lua: &Lua, config: config::Config) -> LuaResult<()> {
    unsafe {
        if LIB_STATE.is_some() {
            log::info!("Called start() with library already created");
            return Ok(());
        }
    }

    log::info!("Starting library");

    init(&config);
    log::info!("Creating channel");

    let (gui_tx, gui_rx) = std::sync::mpsc::channel();
    if config.enable_gui {
        gui::run(gui_rx);
    }

    let (worker_tx, worker_rx) = std::sync::mpsc::channel();

    unsafe {
        LIB_STATE = Some(LibState {
            worker_tx: worker_tx,
            gui_tx: gui_tx,
        });
    }

    let mission_name = dcs::get_mission_name(lua);
    log::info!("Loaded in mission {}", mission_name);
    let worker_cfg = config.clone();

    std::thread::spawn(|| {
        log::info!("Spawning worker thread");
        worker::entry(worker_cfg, mission_name, worker_rx);
    });

    Ok(())
}

#[no_mangle]
pub fn on_frame_begin(lua: &Lua, _: ()) -> LuaResult<()> {
    log::trace!("Frame begun!");
    let t = dcs::get_model_time(lua);
    send_worker_message(worker::Message::NewFrame(t));

    let ballistics = dcs::get_ballistics_objects(lua);
    send_worker_message(worker::Message::BallisticsStateUpdate(ballistics));

    let units = dcs::get_unit_objects(lua);
    send_worker_message(worker::Message::UnitStateUpdate(units));
    Ok(())
}

#[no_mangle]
pub fn on_frame_end(_lua: &Lua, _: ()) -> LuaResult<()> {
    Ok(())
}

#[no_mangle]
pub fn stop(_lua: &Lua, _: ()) -> LuaResult<()> {
    send_worker_message(worker::Message::Stop);
    unsafe {
        LIB_STATE = None;
    }
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
