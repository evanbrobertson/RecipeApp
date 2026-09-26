#!/usr/bin/env python3
"""Native Linux window for the same Crumb server and pages used on the web."""

import ctypes
import os
from pathlib import Path
import signal
import socket
import subprocess
import sys
import time
from urllib.parse import unquote, urlparse
import urllib.request
import gi

# WebKitGTK's DMA-BUF renderer dies with "Error 71 (Protocol error)" on NVIDIA under Wayland.
os.environ.setdefault("WEBKIT_DISABLE_DMABUF_RENDERER", "1")
gi.require_version("Gtk", "3.0")
gi.require_version("WebKit2", "4.1")
from gi.repository import Gio, GLib, Gtk, WebKit2  # noqa: E402

DATA = Path(os.environ.get("XDG_DATA_HOME", Path.home() / ".local/share")) / "crumb"
ROOT = Path(os.environ.get("CRUMB_DESKTOP_ROOT", DATA.parent / "crumb-desktop"))
sys.path.insert(0, str(ROOT / "desktop"))
from omarchy_theme import read_theme, theme_file  # noqa: E402

GLib.set_prgname("crumb-desktop")  # Wayland app_id, matched by the launcher entry


def _die_with_parent():
    # The server must not outlive this window, even if the window crashes.
    PR_SET_PDEATHSIG = 1
    ctypes.CDLL(None, use_errno=True).prctl(PR_SET_PDEATHSIG, signal.SIGTERM)


class CrumbDesktop(Gtk.Application):
    def __init__(self):
        super().__init__(application_id="app.crumb.Desktop", flags=Gio.ApplicationFlags.FLAGS_NONE)
        self.server = None
        self.window = None
        self.webview = None
        self.url = None
        self.theme = None
        self.started_at = 0

    def do_activate(self):
        if self.window:
            self.window.present()
            return
        DATA.mkdir(parents=True, exist_ok=True)
        binary = ROOT / "bin/crumb"
        assets = ROOT / "web"
        if not binary.is_file() or not (assets / "index.html").is_file():
            self._error("Crumb is not installed. Run scripts/install-desktop.sh from the repository.")
            return

        # An ephemeral loopback port avoids conflicting with a running web instance.
        with socket.socket() as sock:
            sock.bind(("127.0.0.1", 0))
            port = sock.getsockname()[1]
        self.url = f"http://127.0.0.1:{port}"
        env = dict(os.environ, HOST="127.0.0.1", PORT=str(port),
                   WEB_DIST=str(assets), DATABASE_PATH=str(DATA / "recipes.db"))
        try:
            self.server = subprocess.Popen(
                [str(binary)], env=env, cwd=ROOT, preexec_fn=_die_with_parent
            )
        except OSError as error:
            self._error(f"Crumb's local server could not start: {error}")
            return
        self.started_at = time.monotonic()

        self.window = Gtk.ApplicationWindow(application=self, title="Crumb")
        self.window.set_default_size(1180, 820)
        self.window.set_size_request(540, 420)
        self.window.connect("destroy", self._stop)
        self.window.add(Gtk.Label(label="Opening Crumb…"))
        self.window.show_all()
        GLib.timeout_add(120, self._ready)

    def _ready(self):
        if self.server.poll() is not None:
            self._error("Crumb's local server could not start. Check the terminal output.")
            return False
        if time.monotonic() - self.started_at > 20:
            self._error("Crumb's local server did not respond within 20 seconds.")
            return False
        try:
            with urllib.request.urlopen(self.url, timeout=0.15) as response:
                response.read(1)
        except Exception:
            return True
        self._open_webview()
        return False

    def _open_webview(self):
        webdata = DATA / "webview"
        cache = Path(os.environ.get("XDG_CACHE_HOME", Path.home() / ".cache")) / "crumb-desktop"
        webdata.mkdir(parents=True, exist_ok=True)
        cache.mkdir(parents=True, exist_ok=True)
        manager = WebKit2.WebsiteDataManager(base_data_directory=str(webdata),
                                             base_cache_directory=str(cache))
        context = WebKit2.WebContext.new_with_website_data_manager(manager)
        context.get_cookie_manager().set_persistent_storage(
            str(webdata / "cookies.sqlite"), WebKit2.CookiePersistentStorage.SQLITE)
        self.webview = WebKit2.WebView.new_with_context(context)
        self.webview.connect("decide-policy", self._navigation)
        self.webview.connect("create", self._new_window)
        self.webview.connect("notify::title", self._title)
        self.webview.connect("print", self._print)
        self.webview.get_context().connect("download-started", self._download)
        self.window.get_child().destroy()
        self.window.add(self.webview)
        self._sync_theme()
        GLib.timeout_add_seconds(2, self._watch_theme)
        self.webview.load_uri(self.url)
        self.window.show_all()

    def _navigation(self, view, decision, decision_type):
        if decision_type not in (WebKit2.PolicyDecisionType.NAVIGATION_ACTION,
                                 WebKit2.PolicyDecisionType.NEW_WINDOW_ACTION):
            return False
        uri = decision.get_navigation_action().get_request().get_uri()
        parsed = urlparse(uri)
        # Internal pages stay in this window; other links use the system browser.
        if uri.startswith(self.url + "/") or uri == self.url:
            if decision_type == WebKit2.PolicyDecisionType.NEW_WINDOW_ACTION:
                decision.ignore()
                self.webview.load_uri(uri)
                return True
            return False
        if parsed.scheme in ("https", "http", "mailto"):
            Gio.AppInfo.launch_default_for_uri(uri, None)
        decision.ignore()
        return True

    def _new_window(self, view, navigation_action):
        # NEW_WINDOW_ACTION is handled by decide-policy; do not create a second webview.
        return None

    def _title(self, view, param):
        self.window.set_title(view.get_title() or "Crumb")

    def _print(self, view, print_operation):
        # WebKit's default handler shows the native print dialog.
        return False

    def _download(self, context, download):
        download.connect("decide-destination", self._download_destination)

    def _download_destination(self, download, suggested_filename):
        directory = Path.home() / "Downloads"
        directory.mkdir(exist_ok=True)
        name = Path(unquote(suggested_filename)).name or "recipe.json"
        path = directory / name
        stem, suffix = path.stem, path.suffix
        count = 1
        while path.exists():
            path = directory / f"{stem} ({count}){suffix}"
            count += 1
        download.set_destination(path.as_uri())
        return True

    def _watch_theme(self):
        if not self.webview:
            return False
        self._sync_theme()
        return True

    def _sync_theme(self):
        updated = read_theme(theme_file())
        if updated == self.theme:
            return
        self.theme = updated
        manager = self.webview.get_user_content_manager()
        manager.remove_all_style_sheets()
        manager.remove_all_scripts()
        if updated is None:
            return  # Original Crumb theme on desktops without Omarchy.
        mode, css = updated
        manager.add_style_sheet(WebKit2.UserStyleSheet.new(
            css, WebKit2.UserContentInjectedFrames.TOP_FRAME,
            WebKit2.UserStyleLevel.USER, None, None))
        script = f'localStorage.setItem("crumb:theme", "{mode}");'
        manager.add_script(WebKit2.UserScript.new(
            script, WebKit2.UserContentInjectedFrames.TOP_FRAME,
            WebKit2.UserScriptInjectionTime.START, None, None))
        # Apply a theme change without reloading an unfinished recipe editor.
        self.webview.evaluate_javascript(
            script + "window.crumbTheme?.apply();", -1, None, None, None, None, None
        )

    def _error(self, message):
        dialog = Gtk.MessageDialog(transient_for=self.window, modal=True,
                                   message_type=Gtk.MessageType.ERROR,
                                   buttons=Gtk.ButtonsType.CLOSE, text=message)
        dialog.run()
        dialog.destroy()
        self.quit()

    def _stop(self, window):
        self.quit()

    def do_shutdown(self):
        # Runs on every quit, including the error paths that never open a window.
        if self.server and self.server.poll() is None:
            self.server.terminate()
            try:
                self.server.wait(timeout=5)
            except subprocess.TimeoutExpired:
                self.server.kill()
                self.server.wait()
        Gtk.Application.do_shutdown(self)


if __name__ == "__main__":
    sys.exit(CrumbDesktop().run(sys.argv))
