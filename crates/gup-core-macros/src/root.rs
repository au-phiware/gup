// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Which crate the generated code names: `gup-core` when the calling crate
//! depends on it (or is it), else `gup` (the facade re-exporting it), else
//! `gup_core` as before. Renamed dependencies are followed.

use proc_macro_crate::{FoundCrate, crate_name};
use proc_macro2::TokenStream;
use quote::{format_ident, quote};

/// The path the expansion roots its names at, e.g. `::gup_core`.
pub(crate) fn crate_root() -> TokenStream {
    resolve(|name| crate_name(name).ok())
}

/// The precedence table, with the manifest lookup injected for testing.
fn resolve(lookup: impl Fn(&str) -> Option<FoundCrate>) -> TokenStream {
    let named = |found: FoundCrate, itself: &str| {
        let id = match found {
            FoundCrate::Itself => format_ident!("{}", itself),
            FoundCrate::Name(n) => format_ident!("{}", n),
        };
        quote!(::#id)
    };
    if let Some(found) = lookup("gup-core") {
        return named(found, "gup_core");
    }
    if let Some(found) = lookup("gup") {
        return named(found, "gup");
    }
    quote!(::gup_core)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(core: Option<FoundCrate>, gup: Option<FoundCrate>) -> String {
        resolve(|n| match n {
            "gup-core" => core.clone(),
            "gup" => gup.clone(),
            _ => None,
        })
        .to_string()
    }

    fn name(n: &str) -> Option<FoundCrate> {
        Some(FoundCrate::Name(n.into()))
    }

    #[test]
    fn precedence_table() {
        let itself = Some(FoundCrate::Itself);
        assert_eq!(run(itself.clone(), None), ":: gup_core");
        assert_eq!(run(itself.clone(), name("gup")), ":: gup_core");
        assert_eq!(run(name("core2"), name("gup")), ":: core2");
        assert_eq!(run(None, itself), ":: gup");
        assert_eq!(run(None, name("charts")), ":: charts");
        assert_eq!(run(None, None), ":: gup_core");
    }
}
