# localization: skip(private)
function __fish_complete_ppp_peer --description 'Complete isp name for pon/poff'
    test -d /etc/ppp/peers; or return
    command -sq find; or return 127
    find /etc/ppp/peers/ -type f -printf '%f\n'

end
