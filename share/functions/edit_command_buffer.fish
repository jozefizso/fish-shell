# localization: tier1
function edit_command_buffer --description 'Edit the command buffer in an external editor'
    set -l editor (__fish_anyeditor)
    or return 1
    set -l native false
    builtin -q __fish_native_fs; and set native true
    set -l tmpdir
    set -l separator /
    if $native
        set tmpdir (__fish_native_fs make-temp-directory | string collect)
        set separator '\\'
    else
        set tmpdir (__fish_mktemp_relative -d fish)
    end
    set -q tmpdir[1]; or return 1
    set -l f $tmpdir$separator'command-line.fish'
    printf '' >$f
    or begin
        if $native
            __fish_native_fs remove-temp-directory $tmpdir
        end
        return 1
    end

    set -l indented_lines (commandline -b | fish_indent --only-indent)
    string join -- \n $indented_lines >$f
    set -l offset (commandline --cursor)
    # compute cursor line/column
    set -l lines (commandline)\n
    set -l line 1
    while test $offset -ge (string length -- $lines[1])
        set offset (math $offset - (string length -- $lines[1]))
        set line (math $line + 1)
        set -e lines[1]
    end
    set -l indent 1 + (string length -- $indented_lines[$line]) - (string length -- $lines[1])
    set -l col (math $offset + 1 + $indent)

    set -l editor_basename (string match -r '[^/]+$' -- $editor[1])
    if $native
        set editor_basename (path basename -- $editor[1] | string replace -ir '[.]exe$' '')
    end
    set -l wrapped_commands
    for wrap_target in (complete -- $editor_basename | string replace -rf '^complete [^/]+ --wraps (.+)$' '$1')
        set -l tmp
        string unescape -- $wrap_target | read -at tmp
        set -a wrapped_commands $tmp[1]
    end
    set -l found false
    set -l cursor_from_editor
    set -l cursor_is_lines false
    for editor_command in $editor_basename $wrapped_commands
        switch $editor_command
            case vi vim nvim
                if test $editor_command = vi && not set -l vi_version "$(vi --version 2>/dev/null)"
                    if string match -q '*BusyBox*' -- $vi_version
                        break
                    end
                    set -a editor +{$line} $f
                    set found true
                    break
                end
                if $native
                    set cursor_from_editor $tmpdir$separator'cursor'
                    set cursor_is_lines true
                    set -l cursor_path (string replace -a '\\' '\\\\' -- $cursor_from_editor | string replace -a '"' '\\"')
                    set -a editor +$line "+norm! $col|" $f \
                        '+au VimLeave * ++once call writefile([bufname(), string(line(".")), string(col("."))], "'$cursor_path'")'
                else
                    set cursor_from_editor (__fish_mktemp_relative fish-edit_command_buffer)
                    or return
                    set -a editor +$line "+norm! $col|" $f \
                        '+au VimLeave * ++once call writefile([printf("%s %s %s", shellescape(bufname()), line("."), col("."))], "'$cursor_from_editor'")'
                end
            case emacs emacsclient gedit
                set -a editor +$line:$col $f
            case kak
                if $native
                    set cursor_from_editor $tmpdir$separator'cursor'
                else
                    set cursor_from_editor (__fish_mktemp_relative fish-edit_command_buffer)
                    or return
                end
                set -a editor +$line:$col $f -e "
                        hook -always -once global ClientClose %val{client} %{
                            echo -to-file $cursor_from_editor -quoting shell \
                                %val{buffile} %val{cursor_line} %val{cursor_column}
                        }
                    "
            case nano
                set -a editor +$line,$col $f
            case joe ee
                set -a editor +$line $f
            case code code-oss
                set -a editor --goto $f:$line:$col --wait
            case subl
                set -a editor $f:$line:$col --wait
            case micro
                set -a editor $f +$line:$col
            case helix hx
                set -a editor $f:$line:$col
            case '*'
                continue
        end
        set found true
        break
    end
    if not $found
        set -a editor $f
    end

    $editor
    set -l editor_status $status

    set -l raw_lines
    if $native
        set raw_lines (string sub --start 1 <$f)
    else
        set raw_lines (command cat $f)
    end
    set -l unindented_lines (string join -- \n $raw_lines | fish_indent --only-unindent)

    # Here we're checking the exit status of the editor.
    if test $editor_status -eq 0 -a -s $f
        # Set the command to the output of the edited command and move the cursor to the
        # end of the edited command.
        commandline -r -- $unindented_lines
        commandline -C 999999
    else
        echo
        echo (_ "Ignoring the output of your editor since its exit status was non-zero")
        echo (_ "or the file was empty")
    end
    if set -q cursor_from_editor[1]; and begin; not $native; or test -f $cursor_from_editor; end
        set -l pos
        if $cursor_is_lines
            set pos (string sub --start 1 <$cursor_from_editor)
        else if $native
            eval set pos "$(string sub --start 1 <$cursor_from_editor)"
        else
            eval set pos "$(command cat $cursor_from_editor)"
        end
        if set -q pos[3]; and test $pos[1] = $f
            set -l line $pos[2]
            set -l indent (math (string length -- "$raw_lines[$line]") - (string length -- "$unindented_lines[$line]"))
            set -l column (math $pos[3] - $indent)
            if not commandline --line $line 2>/dev/null
                commandline -f end-of-buffer
            else
                commandline --column $column 2>/dev/null || commandline -f end-of-line
            end
        end
        if not $native
            command rm $cursor_from_editor
        end
    end
    set -l cleanup_status 0
    if $native
        __fish_native_fs remove-temp-directory $tmpdir
        set cleanup_status $status
    else
        command rm -r (path dirname $f)
    end
    # We've probably opened something that messed with the screen.
    # A repaint seems in order.
    commandline -f repaint
    if $native
        test $cleanup_status -eq 0; or return $cleanup_status
        return $editor_status
    end
end
