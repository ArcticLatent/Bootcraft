// Application-wide Qt settings that cxx-qt-lib doesn't expose: bundled
// fonts, the default font, the window icon and the desktop app id.
#pragma once

#include <QtCore/QString>
#include <QtGui/QFont>
#include <QtGui/QFontDatabase>
#include <QtGui/QGuiApplication>
#include <QtGui/QIcon>

namespace bootcraft {

inline bool add_application_font(const QString &path)
{
    return QFontDatabase::addApplicationFont(path) >= 0;
}

/// Plain `Text` items use the application font, not the window's.
inline void set_application_font_family(const QString &family)
{
    QFont font = QGuiApplication::font();
    font.setFamily(family);
    QGuiApplication::setFont(font);
}

inline void set_window_icon(const QString &path)
{
    QGuiApplication::setWindowIcon(QIcon(path));
}

/// The Wayland app id; matches the installed .desktop file's name.
inline void set_desktop_file_name(const QString &name)
{
    QGuiApplication::setDesktopFileName(name);
}

} // namespace bootcraft
