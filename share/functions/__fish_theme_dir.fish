# localization: skip(private)
function __fish_theme_dir
    if builtin -q __fish_native_fs
        echo $__fish_config_dir'\\themes'
    else
        echo $__fish_config_dir/themes
    end
end
