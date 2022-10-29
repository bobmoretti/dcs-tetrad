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
use std::time::{Duration, Instant};
use std::{fs::File, os::windows::io::FromRawHandle};
use timer::Timer;
use windows::Win32::System::Console;

mod config;
mod dcs;
mod gui;
pub mod worker;

struct FullState {
    is_gui_enabled: bool,
    worker_tx: Sender<worker::Message>,
    worker_join: JoinHandle<()>,
    gui_tx: Sender<gui::Message>,
    gui_context: Option<egui::Context>,
    is_gui_shown: Option<gui::ArcFlag>,
    rx_from_gui: Receiver<gui::ClientMessage>,
    start_time: Instant,
    gui_draw_timer: Timer,
    gui_draw_timer_guard: Option<timer::Guard>,
    gui_draw_interval: f64,
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

impl FullState {
    fn elapsed_time(&self) -> f64 {
        self.start_time.elapsed().as_secs_f64()
    }
}

fn get_elapsed_time() -> f64 {
    get_lib_state().elapsed_time()
}

fn is_gui_shown() -> bool {
    get_lib_state()
        .is_gui_shown
        .as_ref()
        .unwrap()
        .load(std::sync::atomic::Ordering::SeqCst)
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
        let cloned_config = config.clone();
        log::info!("Spawning worker thread");

        let worker_join = std::thread::spawn(move || {
            log::info!("Worker thread");
            worker::entry(config.clone(), mission_name, worker_rx);
        });
        log::info!("Setting GUI context");

        match self {
            Self::GuiStarted(gui_tx, rx, handle, gui_context) => Self::WorkerStarted(FullState {
                is_gui_enabled: cloned_config.clone().enable_gui,
                worker_tx,
                worker_join,
                gui_tx,
                gui_context,
                is_gui_shown: handle,
                rx_from_gui: rx,
                start_time: Instant::now(),
                gui_draw_timer: Timer::new(),
                gui_draw_timer_guard: None,
                gui_draw_interval: cloned_config.gui_update_interval,
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

fn is_real_time_gui() -> bool {
    get_lib_state().gui_draw_interval <= 0.0
}

fn send_gui_message(message: gui::Message) {
    if !get_lib_state().is_gui_enabled {
        return;
    }
    log::trace!("sending message to gui");
    get_lib_state().gui_tx.send(message).unwrap_or(());
    if let Some(ctx) = &get_lib_state().gui_context {
        if is_real_time_gui() {
            ctx.request_repaint();
        }
    }
}

fn start_gui(config: &config::Config) {
    if config.gui_update_interval > 0.0 {
        let repeat =
            chrono::Duration::from_std(Duration::from_secs_f64(config.gui_update_interval))
                .unwrap();
        let guard = get_lib_state()
            .gui_draw_timer
            .schedule_repeating(repeat, || {
                log::trace!("Timer fired");
                if is_gui_shown() {
                    get_lib_state()
                        .gui_context
                        .as_ref()
                        .unwrap()
                        .request_repaint();
                }
            });
        get_lib_state().gui_draw_timer_guard = Some(guard)
    }

    if is_gui_shown() {
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
        start_gui(&config);
    }

    Ok(0)
}

#[no_mangle]
pub fn on_frame_begin(lua: &Lua, _: ()) -> LuaResult<()> {
    let real_time = get_elapsed_time();
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
        real_time: real_time,
    };
    let gui_msg = gui::Message::Update {
        units: units.clone(),
        ballistics: ballistics.clone(),
        game_time: t,
        real_time: real_time,
    };

    send_worker_message(worker_msg);
    if is_gui_shown() {
        send_gui_message(gui_msg);
    }
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
