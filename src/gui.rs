use crate::dcs::{DcsWorldObject, DcsWorldUnit};
use bounded_vec_deque::BoundedVecDeque;
use egui::plot::{Corner, Legend, Line, Plot, PlotPoints};
use egui::{self, Vec2};
use std::sync::{mpsc::Receiver, Arc};
use winit::platform::windows::EventLoopBuilderExtWindows;

pub struct Gui {
    rx: &'static Receiver<Message>,
    num_units: BoundedVecDeque<i32>,
    num_ballistics: BoundedVecDeque<i32>,
    frame_times: BoundedVecDeque<f64>,
}

const PLOT_NUM_PTS: usize = 2048;

pub enum Message {
    Start(egui::Context),
    Update {
        units: Arc<Vec<DcsWorldUnit>>,
        ballistics: Arc<Vec<DcsWorldObject>>,
        game_time: f64,
    },
}

impl Gui {
    pub fn new(rx: &'static Receiver<Message>) -> Self {
        Self {
            rx,
            num_units: BoundedVecDeque::new(PLOT_NUM_PTS),
            num_ballistics: BoundedVecDeque::new(PLOT_NUM_PTS),
            frame_times: BoundedVecDeque::new(PLOT_NUM_PTS),
        }
    }

    fn handle_messages(&mut self) {
        while let Ok(msg) = self.rx.try_recv() {
            self.handle_message(msg);
        }
    }

    fn handle_message(&mut self, msg: Message) {
        match msg {
            Message::Start(_context) => {}
            Message::Update {
                units,
                ballistics,
                game_time,
            } => {
                self.num_units.push_front(units.len() as i32);
                self.num_ballistics.push_front(ballistics.len() as i32);
                self.frame_times.push_front(game_time);
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

                let u_line = make_obj_count_line(&self.num_units, &self.frame_times, "Units");
                let b_line = make_obj_count_line(
                    &self.num_ballistics,
                    &self.frame_times,
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

                let t_now = get_indexed(&self.frame_times, 0).unwrap_or(&0.0);
                let t_last = get_indexed(&self.frame_times, 1).unwrap_or(&0.0);
                let delta_t = t_now - t_last;
                ui.heading(format!("Last frame time: {:0.02} ms", delta_t * 1000.));
                ui.end_row();
                let mut times: Vec<[f64; 2]> = Vec::default();
                for idx in 1..self.frame_times.len() {
                    times.push([
                        self.frame_times[idx],
                        self.frame_times[idx - 1] - self.frame_times[idx],
                    ]);
                }
                let fps_pts: PlotPoints = times
                    .iter()
                    .map(|[t, dt]| {
                        let mut inv = 1.0 / *dt;
                        if inv.is_infinite() || inv.is_nan() {
                            inv = 0.0;
                        }
                        [*t, inv]
                    })
                    .collect();
                let time_line = Line::new(times);

                Plot::new("Frame times")
                    .width(1792.0)
                    .height(256.0)
                    .show(ui, |plot_ui| plot_ui.line(time_line));

                ui.end_row();
                let fps_line = Line::new(fps_pts);

                let fps = 1.0 / delta_t;
                ui.heading(format!("FPS: {:.2}", fps));
                ui.end_row();

                Plot::new("FPS")
                    .width(1792.0)
                    .height(256.0)
                    .show(ui, |plot_ui| plot_ui.line(fps_line));
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

pub fn run(rx: Receiver<Message>) {
    let gui_thread_entry = {
        move || loop {
            let msg = rx.recv().unwrap();
            if let Message::Start(ctx) = msg {
                do_gui(&rx, ctx);
            }
        }
    };
    std::thread::spawn(gui_thread_entry);
}
