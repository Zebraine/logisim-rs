//! Presentation-only translations. Circuit data and file-format identifiers never pass through here.
use std::cell::Cell;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Language {
    #[default]
    English,
    SimplifiedChinese,
}
impl Language {
    pub const ALL: [Self; 2] = [Self::English, Self::SimplifiedChinese];
    pub fn code(self) -> &'static str {
        match self {
            Self::English => "en",
            Self::SimplifiedChinese => "zh-CN",
        }
    }
    pub fn name(self) -> &'static str {
        self.catalog().name
    }
    fn catalog(self) -> &'static Catalog {
        match self {
            Self::English => &ENGLISH,
            Self::SimplifiedChinese => &SIMPLIFIED_CHINESE,
        }
    }
    pub fn from_code(code: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|l| l.code() == code)
    }
}
thread_local! { static LANGUAGE: Cell<Language> = const { Cell::new(Language::English) }; }
pub(crate) fn current_language() -> Language {
    LANGUAGE.get()
}
/// A rendering scope, restored even on unwind. Separate windows/threads cannot leak languages.
/// Hosts should also scope their own file dialogs and simulation presentation.
pub fn with_language<R>(language: Language, render: impl FnOnce() -> R) -> R {
    struct Restore(Language);
    impl Drop for Restore {
        fn drop(&mut self) {
            LANGUAGE.set(self.0);
        }
    }
    let _restore = Restore(LANGUAGE.replace(language));
    render()
}
/// Translate an application-owned label. Never use this on user-supplied names or text.
pub fn tr(english: &str) -> &str {
    KEYS.binary_search(&english)
        .map_or(english, |i| LANGUAGE.get().catalog().messages[i])
}
/// Positional placeholders keep inserted user data opaque, including braces and translation keys.
pub fn format(english: &str, arguments: &[&dyn std::fmt::Display]) -> String {
    let mut parts = tr(english).split("{}");
    let mut result = parts.next().unwrap_or_default().to_owned();
    for (i, part) in parts.enumerate() {
        if let Some(argument) = arguments.get(i) {
            use std::fmt::Write;
            let _ = write!(result, "{argument}");
        } else {
            result.push_str("{}");
        }
        result.push_str(part);
    }
    result
}
#[macro_export]
macro_rules! tf {
    ($key:literal, $($value:expr),* $(,)?) => {{
        const _: () = assert!($crate::i18n::placeholder_count($key) == [$(stringify!($value)),*].len());
        $crate::i18n::format($key, &[$(&$value),*])
    }};
}

#[doc(hidden)]
pub const fn placeholder_count(template: &str) -> usize {
    let bytes = template.as_bytes();
    let mut count = 0;
    let mut i = 0;
    while i + 1 < bytes.len() {
        if bytes[i] == b'{' && bytes[i + 1] == b'}' {
            count += 1;
        }
        i += 1;
    }
    count
}

struct Catalog {
    name: &'static str,
    messages: &'static [&'static str],
}

include!(concat!(env!("OUT_DIR"), "/i18n_catalog.rs"));

#[cfg(test)]
mod tests;
