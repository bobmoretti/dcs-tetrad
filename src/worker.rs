use crate::dcs;
use crate::dcs::DcsWorldObject;
use crate::dcs::DcsWorldUnit;
use std::fs::File;
use std::path::Path;
use std::sync::mpsc::Receiver;
use zstd::stream::write::Encoder as ZstdEncoder;

pub enum Message {
    NewFrame(i32, f64),
    BallisticsStateUpdate(Vec<DcsWorldObject>),
    UnitStateUpdate(Vec<DcsWorldUnit>),
    Stop,
}

impl std::fmt::Debug for Message {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NewFrame(arg0, arg1) => {
                f.debug_tuple("NewFrame").field(arg0).field(arg1).finish()
            }
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

pub fn entry(write_dir: String, mission_name: String, rx: Receiver<Message>) {
    let mut most_recent_time: f64 = 0.0;
    let mut frame_count: i32 = 0;
    let dir_name = Path::new(write_dir.as_str()).join("Logs").join("Tetrad");
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
    let mut encoder = ZstdEncoder::new(csv_file, 10).unwrap();
    let mut csv_writer = csv::WriterBuilder::new()
        .has_headers(false)
        .from_writer(&mut encoder);

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
                    dcs::log_object(frame_count, most_recent_time, &mut csv_writer, &obj);
                }
            }
            Message::UnitStateUpdate(objects) => {
                log::trace!("Logging Units message with {} elements", objects.len());
                for obj in objects.into_iter() {
                    dcs::log_unit(frame_count, most_recent_time, &mut csv_writer, &obj);
                }
            }
            Message::Stop => {
                log::debug!("Stopping!");
                break;
            }
        }
    }
    log::debug!("finishing csv file!");
    csv_writer.flush().unwrap();
}
