# localization: skip(private)
# a function to obtain a list of installed packages with CRUX pkgutils
function __fish_crux_packages -d 'Obtain a list of installed packages'
    command -sq pkginfo; or return 127
    pkginfo -i | string split -f1 ' '
end
