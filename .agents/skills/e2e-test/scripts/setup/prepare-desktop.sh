#!/bin/bash
# The logged-in guest's normal account, after the desktop is ready.
set -euo pipefail
mkdir -p "$HOME/.local/bin" "$HOME/.local/share/muxy-vm"
printf 'SwiftVM Muxy sandbox\n' > "$HOME/.local/share/muxy-vm/managed"
git config --global user.name 'Muxy Test'
git config --global user.email 'muxy-test@example.invalid'
git config --global init.defaultBranch main
# Without a .zshrc, Linux zsh stops at its first-run wizard instead of a prompt.
test -e "$HOME/.zshrc" || printf '# Muxy test account\n' > "$HOME/.zshrc"
if [ "$(uname -s)" = Darwin ]; then
  # Clones get a new host ID, so the ByHost value alone doesn't reach them.
  defaults write com.apple.screensaver idleTime -int 0
  defaults -currentHost write com.apple.screensaver idleTime -int 0
fi
if [ "$(uname -s)" = Linux ]; then
  # No screen blanking, screen saver, lock or suspend: tests run unattended.
  gsettings set org.gnome.desktop.session idle-delay 0
  gsettings set org.gnome.desktop.screensaver idle-activation-enabled false
  gsettings set org.gnome.desktop.screensaver lock-enabled false
  gsettings set org.gnome.desktop.screensaver ubuntu-lock-on-suspend false
  gsettings set org.gnome.settings-daemon.plugins.power idle-dim false
  gsettings set org.gnome.settings-daemon.plugins.power sleep-inactive-ac-type nothing
  gsettings set org.gnome.desktop.interface enable-animations false
  gsettings set org.gnome.desktop.interface toolkit-accessibility true
  # Agent Desktop's Wayland screenshots go through the portal, whose consent dialog
  # takes no synthetic input. Grant it once here, as the dialog's Allow would.
  gdbus call --session --dest org.freedesktop.impl.portal.PermissionStore \
    --object-path /org/freedesktop/impl/portal/PermissionStore \
    --method org.freedesktop.impl.portal.PermissionStore.SetPermission \
    screenshot true screenshot '' "['yes']" > /dev/null
  mkdir -p "$HOME/.local/share/applications"
  cat > "$HOME/.local/share/applications/muxy-vm.desktop" <<EOF
[Desktop Entry]
Type=Application
Name=Muxy VM
Exec=$HOME/.local/bin/muxy-vm-run
Terminal=false
Categories=Development;TerminalEmulator;
EOF
fi
