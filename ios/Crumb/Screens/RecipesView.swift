import CrumbKit
import SwiftUI

/// The recipe box: search and a grid of cards. Offline, the last copy on the phone, with
/// search done on the phone by crumb-core's rule.
struct RecipesView: View {
  @Environment(AppModel.self) private var app
  @State private var state: LoadState<[RecipeSummary]> = .loading
  @State private var query = ""

  var body: some View {
    content
      .canvasBackground()
      .navigationTitle("Recipes")
      .searchable(text: $query, prompt: "Search recipes")
      .toolbar {
        ToolbarItem(placement: .primaryAction) {
          Button {
            app.startAdd()
          } label: {
            Label("Add a recipe", systemImage: "plus")
          }
          .accessibilityIdentifier("recipes-add")
        }
      }
      .task(id: query) {
        // Debounce typing; the first load runs straight away
        if state.value != nil { try? await Task.sleep(for: .milliseconds(250)) }
        guard !Task.isCancelled else { return }
        await load()
      }
      .refreshable { await load() }
  }

  @ViewBuilder
  private var content: some View {
    switch state {
    case .loading:
      LoadingView()
    case .failed(let message):
      MessageView(title: "Couldn't load your recipes", message: message, systemImage: "exclamationmark.triangle") {
        Button("Try again") { Task { await load() } }.buttonStyle(.crumbSecondaryCompact)
      }
    case .ready(let recipes, let offline):
      if recipes.isEmpty {
        let searched = query.trimmingCharacters(in: .whitespaces)
        let searching = !searched.isEmpty
        MessageView(
          title: searching ? "Nothing matches “\(searched)”" : "Your recipe box is empty",
          message: searching ? nil : "Tap + to save a recipe from a link, or share a page to Crumb from Safari.",
          systemImage: "book.closed")
      } else {
        ScrollView {
          VStack(spacing: 12) {
            if offline { OfflineNote() }
            LazyVGrid(columns: [GridItem(.adaptive(minimum: 150), spacing: 12)], spacing: 12) {
              ForEach(recipes) { recipe in
                NavigationLink(value: Route.recipe(recipe.id)) {
                  RecipeCard(recipe: recipe)
                }
                .buttonStyle(.plain)
                .accessibilityIdentifier("recipe-card")
              }
            }
          }
          .padding(16)
        }
        .accessibilityIdentifier("recipe-grid")
      }
    }
  }

  private func load() async {
    do {
      let loaded = try await app.repo.recipes(query: query)
      state = .ready(loaded.value, offline: loaded.offline)
    } catch is CancellationError {
    } catch {
      state = .failed(app.handle(error))
    }
  }
}

/// A recipe in a grid: photo, title, time and category.
struct RecipeCard: View {
  var recipe: RecipeSummary

  var body: some View {
    VStack(alignment: .leading, spacing: 6) {
      RecipePhoto(recipeId: recipe.id, image: recipe.image, width: 200)
        .aspectRatio(4 / 3, contentMode: .fit)
        .clipShape(RoundedRectangle(cornerRadius: Radius.control, style: .continuous))
      Text(recipe.title)
        .font(Typeface.subtitle)
        .foregroundStyle(Palette.ink)
        .lineLimit(2)
        .multilineTextAlignment(.leading)
      let time = RecipeText.duration(recipe.totalTime)
      let meta = [time, recipe.recipeCategory].compactMap { $0 }.joined(separator: " · ")
      if !meta.isEmpty {
        HStack(spacing: 4) {
          if time != nil { Image(systemName: "clock").imageScale(.small) }
          Text(meta).lineLimit(1)
        }
        .font(Typeface.caption)
        .foregroundStyle(Palette.inkMuted)
      }
    }
    .padding(6)
    .padding(.bottom, 6)
    .frame(maxWidth: .infinity, alignment: .leading)
    .paperCard()
    .accessibilityElement(children: .combine)
  }
}
