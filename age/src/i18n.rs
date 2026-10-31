//! Plain-English message lookup.
//!
//! The upstream fluent/proc-macro localisation machinery is replaced by a tiny resolver over the
//! embedded en-US message file. It understands only what that file uses: `{$arg}`, `{-term}`,
//! `{message-id}` and `{"literal"}` placeables, and multi-line values.

use std::collections::HashMap;
use std::sync::OnceLock;

const MESSAGES: &str = include_str!("../i18n/en-US/age.ftl");

fn table() -> &'static HashMap<&'static str, String> {
    static TABLE: OnceLock<HashMap<&'static str, String>> = OnceLock::new();
    TABLE.get_or_init(|| {
        let mut map = HashMap::new();
        let mut lines = MESSAGES.lines().peekable();
        while let Some(line) = lines.next() {
            let Some((id, first)) = line.split_once(" =") else {
                continue;
            };
            if line.starts_with(['#', ' ']) || id.contains(' ') {
                continue;
            }
            let mut rest = vec![];
            while let Some(next) = lines.peek() {
                if next.starts_with(' ') || next.is_empty() {
                    rest.push(*next);
                    lines.next();
                } else {
                    break;
                }
            }
            while rest.last().is_some_and(|l| l.trim().is_empty()) {
                rest.pop();
            }
            let indent = rest
                .iter()
                .filter(|l| !l.trim().is_empty())
                .map(|l| l.len() - l.trim_start().len())
                .min()
                .unwrap_or(0);
            let mut parts = vec![];
            if !first.trim().is_empty() {
                parts.push(first.trim().to_string());
            }
            parts.extend(rest.iter().map(|l| l.get(indent..).unwrap_or("").to_string()));
            map.insert(id, parts.join("\n"));
        }
        map
    })
}

fn render(template: &str, args: &[(&str, String)], out: &mut String) {
    let mut rest = template;
    while let Some(start) = rest.find('{') {
        out.push_str(&rest[..start]);
        let end = rest[start..].find('}').map_or(rest.len(), |e| start + e);
        let key = rest[start + 1..end].trim();
        if let Some(name) = key.strip_prefix('$') {
            match args.iter().find(|(k, _)| *k == name) {
                Some((_, v)) => out.push_str(v),
                None => out.push_str(key),
            }
        } else if let Some(lit) = key.strip_prefix('"').and_then(|k| k.strip_suffix('"')) {
            out.push_str(lit);
        } else if let Some(msg) = table().get(key) {
            render(msg, args, out);
        } else {
            out.push_str(key);
        }
        rest = rest.get(end + 1..).unwrap_or("");
    }
    out.push_str(rest);
}

/// Looks up an en-US message and substitutes its arguments.
#[doc(hidden)]
pub fn lookup(id: &str, args: &[(&str, String)]) -> String {
    let mut out = String::new();
    match table().get(id) {
        Some(msg) => render(msg, args, &mut out),
        None => out.push_str(id),
    }
    out
}

/// Loads a message.
#[doc(hidden)]
#[macro_export]
macro_rules! fl {
    ($message_id:literal) => {{
        $crate::i18n::lookup($message_id, &[])
    }};

    ($message_id:literal, $($key:ident = $value:expr),* $(,)?) => {{
        $crate::i18n::lookup($message_id, &[$((stringify!($key), ($value).to_string())),*])
    }};
}

/// Version of the write! macro that takes a message id.
#[doc(hidden)]
#[macro_export]
macro_rules! wfl {
    ($f:ident, $message_id:literal) => {
        write!($f, "{}", $crate::fl!($message_id))
    };

    ($f:ident, $message_id:literal, $($args:tt)*) => {
        write!($f, "{}", $crate::fl!($message_id, $($args)*))
    };
}

/// Version of the writeln! macro that takes a message id.
#[doc(hidden)]
#[macro_export]
macro_rules! wlnfl {
    ($f:ident, $message_id:literal) => {
        writeln!($f, "{}", $crate::fl!($message_id))
    };

    ($f:ident, $message_id:literal, $($args:tt)*) => {
        writeln!($f, "{}", $crate::fl!($message_id, $($args)*))
    };
}

#[cfg(test)]
mod tests {
    use super::lookup;

    #[test]
    fn resolves_terms_args_and_multiline() {
        assert_eq!(lookup("err-decryption-failed", &[]), "Decryption failed");
        let m = lookup("err-read-rsa-modulus-too-large", &[("max_size", "4096".into())]);
        assert!(m.starts_with("RSA Modulus Too Large\n---"));
        assert!(m.contains("OpenSSH") && m.contains("rage") && m.contains("4096 bits"));
        assert!(!m.contains('{'));
    }

    #[test]
    fn every_message_resolves_without_placeholders_left_over_from_terms() {
        for id in super::table().keys().filter(|k| !k.starts_with('-')) {
            let out = lookup(id, &[]);
            assert!(!out.contains("{-") && !out.contains("{\""), "{id}: {out}");
        }
    }
}
