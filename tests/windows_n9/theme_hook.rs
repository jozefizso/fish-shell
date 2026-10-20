use super::{Family, fixture::{Fixture, exact, quote, run}};
use std::{ffi::OsStr, fs};

pub(super) const FAMILIES: &[Family] = &[
    ("native-webconfig-frozen-theme-notification-and-version-boundaries", frozen_theme_notifications),
];

fn frozen_theme_notifications(f: &Fixture) {
    let config_home = f.root.join("theme hook config 雪 🐟");
    let config = config_home.join("fish");
    let themes = config.join("themes");
    fs::create_dir_all(&themes).unwrap();
    fs::write(themes.join("n9-hook-first.theme"), "\
# name: Owned first theme 雪
fish_color_command brmagenta
fish_color_quote brcyan --bold
fish_color_search_match white --background=blue --bold
fish_pager_color_prefix yellow --underline
").unwrap();
    fs::write(themes.join("n9-hook-second.theme"), "\
# name: Owned second theme 🐟
fish_color_command brgreen
fish_color_quote brred --underline
fish_color_search_match black --background=bryellow --underline
fish_pager_color_prefix blue --bold
").unwrap();
    let frozen = config.join("conf.d/fish_frozen_theme.fish");
    assert!(!frozen.exists());
    let before = f.root.join("theme rendering before");
    let first = f.root.join("theme rendering first");
    let second = f.root.join("theme rendering second");
    let invoke = |args: &[&OsStr]| {
        let mut command = f.command(args);
        command.env("XDG_CONFIG_HOME", &config_home).env("APPDATA", &config_home);
        run(&mut command, b"")
    };
    // fish_config theme save persists universal colors, not a frozen snippet.
    // Use its supported choose/dump CLI to produce real theme data, then the
    // same production freezer used by webconfig.py's /set_color/ handler.
    // A synchronous producer child cannot change the consumer's global colors.
    // The fixture has no persistent-shell notification barrier: this consumer
    // explicitly exercises same-shell universal variable events, not browser UI
    // or asynchronous delivery between independent running shells.
    let code = format!(r#"
test "$__fish_config_dir" -ef {config}; or exit 81
builtin -q __fish_native_fs; or exit 82
set -g fish_color_command red
set -g fish_color_quote yellow
set -g fish_color_search_match white --background=red
set -g fish_pager_color_prefix cyan
set -l baseline_command "$(set_color $fish_color_command)"
set -l baseline_quote "$(set_color $fish_color_quote)"
fish_config theme demo > {before}; or exit 83
# This is the actual hook-autoload registration used by
# __fish_config_interactive; the hook itself is never called or replaced here.
functions -q __fish_webconfig_update_color_hook; or exit 84
function n9_publish_theme -a name
    command "$(status fish-path)" -N -c '
        fish_config theme choose $argv[1]; or exit 81
        __fish_theme_freeze fish_config (fish_config theme dump)
    ' $name
end
function n9_expect_first
    test "$(string join ' ' -- $fish_color_command)" = 'brmagenta --theme=n9-hook-first'; or return 81
    test "$(string join ' ' -- $fish_color_quote)" = 'brcyan --bold --theme=n9-hook-first'; or return 82
    test "$(string join ' ' -- $fish_color_search_match)" = 'white --background=blue --bold --theme=n9-hook-first'; or return 83
    test "$(string join ' ' -- $fish_pager_color_prefix)" = 'yellow --underline --theme=n9-hook-first'; or return 84
    test "$(set_color $fish_color_command)" = "$(set_color brmagenta)"; or return 85
    test "$(set_color $fish_color_quote)" = "$(set_color brcyan --bold)"; or return 86
    test "$(set_color $fish_color_search_match)" = "$(set_color white --background=blue --bold)"; or return 87
    test "$(set_color $fish_pager_color_prefix)" = "$(set_color yellow --underline)"
end
function n9_expect_second
    test "$(string join ' ' -- $fish_color_command)" = 'brgreen --theme=n9-hook-second'; or return 81
    test "$(string join ' ' -- $fish_color_quote)" = 'brred --underline --theme=n9-hook-second'; or return 82
    test "$(string join ' ' -- $fish_color_search_match)" = 'black --background=bryellow --underline --theme=n9-hook-second'; or return 83
    test "$(string join ' ' -- $fish_pager_color_prefix)" = 'blue --bold --theme=n9-hook-second'; or return 84
    test "$(set_color $fish_color_command)" = "$(set_color brgreen)"; or return 85
    test "$(set_color $fish_color_quote)" = "$(set_color brred --underline)"; or return 86
    test "$(set_color $fish_color_search_match)" = "$(set_color black --background=bryellow --underline)"; or return 87
    test "$(set_color $fish_pager_color_prefix)" = "$(set_color blue --bold)"
end
# A valid notification with no snippet must not change the existing colors.
set -U __fish_webconfig_theme_notification 'set-theme-v1-#1'
test "$fish_color_command" = red; or exit 85
test "$fish_color_quote" = yellow; or exit 86
n9_publish_theme n9-hook-first; or exit 87
path is {frozen}; or exit 88
test "$fish_color_command" = red; or exit 89
# An existing frozen file must not be applied by future or malformed messages.
for notification in 'set-theme-v2-#1' 'set-theme-v01-#1' 'set-theme-v1-#' 'set-theme-v1-#x' 'set-theme-v1-#-1' 'set-theme-v1-#1extra' 'prefixset-theme-v1-#1' '' "set-theme-v1-#1
extra" "set-theme-v1-#1
"
    set -U __fish_webconfig_theme_notification "$notification"
    test "$fish_color_command" = red; or exit 90
    test "$fish_color_quote" = yellow; or exit 91
    test "$(set_color $fish_color_command)" = "$baseline_command"; or exit 92
    test "$(set_color $fish_color_quote)" = "$baseline_quote"; or exit 93
end
set -U __fish_webconfig_theme_notification 'set-theme-v1-#2'
n9_expect_first; or exit 94
test "$(set_color $fish_color_command)" != "$baseline_command"; or exit 95
fish_config theme demo > {first}; or exit 96
n9_publish_theme n9-hook-second; or exit 97
n9_expect_first; or exit 98
set -U __fish_webconfig_theme_notification 'set-theme-v1-#3'
n9_expect_second; or exit 99
fish_config theme demo > {second}; or exit 100
# Repeated protocol-v1 notifications must keep loading the current file.
n9_publish_theme n9-hook-first; or exit 101
n9_expect_second; or exit 102
set -U __fish_webconfig_theme_notification 'set-theme-v1-#2'
n9_expect_first; or exit 103
n9_publish_theme n9-hook-second; or exit 104
for notification in 'set-theme-v2-#4' 'set-theme-v1-#4suffix' 'set-theme-v1-#' ''
    set -U __fish_webconfig_theme_notification "$notification"
    n9_expect_first; or exit 105
end
# Missing snippets are also safe after a successful theme application.
__fish_native_fs remove {frozen}; or exit 106
set -U __fish_webconfig_theme_notification 'set-theme-v1-#4'
n9_expect_first; or exit 107
n9_publish_theme n9-hook-second; or exit 108
# Counter 2 originally loaded the first theme; it must now load the second,
# rather than a cached theme associated with that earlier notification.
set -U __fish_webconfig_theme_notification 'set-theme-v1-#2'
n9_expect_second; or exit 109
"#, config=quote(config.to_str().unwrap()), frozen=quote(frozen.to_str().unwrap()), before=quote(before.to_str().unwrap()), first=quote(first.to_str().unwrap()), second=quote(second.to_str().unwrap()));
    exact(invoke(&[OsStr::new("-N"), OsStr::new("-c"), OsStr::new(&code)]), 0, b"", b"");
    let before = fs::read(before).unwrap();
    let first = fs::read(first).unwrap();
    let second = fs::read(second).unwrap();
    assert_ne!(before, first, "accepted notification must change actual theme rendering");
    assert_ne!(first, second, "a later notification must render the replacement theme");
    // A normal fresh shell consumes the real conf.d snippet too, independently
    // of the explicit variable-event consumer above.
    exact(invoke(&[OsStr::new("-c"), OsStr::new("string join ' ' -- $fish_color_command; string join ' ' -- $fish_pager_color_prefix")]), 0, b"brgreen --theme=n9-hook-second\nblue --bold --theme=n9-hook-second\n", b"");
}
