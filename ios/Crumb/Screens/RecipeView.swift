import CrumbKit
import SwiftUI
import UIKit

/// One recipe: photo, times, scaling, ingredients to tick off, the method (with one-tap
/// timers), the cook's notes and where it came from. Scaling, timers, the source link and
/// the shared text all come from crumb-core.
struct RecipeView: View {
  let id: Int64
  @Environment(AppModel.self) private var app
  @Environment(\.dismiss) private var dismiss
  @State private var state: LoadState<Recipe> = .loading
  @State private var scale: Double = 1
  @State private var ticked: Set<String> = []
  @State private var cooked: Cooked?
  @State private var pickingBook = false
  @State private var confirmingDelete = false
  @State private var sharing: ShareItem?

  var body: some View {
    Group {
      switch state {
      case .loading:
        LoadingView()
      case .failed(let message):
        MessageView(title: "Couldn't open this recipe", message: message, systemImage: "exclamationmark.triangle") {
          Button("Try again") { Task { await load() } }.buttonStyle(.crumbSecondaryCompact)
        }
      case .ready(let recipe, let offline):
        content(recipe, offline: offline)
      }
    }
    .canvasBackground()
    .navigationBarTitleDisplayMode(.inline)
    .toolbar {
      if let recipe = state.value {
        ToolbarItem(placement: .primaryAction) { menu(recipe) }
      }
    }
    .task(id: id) {
      await load()
      try? await app.api.logViewed(id: id)
    }
    .sheet(isPresented: $pickingBook) {
      if let recipe = state.value { AddToCookbookView(recipe: recipe) }
    }
    .sheet(item: $sharing) { item in
      ShareSheet(items: [item.text])
    }
    .confirmationDialog("Delete this recipe?", isPresented: $confirmingDelete, titleVisibility: .visible) {
      Button("Delete", role: .destructive) { Task { await delete() } }
    } message: {
      Text("It's removed from your box and every cookbook. This can't be undone.")
    }
  }

  private func content(_ recipe: Recipe, offline: Bool) -> some View {
    ScrollView {
      VStack(alignment: .leading, spacing: 0) {
        if recipe.image != nil {
          RecipePhoto(recipeId: recipe.id, image: recipe.image, width: 720)
            .aspectRatio(4 / 3, contentMode: .fill)
            .frame(maxWidth: .infinity)
            .clipShape(RoundedRectangle(cornerRadius: Radius.control, style: .continuous))
            .padding(.horizontal, 12)
        }
        VStack(alignment: .leading, spacing: 14) {
          if offline { OfflineNote() }
          if !recipe.kicker.isEmpty {
            Text(recipe.kicker.uppercased())
              .font(Typeface.small)
              .tracking(0.6)
              .foregroundStyle(Palette.primary)
          }
          Text(recipe.title)
            .font(Typeface.headline)
            .foregroundStyle(Palette.ink)
            .accessibilityAddTraits(.isHeader)
            .accessibilityIdentifier("recipe-title")
          pills(recipe)
          if let description = recipe.description, !description.isEmpty {
            Text(description).font(Typeface.body(17)).foregroundStyle(Palette.inkMuted)
          }
          if let cooked, let line = RecipeText.cookedLine(cooked) {
            Label(line, systemImage: "checkmark.seal").font(Typeface.small).foregroundStyle(Palette.primary)
          }
          if !recipe.cookSteps.isEmpty {
            Button {
              app.cooking = recipe
            } label: {
              Label("Start cooking", systemImage: "frying.pan")
            }
            .buttonStyle(.crumbPrimary)
            .accessibilityIdentifier("start-cooking")
          }
        }
        .padding(20)

        ingredients(recipe)
        method(recipe)

        if let notes = recipe.notes, !notes.isEmpty {
          VStack(alignment: .leading, spacing: 8) {
            SectionHeading(text: "Cook's notes")
            Text(notes).font(Typeface.hand(24)).foregroundStyle(Palette.primary)
          }
          .padding(20)
        }
        if let source = recipe.sourceLink {
          Link(destination: source) {
            Label("View original" + (recipe.sourceHost.map { " on \($0)" } ?? ""), systemImage: "safari")
          }
          .buttonStyle(.crumbSecondaryCompact)
          .padding(20)
        }
      }
      .frame(maxWidth: 760)
      .frame(maxWidth: .infinity)
      .padding(.bottom, 24)
    }
    .accessibilityIdentifier("recipe")
  }

  private func pills(_ recipe: Recipe) -> some View {
    var items: [PillItem] = []
    if let total = RecipeText.duration(recipe.totalTime) {
      items.append(PillItem(text: "Total \(total)", icon: "clock"))
    }
    if let prep = RecipeText.duration(recipe.prepTime) {
      items.append(PillItem(text: "Prep \(prep)"))
    }
    if let cook = RecipeText.duration(recipe.cookTime) {
      items.append(PillItem(text: "Cook \(cook)"))
    }
    if let serves = recipe.recipeYield, !serves.isEmpty {
      items.append(PillItem(text: serves, icon: "person.2"))
    }
    return FlowLayout(spacing: 8) {
      ForEach(items, id: \.text) { item in Pill(text: item.text, systemImage: item.icon) }
    }
  }

  @ViewBuilder
  private func ingredients(_ recipe: Recipe) -> some View {
    if !recipe.ingredientRows.isEmpty {
      VStack(alignment: .leading, spacing: 4) {
        HStack {
          SectionHeading(text: "Ingredients")
          ScaleControl(scale: $scale)
        }
        .padding(.bottom, 6)
        ForEach(recipe.ingredients.indices, id: \.self) { s in
          let section = recipe.ingredients[s]
          if let name = section.name, !name.isEmpty {
            Text(name).font(Typeface.subtitle).foregroundStyle(Palette.primary).padding(.top, 10)
          }
          ForEach(section.items.indices, id: \.self) { i in
            let key = "\(s):\(i)"
            let done = ticked.contains(key)
            Button {
              if done { ticked.remove(key) } else { ticked.insert(key) }
            } label: {
              HStack(alignment: .firstTextBaseline, spacing: 12) {
                Image(systemName: done ? "checkmark.square.fill" : "square")
                  .foregroundStyle(done ? Palette.primary : Palette.inkMuted)
                  .accessibilityHidden(true)
                Text(RecipeText.scaled(section.items[i], by: scale))
                  .font(Typeface.body(17))
                  .strikethrough(done)
                  .foregroundStyle(done ? Palette.inkMuted : Palette.ink)
                  .frame(maxWidth: .infinity, alignment: .leading)
              }
              .padding(.vertical, 8)
              .contentShape(Rectangle())
            }
            .buttonStyle(.plain)
            .accessibilityAddTraits(done ? [.isSelected] : [])
          }
        }
      }
      .padding(.horizontal, 20)
      .padding(.vertical, 8)
    }
  }

  @ViewBuilder
  private func method(_ recipe: Recipe) -> some View {
    let steps = recipe.cookSteps
    if !steps.isEmpty {
      VStack(alignment: .leading, spacing: 14) {
        SectionHeading(text: "Method")
        ForEach(Array(steps.enumerated()), id: \.offset) { index, step in
          if index == 0 || steps[index - 1].section != step.section, let name = step.section {
            Text(name).font(Typeface.subtitle).foregroundStyle(Palette.primary).padding(.top, 6)
          }
          HStack(alignment: .top, spacing: 14) {
            Text("\(index + 1)")
              .font(Typeface.small)
              .foregroundStyle(Palette.onTile)
              .frame(width: 30, height: 30)
              .background(Circle().fill(Palette.tile))
            VStack(alignment: .leading, spacing: 8) {
              Text(step.text).font(Typeface.body(17)).foregroundStyle(Palette.ink)
              StepTimers(step: step.text, recipe: recipe)
            }
          }
        }
      }
      .padding(20)
    }
  }

  private func menu(_ recipe: Recipe) -> some View {
    Menu {
      Button("Share recipe", systemImage: "square.and.arrow.up") {
        sharing = ShareItem(text: recipe.shareText())
      }
      Button("Share a link", systemImage: "link") { Task { await shareLink(recipe) } }
      Button("Add to cookbook", systemImage: "books.vertical") { pickingBook = true }
      Button("I cooked this", systemImage: "checkmark.seal") { Task { await markCooked(recipe) } }
      if let session = app.session, let web = URL(string: "recipes/\(recipe.id)/edit", relativeTo: session.server) {
        Link(destination: web.absoluteURL) { Label("Edit on the web", systemImage: "pencil") }
      }
      Divider()
      Button("Delete", systemImage: "trash", role: .destructive) { confirmingDelete = true }
    } label: {
      Image(systemName: "ellipsis.circle")
    }
    .accessibilityLabel("Recipe actions")
    .accessibilityIdentifier("recipe-menu")
  }

  private func load() async {
    do {
      let loaded = try await app.repo.recipe(id: id)
      state = .ready(loaded.value, offline: loaded.offline)
    } catch is CancellationError {
    } catch {
      state = .failed(app.handle(error))
    }
  }

  private func markCooked(_ recipe: Recipe) async {
    do {
      let result = try await app.api.markCooked(id: recipe.id)
      cooked = result
      app.show(result.eventId == nil ? "Already logged today" : "Logged as cooked", tone: .success)
    } catch {
      app.show("Couldn't log that", app.handle(error), tone: .error)
    }
  }

  private func shareLink(_ recipe: Recipe) async {
    do {
      let share = try await app.api.createShare(.recipe, id: recipe.id)
      sharing = ShareItem(text: share.url)
    } catch {
      app.show("Couldn't make a share link", app.handle(error), tone: .error)
    }
  }

  private func delete() async {
    do {
      try await app.api.deleteRecipe(id: id)
      app.show("Recipe deleted", tone: .success)
      dismiss()
    } catch {
      app.show("Couldn't delete it", app.handle(error), tone: .error)
    }
  }
}

private struct PillItem {
  var text: String
  var icon: String?
}

/// Something to hand to the share sheet.
struct ShareItem: Identifiable {
  let id = UUID()
  var text: String
}

/// ½×, 1×, 1½×, 2×, 3×: scaling is crumb-core's, so "2 cups" becomes "3 cups" as on the web.
struct ScaleControl: View {
  @Binding var scale: Double
  private let options: [(Double, String)] = [(0.5, "½×"), (1, "1×"), (1.5, "1½×"), (2, "2×"), (3, "3×")]

  var body: some View {
    Menu {
      Picker("Scale", selection: $scale) {
        ForEach(options, id: \.0) { option in Text(option.1).tag(option.0) }
      }
    } label: {
      Label(options.first { $0.0 == scale }?.1 ?? "1×", systemImage: "arrow.up.left.and.arrow.down.right")
        .font(Typeface.small)
        .padding(.horizontal, 12)
        .padding(.vertical, 6)
        .background(Capsule().fill(Palette.tint))
        .foregroundStyle(Palette.ink)
    }
    .accessibilityLabel("Scale the ingredients")
    .accessibilityIdentifier("scale")
  }
}

/// One-tap timers for the durations crumb-core finds in a step ("bake 25–30 minutes").
struct StepTimers: View {
  var step: String
  var recipe: Recipe
  @Environment(AppModel.self) private var app

  var body: some View {
    let timers = RecipeText.timers(in: step)
    if !timers.isEmpty {
      FlowLayout(spacing: 8) {
        ForEach(timers, id: \.label) { timer in
          Button {
            Task { await start(timer) }
          } label: {
            Label(timer.label, systemImage: "timer")
          }
          .font(Typeface.small)
          .padding(.horizontal, 12)
          .padding(.vertical, 6)
          .background(Capsule().fill(Palette.tint))
          .foregroundStyle(Palette.primary)
          .buttonStyle(.plain)
          .accessibilityHint("Starts a \(timer.label) timer")
        }
      }
    }
  }

  private func start(_ timer: StepTimer) async {
    _ = await app.alarms.requestPermission()
    app.timers.start(
      label: timer.label, seconds: Int(timer.seconds), recipeId: recipe.id, recipeTitle: recipe.title)
    app.show("Timer started", "\(timer.label) · \(recipe.title)", tone: .success)
  }
}

/// A row of chips that wraps onto the next line.
struct FlowLayout: Layout {
  var spacing: CGFloat = 8

  func sizeThatFits(proposal: ProposedViewSize, subviews: Subviews, cache: inout ()) -> CGSize {
    let rows = arrange(subviews, width: proposal.width ?? .infinity)
    let height = rows.last.map { $0.y + $0.height } ?? 0
    let width = rows.map(\.width).max() ?? 0
    return CGSize(width: min(width, proposal.width ?? width), height: height)
  }

  func placeSubviews(in bounds: CGRect, proposal: ProposedViewSize, subviews: Subviews, cache: inout ()) {
    for row in arrange(subviews, width: bounds.width) {
      var x = bounds.minX
      for index in row.items {
        let size = subviews[index].sizeThatFits(.unspecified)
        subviews[index].place(at: CGPoint(x: x, y: bounds.minY + row.y), proposal: ProposedViewSize(size))
        x += size.width + spacing
      }
    }
  }

  private struct Row {
    var items: [Int] = []
    var y: CGFloat = 0
    var width: CGFloat = 0
    var height: CGFloat = 0
  }

  private func arrange(_ subviews: Subviews, width: CGFloat) -> [Row] {
    var rows: [Row] = []
    var row = Row()
    for index in subviews.indices {
      let size = subviews[index].sizeThatFits(.unspecified)
      let needed = row.items.isEmpty ? size.width : row.width + spacing + size.width
      if needed > width, !row.items.isEmpty {
        rows.append(row)
        row = Row(y: row.y + row.height + spacing)
      }
      row.width = row.items.isEmpty ? size.width : row.width + spacing + size.width
      row.height = max(row.height, size.height)
      row.items.append(index)
    }
    if !row.items.isEmpty { rows.append(row) }
    return rows
  }
}

/// The system share sheet.
struct ShareSheet: UIViewControllerRepresentable {
  var items: [Any]

  func makeUIViewController(context: Context) -> UIActivityViewController {
    UIActivityViewController(activityItems: items, applicationActivities: nil)
  }

  func updateUIViewController(_ controller: UIActivityViewController, context: Context) {}
}
