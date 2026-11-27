//! Interface text in every language under `locales/`, picked from the
//! settings or LC_ALL/LC_MESSAGES/LANG. Every visible string goes through
//! `t`, `t_args`, `t_named`, `t_date`, or `lookup` when it may be missing.
//! A key missing from the current language falls back to English.
//!
//! Keys are stable ids, not copies of the English: when the English text
//! changes, keep the key. Values are always named (`$count`, `$path`), and
//! numbers reach Fluent as numbers, so translations can pick plural forms.

use chrono::{DateTime, Datelike, Local, Timelike};
use fluent_bundle::concurrent::FluentBundle;
use fluent_bundle::{FluentArgs, FluentResource, FluentValue};
use std::{
    borrow::Cow,
    sync::{
        OnceLock,
        atomic::{AtomicUsize, Ordering},
    },
};
use unic_langid::LanguageIdentifier;

/// Every shipped language: its code and its Fluent file.
const LANGS: &[(&str, &str)] = &[
    ("en", include_str!("../locales/en.ftl")),
    ("fr", include_str!("../locales/fr.ftl")),
    ("sco", include_str!("../locales/sco.ftl")),
];

const FALLBACK: &str = "en";

type Bundle = FluentBundle<FluentResource>;

static BUNDLES: OnceLock<Vec<(&'static str, Bundle)>> = OnceLock::new();

fn bundles() -> &'static Vec<(&'static str, Bundle)> {
    BUNDLES.get_or_init(|| {
        let mut list = Vec::new();
        for (code, _) in LANGS {
            let id: LanguageIdentifier = code
                .parse()
                .unwrap_or_else(|e| panic!("{code} is not a language code: {e}"));
            let mut bundle = FluentBundle::new_concurrent(vec![id]);
            bundle.set_use_isolating(false);
            bundle
                .add_resource(parse(code))
                .unwrap_or_else(|e| panic!("{code}.ftl has duplicate keys: {e:?}"));
            list.push((*code, bundle));
        }
        list
    })
}

fn parse(code: &str) -> FluentResource {
    let (_, text) = LANGS
        .iter()
        .find(|(c, _)| *c == code)
        .unwrap_or_else(|| panic!("no locale file for {code}"));
    FluentResource::try_new(text.to_string())
        .unwrap_or_else(|(_, e)| panic!("{code}.ftl is invalid: {e:?}"))
}

/// Shipped languages as (code, name in that language), for pickers.
pub fn languages() -> Vec<(&'static str, String)> {
    LANGS
        .iter()
        .map(|(code, _)| {
            let name = format_in(code, "lang-name", None)
                .map(Cow::into_owned)
                .unwrap_or_else(|| code.to_string());
            (*code, name)
        })
        .collect()
}

/// The shipped language closest to a locale such as `pt_BR.UTF-8`: the
/// exact tag first, then the language alone, then English.
fn best_match(locale: &str) -> &'static str {
    let tag = locale
        .split(['.', '@'])
        .next()
        .unwrap_or_default()
        .replace('_', "-");
    let base = tag.split('-').next().unwrap_or_default();
    let codes = || LANGS.iter().map(|(c, _)| *c);
    codes()
        .find(|c| c.eq_ignore_ascii_case(&tag))
        .or_else(|| codes().find(|c| c.eq_ignore_ascii_case(base)))
        .unwrap_or(FALLBACK)
}

/// The language from the settings, or else from the environment.
pub fn detect() -> &'static str {
    let chosen = crate::try_svc().and_then(|s| s.config().locale);
    let locale = chosen
        .or_else(|| {
            ["LC_ALL", "LC_MESSAGES", "LANG"]
                .iter()
                .filter_map(|k| std::env::var(k).ok())
                .find(|v| !v.is_empty() && v != "C" && v != "POSIX")
        })
        .unwrap_or_default();
    best_match(&locale)
}

static CUR: AtomicUsize = AtomicUsize::new(usize::MAX);

/// The language in use; resolved once, then changed only by `set`.
pub fn current() -> &'static str {
    match LANGS.get(CUR.load(Ordering::Relaxed)) {
        Some((code, _)) => code,
        None => {
            let code = detect();
            set(code);
            code
        }
    }
}

/// Switch to a shipped language; unknown codes fall back to English.
pub fn set(code: &str) {
    let i = LANGS
        .iter()
        .position(|(c, _)| *c == code)
        .or_else(|| LANGS.iter().position(|(c, _)| *c == FALLBACK))
        .unwrap_or(0);
    CUR.store(i, Ordering::Relaxed);
}

fn format_in(code: &str, key: &str, args: Option<&FluentArgs>) -> Option<Cow<'static, str>> {
    let (_, bundle) = bundles().iter().find(|(c, _)| *c == code)?;
    let pattern = bundle.get_message(key)?.value()?;
    let mut errors = Vec::new();
    Some(bundle.format_pattern(pattern, args, &mut errors))
}

/// A message in the current language, or in English when it lacks the key.
fn format(key: &str, args: Option<&FluentArgs>) -> Option<Cow<'static, str>> {
    format_in(current(), key, args).or_else(|| format_in(FALLBACK, key, args))
}

/// Plain text that always exists.
pub fn t(key: &str) -> &'static str {
    lookup(key).unwrap_or("?")
}

/// Plain text that may be missing: the caller picks the fallback.
pub fn lookup(key: &str) -> Option<&'static str> {
    match format(key, None) {
        Some(Cow::Borrowed(text)) => Some(text),
        _ => None,
    }
}

/// Text with named values, as `$count` or `$path`.
pub fn t_args(key: &str, values: &[(&str, String)]) -> String {
    t_named(key, values).unwrap_or_else(|| "?".into())
}

/// A message with named values, or None when the key does not exist.
pub fn t_named(key: &str, values: &[(&str, String)]) -> Option<String> {
    let mut args = FluentArgs::new();
    for (name, value) in values {
        args.set(*name, value_of(value));
    }
    format(key, Some(&args)).map(Cow::into_owned)
}

/// A plain number becomes a Fluent number, so `{ $count -> [one] ... }`
/// works; anything else, including "05" or "3.10.2", stays text as written.
fn value_of(text: &str) -> FluentValue<'static> {
    let digits = text.strip_prefix('-').unwrap_or(text);
    let (int, frac) = digits.split_once('.').unwrap_or((digits, ""));
    let plain = !int.is_empty()
        && int.bytes().all(|b| b.is_ascii_digit())
        && frac.bytes().all(|b| b.is_ascii_digit())
        && (int == "0" || !int.starts_with('0'))
        && (!digits.contains('.') || !frac.is_empty());
    match text.parse::<fluent_bundle::types::FluentNumber>() {
        Ok(n) if plain => n.into(),
        _ => text.to_string().into(),
    }
}

/// Which parts of a date `t_date` shows.
#[derive(Clone, Copy)]
pub enum DateStyle {
    /// "Nov 11, 2026, 13:13"
    Full,
    /// "Thu 12 Nov, 20:00"
    Event,
    /// "20:00"
    Time,
}

/// A date in the interface language, laid out by the `date-*` messages.
pub fn t_date(at: DateTime<Local>, style: DateStyle) -> String {
    let key = match style {
        DateStyle::Full => "date-full",
        DateStyle::Event => "date-event",
        DateStyle::Time => "date-time",
    };
    let mut args = FluentArgs::new();
    args.set("day", at.day());
    args.set("month", at.month());
    args.set("year", at.year());
    args.set("weekday", at.weekday().number_from_monday());
    args.set("hour", format!("{:02}", at.hour()));
    args.set("minute", format!("{:02}", at.minute()));
    format(key, Some(&args))
        .unwrap_or(Cow::Borrowed("?"))
        .into_owned()
}

#[cfg(test)]
mod tests {

    use super::*;
    use fluent_syntax::ast::Entry;
    use std::collections::BTreeSet;

    const TEMPLATE: &str = include_str!("../locales/lang.ftl.template");

    fn parse_template() -> FluentResource {
        FluentResource::try_new(TEMPLATE.to_string())
            .unwrap_or_else(|(_, e)| panic!("lang.ftl.template is invalid: {e:?}"))
    }

    fn keys(name: &str, resource: &FluentResource) -> BTreeSet<String> {
        let mut keys = BTreeSet::new();
        for entry in resource.entries() {
            if let Entry::Message(message) = entry {
                let key = message.id.name.to_string();
                assert!(keys.insert(key.clone()), "{name}: duplicate key {key}");
            }
        }
        keys
    }

    #[test]
    fn every_finding_has_a_title() {
        let template = keys("lang.ftl.template", &parse_template());
        for id in unilux_diag::FINDING_IDS {
            let key = format!("finding-{id}-title");
            assert!(
                template.contains(&key),
                "lang.ftl.template is missing {key}"
            );
        }
    }

    #[test]
    fn system_locales_pick_a_shipped_language() {
        assert_eq!(best_match("fr_FR.UTF-8"), "fr");
        assert_eq!(best_match("fr_CA@euro"), "fr");
        assert_eq!(best_match("en_GB.UTF-8"), "en");
        assert_eq!(best_match("sco_GB.UTF-8"), "sco");
        assert_eq!(best_match("xx_YY.UTF-8"), FALLBACK);
        assert_eq!(best_match(""), FALLBACK);
    }

    #[test]
    fn every_language_names_itself() {
        for (code, name) in languages() {
            assert_ne!(name, code, "{code}.ftl has no lang-name");
        }
    }

    /// Literal keys passed to t, t_args, t_named and lookup in the GUI sources,
    /// except this file, whose tests would match themselves.
    /// (file, call, key) for every literal key passed to a lookup.
    fn keys_used_in_code() -> Vec<(String, &'static str, String)> {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        let mut out = Vec::new();
        for file in std::fs::read_dir(dir).expect("src is readable") {
            let path = file.expect("src entry").path();
            if path.ends_with("i18n.rs") {
                continue;
            }
            let text = std::fs::read_to_string(&path).expect("source is readable");
            let name = path.file_name().unwrap().to_string_lossy().to_string();
            for call in ["t(", "t_args(", "t_named(", "lookup("] {
                for (at, _) in text.match_indices(call) {
                    let before = text[..at].chars().next_back();
                    if before.is_some_and(|c| c.is_alphanumeric() || c == '_' || c == '.') {
                        continue;
                    }
                    let rest = text[at + call.len()..].trim_start();
                    if let Some(lit) = rest.strip_prefix('"')
                        && let Some(end) = lit.find('"')
                    {
                        out.push((name.clone(), call, lit[..end].to_string()));
                    }
                }
            }
        }
        out
    }

    #[test]
    fn every_key_in_the_code_is_in_the_template() {
        let template = keys("lang.ftl.template", &parse_template());
        let used = keys_used_in_code();
        assert!(used.len() > 300, "found only {} keys", used.len());
        let missing: Vec<_> = used
            .iter()
            .filter(|(_, _, k)| !template.contains(k))
            .collect();
        assert!(
            missing.is_empty(),
            "keys missing from the template: {missing:?}"
        );
    }

    #[test]
    fn locale_codes_are_unique() {
        let mut seen = BTreeSet::new();
        for (code, _) in LANGS {
            assert!(seen.insert(code), "LANGS lists {code} twice");
        }
    }

    #[test]
    fn every_locale_has_the_template_keys() {
        let template = keys("lang.ftl.template", &parse_template());
        for (code, _) in LANGS {
            let locale = keys(code, &parse(code));
            assert_eq!(
                locale.len(),
                template.len(),
                "{code}.ftl has the wrong number of keys"
            );
            let missing: Vec<_> = template.difference(&locale).collect();
            let extra: Vec<_> = locale.difference(&template).collect();
            assert!(missing.is_empty(), "{code}.ftl is missing {missing:?}");
            assert!(
                extra.is_empty(),
                "{code}.ftl has keys the template lacks: {extra:?}"
            );
        }
    }

    /// `t` and `lookup` only hand out text Fluent can lend as is: a
    /// message with a placeable, even a quoted literal like {" "}, would
    /// show as "?". Such text has to go through `t_args` or `t_named`.
    #[test]
    fn plain_lookups_never_show_a_question_mark() {
        let mut broken = Vec::new();
        for (file, call, key) in keys_used_in_code() {
            if call != "t(" && call != "lookup(" {
                continue;
            }
            for (code, _) in LANGS {
                if let Some(Cow::Owned(text)) = format_in(code, &key, None) {
                    broken.push(format!("{file}: {key} in {code}.ftl ({text:?})"));
                }
            }
        }
        assert!(
            broken.is_empty(),
            "these would show \"?\"; use t_args or t_named: {broken:#?}"
        );
    }

    /// Every message of a .ftl file: its key, the variables its
    /// "# Variables:" comment declares, and the variables its text uses.
    fn variables(text: &str) -> Vec<(String, BTreeSet<String>, BTreeSet<String>)> {
        fn names(s: &str) -> BTreeSet<String> {
            s.split('$')
                .skip(1)
                .map(|rest| {
                    rest.chars()
                        .take_while(|c| c.is_ascii_alphanumeric() || *c == '_' || *c == '-')
                        .collect()
                })
                .filter(|n: &String| !n.is_empty())
                .collect()
        }
        let mut out: Vec<(String, BTreeSet<String>, BTreeSet<String>)> = Vec::new();
        let mut declared = BTreeSet::new();
        for line in text.lines() {
            if let Some(list) = line.strip_prefix("# Variables:") {
                declared = names(list);
            } else if line.starts_with('#') || line.is_empty() {
            } else if line.starts_with([' ', '}']) {
                if let Some((_, _, used)) = out.last_mut() {
                    used.extend(names(line));
                }
            } else if let Some((key, value)) = line.split_once(" = ") {
                out.push((key.to_string(), std::mem::take(&mut declared), names(value)));
            }
        }
        out
    }

    #[test]
    fn values_are_named_never_numbered() {
        for (code, text) in LANGS.iter().chain([&("template", TEMPLATE)]) {
            for (key, _, used) in variables(text) {
                let numbered: Vec<_> = used
                    .iter()
                    .filter(|n| {
                        n.len() > 1
                            && n.starts_with('p')
                            && n[1..].bytes().all(|b| b.is_ascii_digit())
                    })
                    .collect();
                assert!(
                    numbered.is_empty(),
                    "{code}: {key} uses {numbered:?}; name the value instead"
                );
            }
        }
    }

    #[test]
    fn every_language_uses_the_variables_the_template_declares() {
        let template: std::collections::BTreeMap<_, _> = variables(TEMPLATE)
            .into_iter()
            .map(|(key, declared, _)| (key, declared))
            .collect();
        for (code, text) in LANGS {
            for (key, _, used) in variables(text) {
                let declared = &template[&key];
                let unknown: Vec<_> = used.difference(declared).collect();
                assert!(
                    unknown.is_empty(),
                    "{code}.ftl: {key} uses {unknown:?}, which the template does not declare"
                );
            }
        }
    }

    #[test]
    fn numbers_reach_fluent_as_numbers() {
        assert!(matches!(value_of("12"), FluentValue::Number(_)));
        assert!(matches!(value_of("0"), FluentValue::Number(_)));
        assert!(matches!(value_of("12.5"), FluentValue::Number(_)));
        assert!(matches!(value_of("-3"), FluentValue::Number(_)));
        for text in [
            "05", "3.10.2", "1.", "inf", "NaN", "1e5", "", "WiVRn", "/home/a",
        ] {
            assert!(
                matches!(value_of(text), FluentValue::String(_)),
                "{text} should stay text"
            );
        }
        // Text kept as written, digits included.
        assert_eq!(
            t_args(
                "events-countdown-hours",
                &[("hours", "2".into()), ("minutes", "05".into())]
            ),
            "2 h 05"
        );
    }
}
