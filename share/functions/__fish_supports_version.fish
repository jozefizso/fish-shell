# localization: skip(private)
function __fish_supports_version
    command -sq -- $argv[1]
    or return 1
    command $argv[1] --version &>/dev/null
end
