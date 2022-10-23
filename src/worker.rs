use crate::config::Config;
use crate::dcs;
use crate::dcs::DcsWorldObject;
use crate::dcs::DcsWorldUnit;
use std::fs::File;
use std::path::Path;
use std::sync::mpsc::Receiver;
use zstd::stream::write::Encoder as ZstdEncoder;

pub enum Message {
    NewFrame(f64),
    BallisticsStateUpdate(Vec<DcsWorldObject>),
    UnitStateUpdate(Vec<DcsWorldUnit>),
    Stop,
}

impl std::fmt::Debug for Message {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NewFrame(arg0) => f.debug_tuple("NewFrame").field(arg0).finish(),
            Self::BallisticsStateUpdate(objs) => f.write_fmt(format_args!(
                "BallisticsStateUpdate with {} objects",
                objs.len()
            )),
            Self::UnitStateUpdate(units) => {
                f.write_fmt(format_args!("UnitStateUpdate with {} objects", units.len()))
            }
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
    writer: &mut csv::Writer<W>,
    objects: Vec<T>,
) {
    for obj in objects.into_iter() {
        obj.log_as_csv(frame_count, t, writer);
    }
}

fn finish<W: std::io::Write>(obj: &mut Option<csv::Writer<W>>) {
    if let Some(ref mut writer) = obj {
        writer.flush().unwrap();
    }
}

fn log_frame(writer: &mut csv::Writer<zstd::Encoder<'_, File>>, t1: f64, t0: f64, n: i32) {
    if t0 != t1 {
        writer.write_field((n - 1).to_string()).unwrap();
        writer.write_field((t1 - t0).to_string()).unwrap();
        writer.write_record(None::<&[u8]>).unwrap();
    }
}

pub fn entry(config: Config, mission_name: String, rx: Receiver<Message>) {
    let mut prev_frame_time: f64;
    let mut most_recent_time: f64 = 0.0;
    let mut frame_count: i32 = 0;
    let log_dir = Path::new(config.write_dir.as_str())
        .join("Logs")
        .join("Tetrad");
    log::debug!("Starting with config {:?}", config);

    let mut object_writer = if config.enable_object_log {
        let writer = create_csv_file(&mission_name, &log_dir.join("objects"));
        Some(writer)
    } else {
        None
    };

    let mut frame_writer = if config.enable_framerate_log {
        let writer = create_csv_file(&mission_name, &log_dir.join("frames"));
        Some(writer)
    } else {
        None
    };
    // fuck the static analyzer... ugh
    // let logs = [&object_writer, &frame_writer];

    loop {
        log::trace!("Waiting for message");
        let msg = rx.recv().expect("Should be able to receive a message");
        match msg {
            Message::NewFrame(t) => {
                frame_count += 1;
                log::trace!("New frame message, n = {}, t = {}", frame_count, t);
                prev_frame_time = most_recent_time;
                most_recent_time = t;
                if let Some(ref mut w) = frame_writer {
                    log_frame(w, t, prev_frame_time, frame_count);
                }
            }
            Message::BallisticsStateUpdate(objects) => {
                log::trace!("Logging Ballistics message with {} elements", objects.len());
                if let Some(ref mut writer) = object_writer {
                    log_dcs_objects(frame_count, most_recent_time, writer, objects);
                }
            }
            Message::UnitStateUpdate(objects) => {
                log::trace!("Logging Units message with {} elements", objects.len());
                if let Some(ref mut writer) = object_writer {
                    log_dcs_objects(frame_count, most_recent_time, writer, objects)
                }
            }
            Message::Stop => {
                log::debug!("Stopping!");
                break;
            }
        }
    }
    log::debug!("finishing csv files!");

    finish(&mut object_writer);
    finish(&mut frame_writer);
}
