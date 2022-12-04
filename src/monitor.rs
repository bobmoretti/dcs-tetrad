use crate::dcs::{DcsWorldObject, DcsWorldUnit};
use num::traits::AsPrimitive;
use ordered_float::OrderedFloat;
use std::collections::VecDeque;
use std::iter::Sum;
use std::sync::mpsc::{Receiver, Sender};
use std::thread::JoinHandle;

enum Message {
    FrameUpdate(FrameState),
}

struct FrameState {
    num_units: i32,
    num_ballistics: i32,
    real_time: f64,
    game_time: f64,
    lib_time: f64,
    sys_cpu: i32,
    sys_wall: i32,
    proc_cpu: i32,
}

pub struct Monitor {
    thread_join: Option<JoinHandle<()>>,
    tx_to_thread: Sender<Message>,
}

#[derive(Debug, Default)]
struct MonitorImpl {
    frame_log: FrameLog,
    last_game_time: f64,
    last_real_time: f64,
    last_logged_time: f64,
    frame_count: i32,
    last_logged_frame: i32,
}

#[derive(Debug, Default)]
struct FrameLog {
    num_units: VecDeque<i32>,
    num_ballistics: VecDeque<i32>,
    real_times: VecDeque<OrderedFloat<f64>>,
    game_times: VecDeque<OrderedFloat<f64>>,
    lib_times: VecDeque<OrderedFloat<f64>>,
    sys_cpu_times: VecDeque<i32>,
    sys_wall_times: VecDeque<i32>,
    proc_cpu_times: VecDeque<i32>,
}

fn get_stats<T>(v: &VecDeque<T>) -> Option<(T, T, f64)>
where
    T: Copy + Ord + Sum + AsPrimitive<f64>,
{
    let minval = *v.iter().min()?;
    let maxval = *v.iter().max()?;

    let total: f64 = v.iter().copied().sum::<T>().as_();

    Some((minval, maxval, total / v.len() as f64))
}

fn time_stats_to_float<T>((t0, t1, t2): (T, T, f64)) -> (f64, f64, f64)
where
    T: AsPrimitive<f64>,
{
    (t0.as_(), t1.as_(), t2.as_())
}

fn float_stats<T>(v: &VecDeque<T>) -> Option<(f64, f64, f64)>
where
    T: Copy + Ord + Sum + AsPrimitive<f64>,
{
    let result = get_stats(v)?;
    Some(time_stats_to_float::<T>(result))
}

fn log_times(series: &VecDeque<i32>, totals: &VecDeque<i32>, name: &str, lvl: log::Level) {
    let result: f64 = series
        .iter()
        .zip(totals.iter())
        .filter(|(_, y)| **y > 0)
        .map(|(x, y)| -> f64 {
            let xf = *x as f64;
            let yf = *y as f64;
            xf / yf
        })
        .sum::<f64>()
        / series.len() as f64;
    log::log!(lvl, "{} {:.3}%", name, result * 100.0);
}

impl FrameLog {
    fn update(&mut self, state: &FrameState, last_game_time: f64, last_real_time: f64) {
        self.num_units.push_back(state.num_units);
        self.num_ballistics.push_back(state.num_ballistics);
        self.real_times
            .push_back(OrderedFloat(state.real_time - last_real_time));
        self.game_times
            .push_back(OrderedFloat(state.game_time - last_game_time));
        self.lib_times.push_back(OrderedFloat(state.lib_time));
        self.sys_cpu_times.push_back(state.sys_cpu);
        self.sys_wall_times.push_back(state.sys_wall);
        self.proc_cpu_times.push_back(state.proc_cpu);
    }

    fn reset(&mut self) {
        self.num_units.clear();
        self.num_ballistics.clear();
        self.game_times.clear();
        self.real_times.clear();
        self.lib_times.clear();
        self.sys_cpu_times.clear();
        self.sys_wall_times.clear();
        self.proc_cpu_times.clear();
    }

    fn is_empty(&self) -> bool {
        self.game_times.len() == 0
    }

    #[allow(dead_code)]
    fn has_data(&self) -> bool {
        !self.is_empty()
    }

    fn log_to_console(&self) {
        if self.is_empty() {
            log::warn!("No new frame in the last five seconds.");
            return;
        }

        let Some((_, max_units, _)) = get_stats(&self.num_units) else {
            log::error!("Units vector was unexpectedly empty");
            return;
        };

        let Some((_, max_ballistics, _)) = get_stats(&self.num_ballistics) else {
            log::error!("Ballistics vector was unexpectedly empty");
            return;
        };

        let Some((g_min, g_max, g_mean)) = float_stats(&self.game_times) else {
            log::error!("Real times vector was unexpectedly empty");
            return;
        };

        let lvl = if g_min < 0.1 {
            log::Level::Info
        } else {
            log::Level::Warn
        };

        log::log!(
            lvl,
            "Frame times (min/max/avg): {:.3}, {:.3}, {:.3} milliseconds",
            g_min * 1000.0,
            g_max * 1000.0,
            g_mean * 1000.0,
        );

        let Some((r_min, r_max, r_mean)) = float_stats(&self.real_times) else {
            log::error!("Real times vector was unexpectedly empty");
            return;
        };

        log::log!(
            lvl,
            "Real times (min/max/avg): {:.3}, {:.3}, {:.3} milliseconds",
            r_min * 1000.0,
            r_max * 1000.0,
            r_mean * 1000.0,
        );

        log::log!(lvl, "Average FPS: {:.03}", 1.0 / g_mean);
        log::log!(
            lvl,
            "Unit count: {}, ballistics count: {}",
            max_units,
            max_ballistics
        );

        log_times(
            &self.proc_cpu_times,
            &self.sys_wall_times,
            "DCS CPU load",
            lvl,
        );
        log_times(
            &self.sys_cpu_times,
            &self.sys_wall_times,
            "Total CPU load",
            lvl,
        );

        let Some((l_min, l_max, l_mean)) = float_stats(&self.lib_times) else {
            log::error!("Lib times vector was unexpectedly empty");
            return;
        };

        log::log!(
            lvl,
            "Time spent in game loop (min/max/avg): {:.6}, {:.6}, {:.6}",
            l_min,
            l_max,
            l_mean
        );

        log::log!(
            lvl,
            "----------------------------------------------------------------"
        );
    }
}
impl MonitorImpl {
    fn update_log(&mut self, state: &FrameState) {
        self.frame_log
            .update(state, self.last_game_time, self.last_real_time);

        if state.game_time - self.last_logged_time >= 5.0 {
            self.frame_log.log_to_console();
            self.frame_log.reset();
            self.last_logged_frame = self.frame_count;
            self.last_logged_time = state.game_time;
        }

        self.last_game_time = state.game_time;
        self.last_real_time = state.real_time;
        self.frame_count += 1;
    }

    fn entry(&mut self, rx: Receiver<Message>) {
        log::debug!("Starting monitor thread");
        log::info!("----------------------------------------------------------------");
        loop {
            let Ok(Message::FrameUpdate(state)) = rx.recv() else {
                log::debug!("Monitor thread RX dropped");
                break;
            };
            self.update_log(&state);
        }
    }
}

impl Monitor {
    pub fn new() -> Self {
        log::debug!("Starting monitor");
        let (tx, rx) = std::sync::mpsc::channel();

        let mut me = Self {
            thread_join: None,
            tx_to_thread: tx,
        };

        let mut imp = MonitorImpl::default();

        let handle = std::thread::spawn(move || {
            imp.entry(rx);
        });

        me.thread_join = Some(handle);
        me
    }

    pub fn update(
        &mut self,
        units: &[DcsWorldUnit],
        ballistics: &[DcsWorldObject],
        real_time: f64,
        game_time: f64,
        lib_time: f64,
        sys_cpu: i32,
        sys_wall: i32,
        proc_cpu: i32,
    ) {
        let fs = FrameState {
            num_units: units.len() as i32,
            num_ballistics: ballistics.len() as i32,
            real_time,
            game_time,
            lib_time,
            sys_cpu,
            sys_wall,
            proc_cpu,
        };
        self.tx_to_thread.send(Message::FrameUpdate(fs)).unwrap();
    }

    pub fn stop(&mut self) -> JoinHandle<()> {
        let join = std::mem::take(&mut self.thread_join).unwrap();
        join
    }
}
