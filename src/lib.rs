use fern::colors::{Color, ColoredLevelConfig};
use mlua::prelude::{LuaResult, LuaTable};
use mlua::Lua;
use std::io::Write;
use std::path::Path;
use std::sync::{
    mpsc::{Receiver, Sender},
    Arc,
};
use std::thread::JoinHandle;
use std::{fs::File, os::windows::io::FromRawHandle};
use windows::Win32::System::Console;

mod config;
mod dcs;
mod gui;
pub mod worker;

struct FullState {
    worker_tx: Sender<worker::Message>,
    worker_join: JoinHandle<()>,
    gui_tx: Sender<gui::Message>,
    gui_context: Option<egui::Context>,
    is_gui_shown: Option<gui::ArcFlag>,
    rx_from_gui: Receiver<gui::ClientMessage>,
}

enum LibState {
    GuiStarted(
        Sender<gui::Message>,
        Receiver<gui::ClientMessage>,
        Option<gui::ArcFlag>,
        Option<egui::Context>,
    ),
    WorkerStarted(FullState),
}

fn setup_logging(config: &config::Config, console: File) -> Result<(), fern::InitError> {
    let colors_line = ColoredLevelConfig::new()
        .error(Color::Red)
        .warn(Color::Yellow)
        // we actually don't need to specify the color for debug and info, they are white by default
        .info(Color::White)
        .debug(Color::White)
        // depending on the terminals color scheme, this is the same as the background color
        .trace(Color::BrightBlack);

    let colors_level = colors_line.clone().info(Color::Green);

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

    fern::Dispatch::new()
        .format(move |out, message, record| {
            out.finish(format_args!(
                "{color_line}[{date}][{target}][{level}{color_line}] {message}\x1B[0m",
                color_line = format_args!(
                    "\x1B[{}m",
                    colors_line.get_color(&record.level()).to_fg_str()
                ),
                date = chrono::Local::now().format("%Y-%m-%d %H:%M:%S"),
                target = record.target(),
                level = colors_level.color(record.level()),
                message = message,
            ));
        })
        .level(level)
        .level_for("wgpu_core", LevelFilter::Warn)
        .level_for("naga", LevelFilter::Info)
        .chain(
            std::fs::OpenOptions::new()
                .write(true)
                .create(true)
                .open(p)?,
        )
        .chain(console)
        .apply()?;

    log_panics::init();
    log::info!("Initialization of logging complete!");

    Ok(())
}

fn create_console() -> windows::core::Result<File> {
    unsafe {
        Console::AllocConsole();
        let h_stdout = Console::GetStdHandle(Console::STD_OUTPUT_HANDLE)?;
        Ok(File::from_raw_handle(h_stdout.0 as *mut libc::c_void))
    }
}

fn wait_for_gui_started(rx_from_gui: &Receiver<gui::ClientMessage>) -> gui::ArcFlag {
    let gui::ClientMessage::ThreadStarted(h) = rx_from_gui.recv().unwrap();
    h
}

impl LibState {
    fn init(config: &config::Config) -> LuaResult<Self> {
        let mut console_out = match create_console() {
            Err(e) => {
                return Err(mlua::Error::RuntimeError(
                    format!("Couldn't create console, very sad. Error was {:#?}", e).into(),
                ));
            }
            Ok(f) => f,
        };
        writeln!(
            console_out,
            "Console creation complete, setting up logging."
        )
        .unwrap();
        if let Err(_e) = setup_logging(&config, console_out) {
            return Err(mlua::Error::RuntimeError(
                "Couldn't set up logging, very sad.".into(),
            ));
        }
        log::info!("Starting library");
        log::info!("Loading DCS tetrad version {}", env!("CARGO_PKG_VERSION"));

        let (gui_tx, gui_rx) = std::sync::mpsc::channel();
        let (tx_to_main, rx_from_gui) = std::sync::mpsc::channel();
        if config.enable_gui {
            log::debug!("Calling gui::run");
            gui::run(gui_rx, tx_to_main);
        }

        let handle = if config.enable_gui {
            log::debug!("waiting for GUI to start");
            Some(wait_for_gui_started(&rx_from_gui))
        } else {
            None
        };

        let state =
            LibState::GuiStarted(gui_tx, rx_from_gui, handle, Some(egui::Context::default()));

        Ok(state)
    }

    fn init_session(self, config: config::Config, mission_name: String) -> Self {
        let (worker_tx, worker_rx) = std::sync::mpsc::channel();
        log::info!("Spawning worker thread");

        let worker_join = std::thread::spawn(|| {
            log::info!("Worker thread");
            worker::entry(config, mission_name, worker_rx);
        });
        log::info!("Setting GUI context");

        match self {
            Self::GuiStarted(gui_tx, rx, handle, gui_context) => Self::WorkerStarted(FullState {
                worker_tx,
                worker_join,
                gui_tx,
                gui_context,
                is_gui_shown: handle,
                rx_from_gui: rx,
            }),
            Self::WorkerStarted { .. } => panic!("Worker already started"),
        }
    }
}

impl<'lua> mlua::FromLua<'lua> for config::Config {
    fn from_lua(lua_value: mlua::Value<'lua>, lua: &'lua mlua::Lua) -> mlua::Result<Self> {
        use mlua::LuaSerdeExt;
        let config: config::Config = lua.from_value(lua_value)?;
        Ok(config)
    }
}

static mut LIB_STATE: Option<LibState> = None;

fn get_lib_state() -> &'static mut FullState {
    if let Some(LibState::WorkerStarted(fs)) = unsafe { LIB_STATE.as_mut() } {
        fs
    } else {
        panic!("Attempted to get lib full state before it was initialized.");
    }
}

fn send_worker_message(message: worker::Message) {
    log::trace!("sending message {:?} to worker", message);
    get_lib_state()
        .worker_tx
        .send(message)
        .expect("Should be able to send message");
}

fn send_gui_message(message: gui::Message) {
    log::trace!("sending message to gui");
    get_lib_state().gui_tx.send(message).unwrap();
    if let Some(ctx) = &get_lib_state().gui_context {
        ctx.request_repaint();
    }
}

#[no_mangle]
pub fn start(lua: &Lua, config: config::Config) -> LuaResult<i32> {
    unsafe {
        if LIB_STATE.is_none() {
            LIB_STATE = Some(LibState::init(&config)?);
        }
    }
    let mission_name = dcs::get_mission_name(lua);
    log::info!("Loaded in mission {}", mission_name);

    unsafe {
        LIB_STATE = Some(
            LIB_STATE
                .take()
                .unwrap()
                .init_session(config.clone(), mission_name),
        );
    }

    if config.enable_gui {
        if get_lib_state()
            .is_gui_shown
            .as_ref()
            .unwrap()
            .load(std::sync::atomic::Ordering::SeqCst)
        {
            let ctx = get_lib_state().gui_context.clone();
            log::debug!("Starting GUI");
            send_gui_message(gui::Message::Start(ctx.unwrap()));
        } else {
            log::debug!("GUI already running, not starting a new GUI");
            send_gui_message(gui::Message::Start(
                get_lib_state().gui_context.clone().unwrap(),
            ));
        }
    }

    Ok(0)
}

#[no_mangle]
pub fn on_frame_begin(lua: &Lua, _: ()) -> LuaResult<()> {
    if dcs::is_paused(lua) {
        log::trace!("DCS is paused");
        return Ok(());
    }
    log::trace!("Frame begun");

    let t = dcs::get_model_time(lua);
    let ballistics = Arc::new(dcs::get_ballistics_objects(lua));
    let units = Arc::new(dcs::get_unit_objects(lua));
    let worker_msg = worker::Message::Update {
        units: units.clone(),
        ballistics: ballistics.clone(),
        game_time: t,
    };
    let gui_msg = gui::Message::Update {
        units: units.clone(),
        ballistics: ballistics.clone(),
        game_time: t,
    };

    send_worker_message(worker_msg);
    send_gui_message(gui_msg);
    Ok(())
}

#[no_mangle]
pub fn on_frame_end(_lua: &Lua, _: ()) -> LuaResult<()> {
    Ok(())
}

#[no_mangle]
pub fn stop(_lua: &Lua, _: ()) -> LuaResult<()> {
    send_worker_message(worker::Message::Stop);
    if let Some(LibState::WorkerStarted(state)) = unsafe { LIB_STATE.take() } {
        state.worker_join.join().unwrap();
        unsafe {
            LIB_STATE = Some(LibState::GuiStarted(
                state.gui_tx,
                state.rx_from_gui,
                state.is_gui_shown,
                state.gui_context,
            ))
        };
    } else {
        panic!("Worker wasn't running!")
    }
    log::logger().flush();
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
