#RUN: fish=%fish %fish %s
#REQUIRES: command -v msgfmt

set -l dir (path dirname -- (path dirname -- (status dirname)))/localization/po

set -l fail_count 0
for file in $dir/*.po
    # We only check the format strings.
    # Later on we might do a full "--check" to also check the headers.
    msgfmt --check-format --output-file=- - < $file > /dev/null
    or set fail_count (math $fail_count + 1)
end

# Prevent setting timestamp if any errors were encountered
if test "$fail_count" -gt 0
    exit 1
end

# No output is good output
