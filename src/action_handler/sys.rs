pub struct SysInfo {}

impl SysInfo {
    pub fn ping() -> String {
        let cpu_usage = psutil::cpu::CpuPercentCollector::new()
            .unwrap()
            .cpu_percent()
            .unwrap();

        let cpu_temp = psutil::sensors::temperatures()
            .first()
            .unwrap()
            .as_ref()
            .unwrap()
            .current()
            .celsius()
            .round();                                                                

        let virtual_memory = psutil::memory::virtual_memory().unwrap();
        let ram_usage = format!(
            "{}/{}",
            virtual_memory.used() / 1024 / 1024,
            virtual_memory.total() / 1024 / 1024
        );

        let uptime_mins = psutil::host::uptime().unwrap().as_secs() / 60;
        
        let minutes = uptime_mins % 60;
        let hours = uptime_mins / 60;

        format!(
            "CPU: {}% {}°C, RAM: {} MiB, uptime: {}h {}m",
            cpu_usage, cpu_temp, ram_usage, hours, minutes
        )
    }
}
