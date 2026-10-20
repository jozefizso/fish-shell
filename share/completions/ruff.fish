command -sq ruff; or return 0
__fish_cache_sourced_completions ruff generate-shell-completion fish 2>/dev/null
or begin
    set -l generated (ruff generate-shell-completion fish)
    set -l result $status
    if test $result -ne 0
        printf '%s\n' $generated >&2
        return $result
    end
    printf '%s\n' $generated | source
end
