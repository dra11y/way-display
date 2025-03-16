SERVICE := 'org.gnome.Mutter.DisplayConfig'
OBJECT_PATH := '/org/gnome/Mutter/DisplayConfig'
OUTPUT := 'src/display_config_proxy.rs'

generate-proxy:
    #!/usr/bin/bash
    if ! zbus-xmlgen --version; then
        cargo binstall -y zbus_xmlgen
    fi
    zbus-xmlgen session --output {{OUTPUT}} {{SERVICE}} {{OBJECT_PATH}}
