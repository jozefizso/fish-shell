# localization: tier1

# This file does some internal fish setup.
# It is not recommended to remove or edit it.

set -g IFS \n\ \t
set -qg __fish_added_user_paths
or set -g __fish_added_user_paths

#
# Set default search paths for completions and shellscript functions
# unless they already exist
#

set -l dir_sep /
set -l basename_pattern '^.*/'
if builtin -q __fish_native_fs
    set dir_sep '\\'
    set basename_pattern '^.*[\\\\/]'
end

# Grab extra directories (as specified by the build process, usually for
# third-party packages to ship completions &c.
set -l __extra_completionsdir
set -l __extra_functionsdir
set -l __extra_confdir
if builtin -q __fish_native_fs
    set __extra_completionsdir $__fish_data_dir$dir_sep'vendor_completions.d'
    set __extra_functionsdir $__fish_data_dir$dir_sep'vendor_functions.d'
    set __extra_confdir $__fish_data_dir$dir_sep'vendor_conf.d'
else
    status get-file __fish_build_paths.fish | source
end

# Compute the directories for vendor configuration.  We want to include
# all of XDG_DATA_DIRS, as well as the __extra_* dirs defined above.
set -l xdg_data_dirs /usr/local/share/fish /usr/share/fish
if builtin -q __fish_native_fs
    set xdg_data_dirs $__fish_data_dir
end
if test -n "$XDG_DATA_DIRS"
    if builtin -q __fish_native_fs
        set xdg_data_dirs (string split ';' -- $XDG_DATA_DIRS)
    else
        set --path xdg_data_dirs $XDG_DATA_DIRS
    end
    if builtin -q __fish_native_fs
        set xdg_data_dirs (string replace -r '[\\\\/]$' '' -- $xdg_data_dirs)$dir_sep'fish'
    else
        set xdg_data_dirs (string replace -r '([^/])/$' '$1' -- $xdg_data_dirs)/fish
    end
end

set -g __fish_vendor_completionsdirs
set -g __fish_vendor_functionsdirs
set -g __fish_vendor_confdirs
# Don't load vendor directories when running unit tests
if not set -q FISH_UNIT_TESTS_RUNNING
    set __fish_vendor_completionsdirs $__fish_user_data_dir$dir_sep'vendor_completions.d' $xdg_data_dirs$dir_sep'vendor_completions.d'
    set __fish_vendor_functionsdirs $__fish_user_data_dir$dir_sep'vendor_functions.d' $xdg_data_dirs$dir_sep'vendor_functions.d'
    set __fish_vendor_confdirs $__fish_user_data_dir$dir_sep'vendor_conf.d' $xdg_data_dirs$dir_sep'vendor_conf.d'

    # Ensure that extra directories are always included.
    if not contains -- $__extra_completionsdir $__fish_vendor_completionsdirs
        set -a __fish_vendor_completionsdirs $__extra_completionsdir
    end
    if not contains -- $__extra_functionsdir $__fish_vendor_functionsdirs
        set -a __fish_vendor_functionsdirs $__extra_functionsdir
    end
    if not contains -- $__extra_confdir $__fish_vendor_confdirs
        set -a __fish_vendor_confdirs $__extra_confdir
    end
end

# Set up function and completion paths. Make sure that the fish
# default functions/completions are included in the respective path.

if not set -q fish_function_path
    set fish_function_path $__fish_config_dir$dir_sep'functions' $__fish_sysconf_dir$dir_sep'functions' $__fish_vendor_functionsdirs
end

if not set -q fish_complete_path
    set fish_complete_path $__fish_config_dir$dir_sep'completions' $__fish_sysconf_dir$dir_sep'completions' $__fish_vendor_completionsdirs
    set -a fish_complete_path $__fish_cache_dir$dir_sep'generated_completions'
end

# Add a handler for when fish_user_path changes, so we can apply the same changes to PATH
function __fish_reconstruct_path -d "Update PATH when fish_user_paths changes" --on-variable fish_user_paths
    # Deduplicate $fish_user_paths
    # This should help with people appending to it in config.fish
    set -l new_user_path
    set -l user_paths $fish_user_paths
    if not builtin -q __fish_native_fs
        set user_paths (string split : -- $user_paths)
    end
    for path in $user_paths
        if not contains -- $path $new_user_path
            set -a new_user_path $path
        end
    end

    if test (count $new_user_path) -lt (count $fish_user_paths)
        # This will end up calling us again, so we return
        set fish_user_paths $new_user_path
        return
    end

    set -l local_path $PATH

    for x in $__fish_added_user_paths
        set -l idx (contains --index -- $x $local_path)
        and set -e local_path[$idx]
    end

    set -g __fish_added_user_paths
    if set -q fish_user_paths
        # Native drive letters are identities, not Unix colon-list delimiters.
        for x in $new_user_path[-1..1]
            if set -l idx (contains --index -- $x $local_path)
                set -e local_path[$idx]
            else
                set -ga __fish_added_user_paths $x
            end
            set -p local_path $x
        end
    end

    set -xg PATH $local_path
end

#
# Launch debugger on SIGTRAP
#
if not builtin -q __fish_native_fs
    function fish_sigtrap_handler --on-signal TRAP --no-scope-shadowing --description "TRAP handler: debug prompt"
        breakpoint
    end
end

#
# When a prompt is first displayed, make sure that interactive
# mode-specific initializations have been performed.
# This includes a `read` prompt, hence the fish_read event.
# This handler removes itself after it is first called.
#
function __fish_on_interactive --on-event fish_prompt --on-event fish_read
    # We erase this *first* so it can't be called again,
    # e.g. if fish_greeting calls "read".
    functions -e __fish_on_interactive
    __fish_config_interactive
end

# Set the locale if it isn't explicitly set. Allowing the lack of locale env vars to imply the
# C/POSIX locale causes too many problems. Do this before reading the snippets because they might be
# in UTF-8 (with non-ASCII characters).
not set -q LANG # (fast path - no need to load the file if we have $LANG)
and __fish_set_locale

#
# Some things should only be done for login terminals
# This used to be in etc/config.fish - keep it here to keep the semantics
#
if status is-login && command -sq /usr/libexec/path_helper
    __fish_macos_set_env PATH /etc/paths '/etc/paths.d'
    if test -n "$MANPATH"
        __fish_macos_set_env MANPATH /etc/manpaths '/etc/manpaths.d'
    end
    functions -e __fish_macos_set_env
end

# Invoke this here to apply the current value of fish_user_path after
# PATH is possibly set above.
__fish_reconstruct_path

if status is-interactive
    __fish_theme_migrate
end
if status is-interactive || set -qgx __fish_force_load_default_theme
    fish_config theme choose default --no-override
end

# As last part of initialization, source the conf directories.
# Implement precedence (User > Admin > Extra (e.g. vendors) > Fish) by basically doing "basename".
set -l sourcelist
for file in $__fish_config_dir$dir_sep'conf.d'$dir_sep*.fish $__fish_sysconf_dir$dir_sep'conf.d'$dir_sep*.fish $__fish_vendor_confdirs$dir_sep*.fish
    set -l basename (string replace -r $basename_pattern '' -- $file)
    contains -- $basename $sourcelist
    and continue
    set sourcelist $sourcelist $basename
    # Also skip non-files or unreadable files.
    # This allows one to use e.g. symlinks to /dev/null to "mask" something (like in systemd).
    test -f $file -a -r $file
    and source $file
end
