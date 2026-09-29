// A tiny platform shim: cxx-qt-lib 0.10 does not wrap QFontDatabase or QStyleHints, so the
// couple of calls Crumb needs live here behind plain C++ functions.

#include "native.h"

#include <QtCore/QMutex>
#include <QtCore/QMutexLocker>
#include <QtCore/QStringList>
#include <QtCore/QUrl>
#include <QtGui/QFontDatabase>
#include <QtGui/QGuiApplication>
#include <QtGui/QStyleHints>
#include <QtNetwork/QNetworkAccessManager>
#include <QtNetwork/QNetworkRequest>
#include <QtCore/QFile>
#include <QtGui/QImage>
#include <QtGui/QPainter>
#include <QtQml/QQmlApplicationEngine>
#include <QtQuick/QQuickImageProvider>
#include <QtSvg/QSvgRenderer>
#include <QtQml/QQmlNetworkAccessManagerFactory>

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


// ─── Photos ───
// QML's `Image` loads through the engine's own network managers, one per loader thread and
// made whenever the engine likes (often before sign-in). So the session isn't baked into a
// manager: each request reads the current one, and gets the cookie only when its scheme,
// host and port are exactly the server's.

namespace {

QMutex g_sessionLock;
QString g_origin;
QString g_cookie;

QString originOf(const QUrl &url) {
    const QString scheme = url.scheme().toLower();
    if (scheme != QStringLiteral("http") && scheme != QStringLiteral("https")) {
        return QString();
    }
    const int defaultPort = scheme == QStringLiteral("https") ? 443 : 80;
    const int port = url.port(defaultPort);
    QString host = url.host(QUrl::FullyEncoded).toLower();
    if (host.contains(QLatin1Char(':'))) {
        host = QStringLiteral("[") + host + QStringLiteral("]");
    }
    return port == defaultPort ? scheme + QStringLiteral("://") + host
                               : scheme + QStringLiteral("://") + host + QLatin1Char(':')
                                     + QString::number(port);
}

class SessionManager : public QNetworkAccessManager {
public:
    using QNetworkAccessManager::QNetworkAccessManager;

protected:
    QNetworkReply *createRequest(Operation op, const QNetworkRequest &original,
                                 QIODevice *data) override {
        QNetworkRequest request(original);
        QString origin;
        QString cookie;
        {
            QMutexLocker lock(&g_sessionLock);
            origin = g_origin;
            cookie = g_cookie;
        }
        if (!origin.isEmpty() && !cookie.isEmpty() && originOf(request.url()) == origin) {
            request.setRawHeader("Cookie", cookie.toUtf8());
        }
        return QNetworkAccessManager::createRequest(op, request, data);
    }
};

class SessionFactory : public QQmlNetworkAccessManagerFactory {
public:
    QNetworkAccessManager *create(QObject *parent) override {
        return new SessionManager(parent);
    }
};

} // namespace

// ─── Icons ───
// `image://icon/<name>/<color>`: a lucide icon from `:/icons`, drawn in the colour given
// (`#rrggbb` or `#aarrggbb`, the `#` URL-encoded or not) at the size the Image asks for.
// Lucide strokes in `currentColor`, which Qt would draw black.

namespace {

class IconProvider : public QQuickImageProvider {
public:
    IconProvider() : QQuickImageProvider(QQuickImageProvider::Image) {}

    QImage requestImage(const QString &id, QSize *size, const QSize &requested) override {
        const int slash = id.indexOf(QLatin1Char('/'));
        const QString name = slash < 0 ? id : id.left(slash);
        QString color = slash < 0 ? QStringLiteral("#000000") : id.mid(slash + 1);
        color = QUrl::fromPercentEncoding(color.toUtf8());
        if (!color.startsWith(QLatin1Char('#'))) {
            color.prepend(QLatin1Char('#'));
        }
        QFile file(QStringLiteral(":/icons/") + name + QStringLiteral(".svg"));
        const int w = requested.width() > 0 ? requested.width() : 24;
        const int h = requested.height() > 0 ? requested.height() : w;
        QImage image(w, h, QImage::Format_ARGB32_Premultiplied);
        image.fill(Qt::transparent);
        if (file.open(QIODevice::ReadOnly)) {
            QByteArray svg = file.readAll();
            svg.replace("currentColor", color.toUtf8());
            QSvgRenderer renderer(svg);
            QPainter painter(&image);
            painter.setRenderHint(QPainter::Antialiasing);
            renderer.render(&painter);
        }
        if (size) {
            *size = image.size();
        }
        return image;
    }
};

} // namespace

void crumbInstallNetwork(QQmlApplicationEngine &engine) {
    // The engine takes ownership of the factory and the provider
    engine.setNetworkAccessManagerFactory(new SessionFactory);
    engine.addImageProvider(QStringLiteral("icon"), new IconProvider);
}

void crumbSetPhotoSession(const QString &origin, const QString &cookie) {
    QMutexLocker lock(&g_sessionLock);
    g_origin = origin;
    g_cookie = cookie;
}
