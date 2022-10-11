use mlua::prelude::{LuaFunction, LuaTable};
use mlua::Lua;
use serde::{Deserialize, Serialize};
use std::io::Write;

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
pub struct DcsWorldObject {
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
pub struct DcsWorldUnit {
    object: DcsWorldObject,
    unit_name: String,
    group_name: String,
}

pub trait Loggable {
    fn log_as_csv<W: Write>(self, frame_count: i32, frame_time: f64, writer: &mut csv::Writer<W>);
}

impl<'lua> DcsWorldObject {
    pub fn from_lua_with_id(id: i32, table: &LuaTable<'lua>) -> mlua::Result<Self> {
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
    pub fn from_lua_with_id(id: i32, table: LuaTable<'lua>) -> mlua::Result<Self> {
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

#[derive(Debug, Clone, Serialize)]
struct FrameObjectRecord<'a> {
    frame_count: i32,
    frame_time: f64,
    unit_name: &'a str,
    group_name: &'a str,
}

impl Loggable for DcsWorldObject {
    fn log_as_csv<W: Write>(self, frame_count: i32, frame_time: f64, writer: &mut csv::Writer<W>) {
        writer
            .serialize((
                FrameObjectRecord {
                    frame_count,
                    frame_time,
                    unit_name: "",
                    group_name: "",
                },
                self,
            ))
            .unwrap();
    }
}

impl Loggable for DcsWorldUnit {
    fn log_as_csv<W: Write>(self, frame_count: i32, frame_time: f64, writer: &mut csv::Writer<W>) {
        writer
            .serialize((
                FrameObjectRecord {
                    frame_count,
                    frame_time,
                    unit_name: self.unit_name.as_str(),
                    group_name: self.group_name.as_str(),
                },
                &self.object,
            ))
            .unwrap();
    }
}

pub fn get_model_time(lua: &Lua) -> f64 {
    let export: LuaTable = lua.globals().get("Export").unwrap();
    let get_model_time: LuaFunction = export.get("LoGetModelTime").unwrap();
    get_model_time.call::<_, f64>(()).unwrap()
}

pub fn get_lo_get_world_objects(lua: &Lua) -> LuaFunction {
    let export: LuaTable = lua.globals().get("Export").unwrap();
    export.get("LoGetWorldObjects").unwrap()
}

pub fn get_ballistics_objects(lua: &Lua) -> Vec<DcsWorldObject> {
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

pub fn get_unit_objects(lua: &Lua) -> Vec<DcsWorldUnit> {
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

pub fn get_mission_name(lua: &Lua) -> String {
    let dcs: LuaTable = lua.globals().get("DCS").unwrap();
    let get_mission_name: LuaFunction = dcs.get("getMissionName").unwrap();
    get_mission_name.call::<_, String>(()).unwrap()
}
