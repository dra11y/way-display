SERVICE := 'org.gnome.Mutter.DisplayConfig'
OBJECT_PATH := '/org/gnome/Mutter/DisplayConfig'

generate-proxy:
    #!/usr/bin/bash
    if ! zbus-xmlgen --version; then
        cargo binstall -y zbus_xmlgen
    fi
    zbus-xmlgen session --output src/display_config.rs {{SERVICE}} {{OBJECT_PATH}}
