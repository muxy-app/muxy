#!/bin/bash
set -euo pipefail
export DEBIAN_FRONTEND=noninteractive
apt-get -y -o DPkg::Lock::Timeout=900 update
apt-get -y -o DPkg::Lock::Timeout=900 install build-essential pkg-config clang cmake libssl-dev libfontconfig1-dev libfreetype-dev libxkbcommon-dev libxkbcommon-x11-dev libwayland-dev libxcb1-dev libvulkan-dev libwebkit2gtk-4.1-0 mesa-vulkan-drivers vulkan-tools xdg-utils xdg-desktop-portal xdg-desktop-portal-gnome bubblewrap xdg-dbus-proxy xvfb zsh fish binutils vim-tiny less unzip git curl ca-certificates python3 xz-utils dbus-x11 wl-clipboard xclip
sed -i 's/^Prompt=.*/Prompt=never/' /etc/update-manager/release-upgrades
apt-get clean
