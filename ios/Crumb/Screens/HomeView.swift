import CrumbKit
import SwiftUI

/// A greeting, Add, and Try next: the server's picks for what to cook (ranked by
/// crumb-core's suggest module, blurbs by Wee Chef when the server has a key).
struct HomeView: View {
  @Environment(AppModel.self) private var app
  @State private var state: LoadState<Suggestions> = .loading
  @State private var surprising = false

  var body: some View {
    ScrollView {
      VStack(alignment: .leading, spacing: 20) {
        VStack(alignment: .leading, spacing: 4) {
          Text(greeting)
            .font(Typeface.hand(30))
            .foregroundStyle(Palette.primary)
          Text("What's cooking?")
            .font(Typeface.display)
            .foregroundStyle(Palette.ink)
        }

        HStack(spacing: 12) {
          Button {
            app.startAdd()
          } label: {
            Label("Add a recipe", systemImage: "plus")
          }
          .buttonStyle(.crumbPrimary)
          .accessibilityIdentifier("home-add")

          Button {
            Task { await surprise() }
          } label: {
            Label("Surprise me", systemImage: "dice")
          }
          .buttonStyle(.crumbSecondary)
          .disabled(surprising)
          .accessibilityIdentifier("surprise")
        }

        SectionHeading(text: "Try next")
        switch state {
        case .loading:
          ProgressView().tint(Palette.primary).frame(maxWidth: .infinity, minHeight: 120)
        case .failed(let message):
          Text(message).font(Typeface.body(15)).foregroundStyle(Palette.inkMuted)
          Button("Try again") { Task { await load() } }.buttonStyle(.crumbSecondaryCompact)
        case .ready(let suggestions, _):
          if suggestions.items.isEmpty {
            Text("Save a few recipes and Crumb will suggest what to cook next.")
              .font(Typeface.body(15))
              .foregroundStyle(Palette.inkMuted)
          } else {
            LazyVGrid(columns: [GridItem(.adaptive(minimum: 160), spacing: 12)], spacing: 12) {
              ForEach(suggestions.items, id: \.recipe.id) { item in
                NavigationLink(value: Route.recipe(item.recipe.id)) {
                  SuggestionCard(item: item)
                }
                .buttonStyle(.plain)
              }
            }
          }
          if let idea = suggestions.idea {
            IdeaCard(idea: idea)
          }
        }
      }
      .padding(20)
      .frame(maxWidth: 900)
      .frame(maxWidth: .infinity)
    }
    .canvasBackground()
    .toolbar(.hidden, for: .navigationBar)
    .refreshable { await load() }
    .task { if state.value == nil { await load() } }
  }

  private var greeting: String {
    switch Calendar.current.component(.hour, from: Date()) {
    case 5..<12: return "Good morning"
    case 12..<17: return "Good afternoon"
    default: return "Good evening"
    }
  }

  private func load() async {
    do {
      state = .ready(try await app.api.suggestions(limit: 4), offline: false)
    } catch is CancellationError {
    } catch {
      state = .failed(app.handle(error))
    }
  }

  private func surprise() async {
    surprising = true
    defer { surprising = false }
    do {
      if let pick = try await app.api.randomRecipe() {
        app.open(.recipe(pick.id))
      } else {
        app.show("Nothing to pick from yet", "Save a recipe first.")
      }
    } catch {
      app.show("Couldn't pick a recipe", app.handle(error), tone: .error)
    }
  }
}

/// One Try next card: photo, title, and why it was picked.
private struct SuggestionCard: View {
  var item: Suggestion

  var body: some View {
    VStack(alignment: .leading, spacing: 8) {
      RecipePhoto(recipeId: item.recipe.id, image: item.recipe.image, width: 200)
        .aspectRatio(4 / 3, contentMode: .fit)
        .clipShape(RoundedRectangle(cornerRadius: Radius.control, style: .continuous))
      Text(item.recipe.title)
        .font(Typeface.subtitle)
        .foregroundStyle(Palette.ink)
        .lineLimit(2)
      HStack(alignment: .top, spacing: 4) {
        if item.ai == true {
          Image(systemName: "sparkles").accessibilityLabel("Wee Chef")
        }
        Text(item.reason).lineLimit(3)
      }
      .font(Typeface.caption)
      .foregroundStyle(Palette.inkMuted)
    }
    .padding(8)
    .frame(maxWidth: .infinity, alignment: .leading)
    .paperCard()
    .accessibilityElement(children: .combine)
  }
}

/// Wee Chef's idea for something not in the box yet, with a search link.
private struct IdeaCard: View {
  var idea: Idea
  @Environment(\.openURL) private var openURL

  var body: some View {
    VStack(alignment: .leading, spacing: 8) {
      Label("Wee Chef's idea", systemImage: "sparkles")
        .font(Typeface.small)
        .foregroundStyle(Palette.primary)
      Text(idea.title).font(Typeface.title).foregroundStyle(Palette.ink)
      Text(idea.why).font(Typeface.body(15)).foregroundStyle(Palette.inkMuted)
      if let url = URL(string: idea.searchUrl) {
        Button("Find a recipe", systemImage: "magnifyingglass") { openURL(url) }
          .buttonStyle(.crumbSecondaryCompact)
      }
    }
    .padding(16)
    .frame(maxWidth: .infinity, alignment: .leading)
    .paperCard()
  }
}
