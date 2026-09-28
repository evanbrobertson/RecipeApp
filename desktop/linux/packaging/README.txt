Crumb - Linux desktop app
=========================

Crumb is a private recipe box. This is the native Qt6/QML desktop app. It talks to a Crumb
server (the same one the web app uses) and keeps your login in the system keyring.

Requirements
------------

  * Qt 6.5 or newer (qt6-base, qt6-declarative, qt6-wayland)
  * A Crumb server to sign in to

Install
-------

  ./install.sh

This installs crumb-desktop into ~/.local/bin, the desktop entry into
~/.local/share/applications and the icon into
~/.local/share/icons/hicolor/512x512/apps. Make sure ~/.local/bin is on your PATH and the
app will appear in your launcher.

Uninstall
---------

  ./install.sh --uninstall

Settings live under ~/.config/crumb and the saved session lives in your system keyring;
uninstalling leaves both in place.

Crumb is MIT licensed; see LICENSE.
