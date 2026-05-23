use indoc::indoc;

pub fn init() {
    std::panic::set_hook(Box::new(move |panic_info| {
        ratatui::restore();

        // developer-facing if the dev wants to see the full stack trace with source context
        better_panic::Settings::auto()
            .most_recent_first(false)
            .lineno_suffix(true)
            .verbosity(better_panic::Verbosity::Full)
            .create_panic_handler()(panic_info);

        // user-facing new-to-rust error
        let metadata = human_panic::metadata!().support(indoc! {"
            - Please report an issue to the repository.
            - If you understand rust, scroll up to find the full stack trace and see if anything relevant comes up and include it.
            - Include what you were doing when the error occurred.
        "});
        let file_path = human_panic::handle_dump(&metadata, panic_info);

        human_panic::print_msg(file_path, &metadata)
            .expect("human-panic: printing error message to console failed");

        std::process::exit(1);
    }));
}

/// Similar to the `std::dbg!` macro, but generates `tracing` events rather
/// than printing to stdout.
///
/// By default, the verbosity level for the generated events is `DEBUG`, but
/// this can be customized.
#[macro_export]
macro_rules! trace_dbg {
        (target: $target:expr, level: $level:expr, $ex:expr) => {
            {
                match $ex {
                        value => {
                                tracing::event!(target: $target, $level, ?value, stringify!($ex));
                                value
                        }
                }
            }
        };
        (level: $level:expr, $ex:expr) => {
                trace_dbg!(target: module_path!(), level: $level, $ex)
        };
        (target: $target:expr, $ex:expr) => {
                trace_dbg!(target: $target, level: tracing::Level::DEBUG, $ex)
        };
        ($ex:expr) => {
                trace_dbg!(level: tracing::Level::DEBUG, $ex)
        };
}
