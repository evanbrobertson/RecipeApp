#pragma once

#include <QtCore/QString>

/// Registers a font file (including WOFF2, when Qt's FreeType has Brotli) and returns the
/// Qt font id, or -1 on failure.
int crumbAddApplicationFont(const QString &path);

/// The first family name registered for a font id, or an empty string.
QString crumbApplicationFontFamily(int id);

/// Whether the desktop prefers a dark colour scheme.
bool crumbPrefersDark();

/// Puts text on the system clipboard.
void crumbSetClipboardText(const QString &text);

/// Opens an http(s) URL in the user's default browser; false when nothing handled it.
bool crumbOpenUrl(const QString &url);
