import CrumbKit
import SwiftUI
import UIKit
import UniformTypeIdentifiers

/// Share → Crumb: a page from Safari, text from Notes, or photos of a recipe card go
/// straight to your Crumb with the app's session (the Keychain and App Group are shared).
/// Nothing is parsed here; the server scrapes, reads and saves, as it does for the web.
final class ShareViewController: UIViewController {
  private let model = ShareModel()

  override func viewDidLoad() {
    super.viewDidLoad()
    let host = UIHostingController(rootView: ShareView(model: model, close: { [weak self] in self?.finish() }))
    addChild(host)
    host.view.frame = view.bounds
    host.view.autoresizingMask = [.flexibleWidth, .flexibleHeight]
    host.view.backgroundColor = .clear
    view.addSubview(host.view)
    host.didMove(toParent: self)

    let items = (extensionContext?.inputItems as? [NSExtensionItem]) ?? []
    Task { await model.save(items) }
  }

  private func finish() {
    extensionContext?.completeRequest(returningItems: nil)
  }
}

@Observable
@MainActor
final class ShareModel {
  enum Phase: Equatable {
    case working(String)
    case saved(title: String, detail: String)
    case failed(String)
  }

  var phase: Phase = .working("Saving to Crumb…")

  func save(_ items: [NSExtensionItem]) async {
    let sessions = SessionStore(secrets: KeychainStore())
    guard sessions.current != nil else {
      phase = .failed("Open Crumb and sign in first, then share again.")
      return
    }
    let api = CrumbAPI(session: { sessions.current })
    do {
      let shared = await SharedContent.load(from: items)
      let result: ImportResult
      if !shared.photos.isEmpty {
        phase = .working("Wee Chef is reading your photos…")
        result = try await api.importPhotos(shared.photos, hint: shared.text)
      } else if let text = shared.importText {
        let link = CrumbCore.classifyImport(raw: text).isLink
        phase = .working(link ? "Fetching the recipe…" : "Reading the recipe…")
        result = try await api.importInput(text)
      } else {
        phase = .failed("There's no link, text or photo here for Crumb to save.")
        return
      }
      if let book = result.cookbook {
        phase = .saved(
          title: "Saved the cookbook",
          detail: CrumbCore.bookImportedTitle(
            name: book.name, added: UInt32(clamping: book.added ?? 0),
            duplicates: UInt32(clamping: book.duplicates ?? 0), skipped: book.skipped.map { UInt32(clamping: $0) }))
      } else {
        phase = .saved(title: result.isNew ? "Saved to Crumb" : "Already in your box", detail: result.title)
      }
      UINotificationFeedbackGenerator().notificationOccurred(.success)
    } catch {
      phase = .failed(error.friendlyMessage)
    }
  }
}

extension Optional where Wrapped == ImportInput {
  fileprivate var isLink: Bool {
    if case .link = self { return true }
    return false
  }
}

/// What was shared: a page's link (with its title), plain text, and photos.
struct SharedContent {
  var url: URL?
  var text: String?
  var photos: [Data] = []

  /// The text to import: the link when there is one (crumb-core pulls it out of "Title
  /// https://…" too), else the text itself.
  var importText: String? {
    if let url { return url.absoluteString }
    return text?.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty == false ? text : nil
  }

  static func load(from items: [NSExtensionItem]) async -> SharedContent {
    var content = SharedContent()
    for item in items {
      for provider in item.attachments ?? [] {
        if content.url == nil, provider.hasItemConformingToTypeIdentifier(UTType.url.identifier),
          let url = try? await provider.loadItem(forTypeIdentifier: UTType.url.identifier) as? URL,
          url.scheme == "http" || url.scheme == "https"
        {
          content.url = url
        } else if provider.hasItemConformingToTypeIdentifier(UTType.image.identifier), content.photos.count < 6,
          let data = await imageData(provider)
        {
          content.photos.append(data)
        } else if content.text == nil, provider.hasItemConformingToTypeIdentifier(UTType.plainText.identifier),
          let text = try? await provider.loadItem(forTypeIdentifier: UTType.plainText.identifier) as? String
        {
          content.text = text
        }
      }
      if content.text == nil, let body = item.attributedContentText?.string, !body.isEmpty {
        content.text = body
      }
    }
    return content
  }

  /// A shared photo as JPEG, whether it arrives as a file, data or an image.
  private static func imageData(_ provider: NSItemProvider) async -> Data? {
    guard let item = try? await provider.loadItem(forTypeIdentifier: UTType.image.identifier) else { return nil }
    let image: UIImage?
    switch item {
    case let url as URL: image = UIImage(contentsOfFile: url.path)
    case let data as Data: image = UIImage(data: data)
    case let picture as UIImage: image = picture
    default: image = nil
    }
    return image?.jpegData(compressionQuality: 0.8)
  }
}

/// The sheet: a spinner while saving, then what was saved (or why not).
struct ShareView: View {
  var model: ShareModel
  var close: () -> Void

  var body: some View {
    VStack(spacing: 16) {
      switch model.phase {
      case .working(let message):
        ProgressView().controlSize(.large)
        Text(message).font(.headline)
      case .saved(let title, let detail):
        Image(systemName: "checkmark.circle.fill").font(.system(size: 44)).foregroundStyle(.green)
        Text(title).font(.headline)
        Text(detail).font(.subheadline).foregroundStyle(.secondary).multilineTextAlignment(.center)
        Button("Done", action: close).buttonStyle(.borderedProminent)
      case .failed(let message):
        Image(systemName: "exclamationmark.triangle.fill").font(.system(size: 40)).foregroundStyle(.orange)
        Text("Couldn't save that").font(.headline)
        Text(message).font(.subheadline).foregroundStyle(.secondary).multilineTextAlignment(.center)
        Button("Close", action: close).buttonStyle(.bordered)
      }
    }
    .padding(28)
    .frame(maxWidth: 360)
    .background(RoundedRectangle(cornerRadius: 16, style: .continuous).fill(Color(.systemBackground)))
    .frame(maxWidth: .infinity, maxHeight: .infinity)
    .background(Color.black.opacity(0.25).ignoresSafeArea())
    .task(id: model.phase) {
      // Close on its own a moment after a successful save
      if case .saved = model.phase {
        try? await Task.sleep(for: .seconds(1.6))
        close()
      }
    }
  }
}
