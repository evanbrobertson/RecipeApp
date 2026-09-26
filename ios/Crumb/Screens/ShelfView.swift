import CrumbKit
import SwiftUI

/// Your shelf: cookbooks as cloth-bound covers in the web's six colours.
struct ShelfView: View {
  @Environment(AppModel.self) private var app
  @State private var state: LoadState<[CookbookListItem]> = .loading
  @State private var creating = false

  var body: some View {
    content
      .canvasBackground()
      .navigationTitle("Shelf")
      .toolbar {
        ToolbarItem(placement: .primaryAction) {
          Button {
            creating = true
          } label: {
            Label("New cookbook", systemImage: "plus")
          }
        }
      }
      .sheet(isPresented: $creating) {
        CookbookEditor(book: nil) { _ in Task { await load() } }
      }
      .task { await load() }
      .refreshable { await load() }
  }

  @ViewBuilder
  private var content: some View {
    switch state {
    case .loading:
      LoadingView()
    case .failed(let message):
      MessageView(title: "Couldn't load your shelf", message: message, systemImage: "exclamationmark.triangle") {
        Button("Try again") { Task { await load() } }.buttonStyle(.crumbSecondaryCompact)
      }
    case .ready(let books, let offline):
      if books.isEmpty {
        MessageView(
          title: "No cookbooks yet", message: "Gather recipes into cookbooks: weeknights, baking, Gran's.",
          systemImage: "books.vertical"
        ) {
          Button("New cookbook") { creating = true }.buttonStyle(.crumbSecondaryCompact)
        }
      } else {
        ScrollView {
          VStack(spacing: 12) {
            if offline { OfflineNote() }
            LazyVGrid(columns: [GridItem(.adaptive(minimum: 140), spacing: 16)], spacing: 20) {
              ForEach(books) { book in
                NavigationLink(value: Route.cookbook(book.id)) {
                  BookCover(name: book.name, color: book.color, count: book.recipeCount)
                }
                .buttonStyle(.plain)
              }
            }
          }
          .padding(16)
        }
      }
    }
  }

  private func load() async {
    do {
      let loaded = try await app.repo.cookbooks()
      state = .ready(loaded.value, offline: loaded.offline)
    } catch is CancellationError {
    } catch {
      state = .failed(app.handle(error))
    }
  }
}

/// A cookbook's cover: cloth, a spine shadow, the title in foil, and how many recipes.
struct BookCover: View {
  var name: String
  var color: String?
  var count: Int64

  var body: some View {
    let look = Palette.book(color)
    VStack(alignment: .leading, spacing: 8) {
      ZStack(alignment: .topLeading) {
        UnevenRoundedRectangle(
          topLeadingRadius: 3, bottomLeadingRadius: 3, bottomTrailingRadius: Radius.control,
          topTrailingRadius: Radius.control, style: .continuous
        )
        .fill(look.cloth)
        .overlay(alignment: .leading) {
          Rectangle().fill(look.shade).frame(width: 10)
        }
        .overlay(
          UnevenRoundedRectangle(
            topLeadingRadius: 3, bottomLeadingRadius: 3, bottomTrailingRadius: Radius.control,
            topTrailingRadius: Radius.control, style: .continuous
          )
          .stroke(Color.black.opacity(0.12), lineWidth: 1)
        )
        Text(name)
          .font(Typeface.serif(20))
          .foregroundStyle(look.foil)
          .lineLimit(4)
          .padding(.leading, 22)
          .padding([.top, .trailing], 14)
      }
      .aspectRatio(3 / 4, contentMode: .fit)
      Text(count == 1 ? "1 recipe" : "\(count) recipes")
        .font(Typeface.caption)
        .foregroundStyle(Palette.inkMuted)
    }
    .accessibilityElement(children: .combine)
  }
}

/// One cookbook's recipes.
struct CookbookView: View {
  let id: Int64
  @Environment(AppModel.self) private var app
  @Environment(\.dismiss) private var dismiss
  @State private var state: LoadState<Cookbook> = .loading
  @State private var editing = false
  @State private var confirmingDelete = false
  @State private var sharing: ShareItem?

  var body: some View {
    content
      .canvasBackground()
      .navigationTitle(state.value?.name ?? "")
      .toolbar {
        if let book = state.value {
          ToolbarItem(placement: .primaryAction) {
            Menu {
              Button("Share cookbook", systemImage: "square.and.arrow.up") {
                sharing = ShareItem(
                  text: CrumbCore.bookToText(name: book.name, titles: book.recipes.map(\.title), link: nil))
              }
              Button("Share a link", systemImage: "link") { Task { await shareLink(book) } }
              Button("Edit", systemImage: "pencil") { editing = true }
              Divider()
              Button("Delete cookbook", systemImage: "trash", role: .destructive) { confirmingDelete = true }
            } label: {
              Image(systemName: "ellipsis.circle")
            }
            .accessibilityLabel("Cookbook actions")
          }
        }
      }
      .sheet(isPresented: $editing) {
        if let book = state.value {
          CookbookEditor(book: book) { _ in Task { await load() } }
        }
      }
      .sheet(item: $sharing) { ShareSheet(items: [$0.text]) }
      .confirmationDialog("Delete this cookbook?", isPresented: $confirmingDelete, titleVisibility: .visible) {
        Button("Delete", role: .destructive) { Task { await delete() } }
      } message: {
        Text("Its recipes stay in your box.")
      }
      .task(id: id) { await load() }
      .refreshable { await load() }
  }

  @ViewBuilder
  private var content: some View {
    switch state {
    case .loading:
      LoadingView()
    case .failed(let message):
      MessageView(title: "Couldn't open this cookbook", message: message, systemImage: "exclamationmark.triangle") {
        Button("Try again") { Task { await load() } }.buttonStyle(.crumbSecondaryCompact)
      }
    case .ready(let book, let offline):
      ScrollView {
        VStack(alignment: .leading, spacing: 12) {
          if offline { OfflineNote() }
          if let description = book.description, !description.isEmpty {
            Text(description).font(Typeface.body(17)).foregroundStyle(Palette.inkMuted)
          }
          if book.recipes.isEmpty {
            Text("No recipes in this cookbook yet. Open a recipe and choose Add to cookbook.")
              .font(Typeface.body(15))
              .foregroundStyle(Palette.inkMuted)
          }
          LazyVGrid(columns: [GridItem(.adaptive(minimum: 150), spacing: 12)], spacing: 12) {
            ForEach(book.recipes) { recipe in
              NavigationLink(value: Route.recipe(recipe.id)) { RecipeCard(recipe: recipe) }
                .buttonStyle(.plain)
                .contextMenu {
                  Button("Remove from cookbook", systemImage: "minus.circle", role: .destructive) {
                    Task { await remove(recipe) }
                  }
                }
            }
          }
        }
        .padding(16)
      }
    }
  }

  private func load() async {
    do {
      let loaded = try await app.repo.cookbook(id: id)
      state = .ready(loaded.value, offline: loaded.offline)
    } catch is CancellationError {
    } catch {
      state = .failed(app.handle(error))
    }
  }

  private func remove(_ recipe: RecipeSummary) async {
    do {
      try await app.api.removeFromCookbook(bookId: id, recipeId: recipe.id)
      await load()
    } catch {
      app.show("Couldn't remove it", app.handle(error), tone: .error)
    }
  }

  private func shareLink(_ book: Cookbook) async {
    do {
      let share = try await app.api.createShare(.cookbook, id: book.id)
      sharing = ShareItem(text: share.url)
    } catch {
      app.show("Couldn't make a share link", app.handle(error), tone: .error)
    }
  }

  private func delete() async {
    do {
      try await app.api.deleteCookbook(id: id)
      dismiss()
    } catch {
      app.show("Couldn't delete it", app.handle(error), tone: .error)
    }
  }
}

/// New or edit: name, description and cloth colour (crumb-core's `bookColors()`).
struct CookbookEditor: View {
  var book: Cookbook?
  var onSaved: (Int64) -> Void
  @Environment(AppModel.self) private var app
  @Environment(\.dismiss) private var dismiss
  @State private var name = ""
  @State private var details = ""
  @State private var color = "tile"
  @State private var busy = false

  var body: some View {
    NavigationStack {
      Form {
        Section("Name") {
          TextField("Weeknights", text: $name).accessibilityIdentifier("cookbook-name")
        }
        Section("Description") {
          TextField("Optional", text: $details, axis: .vertical)
        }
        Section("Colour") {
          HStack(spacing: 12) {
            ForEach(CrumbCore.bookColors(), id: \.self) { option in
              Button {
                color = option
              } label: {
                Circle()
                  .fill(Palette.book(option).cloth)
                  .frame(width: 34, height: 34)
                  .overlay(Circle().strokeBorder(Palette.ink.opacity(option == color ? 0.9 : 0.15), lineWidth: 2))
              }
              .buttonStyle(.plain)
              .accessibilityLabel(option)
              .accessibilityAddTraits(option == color ? [.isSelected] : [])
            }
          }
        }
      }
      .scrollContentBackground(.hidden)
      .canvasBackground()
      .navigationTitle(book == nil ? "New cookbook" : "Edit cookbook")
      .navigationBarTitleDisplayMode(.inline)
      .toolbar {
        ToolbarItem(placement: .cancellationAction) { Button("Cancel") { dismiss() } }
        ToolbarItem(placement: .confirmationAction) {
          Button("Save") { Task { await save() } }
            .disabled(busy || name.trimmingCharacters(in: .whitespaces).isEmpty)
        }
      }
      .onAppear {
        if let book {
          name = book.name
          details = book.description ?? ""
          color = CrumbCore.bookColor(name: book.color ?? "") ?? "tile"
        } else {
          color = CrumbCore.bookColors().randomElement() ?? "tile"
        }
      }
    }
  }

  private func save() async {
    busy = true
    defer { busy = false }
    let trimmed = name.trimmingCharacters(in: .whitespacesAndNewlines)
    do {
      if let book {
        let saved = try await app.api.updateCookbook(id: book.id, name: trimmed, description: details, color: color)
        onSaved(saved.id)
      } else {
        let saved = try await app.api.createCookbook(name: trimmed, description: details, color: color)
        onSaved(saved.id)
      }
      dismiss()
    } catch {
      app.show("Couldn't save the cookbook", app.handle(error), tone: .error)
    }
  }
}

/// Pick the cookbooks a recipe belongs in.
struct AddToCookbookView: View {
  let recipe: Recipe
  @Environment(AppModel.self) private var app
  @Environment(\.dismiss) private var dismiss
  @State private var books: [CookbookListItem] = []
  @State private var holding: Set<Int64> = []
  @State private var loading = true

  var body: some View {
    NavigationStack {
      List(books) { book in
        Button {
          Task { await toggle(book) }
        } label: {
          HStack {
            RoundedRectangle(cornerRadius: 3).fill(Palette.book(book.color).cloth).frame(width: 14, height: 28)
            Text(book.name).font(Typeface.body(17)).foregroundStyle(Palette.ink)
            Spacer()
            if holding.contains(book.id) {
              Image(systemName: "checkmark").foregroundStyle(Palette.primary)
            }
          }
        }
      }
      .overlay {
        if loading {
          ProgressView()
        } else if books.isEmpty {
          MessageView(title: "No cookbooks yet", message: "Make one on the Shelf tab.", systemImage: "books.vertical")
        }
      }
      .navigationTitle("Add to cookbook")
      .navigationBarTitleDisplayMode(.inline)
      .toolbar {
        ToolbarItem(placement: .confirmationAction) { Button("Done") { dismiss() } }
      }
      .task {
        let api = app.api
        let recipeId = recipe.id
        async let list = api.cookbooks()
        async let ids = api.recipeCookbooks(id: recipeId)
        books = (try? await list) ?? []
        holding = Set((try? await ids) ?? [])
        loading = false
      }
    }
    .presentationDetents([.medium, .large])
  }

  private func toggle(_ book: CookbookListItem) async {
    do {
      if holding.contains(book.id) {
        try await app.api.removeFromCookbook(bookId: book.id, recipeId: recipe.id)
        holding.remove(book.id)
      } else {
        _ = try await app.api.addToCookbook(bookId: book.id, recipeIds: [recipe.id])
        holding.insert(book.id)
      }
    } catch {
      app.show("Couldn't update the cookbook", app.handle(error), tone: .error)
    }
  }
}
