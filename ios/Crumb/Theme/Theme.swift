import CrumbKit
import SwiftUI
import UIKit

/// The Green Tile tokens from web/src/styles/app.css, light and "evening kitchen" dark
/// (the same values as android/…/ui/theme/Theme.kt). Each colour follows the window's
/// light/dark style, which CrumbApp sets from the theme setting.
enum Palette {
  static let canvas = dynamic(0xF5F1E6, 0x141C17)
  static let sunk = dynamic(0xEFEADD, 0x18211B)
  static let paper = dynamic(0xFFFDF8, 0x1D2721)
  static let tint = dynamic(0xE4ECE3, 0x24322A)
  static let line = dynamic(0xE3DFD0, 0x2C3830)
  static let lineStrong = dynamic(0xD3CEBD, 0x36453B)
  static let ink = dynamic(0x1C2B22, 0xEFE9DA)
  static let inkMuted = dynamic(0x56635A, 0xA8B3AA)
  static let primary = dynamic(0x2F6B4F, 0x93C4A3)
  static let tile = dynamic(0x2F6B4F, 0x3A7859)
  static let onTile = dynamic(0xFFFDF8, 0xF5F0E2)
  static let butter = dynamic(0xF3DA8B, 0xF0D582)
  static let onButter = dynamic(0x1C2B22, 0x141C17)
  static let nav = dynamic(0x1C2B22, 0x0D130F)
  static let navInk = dynamic(0xB7C4BB, 0x8E9C92)
  static let error = dynamic(0xA8432C, 0xE59A83)
  static let accented = dynamic(0xD6E2D5, 0x2C3D33)

  /// A cookbook's cloth and foil (web/src/lib/books.ts's BOOK_PALETTE; the same in light
  /// and dark). The stored name is normalised by crumb-core's `bookColor`.
  static func book(_ name: String?) -> (cloth: Color, shade: Color, foil: Color) {
    switch CrumbCore.bookColor(name: (name ?? "").lowercased()) ?? "tile" {
    case "forest": return (Color(hex: 0x1C2B22), Color(hex: 0x111A15), Color(hex: 0xF3DA8B))
    case "sage": return (Color(hex: 0xA9C4AE), Color(hex: 0x8AA890), Color(hex: 0x1C2B22))
    case "butter": return (Color(hex: 0xF3DA8B), Color(hex: 0xD9BD67), Color(hex: 0x1C2B22))
    case "clay": return (Color(hex: 0xA55A40), Color(hex: 0x7E412D), Color(hex: 0xFFF8EA))
    case "cream": return (Color(hex: 0xF8F3E6), Color(hex: 0xDDD5C1), Color(hex: 0x1C2B22))
    default: return (Color(hex: 0x2F6B4F), Color(hex: 0x224F3A), Color(hex: 0xFFFDF8))
    }
  }

  private static func dynamic(_ light: UInt32, _ dark: UInt32) -> Color {
    Color(
      uiColor: UIColor { traits in
        UIColor(hex: traits.userInterfaceStyle == .dark ? dark : light)
      })
  }
}

extension UIColor {
  convenience init(hex: UInt32) {
    self.init(
      red: CGFloat((hex >> 16) & 0xFF) / 255, green: CGFloat((hex >> 8) & 0xFF) / 255,
      blue: CGFloat(hex & 0xFF) / 255, alpha: 1)
  }
}

extension Color {
  init(hex: UInt32) {
    self.init(uiColor: UIColor(hex: hex))
  }
}

/// Four radii only: 16 cards, 12 controls and photos, full pills, and book spines.
enum Radius {
  static let card: CGFloat = 16
  static let control: CGFloat = 12
}

/// Nunito Sans for text, DM Serif Display for titles, Caveat for greetings and the cook's
/// notes only. All scale with Dynamic Type.
enum Typeface {
  static func body(
    _ size: CGFloat = 17, weight: Font.Weight = .regular, relativeTo style: Font.TextStyle = .body
  )
    -> Font
  {
    let name: String
    switch weight {
    case .semibold: name = "NunitoSans-SemiBold"
    case .bold: name = "NunitoSans-Bold"
    case .heavy, .black: name = "NunitoSans-ExtraBold"
    default: name = "NunitoSans-Regular"
    }
    return .custom(name, size: size, relativeTo: style)
  }

  static func serif(_ size: CGFloat, relativeTo style: Font.TextStyle = .title) -> Font {
    .custom("DMSerifDisplay-Regular", size: size, relativeTo: style)
  }

  static func hand(_ size: CGFloat, relativeTo style: Font.TextStyle = .title2) -> Font {
    .custom("Caveat-Bold", size: size, relativeTo: style)
  }

  static let display = serif(34, relativeTo: .largeTitle)
  static let headline = serif(28, relativeTo: .title)
  static let title = serif(22, relativeTo: .title2)
  static let subtitle = body(15, weight: .bold, relativeTo: .headline)
  static let label = body(15, weight: .bold, relativeTo: .callout)
  static let small = body(13, weight: .semibold, relativeTo: .footnote)
  static let caption = body(13, relativeTo: .footnote)
}

extension View {
  /// The app's canvas behind a whole screen.
  func canvasBackground() -> some View {
    background(Palette.canvas.ignoresSafeArea())
  }
}
