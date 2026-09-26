import CrumbKit
import SwiftUI
import UIKit

/// Butter: only the one main action on a screen.
struct PrimaryButtonStyle: ButtonStyle {
  @Environment(\.isEnabled) private var enabled

  func makeBody(configuration: Configuration) -> some View {
    configuration.label
      .font(Typeface.label)
      .frame(maxWidth: .infinity, minHeight: 50)
      .padding(.horizontal, 16)
      .foregroundStyle(enabled ? Palette.onButter : Palette.inkMuted)
      .background(
        RoundedRectangle(cornerRadius: Radius.control, style: .continuous)
          .fill(enabled ? Palette.butter : Palette.sunk)
          .opacity(configuration.isPressed ? 0.85 : 1)
      )
      .contentShape(RoundedRectangle(cornerRadius: Radius.control, style: .continuous))
  }
}

/// Paper with a strong line: everything that isn't the main action.
struct SecondaryButtonStyle: ButtonStyle {
  var fill = true

  func makeBody(configuration: Configuration) -> some View {
    configuration.label
      .font(Typeface.label)
      .frame(maxWidth: fill ? .infinity : nil, minHeight: 46)
      .padding(.horizontal, 16)
      .foregroundStyle(Palette.ink)
      .background(
        RoundedRectangle(cornerRadius: Radius.control, style: .continuous)
          .fill(configuration.isPressed ? Palette.accented : Palette.paper)
      )
      .overlay(
        RoundedRectangle(cornerRadius: Radius.control, style: .continuous)
          .strokeBorder(Palette.lineStrong, lineWidth: 1)
      )
      .contentShape(RoundedRectangle(cornerRadius: Radius.control, style: .continuous))
  }
}

extension ButtonStyle where Self == PrimaryButtonStyle {
  static var crumbPrimary: PrimaryButtonStyle { PrimaryButtonStyle() }
}

extension ButtonStyle where Self == SecondaryButtonStyle {
  static var crumbSecondary: SecondaryButtonStyle { SecondaryButtonStyle() }
  static var crumbSecondaryCompact: SecondaryButtonStyle { SecondaryButtonStyle(fill: false) }
}

/// A flat paper card with the 1px line border.
struct PaperCard: ViewModifier {
  func body(content: Content) -> some View {
    content
      .background(
        RoundedRectangle(cornerRadius: Radius.card, style: .continuous).fill(Palette.paper)
      )
      .overlay(
        RoundedRectangle(cornerRadius: Radius.card, style: .continuous).strokeBorder(Palette.line, lineWidth: 1)
      )
  }
}

extension View {
  func paperCard() -> some View { modifier(PaperCard()) }
}

/// A small rounded label: "Total 1h 5m", "Serves 8".
struct Pill: View {
  var text: String
  var systemImage: String?

  var body: some View {
    HStack(spacing: 5) {
      if let systemImage {
        Image(systemName: systemImage).imageScale(.small)
      }
      Text(text)
    }
    .font(Typeface.small)
    .foregroundStyle(Palette.ink)
    .padding(.horizontal, 12)
    .padding(.vertical, 6)
    .background(Capsule().fill(Palette.tint))
  }
}

/// A recipe photo through the server's resizer (crumb-core's photo URLs, the session
/// cookie attached). If that fails, the original image when it's a web address, as the web
/// app does; otherwise a quiet tinted tile.
struct RecipePhoto: View {
  var recipeId: Int64
  var image: String?
  /// About how wide it's shown, in points.
  var width: CGFloat

  @Environment(AppModel.self) private var app
  @Environment(\.displayScale) private var scale
  @State private var loaded: UIImage?
  @State private var failed = false

  var body: some View {
    ZStack {
      Palette.tint
      if let loaded {
        Image(uiImage: loaded).resizable().scaledToFill()
      } else if failed || image == nil {
        Image(systemName: "fork.knife")
          .font(.system(size: 22, weight: .medium))
          .foregroundStyle(Palette.primary.opacity(0.5))
      }
    }
    .clipped()
    .accessibilityHidden(true)
    .task(id: "\(recipeId)|\(image ?? "")|\(Int(width))") {
      await load()
    }
  }

  private func load() async {
    failed = false
    guard let image else { return }
    let pixels = Int((width * scale).rounded(.up))
    if let url = app.api.photoURL(recipeId: recipeId, image: image, pixels: pixels),
      let resized = await PhotoPipeline.shared.image(for: app.api.photoRequest(url))
    {
      loaded = resized
      return
    }
    if image.hasPrefix("https://") || image.hasPrefix("http://"), let url = URL(string: image),
      let original = await PhotoPipeline.shared.image(for: URLRequest(url: url))
    {
      loaded = original
      return
    }
    failed = true
  }
}

/// Recipe photos, kept in memory and in a 200 MB disk cache so recipes opened before still
/// have their pictures offline. Photo URLs change when the image does (`?v=`), so cached
/// responses never go stale.
final class PhotoPipeline: @unchecked Sendable {
  static let shared = PhotoPipeline()

  private let memory = NSCache<NSURL, UIImage>()
  private let session: URLSession

  init() {
    let config = URLSessionConfiguration.default
    config.httpShouldSetCookies = false
    config.httpCookieStorage = nil
    config.requestCachePolicy = .returnCacheDataElseLoad
    config.urlCache = URLCache(memoryCapacity: 16 << 20, diskCapacity: 200 << 20, directory: nil)
    config.timeoutIntervalForRequest = 30
    session = URLSession(configuration: config)
    memory.countLimit = 150
  }

  func image(for request: URLRequest) async -> UIImage? {
    guard let url = request.url else { return nil }
    if let hit = memory.object(forKey: url as NSURL) { return hit }
    guard let result = try? await session.data(for: request),
      let http = result.1 as? HTTPURLResponse, (200..<300).contains(http.statusCode),
      let image = UIImage(data: result.0)
    else { return nil }
    let prepared = await image.byPreparingForDisplay() ?? image
    memory.setObject(prepared, forKey: url as NSURL)
    return prepared
  }

  func clear() {
    memory.removeAllObjects()
    session.configuration.urlCache?.removeAllCachedResponses()
  }
}

/// "You're offline: showing what's on this iPhone."
struct OfflineNote: View {
  var body: some View {
    Label("Offline: showing the copy saved on this iPhone", systemImage: "wifi.slash")
      .font(Typeface.small)
      .foregroundStyle(Palette.ink)
      .frame(maxWidth: .infinity, alignment: .leading)
      .padding(12)
      .background(RoundedRectangle(cornerRadius: Radius.control, style: .continuous).fill(Palette.tint))
      .accessibilityIdentifier("offline-note")
  }
}

/// A centred message for empty and failed screens, with an optional action.
struct MessageView<Action: View>: View {
  var title: String
  var message: String?
  var systemImage: String?
  @ViewBuilder var action: () -> Action

  var body: some View {
    VStack(spacing: 12) {
      if let systemImage {
        Image(systemName: systemImage)
          .font(.system(size: 36, weight: .regular))
          .foregroundStyle(Palette.primary)
          .padding(.bottom, 4)
      }
      Text(title)
        .font(Typeface.title)
        .foregroundStyle(Palette.ink)
        .multilineTextAlignment(.center)
      if let message {
        Text(message)
          .font(Typeface.body(15))
          .foregroundStyle(Palette.inkMuted)
          .multilineTextAlignment(.center)
      }
      action().padding(.top, 8)
    }
    .padding(32)
    .frame(maxWidth: 440)
    .frame(maxWidth: .infinity, maxHeight: .infinity)
  }
}

extension MessageView where Action == EmptyView {
  init(title: String, message: String? = nil, systemImage: String? = nil) {
    self.init(title: title, message: message, systemImage: systemImage) { EmptyView() }
  }
}

struct LoadingView: View {
  var body: some View {
    ProgressView()
      .tint(Palette.primary)
      .controlSize(.large)
      .frame(maxWidth: .infinity, maxHeight: .infinity)
  }
}

/// A screen that loads one thing: spinner, content (maybe offline), or a failure with
/// Try again. A 401 signs out, as it does everywhere in the app.
enum LoadState<T> {
  case loading
  case ready(T, offline: Bool)
  case failed(String)

  var value: T? {
    if case .ready(let value, _) = self { return value }
    return nil
  }
}

/// A small heading over a list: "Ingredients", "Method".
struct SectionHeading: View {
  var text: String

  var body: some View {
    Text(text)
      .font(Typeface.title)
      .foregroundStyle(Palette.ink)
      .frame(maxWidth: .infinity, alignment: .leading)
      .accessibilityAddTraits(.isHeader)
  }
}

/// Text fields on paper with the line border.
struct CrumbFieldStyle: TextFieldStyle {
  func _body(configuration: TextField<Self._Label>) -> some View {
    configuration
      .font(Typeface.body(17))
      .foregroundStyle(Palette.ink)
      .padding(.horizontal, 14)
      .frame(minHeight: 50)
      .background(RoundedRectangle(cornerRadius: Radius.control, style: .continuous).fill(Palette.paper))
      .overlay(
        RoundedRectangle(cornerRadius: Radius.control, style: .continuous)
          .strokeBorder(Palette.lineStrong, lineWidth: 1)
      )
  }
}
