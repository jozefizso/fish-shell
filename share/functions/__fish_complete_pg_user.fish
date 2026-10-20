# localization: skip(private)
function __fish_complete_pg_user
    command -sq psql
    or return 127
    psql -AtqwXc 'select usename from pg_user' template1 2>/dev/null | string replace -r '\r$' ''
    return $pipestatus[1]
end
