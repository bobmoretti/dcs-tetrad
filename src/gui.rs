use egui;
use std::sync::mpsc::Receiver;

pub struct Gui {
    rx: Receiver<Message>,
}

#[derive(Debug)]
pub enum Message {}

use winit::platform::windows::EventLoopBuilderExtWindows;

impl Gui {
    pub fn new(_cc: &eframe::CreationContext<'_>, rx: Receiver<Message>) -> Self {
        Self { rx }
    }

    fn handle_message(&self, msg: Message) {}
}

impl eframe::App for Gui {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            while let Ok(message) = self.rx.try_recv() {
                self.handle_message(message);
            }

            ui.heading("DCS state monitor");

            egui::Grid::new("main_grid").show(ui, |ui| {
                ui.label("DCS connection:");
                ui.label("connection_string");
                ui.end_row();
            });
        });
    }
}

pub fn run(rx: Receiver<Message>) {
    std::thread::spawn(|| {
        let mut native_options = eframe::NativeOptions::default();
        native_options.event_loop_builder = Some(Box::new(|builder| {
            log::debug!("Calling eframe event loop hook");
            builder.with_any_thread(true);
        }));
        native_options.renderer = eframe::Renderer::Wgpu;
        log::info!("Spawning worker thread");
        eframe::run_native(
            "DCS Tetrad",
            native_options,
            Box::new(|cc| Box::new(Gui::new(cc, rx))),
        );
        log::info!("CLOSED!!");
    });
}
