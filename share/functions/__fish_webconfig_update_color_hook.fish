# localization: skip(private)
function __fish_webconfig_update_color_hook --on-variable __fish_webconfig_theme_notification
    if not string match -qr -- '\Aset-theme-v1-#\d+\z' $__fish_webconfig_theme_notification
        # Early return for forward compatibility - a future "fish_config"
        # may not want to update old shells.
        return
    end
    set -l snippet_file $__fish_config_dir/conf.d/fish_frozen_theme.fish
    if builtin -q __fish_native_fs
        set snippet_file $__fish_config_dir'\\conf.d\\fish_frozen_theme.fish'
    end
    if not path is $snippet_file
        return
    end
    source $snippet_file
end
