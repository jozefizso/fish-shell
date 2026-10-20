# localization: skip(private)
function __fish_theme_freeze
    set -l data_source $argv[1]
    set -l theme_data $argv[2..]
    set -l separator /
    if builtin -q __fish_native_fs
        set separator '\\'
    end
    set -l relative_path conf.d$separator'fish_frozen_theme.fish'
    __fish_backup_config_files $relative_path
    or return

    set -l help_section interactive#syntax-highlighting
    status get-file help_sections | string match -q $help_section
    or echo "fish: internal error: missing help section '$help_section'"

    if builtin -q __fish_native_fs
        __fish_native_fs mkdir $__fish_config_dir$separator'conf.d'
    else
        mkdir -p -- $__fish_config_dir/conf.d
    end
    or return
    set -l frozen_theme (printf %s\n \
        $(test $data_source = __fish_theme_migrate &&
            echo "\
# This file was created by fish when upgrading to version 4.3, to migrate
# theme variables from universal to global scope.") \
        "\
# Don't edit this file, as it will be written by the web-config tool (`fish_config`).
# To customize your theme, delete this file and see
#     help $help_section
# or
#     man fish-interactive | less +/^SYNTAX.HIGHLIGHTING
# for appropriate commands to add to ~/.config/fish/config.fish instead." \
        $(test $data_source = __fish_theme_migrate &&
            echo '# See also the release notes for fish 4.3.0 (run `help relnotes`).') \
        "" \
        'set --global '$theme_data)
    if builtin -q __fish_native_fs
        printf '%s\n' $frozen_theme |
            __fish_native_fs atomic-write $__fish_config_dir$separator$relative_path
    else
        printf '%s\n' $frozen_theme >$__fish_config_dir$separator$relative_path
    end
end
