use crate::config::Config;
use crate::dcs;
use crate::dcs::DcsWorldObject;
use crate::dcs::DcsWorldUnit;
use std::fs::File;
use std::path::Path;
use std::sync::{mpsc::Receiver, Arc};
use zstd::stream::write::Encoder as ZstdEncoder;

pub enum Message {
    Update {
        units: Arc<Vec<DcsWorldUnit>>,
        ballistics: Arc<Vec<DcsWorldObject>>,
        game_time: f64,
        real_time: f64,
    },
    Stop,
}

impl std::fmt::Debug for Message {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Update {
                units,
                ballistics,
                game_time,
                real_time: _,
            } => f.write_fmt(format_args!(
                "Update at t={} with {} units and {} ballistics objects",
                game_time,
                units.len(),
                ballistics.len()
            )),
            Self::Stop => write!(f, "Stop"),
        }
    }
}

fn format_now() -> String {
    let date = chrono::Local::now();
    date.format("%Y-%m-%d %H-%M-%S").to_string()
}

fn create_csv_file(mission_name: &str, dir_name: &Path) -> csv::Writer<ZstdEncoder<'static, File>> {
    std::fs::create_dir_all(&dir_name).unwrap();

    let fname = dir_name.join(format!("{} - {}.csv.zstd", mission_name, format_now()));
    log::debug!("Trying to open csv file: {:?}", fname);

    let csv_file = match File::create(&fname) {
        Err(why) => {
            log::error!("Couldn't open file {:?} because {}", fname, why);
            panic!("failed")
        }
        Ok(file) => file,
    };
    let encoder = ZstdEncoder::new(csv_file, 10).unwrap();
    let csv_writer = csv::WriterBuilder::new()
        .has_headers(false)
        .from_writer(encoder);
    csv_writer
}

fn log_dcs_objects<W: std::io::Write, T: dcs::Loggable>(
    frame_count: i32,
    t: f64,
    real_time: f64,
    writer: &mut csv::Writer<W>,
    objects: &[T],
) {
    for obj in objects.into_iter() {
        obj.log_as_csv(frame_count, t, real_time, writer);
    }
}

fn finish<W: std::io::Write>(obj: &mut Option<csv::Writer<W>>) {
    if let Some(ref mut writer) = obj {
        writer.flush().unwrap();
    }
}

fn log_frame(
    writer: &mut csv::Writer<zstd::Encoder<'_, File>>,
    game_time: f64,
    real_time: f64,
    n: i32,
    num_units: i32,
    num_ballistics: i32,
) {
    writer.write_field((n - 1).to_string()).unwrap();
    writer.write_field(format!("{:.8}", game_time)).unwrap();
    writer.write_field(format!("{:.8}", real_time)).unwrap();
    writer.write_field(num_units.to_string()).unwrap();
    writer.write_field(num_ballistics.to_string()).unwrap();
    writer.write_record(None::<&[u8]>).unwrap();
}

type OutputWriter = csv::Writer<ZstdEncoder<'static, File>>;

struct Logger {
    prev_game_time: f64,
    most_recent_game_time: f64,
    current_real_time: f64,
    frame_count: i32,
    frame_writer: Option<OutputWriter>,
    object_writer: Option<OutputWriter>,
}

impl Logger {
    fn new(frame_writer: Option<OutputWriter>, object_writer: Option<OutputWriter>) -> Self {
        Self {
            prev_game_time: 0.0,
            current_real_time: 0.0,
            most_recent_game_time: 0.0,
            frame_count: 0,
            frame_writer,
            object_writer,
        }
    }

    fn log_frame(&mut self, t: f64, units: &[DcsWorldUnit], ballistics: &[DcsWorldObject]) {
        log_frame(
            self.frame_writer.as_mut().unwrap(),
            t,
            self.current_real_time,
            self.frame_count,
            units.len() as i32,
            ballistics.len() as i32,
        );
    }

    fn log_objects(&mut self, units: &[DcsWorldUnit], ballistics: &[DcsWorldObject]) {
        log::trace!("Logging Units message with {} elements", units.len());
        let n = self.frame_count;
        let t = self.most_recent_game_time;
        log_dcs_objects(
            n,
            t,
            self.current_real_time,
            self.object_writer.as_mut().unwrap(),
            units,
        );

        log::trace!(
            "Logging Ballistics message with {} elements",
            ballistics.len()
        );
        log_dcs_objects(
            n,
            t,
            self.current_real_time,
            self.object_writer.as_mut().unwrap(),
            ballistics,
        );
    }

    fn handle_update(
        &mut self,
        units: &Vec<DcsWorldUnit>,
        ballistics: &Vec<DcsWorldObject>,
        game_time: f64,
        real_time: f64,
    ) {
        let n = self.frame_count;
        log::trace!("New frame message, n = {}, t = {}", n, game_time);

        self.prev_game_time = self.most_recent_game_time;
        self.most_recent_game_time = game_time;
        self.current_real_time = real_time;
        if self.frame_writer.is_some() {
            self.log_frame(game_time, units.as_slice(), &ballistics.as_slice());
        }
        if self.object_writer.is_some() {
            self.log_objects(units.as_slice(), ballistics.as_slice());
        }
    }

    fn handle_message(&mut self, msg: Message) -> bool {
        match msg {
            Message::Update {
                units,
                ballistics,
                game_time,
                real_time,
            } => {
                self.handle_update(&units, &ballistics, game_time, real_time);
            }
            Message::Stop => {
                log::debug!("Stopping!");
                return true;
            }
        }
        false
    }

    fn finish(&mut self) {
        finish(&mut self.object_writer);
        finish(&mut self.frame_writer);
    }
}

pub fn entry(config: Config, mission_name: String, rx: Receiver<Message>) {
    let log_dir = Path::new(config.write_dir.as_str())
        .join("Logs")
        .join("Tetrad");

    let frame_writer = if config.enable_framerate_log {
        let writer = create_csv_file(&mission_name, &log_dir.join("frames"));
        Some(writer)
    } else {
        None
    };

    let object_writer = if config.enable_object_log {
        let writer = create_csv_file(&mission_name, &log_dir.join("objects"));
        Some(writer)
    } else {
        None
    };

    let mut logger = Logger::new(frame_writer, object_writer);
    log::debug!("Starting with config {:?}", config);

    loop {
        log::trace!("Waiting for message");
        let msg = rx.recv().expect("Should be able to receive a message");
        let done = logger.handle_message(msg);
        if done {
            break;
        }
    }
    log::debug!("finishing csv files!");
    logger.finish();
}
