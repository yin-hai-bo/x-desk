#[cfg(debug_assertions)]
struct ConsoleLogger;

#[cfg(debug_assertions)]
impl log::Log for ConsoleLogger {
    fn enabled(&self, metadata: &log::Metadata<'_>) -> bool {
        metadata.level() <= log::Level::Debug
    }

    fn log(&self, record: &log::Record<'_>) {
        if self.enabled(record.metadata()) {
            eprintln!("[{}] {}", record.level(), record.args());
        }
    }

    fn flush(&self) {}
}

#[cfg(debug_assertions)]
static LOGGER: ConsoleLogger = ConsoleLogger;

pub fn init() {
    #[cfg(debug_assertions)]
    {
        if log::set_logger(&LOGGER).is_ok() {
            log::set_max_level(log::LevelFilter::Debug);
        }
    }
}
