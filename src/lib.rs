use mlua::prelude::{LuaResult, LuaTable};
use mlua::Lua;
use serde::{Deserialize, Serialize};
use std::fs::File;
use std::path::Path;
use std::sync::mpsc::{Receiver, Sender};
use zstd::stream::write::Encoder as ZstdEncoder;

mod dcs;
use dcs::DcsWorldObject;
use dcs::DcsWorldUnit;

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


#[derive(Debug)]
enum Message {
    NewFrame(i32, f64),
    BallisticsStateUpdate(Vec<DcsWorldObject>),
    UnitStateUpdate(Vec<DcsWorldUnit>),
    Stop,
}

struct LibState {
    // time before integer overflow > 1 year @ 120 FPS
    frame_count: i32,
    tx: Sender<Message>,
}

static mut LIB_STATE: Option<LibState> = None;

fn get_lib_state() -> &'static mut LibState {
    unsafe { LIB_STATE.as_mut().expect("msg") }
}

fn increment_frame_count() {
    get_lib_state().frame_count += 1;
}


fn send_message(message: Message) {
    log::debug!("sending message {:?}", message);
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
pub fn start(_: &Lua, config: Config) -> LuaResult<()> {
    init(&config);
    log::info!("Creating channel");

    let (tx, rx) = std::sync::mpsc::channel();

    unsafe {
        LIB_STATE = Some(LibState {
            frame_count: 0,
            tx: tx,
        });
    }

    std::thread::spawn(|| {
        log::info!("Spawning worker thread");
        worker_entry(config.write_dir, rx);
    });

    Ok(())
}

#[no_mangle]
pub fn on_frame_begin(lua: &Lua, _: ()) -> LuaResult<()> {
    log::trace!("Frame {} begun!", get_lib_state().frame_count);
    increment_frame_count();
    let t = dcs::get_model_time(lua);
    let n = get_lib_state().frame_count;
    send_message(Message::NewFrame(n, t));

    let ballistics = dcs::get_ballistics_objects(lua);
    send_message(Message::BallisticsStateUpdate(ballistics));

    let units = dcs::get_unit_objects(lua);
    send_message(Message::UnitStateUpdate(units));
    Ok(())
}

#[no_mangle]
pub fn on_frame_end(_lua: &Lua, _: ()) -> LuaResult<()> {
    Ok(())
}

#[no_mangle]
pub fn stop(_lua: &Lua, _: ()) -> LuaResult<()> {
    send_message(Message::Stop);
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

fn worker_entry(write_dir: String, rx: Receiver<Message>) {
    let mut most_recent_time: f64 = 0.0;
    let mut frame_count: i32 = 0;
    let dir_name = Path::new(write_dir.as_str());
    let fname = dir_name.join("tetrad_frame_log.csv.zstd");
    log::debug!("Trying to open csv file: {:?}", fname);

    let csv_file = match File::create(&fname) {
        Err(why) => {
            log::error!("Couldn't open file {:?} because {}", fname, why);
            panic!("failed")
        }
        Ok(file) => file,
    };
    let mut encoder = ZstdEncoder::new(csv_file, 10).unwrap();

    loop {
        log::trace!("Waiting for message");
        let msg = rx.recv().expect("Should be able to receive a message");
        match msg {
            Message::NewFrame(n, t) => {
                most_recent_time = t;
                frame_count = n;
            }
            Message::BallisticsStateUpdate(objects) => {
                log::trace!("Logging Ballistics message with {} elements", objects.len());
                for obj in objects.into_iter() {
                    dcs::log_object(frame_count, most_recent_time, &mut encoder, &obj);
                }
            }
            Message::UnitStateUpdate(objects) => {
                log::trace!("Logging Units message with {} elements", objects.len());
                for obj in objects.into_iter() {
                    dcs::log_unit(frame_count, most_recent_time, &mut encoder, &obj);
                }
            }
            Message::Stop => {
                break;
            }
        }
    }
    encoder.finish().unwrap();
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        let result = 2 + 2;
        assert_eq!(result, 4);
    }
}
