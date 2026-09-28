// A tiny platform shim: cxx-qt-lib 0.10 does not wrap QFontDatabase or QStyleHints, so the
// couple of calls Crumb needs live here behind plain C++ functions.

#include "native.h"

#include <QtCore/QStringList>
#include <QtCore/QUrl>
#include <QtGui/QClipboard>
#include <QtGui/QDesktopServices>
#include <QtGui/QFontDatabase>
#include <QtGui/QGuiApplication>
#include <QtGui/QStyleHints>

int crumbAddApplicationFont(const QString &path) {
    return QFontDatabase::addApplicationFont(path);
}

QString crumbApplicationFontFamily(int id) {
    const QStringList families = QFontDatabase::applicationFontFamilies(id);
    return families.isEmpty() ? QString() : families.first();
}

bool crumbPrefersDark() {
#if QT_VERSION >= QT_VERSION_CHECK(6, 5, 0)
    // Unknown (no desktop hint) prefers dark, matching Crumb's default palette.
    return QGuiApplication::styleHints()->colorScheme() != Qt::ColorScheme::Light;
#else
    return true;
#endif
}

void crumbSetClipboardText(const QString &text) {
    QGuiApplication::clipboard()->setText(text);
}

bool crumbOpenUrl(const QString &url) {
    return QDesktopServices::openUrl(QUrl(url));
}
