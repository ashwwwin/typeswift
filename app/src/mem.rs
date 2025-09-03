// macOS-only: get current process resident memory (RSS) in MB.
// Uses Mach task_info with MACH_TASK_BASIC_INFO flavor.

#[allow(non_camel_case_types)]
type natural_t = u32;

#[repr(C)]
#[derive(Default, Copy, Clone)]
struct time_value_t {
    seconds: i32,
    microseconds: i32,
}

#[repr(C)]
#[derive(Copy, Clone)]
struct mach_task_basic_info {
    virtual_size: u64,
    resident_size: u64,
    resident_size_max: u64,
    user_time: time_value_t,
    system_time: time_value_t,
    policy: i32,
    suspend_count: i32,
}

impl Default for mach_task_basic_info {
    fn default() -> Self {
        Self {
            virtual_size: 0,
            resident_size: 0,
            resident_size_max: 0,
            user_time: time_value_t { seconds: 0, microseconds: 0 },
            system_time: time_value_t { seconds: 0, microseconds: 0 },
            policy: 0,
            suspend_count: 0,
        }
    }
}

extern "C" {
    fn mach_task_self() -> u32;
    fn task_info(
        target_task: u32,
        flavor: i32,
        task_info_out: *mut u8,
        task_info_outCnt: *mut natural_t,
    ) -> i32;
}

const MACH_TASK_BASIC_INFO: i32 = 20; // TASK_BASIC_INFO

pub fn current_rss_mb() -> Option<f64> {
    unsafe {
        let task = mach_task_self();
        let mut info: mach_task_basic_info = Default::default();
        let mut count: natural_t = (std::mem::size_of::<mach_task_basic_info>() / std::mem::size_of::<natural_t>()) as natural_t;
        let kr = task_info(
            task,
            MACH_TASK_BASIC_INFO,
            &mut info as *mut _ as *mut u8,
            &mut count,
        );
        if kr != 0 {
            return None;
        }
        let mb = (info.resident_size as f64) / (1024.0 * 1024.0);
        Some(mb)
    }
}

