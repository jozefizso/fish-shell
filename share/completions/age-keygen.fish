complete -c age-keygen -s o -l output -r -n "__fish_prev_arg_in -o --output; or not __fish_contains_opt -s o output" -d "output file for identity or recipients"
complete -c age-keygen -s y -n "not __fish_contains_opt -s y" -d "read identity file, print recipient(s)"
complete -c age-keygen -l version -d "print version number"
