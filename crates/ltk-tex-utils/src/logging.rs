use tracing::Level;
use tracing::level_filters::LevelFilter;
use tracing_subscriber::prelude::*;

pub fn init() {
    let common_format = tracing_subscriber::fmt::format()
        .with_ansi(true)
        .with_level(true)
        .with_source_location(false)
        .with_line_number(false)
        .with_target(false)
        .with_timer(tracing_subscriber::fmt::time::time());

    // stdout: INFO/DEBUG/TRACE
    let stdout_layer = tracing_subscriber::fmt::layer()
        .with_writer(std::io::stdout)
        .event_format(common_format.clone())
        .with_filter(tracing_subscriber::filter::filter_fn(|metadata| {
            let level = *metadata.level();
            level == Level::INFO || level == Level::DEBUG || level == Level::TRACE
        }));

    // stderr: WARN/ERROR
    let stderr_layer = tracing_subscriber::fmt::layer()
        .with_writer(std::io::stderr)
        .event_format(common_format)
        .with_filter(tracing_subscriber::filter::filter_fn(|metadata| {
            let level = *metadata.level();
            level == Level::WARN || level == Level::ERROR
        }));

    tracing_subscriber::registry()
        .with(stdout_layer)
        .with(stderr_layer)
        .with(LevelFilter::TRACE)
        .init();
}
