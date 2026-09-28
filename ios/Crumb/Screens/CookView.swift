import CrumbKit
import SwiftUI
import UIKit

/// Cook mode: one step per page in big type, the ingredients that step uses (crumb-core
/// matches them, preferring the step's own section), one-tap timers, and the screen kept
/// awake. The last page logs the cook.
struct CookView: View {
  let recipe: Recipe
  @Environment(AppModel.self) private var app
  @Environment(\.dismiss) private var dismiss
  @State private var page = 0
  @State private var showIngredients = false
  @State private var logged = false

  private var steps: [CookStep] { recipe.cookSteps }

  var body: some View {
    let steps = self.steps
    VStack(spacing: 0) {
      header(total: steps.count)
      TabView(selection: $page) {
        ForEach(Array(steps.enumerated()), id: \.offset) { index, step in
          stepPage(step, index: index, total: steps.count).tag(index)
        }
        donePage.tag(steps.count)
      }
      .tabViewStyle(.page(indexDisplayMode: .never))
      .accessibilityIdentifier("cook-pager")
      footer(total: steps.count)
    }
    .canvasBackground()
    .safeAreaInset(edge: .bottom, spacing: 0) { TimerDock() }
    .onAppear { UIApplication.shared.isIdleTimerDisabled = true }
    .onDisappear { UIApplication.shared.isIdleTimerDisabled = false }
    .sheet(isPresented: $showIngredients) { ingredientsSheet }
  }

  private func header(total: Int) -> some View {
    VStack(spacing: 10) {
      HStack {
        Button {
          dismiss()
        } label: {
          Image(systemName: "xmark").font(.system(size: 18, weight: .semibold))
        }
        .accessibilityLabel("Stop cooking")
        .accessibilityIdentifier("close-cook")
        Text(recipe.title)
          .font(Typeface.subtitle)
          .lineLimit(1)
          .frame(maxWidth: .infinity)
        Button {
          showIngredients = true
        } label: {
          Image(systemName: "list.bullet").font(.system(size: 18, weight: .semibold))
        }
        .accessibilityLabel("Ingredients")
      }
      .foregroundStyle(Palette.ink)
      ProgressView(value: Double(min(page + 1, max(total, 1))), total: Double(max(total, 1)))
        .tint(Palette.primary)
    }
    .padding(.horizontal, 20)
    .padding(.top, 12)
  }

  private func stepPage(_ step: CookStep, index: Int, total: Int) -> some View {
    ScrollView {
      VStack(alignment: .leading, spacing: 18) {
        Text(["Step \(index + 1) of \(total)", step.section].compactMap { $0 }.joined(separator: " · "))
          .font(Typeface.label)
          .foregroundStyle(Palette.primary)
        Text(step.text)
          .font(Typeface.body(26, weight: .semibold, relativeTo: .title2))
          .foregroundStyle(Palette.ink)
          .fixedSize(horizontal: false, vertical: true)
          .accessibilityIdentifier("cook-step")
        StepTimers(step: step.text, recipe: recipe)
        let uses = recipe.ingredients(for: step)
        if !uses.isEmpty {
          VStack(alignment: .leading, spacing: 8) {
            Text("You'll need").font(Typeface.small).foregroundStyle(Palette.inkMuted)
            ForEach(uses) { row in
              Label(row.text, systemImage: "circle.fill")
                .labelStyle(BulletLabelStyle())
                .font(Typeface.body(17))
                .foregroundStyle(Palette.ink)
            }
          }
          .padding(16)
          .frame(maxWidth: .infinity, alignment: .leading)
          .paperCard()
        }
      }
      .padding(24)
      .frame(maxWidth: 680)
      .frame(maxWidth: .infinity)
    }
  }

  private var donePage: some View {
    VStack(alignment: .leading, spacing: 16) {
      Text("All done").font(Typeface.display).foregroundStyle(Palette.ink)
      Text("Enjoy it. Want to note that you cooked this?")
        .font(Typeface.body(17))
        .foregroundStyle(Palette.inkMuted)
      Button {
        Task { await markCooked() }
      } label: {
        Label(logged ? "Logged" : "I cooked this", systemImage: "checkmark")
      }
      .buttonStyle(.crumbPrimary)
      .disabled(logged)
      .accessibilityIdentifier("cooked")
    }
    .padding(24)
    .frame(maxWidth: 680, maxHeight: .infinity)
  }

  private func footer(total: Int) -> some View {
    HStack(spacing: 12) {
      Button {
        withAnimation { page = max(0, page - 1) }
      } label: {
        Label("Back", systemImage: "arrow.left")
      }
      .buttonStyle(.crumbSecondary)
      .disabled(page == 0)
      if page < total {
        Button {
          withAnimation { page += 1 }
        } label: {
          Label(page == total - 1 ? "Finish" : "Next", systemImage: "arrow.right")
        }
        .buttonStyle(.crumbPrimary)
        .accessibilityIdentifier("next-step")
      } else {
        Button("Close") { dismiss() }.buttonStyle(.crumbPrimary)
      }
    }
    .padding(16)
  }

  private var ingredientsSheet: some View {
    NavigationStack {
      List {
        ForEach(recipe.ingredients.indices, id: \.self) { s in
          let section = recipe.ingredients[s]
          Section(section.name ?? "") {
            ForEach(section.items, id: \.self) { Text($0).font(Typeface.body(17)) }
          }
        }
      }
      .navigationTitle("Ingredients")
      .navigationBarTitleDisplayMode(.inline)
      .toolbar {
        ToolbarItem(placement: .confirmationAction) { Button("Done") { showIngredients = false } }
      }
    }
    .presentationDetents([.medium, .large])
  }

  private func markCooked() async {
    do {
      _ = try await app.api.markCooked(id: recipe.id)
      logged = true
      UINotificationFeedbackGenerator().notificationOccurred(.success)
    } catch {
      app.show("Couldn't log that", app.handle(error), tone: .error)
    }
  }
}

/// A small dot instead of an icon, for ingredient lists.
struct BulletLabelStyle: LabelStyle {
  func makeBody(configuration: Configuration) -> some View {
    HStack(alignment: .firstTextBaseline, spacing: 10) {
      Circle().fill(Palette.primary).frame(width: 6, height: 6).alignmentGuide(.firstTextBaseline) { $0[.bottom] }
      configuration.title
    }
  }
}
