import CrumbKit
import PhotosUI
import SwiftUI
import UIKit
import UniformTypeIdentifiers

/// Save a recipe from a link or pasted text (crumb-core decides which it is), from photos
/// of a card or cookbook page (read by Wee Chef on the server), or from backup files.
struct AddView: View {
  var initial: String?
  @Environment(AppModel.self) private var app
  @Environment(\.dismiss) private var dismiss
  @State private var draft = ""
  @State private var busy = false
  @State private var error: String?
  @State private var photoItems: [PhotosPickerItem] = []
  @State private var takingPhoto = false
  @State private var importingFiles = false
  @FocusState private var editing: Bool

  private var input: ImportInput? { CrumbCore.classifyImport(raw: draft) }

  var body: some View {
    NavigationStack {
      ScrollView {
        VStack(alignment: .leading, spacing: 14) {
          Text("Paste a link to a recipe, or the recipe itself. In Safari, you can also share a page to Crumb.")
            .font(Typeface.body(15))
            .foregroundStyle(Palette.inkMuted)
          TextEditor(text: $draft)
            .font(Typeface.body(17))
            .foregroundStyle(Palette.ink)
            .scrollContentBackground(.hidden)
            .padding(10)
            .frame(minHeight: 170)
            .background(RoundedRectangle(cornerRadius: Radius.control, style: .continuous).fill(Palette.paper))
            .overlay(alignment: .topLeading) {
              if draft.isEmpty {
                Text("https://… or paste the ingredients and method")
                  .font(Typeface.body(17))
                  .foregroundStyle(Palette.inkMuted.opacity(0.7))
                  .padding(.horizontal, 15)
                  .padding(.vertical, 18)
                  .allowsHitTesting(false)
              }
            }
            .overlay(
              RoundedRectangle(cornerRadius: Radius.control, style: .continuous)
                .strokeBorder(Palette.lineStrong, lineWidth: 1)
            )
            .focused($editing)
            .disabled(busy)
            .accessibilityIdentifier("import-draft")
          if let error {
            Text(error).font(Typeface.body(15)).foregroundStyle(Palette.error).accessibilityIdentifier("import-error")
          }
          Button {
            Task { await save() }
          } label: {
            HStack(spacing: 8) {
              if busy { ProgressView().tint(Palette.onButter) }
              Text(saveLabel)
            }
          }
          .buttonStyle(.crumbPrimary)
          .disabled(input == nil || busy)
          .accessibilityIdentifier("import-save")

          if draft.isEmpty {
            Button {
              if let text = UIPasteboard.general.string { draft = text }
            } label: {
              Label("Paste", systemImage: "doc.on.clipboard")
            }
            .buttonStyle(.crumbSecondary)
          }

          Divider().padding(.vertical, 8)
          photoSection
          Button {
            importingFiles = true
          } label: {
            Label("Import files", systemImage: "folder")
          }
          .buttonStyle(.crumbSecondary)
          .disabled(busy)
          Text("Paprika, Crumb backups, JSON, web pages, PDFs, text, or a zip of them.")
            .font(Typeface.caption)
            .foregroundStyle(Palette.inkMuted)
        }
        .padding(20)
        .frame(maxWidth: 720)
        .frame(maxWidth: .infinity)
      }
      .canvasBackground()
      .navigationTitle("Add a recipe")
      .navigationBarTitleDisplayMode(.inline)
      .toolbar {
        ToolbarItem(placement: .cancellationAction) {
          Button("Cancel") { dismiss() }.disabled(busy)
        }
      }
      .onAppear {
        if let initial, draft.isEmpty { draft = initial }
        app.addDraft = nil
      }
      .onChange(of: draft) { error = nil }
      .onChange(of: photoItems) { _, items in
        guard !items.isEmpty else { return }
        Task { await importPicked(items) }
      }
      .fullScreenCover(isPresented: $takingPhoto) {
        CameraPicker { image in
          guard let data = image.jpegData(compressionQuality: 0.8) else { return }
          Task { await importPhotos([data]) }
        }
        .ignoresSafeArea()
      }
      .fileImporter(
        isPresented: $importingFiles, allowedContentTypes: [.json, .html, .pdf, .plainText, .zip, .data],
        allowsMultipleSelection: true
      ) { result in
        if case .success(let urls) = result { Task { await importFiles(urls) } }
      }
      .interactiveDismissDisabled(busy)
    }
  }

  @ViewBuilder
  private var photoSection: some View {
    if app.connector?.vision == true {
      HStack(spacing: 12) {
        PhotosPicker(selection: $photoItems, maxSelectionCount: 6, matching: .images) {
          Label("Photos", systemImage: "photo.on.rectangle")
        }
        .buttonStyle(.crumbSecondary)
        if UIImagePickerController.isSourceTypeAvailable(.camera) {
          Button {
            takingPhoto = true
          } label: {
            Label("Camera", systemImage: "camera")
          }
          .buttonStyle(.crumbSecondary)
        }
      }
      .disabled(busy)
      Text("Photograph a recipe card or cookbook page (up to 6) and Wee Chef will type it up.")
        .font(Typeface.caption)
        .foregroundStyle(Palette.inkMuted)
    }
  }

  private var saveLabel: String {
    switch (busy, input) {
    case (true, .link?): return "Fetching the recipe…"
    case (true, _): return "Reading the recipe…"
    case (false, .text?): return "Save pasted recipe"
    default: return "Save recipe"
    }
  }

  private func save() async {
    editing = false
    busy = true
    defer { busy = false }
    do {
      let result = try await app.api.importInput(draft)
      draft = ""
      saved(result)
    } catch {
      self.error = app.handle(error)
    }
  }

  private func importPicked(_ items: [PhotosPickerItem]) async {
    var photos: [Data] = []
    for item in items.prefix(6) {
      if let data = try? await item.loadTransferable(type: Data.self), let image = UIImage(data: data),
        let jpeg = image.jpegData(compressionQuality: 0.8)
      {
        photos.append(jpeg)
      }
    }
    photoItems = []
    if !photos.isEmpty { await importPhotos(photos) }
  }

  private func importPhotos(_ photos: [Data]) async {
    busy = true
    defer { busy = false }
    do {
      saved(try await app.api.importPhotos(photos))
    } catch {
      self.error = app.handle(error)
    }
  }

  private func importFiles(_ urls: [URL]) async {
    busy = true
    defer { busy = false }
    var files: [UploadFile] = []
    for url in urls {
      let access = url.startAccessingSecurityScopedResource()
      defer { if access { url.stopAccessingSecurityScopedResource() } }
      guard let data = try? Data(contentsOf: url) else { continue }
      let type = UTType(filenameExtension: url.pathExtension)?.preferredMIMEType ?? "application/octet-stream"
      files.append(UploadFile(name: url.lastPathComponent, mimeType: type, data: data))
    }
    do {
      let summaries = try await app.api.importFiles(files)
      let created = summaries.flatMap(\.created)
      let problems = summaries.compactMap(\.error)
      if created.count == 1, let only = created.first {
        dismiss()
        app.open(.recipe(only.id), in: .recipes)
      } else if !created.isEmpty {
        dismiss()
        app.tab = .recipes
      }
      app.show(
        created.isEmpty ? "Nothing new to import" : "Imported \(created.count) recipe\(created.count == 1 ? "" : "s")",
        problems.first, tone: problems.isEmpty ? .success : .error)
    } catch {
      self.error = app.handle(error)
    }
  }

  private func saved(_ result: ImportResult) {
    dismiss()
    if let book = result.cookbook {
      app.show(
        CrumbCore.bookImportedTitle(
          name: book.name, added: UInt32(clamping: book.added ?? 0), duplicates: UInt32(clamping: book.duplicates ?? 0),
          skipped: book.skipped.map { UInt32(clamping: $0) }), tone: .success)
      app.open(.cookbook(book.id), in: .shelf)
    } else {
      app.show(result.isNew ? "Saved to your box" : "Already in your box", result.title, tone: .success)
      app.open(.recipe(result.id), in: .recipes)
    }
  }
}

/// The camera, for a photo of a recipe card.
struct CameraPicker: UIViewControllerRepresentable {
  var onPhoto: (UIImage) -> Void
  @Environment(\.dismiss) private var dismiss

  func makeUIViewController(context: Context) -> UIImagePickerController {
    let picker = UIImagePickerController()
    picker.sourceType = .camera
    picker.delegate = context.coordinator
    return picker
  }

  func updateUIViewController(_ controller: UIImagePickerController, context: Context) {}

  func makeCoordinator() -> Coordinator { Coordinator(self) }

  final class Coordinator: NSObject, UIImagePickerControllerDelegate, UINavigationControllerDelegate {
    let parent: CameraPicker

    init(_ parent: CameraPicker) {
      self.parent = parent
    }

    func imagePickerController(
      _ picker: UIImagePickerController, didFinishPickingMediaWithInfo info: [UIImagePickerController.InfoKey: Any]
    ) {
      if let image = info[.originalImage] as? UIImage { parent.onPhoto(image) }
      parent.dismiss()
    }

    func imagePickerControllerDidCancel(_ picker: UIImagePickerController) {
      parent.dismiss()
    }
  }
}
