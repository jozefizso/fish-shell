# localization: skip(private)
function __fish_complete_pg_database
    command -sq psql
    or return 127
    psql -AtqwlXF \t 2>/dev/null | while read -l line
        set -l fields (string split -m 1 \t -- $line)
        set -q fields[2]; or continue
        printf '%s\n' $fields[1]
    end
    return $pipestatus[1]
end
