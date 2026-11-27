//! The shared translation layer (`unilux-i18n`), with the language the
//! user picked in the settings.

pub use unilux_i18n::*;

/// The language from the settings, or else from the environment.
pub fn detect() -> &'static str {
    resolve(crate::try_svc().and_then(|s| s.config().locale).as_deref())
}
