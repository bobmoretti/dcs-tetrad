use crate::dcs::DcsWorldUnit;
use egui;
use std::sync::mpsc::Receiver;
use winit::platform::windows::EventLoopBuilderExtWindows;

pub struct Gui {
    rx: &'static Receiver<Message>,
    num_objects: i32,
}

#[derive(Debug, Default)]
pub enum Message {
    #[default]
    Start,
    Update(Vec<DcsWorldUnit>),
}

impl Gui {
    pub fn new(rx: &'static Receiver<Message>) -> Self {
        Self { rx, num_objects: 0 }
    }

    fn handle_messages(&mut self) {
        while let Ok(msg) = self.rx.try_recv() {
            self.handle_message(msg);
        }
    }

    fn handle_message(&mut self, msg: Message) {
        match msg {
            Message::Start => {}
            Message::Update(units) => {
                self.num_objects = units.len() as i32;
            }
        };
    }
}

impl eframe::App for Gui {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            self.handle_messages();

            ui.heading("DCS state monitor");

            egui::Grid::new("main_grid").show(ui, |ui| {
                ui.label("Num objects: ");
                ui.label(format!("{}", self.num_objects));
                ui.end_row();
            });
        });
    }
}

fn do_gui(rx: &Receiver<Message>) {
    let mut native_options = eframe::NativeOptions::default();
    native_options.event_loop_builder = Some(Box::new(|builder| {
        log::debug!("Calling eframe event loop hook");
        builder.with_any_thread(true);
    }));
    native_options.renderer = eframe::Renderer::Wgpu;
    native_options.context = Some(egui::Context::default());
    log::info!("Spawning worker thread");
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
            if let Message::Start = msg {
                do_gui(&rx);
            }
        }
    };
    std::thread::spawn(gui_thread_entry);
}
