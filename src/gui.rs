use egui;
use std::sync::mpsc::Receiver;

pub struct Gui {
    rx: &'static Receiver<Message>,
}

enum State {
    Stopped,
    Started,
}

#[derive(Debug)]
pub enum Message {
    Start,
    Update,
}

use winit::platform::windows::EventLoopBuilderExtWindows;

impl Gui {
    pub fn new(_cc: &eframe::CreationContext<'_>, rx: &'static Receiver<Message>) -> Self {
        Self { rx }
    }

    fn handle_messages(&self) {
        while let Ok(msg) = self.rx.try_recv() {
            self.handle_message(msg);
        }
    }

    fn handle_message(&self, _msg: Message) {}

    fn handle_message_blocking(&self) {
        let msg = self.rx.recv().unwrap();
        self.handle_message(msg);
    }
}

impl eframe::App for Gui {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            self.handle_messages();

            ui.heading("DCS state monitor");

            egui::Grid::new("main_grid").show(ui, |ui| {
                ui.label("DCS connection:");
                ui.label("connection_string");
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
    log::info!("Spawning worker thread");
    let rx_forever: &'static Receiver<Message> = unsafe { std::mem::transmute(rx) };

    eframe::run_native(
        "DCS Tetrad",
        native_options,
        Box::new(move |cc| Box::new(Gui::new(cc, rx_forever))),
    );
    log::info!("CLOSED!!");
}

pub fn run(rx: Receiver<Message>) {
    std::thread::spawn(move || loop {
        let msg = rx.recv().unwrap();
        if let Message::Start = msg {
            do_gui(&rx);
        }
    });
}
