#pragma once

#include <QtCore/QString>

/// Records any Qt warning/critical/fatal message and prints it to stderr.
void crumbSmokeInstallHandler();

/// True once any warning or error was seen.
bool crumbSmokeFailed();

/// Sends a mouse event to the app's window at window coordinates: 0 move, 1 press, 2 release.
/// For smoke scenarios that hover and click (the shelf's).
void crumbSmokePointer(double x, double y, int kind);

/// Saves the app's window, overlays and all, as an image; false if it couldn't.
bool crumbSmokeGrab(const QString &path);
