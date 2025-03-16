SERVICE := 'org.gnome.Mutter.DisplayConfig'
OBJECT_PATH := '/org/gnome/Mutter/DisplayConfig'
OUTPUT := 'src/display_config_proxy.g.rs'
RENAME_TO := 'src/display_config_proxy.rs'

generate-proxy:
    #!/usr/bin/bash
    if ! zbus-xmlgen --version; then
        cargo binstall -y zbus_xmlgen
    fi
    zbus-xmlgen session --output {{OUTPUT}} {{SERVICE}} {{OBJECT_PATH}}
    echo "1. Edit {{OUTPUT}}"
    echo "2. Ensure type signature of apply_monitors_config and edit logical_monitors param type ApplyLogicalMonitorTuple<'_>"
    echo "3. Rename to {{RENAME_TO}}"
