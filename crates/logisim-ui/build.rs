use serde::{Deserialize, Deserializer, de};
use std::{collections::BTreeMap, env, fmt, fmt::Write, fs, path::PathBuf};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Locale {
    name: String,
    #[serde(deserialize_with = "unique_messages")]
    messages: BTreeMap<String, String>,
}

fn unique_messages<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<BTreeMap<String, String>, D::Error> {
    struct Messages;
    impl<'de> de::Visitor<'de> for Messages {
        type Value = BTreeMap<String, String>;

        fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            formatter.write_str("an object with unique message keys and string translations")
        }

        fn visit_map<M: de::MapAccess<'de>>(self, mut map: M) -> Result<Self::Value, M::Error> {
            let mut messages = BTreeMap::new();
            while let Some((key, value)) = map.next_entry::<String, String>()? {
                if messages.contains_key(&key) {
                    return Err(de::Error::custom(format!("duplicate message key: {key:?}")));
                }
                messages.insert(key, value);
            }
            Ok(messages)
        }
    }
    deserializer.deserialize_map(Messages)
}

fn load(code: &str) -> Locale {
    let path = format!("locales/{code}.json");
    println!("cargo:rerun-if-changed={path}");
    let text = fs::read_to_string(&path).unwrap_or_else(|error| panic!("{path}: {error}"));
    let locale: Locale =
        serde_json::from_str(&text).unwrap_or_else(|error| panic!("{path}: {error}"));
    assert!(
        !locale.name.trim().is_empty(),
        "{path}: empty language name"
    );
    assert!(!locale.messages.is_empty(), "{path}: empty catalog");
    for (key, value) in &locale.messages {
        assert!(!key.trim().is_empty(), "{path}: empty message key");
        assert!(
            !value.trim().is_empty(),
            "{path}: empty translation for {key:?}"
        );
        assert_eq!(
            key.matches("{}").count(),
            value.matches("{}").count(),
            "{path}: placeholder count differs for {key:?}"
        );
    }
    locale
}

fn main() {
    let english = load("en");
    let chinese = load("zh-CN");
    for key in english.messages.keys() {
        assert!(
            chinese.messages.contains_key(key),
            "locales/zh-CN.json: missing message key: {key:?}"
        );
    }
    for key in chinese.messages.keys() {
        assert!(
            english.messages.contains_key(key),
            "locales/zh-CN.json: unknown message key: {key:?}"
        );
    }

    let mut output = String::from("// Generated from locales/*.json; do not edit.\n");
    output.push_str("const KEYS: &[&str] = &[\n");
    for key in english.messages.keys() {
        writeln!(output, "{key:?},").unwrap();
    }
    output.push_str("];\n");
    for (symbol, locale) in [("ENGLISH", english), ("SIMPLIFIED_CHINESE", chinese)] {
        writeln!(
            output,
            "const {symbol}: Catalog = Catalog {{ name: {:?}, messages: &[",
            locale.name
        )
        .unwrap();
        for value in locale.messages.values() {
            writeln!(output, "{value:?},").unwrap();
        }
        output.push_str("] };\n");
    }
    let path = PathBuf::from(env::var_os("OUT_DIR").expect("Cargo supplies OUT_DIR"))
        .join("i18n_catalog.rs");
    fs::write(path, output).expect("write generated language catalogs");
}
