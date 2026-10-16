#!/usr/bin/env python3
"""Exercise the desktop's system WebKitGTK, JavaScript and TLS backend under a test display."""

import ctypes as c
from pathlib import Path
import shutil
import subprocess
import time


def verify():
    def load(name):
        return c.CDLL(name, mode=c.RTLD_GLOBAL)
    gtk = load("libgtk-3.so.0")
    webkit = load("libwebkit2gtk-4.1.so.0")
    gio = load("libgio-2.0.so.0")
    glib = load("libglib-2.0.so.0")
    gtk.gtk_init_check.argtypes = [c.c_void_p, c.c_void_p]
    assert gtk.gtk_init_check(None, None), "GTK could not open the test display"
    gio.g_tls_backend_get_default.restype = c.c_void_p
    gio.g_tls_backend_supports_tls.argtypes = [c.c_void_p]
    assert gio.g_tls_backend_supports_tls(gio.g_tls_backend_get_default()), "System TLS backend is unavailable"
    webkit.webkit_web_context_new.restype = c.c_void_p
    context = webkit.webkit_web_context_new()
    webkit.webkit_web_context_set_sandbox_enabled.argtypes = [c.c_void_p, c.c_int]
    sandbox = bool(Path("/usr/bin/xdg-dbus-proxy").exists() and shutil.which("bwrap")) and subprocess.run(
        ["bwrap", "--ro-bind", "/", "/", "true"], capture_output=True).returncode == 0
    webkit.webkit_web_context_set_sandbox_enabled(context, int(sandbox))
    webkit.webkit_web_view_new_with_context.argtypes = [c.c_void_p]
    webkit.webkit_web_view_new_with_context.restype = c.c_void_p
    view = webkit.webkit_web_view_new_with_context(context)
    webkit.webkit_web_view_load_html.argtypes = [c.c_void_p, c.c_char_p, c.c_char_p]
    webkit.webkit_web_view_get_title.argtypes = [c.c_void_p]
    webkit.webkit_web_view_get_title.restype = c.c_char_p
    glib.g_main_context_iteration.argtypes = [c.c_void_p, c.c_int]
    webkit.webkit_web_view_load_html(view, b"<html><script>document.title='Muxy system WebKit works'</script></html>", b"https://example.invalid/")
    deadline = time.monotonic() + 30
    while webkit.webkit_web_view_get_title(view) != b"Muxy system WebKit works":
        if time.monotonic() >= deadline:
            raise RuntimeError("System WebKit did not execute the page script")
        glib.g_main_context_iteration(None, 0)
        time.sleep(0.01)
    gtk.gtk_widget_destroy.argtypes = [c.c_void_p]
    gtk.gtk_widget_destroy(view)
    print(f"System WebKit rendered a scripted page; TLS available; sandbox={sandbox}")


if __name__ == "__main__":
    verify()
