use crate::dcs::{DcsWorldObject, DcsWorldUnit};
use bounded_vec_deque::BoundedVecDeque;
use egui::plot::{Corner, Legend, Line, Plot, PlotPoints};
use egui::{self, Vec2};
use std::sync::{
    atomic::AtomicBool,
    mpsc::{Receiver, Sender},
    Arc,
};

use winit::platform::windows::EventLoopBuilderExtWindows;

#[derive(Default)]
pub struct GuiInterface {}

pub type ArcFlag = Arc<AtomicBool>;

struct Gui {
    rx: &'static Receiver<Message>,
    num_units: BoundedVecDeque<i32>,
    num_ballistics: BoundedVecDeque<i32>,
    game_times: BoundedVecDeque<f64>,
    real_times: BoundedVecDeque<f64>,
}

const PLOT_NUM_PTS: usize = 2048;

pub enum Message {
    Start(egui::Context),
    Update {
        units: Arc<Vec<DcsWorldUnit>>,
        ballistics: Arc<Vec<DcsWorldObject>>,
        game_time: f64,
        real_time: f64,
    },
}

pub enum ClientMessage {
    ThreadStarted(ArcFlag),
}

impl Gui {
    pub fn new(rx: &'static Receiver<Message>) -> Self {
        Self {
            rx,
            num_units: BoundedVecDeque::new(PLOT_NUM_PTS),
            num_ballistics: BoundedVecDeque::new(PLOT_NUM_PTS),
            game_times: BoundedVecDeque::new(PLOT_NUM_PTS),
            real_times: BoundedVecDeque::new(PLOT_NUM_PTS),
        }
    }

    fn handle_messages(&mut self) {
        while let Ok(msg) = self.rx.try_recv() {
            self.handle_message(msg);
        }
    }

    fn handle_message(&mut self, msg: Message) {
        match msg {
            Message::Start(_context) => {
                self.num_ballistics.clear();
                self.num_units.clear();
                self.game_times.clear();
            }
            Message::Update {
                units,
                ballistics,
                game_time,
                real_time,
            } => {
                self.num_units.push_front(units.len() as i32);
                self.num_ballistics.push_front(ballistics.len() as i32);
                self.game_times.push_front(game_time);
                self.real_times.push_front(real_time);
            }
        };
    }
}

fn make_obj_count_line(v: &BoundedVecDeque<i32>, times: &BoundedVecDeque<f64>, name: &str) -> Line {
    let pts: PlotPoints = v
        .iter()
        .enumerate()
        .map(|(idx, y)| [times[idx], *y as f64])
        .collect();
    let line = Line::new(pts).name(name);
    line
}

fn get_indexed<T>(q: &BoundedVecDeque<T>, index: isize) -> Option<&T> {
    let i = if index < 0 {
        let l = q.len() as isize;
        let r = std::cmp::max(0, l + index) as usize;
        r
    } else {
        index as usize
    };
    q.get(i)
}

fn most_recent_time_delta(queue: &BoundedVecDeque<f64>) -> f64 {
    let t_now = get_indexed(queue, 0).unwrap_or(&0.0);
    let t_last = get_indexed(queue, 1).unwrap_or(&0.0);
    let delta_t = t_now - t_last;
    delta_t
}

fn make_time_line(
    ref_times: &BoundedVecDeque<f64>,
    times: &BoundedVecDeque<f64>,
    name: &str,
) -> (Line, Line) {
    let mut time_pairs: Vec<[f64; 2]> = Vec::default();
    for idx in 1..times.len() {
        time_pairs.push([ref_times[idx], times[idx - 1] - times[idx]]);
    }
    let fps_pts: PlotPoints = time_pairs
        .iter()
        .map(|[t, dt]| {
            let mut inv = 1.0 / *dt;
            if inv.is_infinite() || inv.is_nan() {
                inv = 0.0;
            }
            [*t, inv]
        })
        .collect();
    let time_line = Line::new(time_pairs).name(name);
    let fps_line = Line::new(fps_pts).name(name);
    (time_line, fps_line)
}

impl eframe::App for Gui {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            self.handle_messages();

            ui.heading("Server Monitor");

            egui::Grid::new("main_grid").show(ui, |ui| {
                ui.heading(format!(
                    "Active unit count: {}",
                    self.num_units.front().unwrap_or(&0)
                ));
                ui.end_row();
                ui.heading(format!(
                    "Active ballistics count: {}",
                    self.num_ballistics.front().unwrap_or(&0)
                ));
                ui.end_row();

                let u_line = make_obj_count_line(&self.num_units, &self.game_times, "Units");
                let b_line = make_obj_count_line(
                    &self.num_ballistics,
                    &self.game_times,
                    "Ballistic objects",
                );

                Plot::new("Objects")
                    .width(1792.0)
                    .height(256.0)
                    .legend(Legend::default().position(Corner::RightBottom))
                    .show(ui, |plot_ui| {
                        plot_ui.line(u_line);
                        plot_ui.line(b_line);
                    });
                ui.end_row();

                let last_frame_game_time_ms = most_recent_time_delta(&self.game_times) * 1000.0;
                let last_frame_real_time_ms = most_recent_time_delta(&self.real_times) * 1000.0;
                ui.heading(format!(
                    "Last frame game time: {:0.02} ms, real_time: {:0.02} ms",
                    last_frame_game_time_ms, last_frame_real_time_ms
                ));
                ui.end_row();
                let (game_time_line, game_time_fps_line) =
                    make_time_line(&self.game_times, &self.game_times, "Game time");
                let (real_time_line, _real_time_fps_line) =
                    make_time_line(&self.game_times, &self.real_times, "Real time");

                Plot::new("Frame times")
                    .width(1792.0)
                    .height(256.0)
                    .legend(Legend::default().position(Corner::RightBottom))
                    .show(ui, |plot_ui| {
                        plot_ui.line(game_time_line);
                        plot_ui.line(real_time_line);
                    });

                ui.end_row();

                let fps = 1.0 / last_frame_game_time_ms;
                ui.heading(format!("FPS: {:.2}", fps));
                ui.end_row();

                Plot::new("FPS")
                    .width(1792.0)
                    .height(256.0)
                    .show(ui, |plot_ui| plot_ui.line(game_time_fps_line));
                ui.end_row();
            });
        });
    }
}

fn do_gui(rx: &Receiver<Message>, egui_context: egui::Context) {
    let mut native_options = eframe::NativeOptions::default();
    native_options.event_loop_builder = Some(Box::new(|builder| {
        log::debug!("Calling eframe event loop hook");
        builder.with_any_thread(true);
    }));
    native_options.renderer = eframe::Renderer::Wgpu;
    native_options.context = Some(egui_context);
    native_options.initial_window_size = Some(Vec2 {
        x: 1880.0,
        y: 256.0 * 4.0,
    });
    log::info!("Spawning GUI thread");
    let rx_forever: &'static Receiver<Message> = unsafe { std::mem::transmute(rx) };

    let gui = Gui::new(rx_forever);

    eframe::run_native(
        "DCS Tetrad",
        native_options,
        Box::new(move |_cc| Box::new(gui)),
    );

    log::info!("Gui closed");
}

pub fn run(rx: Receiver<Message>, tx_to_main: Sender<ClientMessage>) {
    let is_gui_shown = ArcFlag::new(AtomicBool::new(false));

    let gui_thread_entry = {
        move || loop {
            log::debug!("Waiting for GUI start message");
            tx_to_main
                .send(ClientMessage::ThreadStarted(is_gui_shown.clone()))
                .unwrap();

            let msg = rx.recv().unwrap();
            if let Message::Start(ctx) = msg {
                log::debug!("Got a GUI start message");
                is_gui_shown.store(true, std::sync::atomic::Ordering::SeqCst);
                do_gui(&rx, ctx);
                is_gui_shown.store(false, std::sync::atomic::Ordering::SeqCst);
            }
        }
    };
    std::thread::spawn(gui_thread_entry);
}
