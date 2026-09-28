// A tiny shim so `crumb-desktop --smoke` can fail on any Qt/QML warning.
//
// `qInstallMessageHandler` takes a native C++ callback, which is awkward to declare
// through cxx, so the handler lives here and Rust only sees the two plain functions.

#include <QtCore/QByteArray>
#include <QtCore/QString>
#include <QtCore/qlogging.h>

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
