use crate::args_format::EscapeMode;

/// Returns the default quote mode for config files during serde deserialization
pub(crate) fn default_quote() -> EscapeMode {
    EscapeMode::Always
}

// /// Returns true, for serde deserialization defaults
// pub(crate) fn default_true() -> bool {
//     true
// }

/// Returns false, for serde deserialization defaults
pub(crate) fn default_false() -> bool {
    false
}
