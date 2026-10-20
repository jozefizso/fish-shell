//! Process-isolated probes for the actual Fluent/gettext owners (not mock catalogs).
#[cfg(all(windows, feature = "localize-messages"))]
fn main() {
    use fish::env::{EnvMode, EnvSetMode, EnvStack};
    use fish::localization::localizable_string;
    use fish_widestring::{L, WString};
    let scenario = std::env::args().nth(1).expect("scenario");
    let vars = EnvStack::new();
    let set = |key, value: &str| {
        vars.set_one(key, EnvSetMode::new_at_early_startup(EnvMode::GLOBAL_EXPORTED), WString::from_str(value));
    };
    let translated = || fish::wgettext!("Print the working directory").to_string();
    match scenario.as_str() {
        "precedence" => {
            set(L!("LANG"), "de-DE");
            fish::localization::update_from_env(&vars);
            assert_eq!(translated(), "Das Arbeitsverzeichnis ausgeben");
            set(L!("LC_MESSAGES"), "fr-FR");
            fish::localization::update_from_env(&vars);
            assert_eq!(translated(), "Afficher le dossier de travail");
            set(L!("LANGUAGE"), "de:fr");
            fish::localization::update_from_env(&vars);
            assert_eq!(translated(), "Das Arbeitsverzeichnis ausgeben");
            set(L!("LC_ALL"), "C.UTF-8");
            fish::localization::update_from_env(&vars);
            assert_eq!(translated(), "Print the working directory");
            fish::localization::update_from_status_language_builtin(&["fr"]);
            set(L!("LC_ALL"), "de-DE");
            fish::localization::update_from_env(&vars);
            assert_eq!(translated(), "Afficher le dossier de travail");
            fish::localization::unset_from_status_language_builtin(&vars);
            assert_eq!(translated(), "Das Arbeitsverzeichnis ausgeben");
        }
        "fallback" => {
            set(L!("LANG"), "de-AT");
            set(L!("LANGUAGE"), "de-AT:fr");
            fish::localization::update_from_env(&vars);
            assert_eq!(translated(), "Das Arbeitsverzeichnis ausgeben");
            assert_eq!(fish::wgettext!("N9 absent catalog entry λ").to_string(), "N9 absent catalog entry λ");
            let mut args = fluent::FluentArgs::new();
            args.set("package_name", "fish λ");
            args.set("version", "N9");
            assert_eq!(fish_fluent::format_localized("fish-version", &args), "fish λ, Version N9");
            let mut fallback_args = fluent::FluentArgs::new();
            fallback_args.set("first", "café");
            fallback_args.set("second", "λ");
            assert_eq!(fish_fluent::format_localized("test-with-args", &fallback_args), "Two arguments: café, λ");
            fish::localization::update_from_status_language_builtin(&["en"]);
            assert_eq!(fish_fluent::format_localized("fish-version", &args), "fish λ, version N9");
            assert_eq!(translated(), "Print the working directory");
        }
        "format-errors" => {
            // Panics are the existing Fluent contract. They must not poison global locks.
            for id in ["n9-no-such-message", "fish-version"] {
                assert!(std::panic::catch_unwind(|| fish_fluent::format_localized(id, &fluent::FluentArgs::new())).is_err());
            }
            let mut args = fluent::FluentArgs::new();
            args.set("package_name", "fish");
            args.set("version", "N9");
            assert_eq!(fish_fluent::format_localized("fish-version", &args), "fish, version N9");
        }
        "native-default" => {
            fish::localization::update_from_env(&vars);
            let expected = fish::locale::native_message_locale().split('-').next().unwrap().to_ascii_lowercase();
            let state = fish::localization::status_language().to_string();
            assert!(state.contains("Fluent:"));
            if fish_fluent::get_available_languages().iter().any(|language| language.as_ref().0 == expected) {
                assert!(state.contains(&format!("Fluent: {expected}")), "{state}");
            }
        }
        _ => panic!("unknown catalog scenario"),
    }
    println!("catalog-{scenario}-ok");
}

#[cfg(not(all(windows, feature = "localize-messages")))]
fn main() {
    // This is an explicit feature profile, not a successful localized probe.
    assert!(!cfg!(feature = "localize-messages"));
    println!("catalog-profile=English-only");
}
