use log::LevelFilter;
pub fn setup_logger(crate_level: LevelFilter) -> Result<(), fern::InitError> {
    fern::Dispatch::new()
        .format(|out, message, record| {
            out.finish(format_args!(
                "{}[{}][{}] {}",
                chrono::Local::now().format("[%Y-%m-%d][%H:%M:%S]"),
                record.target(),
                record.level(),
                message
            ))
        })
        // output all messages
        .level(log::LevelFilter::Warn)
        .level_for("auto_invite_matrix_bot", crate_level)
        .level_for("ruma_client", crate_level)
        .chain(std::io::stdout())
        .chain(fern::log_file("output.log")?)
        .apply()?;
    Ok(())
}
