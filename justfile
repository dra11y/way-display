SERVICE := 'org.gnome.Mutter.DisplayConfig'
OBJECT_PATH := '/org/gnome/Mutter/DisplayConfig'
OUTPUT := 'src/display_config_proxy.rs'

install:
    sudo mkdir -p /usr/local/bin
    just build
    sudo cp target/debug/way-display /usr/local/bin/
    killall way-display || true
    sudo -u gdm killall way-display || true
    sudo cp way-display.desktop /usr/share/gdm/greeter/autostart/
    sudo cp way-display.desktop /etc/xdg/autostart/

status:
    systemctl --user status way-display.service

journal args:
    journalctl --user -u way-display.service "{{args}}"

gdm:
    just install

build:
    cargo build

clean:
    cargo clean

generate-proxy:
    #!/usr/bin/bash
    if ! zbus-xmlgen --version; then
        cargo binstall -y zbus_xmlgen
    fi
    zbus-xmlgen session --output {{OUTPUT}} {{SERVICE}} {{OBJECT_PATH}}
