#pragma once

/// Records any Qt warning/critical/fatal message and prints it to stderr.
void crumbSmokeInstallHandler();

/// True once any warning or error was seen.
bool crumbSmokeFailed();
