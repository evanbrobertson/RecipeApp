// A tiny shim so `crumb-desktop --smoke` can fail on any Qt/QML warning.
//
// `qInstallMessageHandler` takes a native C++ callback, which is awkward to declare
// through cxx, so the handler lives here and Rust only sees the two plain functions.

#include <QtCore/QByteArray>
#include <QtCore/QString>
#include <QtCore/qlogging.h>
#include <QtGui/QGuiApplication>
#include <QtGui/QMouseEvent>
#include <QtGui/QWindow>
#include <QtQuick/QQuickWindow>

#include <cstdio>
#include <cstring>

namespace {

bool g_failed = false;

void crumbSmokeHandler(QtMsgType type, const QMessageLogContext &context, const QString &message) {
    if (type != QtWarningMsg && type != QtCriticalMsg && type != QtFatalMsg) {
        return;
    }
    const char *category = context.category != nullptr ? context.category : "";
    // Platform-plugin chatter is not the QML we are checking.
    if (std::strncmp(category, "qt.qpa", 6) == 0) {
        return;
    }
    if (message.contains(QStringLiteral("XDG_RUNTIME_DIR"))) {
        return;
    }
    g_failed = true;
    const QByteArray text = message.toLocal8Bit();
    std::fprintf(stderr, "crumb-desktop: %s\n", text.constData());
    std::fflush(stderr);
}

} // namespace

void crumbSmokeInstallHandler() {
    qInstallMessageHandler(crumbSmokeHandler);
}

bool crumbSmokeFailed() {
    return g_failed;
}

namespace {

QWindow *crumbSmokeWindow() {
    for (QWindow *candidate : QGuiApplication::topLevelWindows()) {
        if (candidate->isVisible()) {
            return candidate;
        }
    }
    return nullptr;
}

} // namespace

void crumbSmokePointer(double x, double y, int kind) {
    QWindow *window = crumbSmokeWindow();
    if (window == nullptr) {
        return;
    }
    const QPointF at(x, y);
    const QEvent::Type type = kind == 1   ? QEvent::MouseButtonPress
                              : kind == 2 ? QEvent::MouseButtonRelease
                                          : QEvent::MouseMove;
    const Qt::MouseButton button = kind == 0 ? Qt::NoButton : Qt::LeftButton;
    const Qt::MouseButtons buttons = kind == 1 ? Qt::LeftButton : Qt::NoButton;
    QMouseEvent event(type, at, window->mapToGlobal(at), button, buttons, Qt::NoModifier);
    QGuiApplication::sendEvent(window, &event);
}

bool crumbSmokeGrab(const QString &path) {
    auto *window = qobject_cast<QQuickWindow *>(crumbSmokeWindow());
    return window != nullptr && window->grabWindow().save(path);
}
