# localization: skip(private)
function __fish_complete_magick_options --description 'Complete Magick options' --argument-names what
    set -l kind $what
    # FreeType character maps and log substitutions are not iconv encodings or
    # image formats. Neither has a corresponding `magick -list` category.
    switch $what
        case encoding
            printf '%s\n' AdobeCustom AdobeExpert AdobeStandard AppleRoman BIG5 GB2312 Latin-1 Latin-2 None SJIScode Symbol Unicode Wansung
            return 0
        case log
            printf '%s\t%s\n' '%d' Domain '%e' Event '%f' Function '%l' Line '%m' Module '%p' 'Process ID' '%r' 'Real CPU time' '%t' 'Wall-clock time' '%u' 'User CPU time' '%%' 'Literal percent'
            return 0
    end
    switch $what
        case format Format
            set kind Format
        case font Font family
            set kind Font
    end
    command -sq magick; or return 127
    set -l lines (command magick -list $kind)
    or return $status
    set lines (string replace -r '\r$' '' -- $lines)
    switch $what
        case format Format
            # Newer builds omit the coder-module column; both layouts expose
            # the same three-character read/write capability field.
            string replace -rf '^\s*([\w-]+)\*?\s+(?:\S+\s+)?([rw+-]{3})\s+(.+)$' '$1\t$2 $3' -- $lines
        case color Color
            string replace -rf '^(\w+)\s+(\w+\(.+\))\s+(.+)$' '$1\t$2 $3' -- $lines
        case family
            string replace -rf '^\s*family: ' '' -- $lines | path sort -u
        case font Font
            string replace -rf '^\s*Font: ' '' -- $lines | path sort -u
        case '*'
            set -q lines[1]; and printf '%s\n' $lines
    end
    return 0
end
