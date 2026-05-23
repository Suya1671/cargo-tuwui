use error_stack::{Report, ResultExt};
use tracing_error::ErrorLayer;
use tracing_subscriber::{
    EnvFilter,
    fmt::format::{self},
    prelude::*,
};

use crate::{Error, config::Config};

pub fn init(config: &Config) -> Result<(), Report<Error>> {
    std::fs::create_dir_all(config.logging_path().parent().unwrap())
        .attach("Could not create logging path directory")
        .change_context(Error::Init)?;

    let log_file = std::fs::File::create(config.logging_path())
        .attach("Could not create log file")
        .change_context(Error::Init)?;

    let env_filter = EnvFilter::builder()
        .with_default_directive(tracing::Level::INFO.into())
        .with_env_var("TUWUI_LOG");

    let env_filter = env_filter
        .from_env()
        .attach("Could not parse TUWUI_LOG environment variable")
        .change_context(Error::Init)?;

    let file_subscriber = tracing_subscriber::fmt::layer()
        .with_file(true)
        .with_line_number(true)
        .with_target(false)
        .with_ansi(false)
        .with_writer(log_file)
        .with_thread_ids(true)
        .with_thread_names(true)
        .event_format(format::format().pretty())
        .fmt_fields(format::PrettyFields::new())
        .with_filter(env_filter);

    tracing_subscriber::registry()
        .with(file_subscriber)
        .with(ErrorLayer::default())
        .try_init()
        .attach("Could not initialize tracing")
        .change_context(Error::Init)?;

    Ok(())
}

/// Records one or more fields in the current span, or a specific span.
///
/// # Examples
///
/// ```rs
/// fields!(user_name = user.name, system_id = %system.id, member_id = ?member.id);
/// ```
#[macro_export]
macro_rules! fields {
    // recursive cases
    ($name:ident = %$value:expr, $($rest:tt)*) => {
        ::tracing::Span::current().record(::std::stringify!($name), ::tracing::field::display($value));
        $crate::fields!($($rest)*);
    };
    ($name:ident = ?$value:expr, $($rest:tt)*) => {
        ::tracing::Span::current().record(::std::stringify!($name), ::tracing::field::debug($value));
        $crate::fields!($($rest)*);
    };
    ($name:ident = $value:expr, $($rest:tt)*) => {
        ::tracing::Span::current().record(::std::stringify!($name), $value);
        $crate::fields!($($rest)*);
    };

    // trailing comma base cases
    ($name:ident = %$value:expr,) => { $crate::fields!($name = %$value) };
    ($name:ident = ?$value:expr,) => { $crate::fields!($name = ?$value) };
    ($name:ident = $value:expr,) => { $crate::fields!($name = $value) };

    // base cases
    ($name:ident = %$value:expr) => {
        ::tracing::Span::current().record(::std::stringify!($name), ::tracing::field::display($value));
    };
    ($name:ident = ?$value:expr) => {
        ::tracing::Span::current().record(::std::stringify!($name), ::tracing::field::debug($value));
    };
    ($name:ident = $value:expr) => {
        ::tracing::Span::current().record(::std::stringify!($name), $value);
    };

    ($span:expr;) => {};
    () => {};
}
