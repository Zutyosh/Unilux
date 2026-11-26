//! UI strings in English and French, picked from LC_ALL/LANG. Every
//! visible string goes through `t`, or through `tr` when it is used in a
//! single place and reads better next to its code.

use fluent_bundle::concurrent::FluentBundle;
use fluent_bundle::{FluentArgs, FluentResource};
use std::{
    borrow::Cow,
    sync::{
        OnceLock,
        atomic::{AtomicU8, Ordering},
    },
};
use unic_langid::LanguageIdentifier;

#[derive(Clone, Copy, PartialEq)]
#[deprecated(note = "Use LANGS keys instead")]
pub enum Lang {
    En,
    Fr,
}

const LANGS: &[(&str, &str)] = &[
    ("en", include_str!("../locales/en.ftl")),
    ("fr", include_str!("../locales/fr.ftl")),
];

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

pub fn detect() -> Lang {
    let chosen = crate::try_svc().and_then(|s| s.config().locale);
    let v = chosen.unwrap_or_else(|| {
        std::env::var("LC_ALL")
            .or_else(|_| std::env::var("LANG"))
            .unwrap_or_default()
    });
    if v.starts_with("fr") {
        Lang::Fr
    } else {
        Lang::En
    }
}

static CUR: AtomicU8 = AtomicU8::new(0);

/// The language in use; resolved once, then changed only by `set`.
pub fn current() -> Lang {
    match CUR.load(Ordering::Relaxed) {
        1 => Lang::En,
        2 => Lang::Fr,
        _ => {
            let l = detect();
            set(l);
            l
        }
    }
}

fn parse(key: &str) -> FluentResource {
    let source = LANGS
        .iter()
        .find(|(lang_key, _)| *lang_key == key)
        .unwrap_or_else(|| panic!("no locale file for {key}"));

    let language = source.1;

    FluentResource::try_new(language.to_string())
        .unwrap_or_else(|(_, e)| panic!("{key}.ftl is invalid: {e:?}"))
}

fn fluent(key: &str, args: Option<&[String]>) -> Option<Cow<'static, str>> {
    let code = match current() {
        Lang::En => "en",
        Lang::Fr => "fr",
    };

    let args = args.map(|list| {
        let mut fluent_args = FluentArgs::new();
        for (i, value) in list.iter().enumerate() {
            fluent_args.set(format!("p{}", i + 1), value.clone());
        }
        fluent_args
    });

    let (_, bundle) = bundles().iter().find(|(c, _)| *c == code)?;

    let id = key.replace('.', "-");
    let pattern = bundle.get_message(&id)?.value()?;

    let mut errors = Vec::new();
    let text = bundle.format_pattern(pattern, args.as_ref(), &mut errors);
    Some(text)
}

pub fn set(l: Lang) {
    CUR.store(if l == Lang::Fr { 2 } else { 1 }, Ordering::Relaxed);
}

pub fn t(key: &str) -> &'static str {
    lookup(key).unwrap_or("?")
}

pub fn t_with(key: &str, args: &[String]) -> String {
    fluent(key, Some(args))
        .unwrap_or(Cow::Borrowed("?"))
        .into_owned()
}

#[deprecated(note = "Add text to localisation files and use t() instead")]
/// A string used in one place: English and French side by side.
pub fn tr(en: &'static str, fr: &'static str) -> &'static str {
    match current() {
        Lang::En => en,
        Lang::Fr => fr,
    }
}

/// "{}" slots filled in order.
pub fn fill(template: &str, args: &[String]) -> String {
    let mut out = template.to_string();
    for a in args {
        out = out.replacen("{}", a, 1);
    }
    out
}

#[deprecated(note = "Obselete use fluent() instead")]
/// A string that may be absent: the caller falls back to something else.
pub fn lookup(key: &str) -> Option<&'static str> {
    match fluent(key, None) {
        Some(Cow::Borrowed(text)) => Some(text),
        _ => None,
    }
}

/// A fix's name in the user's language; unknown (custom) fixes keep theirs.
pub fn fix_title(id: &str, fallback: &str) -> String {
    lookup(&format!("fix.{id}.title"))
        .unwrap_or(fallback)
        .to_string()
}

/// What a fix does, in plain words.
pub fn fix_summary(id: &str, fallback: &str) -> String {
    lookup(&format!("fix.{id}.sum"))
        .unwrap_or(fallback)
        .to_string()
}

/// A shipped recipe's name and description, translated.
pub fn recipe_title(id: &str, fallback: &str) -> String {
    lookup(&format!("recipe.{id}.title"))
        .unwrap_or(fallback)
        .to_string()
}

pub fn recipe_desc(id: &str, fallback: &str) -> String {
    lookup(&format!("recipe.{id}.desc"))
        .unwrap_or(fallback)
        .to_string()
}

/// Step label for an install step id, falling back to the id.
pub fn step(id: &str) -> String {
    if let Some(s) = lookup(&format!("step.{id}")) {
        s.to_string()
    } else {
        id.replace('_', " ")
    }
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
}
