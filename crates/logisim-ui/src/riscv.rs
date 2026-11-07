//! Presentation adapter; instruction text itself is never translated.
pub(crate) fn decode(word: u32) -> String {
    if word == 0 {
        crate::i18n::tr("Nop/Invalid Instr").into()
    } else {
        logisim_course::decode_instruction(word)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn translation_stays_in_the_adapter_and_never_changes_instructions() {
        use crate::i18n::{Language, with_language};
        for language in Language::ALL {
            with_language(language, || {
                assert_eq!(decode(0), crate::i18n::tr("Nop/Invalid Instr"));
                assert_eq!(decode(0x800211f3), "csrrw gp,2048,tp");
                assert_eq!(logisim_course::decode_instruction(0), "Nop/Invalid Instr");
            });
        }
    }
}
