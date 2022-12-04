use windows::Win32::Foundation::FILETIME;
use windows::Win32::System::Threading::{GetCurrentProcess, GetProcessTimes, GetSystemTimes};

fn to_i64(ft: FILETIME) -> i64 {
    ft.dwLowDateTime as i64 + ((ft.dwHighDateTime as i64) << 32)
}

#[derive(Default)]
pub struct PerfMonitor {
    system: PerfRecord,
    process: PerfRecord,
}

impl PerfMonitor {
    pub fn update_system_time(&mut self) -> (i32, i32) {
        let mut idle = FILETIME::default();
        let mut kernel = FILETIME::default();
        let mut user = FILETIME::default();
        unsafe {
            let success = GetSystemTimes(Some(&mut idle), Some(&mut kernel), Some(&mut user));
            assert!(success.as_bool());
        }
        self.system
            .update(to_i64(idle), to_i64(kernel), to_i64(user))
    }

    pub fn update_process_time(&mut self) -> (i32, i32) {
        let mut kernel = FILETIME::default();
        let mut user = FILETIME::default();
        let mut creation = FILETIME::default();
        let mut exit = FILETIME::default();
        unsafe {
            let proc = GetCurrentProcess();
            let success = GetProcessTimes(proc, &mut creation, &mut exit, &mut kernel, &mut user);
            assert!(success.as_bool());
        }
        self.process.update(0, to_i64(kernel), to_i64(user))
    }
}

#[derive(Default)]
struct PerfRecord {
    last_idle_time: i64,
    last_kernel_time: i64,
    last_user_time: i64,
}

impl PerfRecord {
    fn update(&mut self, idle: i64, kernel: i64, user: i64) -> (i32, i32) {
        let di = (idle - self.last_idle_time) as i32;
        let dk = (kernel - self.last_kernel_time) as i32;
        let du = (user - self.last_user_time) as i32;
        let total_time = dk + du;
        let busy_time = total_time - di;
        self.last_idle_time = idle;
        self.last_kernel_time = kernel;
        self.last_user_time = user;

        (busy_time, total_time)
    }
}
