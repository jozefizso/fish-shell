# localization: skip(private)
function __fish_print_service_names -d 'All services known to the system'
    if test -d /run/systemd/system; and command -sq systemctl # Systemd systems
        # For the `service` command, this needs to be without the type suffix
        # on Debian at least
        __fish_systemctl_services | string replace -r '.service$' ''
    else if command -sq rc-service # OpenRC (Gentoo)
        command rc-service -l
    else if test -d /etc/init.d # SysV on Debian and other linuxen
        string replace '/etc/init.d/' '' -- /etc/init.d/*
    else if test (__fish_uname) = FreeBSD; and command -sq service
        command service -l
    end
end
