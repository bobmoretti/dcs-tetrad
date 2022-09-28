use mlua::prelude::{LuaFunction, LuaResult, LuaTable};
use mlua::Lua;
use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::Write;
use std::path::Path;
use std::sync::mpsc::{Receiver, Sender};
use zstd::stream::write::Encoder as ZstdEncoder;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Config {
    pub write_dir: String,
    pub lua_path: String,
    pub dll_path: String,
    #[serde(default)]
    pub debug: bool,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct LatLonAlt {
    lat: f64,
    lon: f64,
    alt: f64,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct DcsPosition {
    x: f64,
    y: f64,
    z: f64,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct DcsWorldObject {
    id: i32,
    name: String,
    country: i32,
    coalition: String,
    coalition_id: i32,
    lat_lon_alt: LatLonAlt,
    heading: f64,
    pitch: f64,
    bank: f64,
    position: DcsPosition,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct DcsWorldUnit {
    object: DcsWorldObject,
    unit_name: String,
    group_name: String,
}

impl<'lua> mlua::FromLua<'lua> for Config {
    fn from_lua(lua_value: mlua::Value<'lua>, lua: &'lua mlua::Lua) -> mlua::Result<Self> {
        use mlua::LuaSerdeExt;
        let config: Config = lua.from_value(lua_value)?;
        Ok(config)
    }
}

impl<'lua> DcsWorldObject {
    fn from_lua_with_id(id: i32, table: &LuaTable<'lua>) -> mlua::Result<Self> {
        let lat_lon_alt = match table.get("LatLongAlt").unwrap() {
            mlua::Value::Table(t) => t,
            _ => panic!(""),
        };

        let position = match table.get("Position").unwrap() {
            mlua::Value::Table(t) => t,
            _ => panic!(""),
        };

        let lat_lon_alt = LatLonAlt {
            lat: lat_lon_alt.get("Lat").unwrap(),
            lon: lat_lon_alt.get("Long").unwrap(),
            alt: lat_lon_alt.get("Alt").unwrap(),
        };

        let pos = DcsPosition {
            x: position.get("x").unwrap(),
            y: position.get("y").unwrap(),
            z: position.get("z").unwrap(),
        };

        Ok(Self {
            id: id,
            name: table.get("Name").unwrap(),
            country: table.get("Country").unwrap(),
            coalition: table.get("Coalition").unwrap(),
            coalition_id: table.get("CoalitionID").unwrap(),
            lat_lon_alt: lat_lon_alt,
            heading: table.get("Heading").unwrap(),
            pitch: table.get("Pitch").unwrap(),
            bank: table.get("Bank").unwrap(),
            position: pos,
        })
    }
}

impl<'lua> DcsWorldUnit {
    fn from_lua_with_id(id: i32, table: LuaTable<'lua>) -> mlua::Result<Self> {
        let unit_name: String = match table.get("UnitName") {
            Err(_e) => "NoName".to_string(),
            Ok(val) => val,
        };

        let group_name: String = match table.get("GroupName") {
            Err(_e) => "NoName".to_string(),
            Ok(val) => val,
        };

        Ok(Self {
            object: DcsWorldObject::from_lua_with_id(id, &table).unwrap(),
            unit_name: unit_name,
            group_name: group_name,
        })
    }
}

#[derive(Debug)]
enum Message {
    NewFrame(i32, f64),
    BallisticsStateUpdate(Vec<DcsWorldObject>),
    UnitStateUpdate(Vec<DcsWorldUnit>),
    Stop,
}

struct LuaInterface {
    // time before integer overflow > 1 year @ 120 FPS
    frame_count: i32,
    tx: Sender<Message>,
}

fn increment_frame_count() {
    get_lua_interface().frame_count += 1;
}

static mut LUA_INTERFACE: Option<LuaInterface> = None;

fn get_lua_interface() -> &'static mut LuaInterface {
    unsafe { LUA_INTERFACE.as_mut().expect("msg") }
}

fn send_message(message: Message) {
    log::debug!("sending message {:?}", message);
    get_lua_interface()
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
        LUA_INTERFACE = Some(LuaInterface {
            frame_count: 0,
            tx: tx,
        });
    }

    std::thread::spawn(|| {
        log::info!("Spawning thread");
        worker_entry(config.write_dir, rx);
    });

    Ok(())
}

#[no_mangle]
pub fn on_frame_begin(lua: &Lua, _: ()) -> LuaResult<()> {
    log::trace!("Frame {} begun!", get_lua_interface().frame_count);
    increment_frame_count();
    let t = get_model_time(lua);
    let n = get_lua_interface().frame_count;
    send_message(Message::NewFrame(n, t));

    let ballistics = get_ballistics_objects(lua);
    send_message(Message::BallisticsStateUpdate(ballistics));

    let units = get_unit_objects(lua);
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

fn log_object<T: Write>(frame_count: i32, frame_time: f64, file: &mut T, o: &DcsWorldObject) {
    write!(
        file,
        "{}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {},,\n",
        frame_count,
        frame_time,
        o.name,
        o.country,
        o.coalition,
        o.coalition_id,
        o.lat_lon_alt.lat,
        o.lat_lon_alt.lon,
        o.lat_lon_alt.alt,
        o.heading,
        o.pitch,
        o.bank,
        o.position.x,
        o.position.y,
        o.position.z
    )
    .unwrap();
}

fn log_unit<T: Write>(frame_count: i32, frame_time: f64, file: &mut T, unit: &DcsWorldUnit) {
    write!(
        file,
        "{}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {},{},{}\n",
        frame_count,
        frame_time,
        unit.object.name,
        unit.object.country,
        unit.object.coalition,
        unit.object.coalition_id,
        unit.object.lat_lon_alt.lat,
        unit.object.lat_lon_alt.lon,
        unit.object.lat_lon_alt.alt,
        unit.object.heading,
        unit.object.pitch,
        unit.object.bank,
        unit.object.position.x,
        unit.object.position.y,
        unit.object.position.z,
        unit.unit_name,
        unit.group_name,
    )
    .unwrap();
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
                    log_object(frame_count, most_recent_time, &mut encoder, &obj);
                }
            }
            Message::UnitStateUpdate(objects) => {
                log::trace!("Logging Units message with {} elements", objects.len());
                for obj in objects.into_iter() {
                    log_unit(frame_count, most_recent_time, &mut encoder, &obj);
                }
            }
            Message::Stop => {
                break;
            }
        }
    }
    encoder.finish().unwrap();
}

fn get_model_time(lua: &Lua) -> f64 {
    let get_model_time: LuaFunction = lua.globals().get("LoGetModelTime").unwrap();
    get_model_time.call::<_, f64>(()).unwrap()
}

fn get_lo_get_world_objects(lua: &Lua) -> LuaFunction {
    lua.globals().get("LoGetWorldObjects").unwrap()
}

fn get_ballistics_objects(lua: &Lua) -> Vec<DcsWorldObject> {
    let lo_get_world_objects = get_lo_get_world_objects(lua);
    let table = lo_get_world_objects
        .call::<_, LuaTable>("ballistic")
        .unwrap();
    let mut v: Vec<DcsWorldObject> = Vec::new();
    for pair in table.pairs::<i32, LuaTable>() {
        let (key, value) = pair.unwrap();
        v.push(DcsWorldObject::from_lua_with_id(key, &value).unwrap());
    }
    log::trace!("got {} ballistics elements", v.len());
    v
}

fn get_unit_objects(lua: &Lua) -> Vec<DcsWorldUnit> {
    let lo_get_world_objects = get_lo_get_world_objects(lua);
    let table = lo_get_world_objects.call::<_, LuaTable>(()).unwrap();
    let mut v: Vec<DcsWorldUnit> = Vec::new();
    for pair in table.pairs::<i32, LuaTable>() {
        let (key, value) = pair.unwrap();
        v.push(DcsWorldUnit::from_lua_with_id(key, value).unwrap());
    }
    log::trace!("got {} unit elements", v.len());
    v
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        let result = 2 + 2;
        assert_eq!(result, 4);
    }
}
