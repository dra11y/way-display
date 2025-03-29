GNOME_SERVICE := 'org.gnome.Mutter.DisplayConfig'
CINNAMON_SERVICE := 'org.cinnamon.Muffin.DisplayConfig'

GNOME_OBJECT_PATH := '/org/gnome/Mutter/DisplayConfig'
CINNAMON_OBJECT_PATH := '/org/cinnamon/Muffin/DisplayConfig'

GEN_DIR := 'src/generated'
GNOME_OUTPUT := '{{GEN_DIR}}/gnome_proxy.rs'
CINNAMON_OUTPUT := '{{GEN_DIR}}/cinnamon_proxy.rs'

run:
    just install
    nohup /usr/local/bin/set-display -w auto --external product=Acer --mirror product="LG TV" 2>&1 >/home/tom/set-display.log &

install:
    sudo mkdir -p /usr/local/bin
    just build
    killall set-display || true
    sudo -u gdm killall set-display || true
    sudo cp target/debug/set-display /usr/local/bin/
    sudo cp set-display.desktop /usr/share/gdm/greeter/autostart/
    sudo cp set-display.desktop /etc/xdg/autostart/

status:
    systemctl --user status set-display.service

journal args:
    journalctl --user -u set-display.service "{{args}}"

gdm:
    just install

build:
    cargo build

clean:
    cargo clean

generate-proxy:
    #!/usr/bin/bash
    mkdir -p {{GEN_DIR}}
    if ! zbus-xmlgen --version; then
        cargo binstall -y zbus_xmlgen
    fi
    # Generate proxies
    zbus-xmlgen session --output {{GNOME_OUTPUT}} {{GNOME_SERVICE}} {{GNOME_OBJECT_PATH}}
    zbus-xmlgen session --output {{CINNAMON_OUTPUT}} {{CINNAMON_SERVICE}} {{CINNAMON_OBJECT_PATH}}
    # Generate mod.rs
    echo "// Auto-generated module declarations" > {{GEN_DIR}}/mod.rs
    for f in {{GEN_DIR}}/*.rs; do
        name=$(basename "$f" .rs)
        if [ "$name" = "mod" ]; then
            continue
        fi
        echo "pub mod ${name};" >> {{GEN_DIR}}/mod.rs
    done
