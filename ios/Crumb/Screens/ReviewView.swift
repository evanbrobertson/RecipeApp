import CrumbKit
import SwiftUI

/// Wee Chef's Review: where Check all stands (in crumb-core's words), and the recipes with
/// suggestions waiting. Tapping one shows what Wee Chef flagged, with Keep as is and Undo.
struct ReviewView: View {
  @Environment(AppModel.self) private var app
  @State private var status: ChecksStatus?
  @State private var recipes: LoadState<[ReviewRecipe]> = .loading
  @State private var run: Int?
  @State private var reviewing: ReviewRecipe?

  var body: some View {
    List {
      Section {
        VStack(alignment: .leading, spacing: 10) {
          Label("Wee Chef", systemImage: "sparkles").font(Typeface.label).foregroundStyle(Palette.primary)
          if let status {
            if status.enabled {
              Text(RecipeText.checksStatus(status, run: run))
                .font(Typeface.body(15))
                .foregroundStyle(Palette.ink)
                .accessibilityIdentifier("checks-status")
              if status.due > 0 && status.pending == 0 {
                Button("Check all") { Task { await checkAll() } }.buttonStyle(.crumbSecondaryCompact)
              }
            } else {
              Text("Wee Chef's checks need an AI key on your Crumb server.")
                .font(Typeface.body(15))
                .foregroundStyle(Palette.inkMuted)
            }
          } else {
            ProgressView()
          }
        }
        .padding(.vertical, 6)
      }
      .listRowBackground(Palette.paper)

      Section("To look at") {
        switch recipes {
        case .loading:
          ProgressView()
        case .failed(let message):
          Text(message).foregroundStyle(Palette.inkMuted)
        case .ready(let list, _):
          if list.isEmpty {
            Text("Nothing to look at. Wee Chef will flag lines that seem off when it checks a recipe.")
              .font(Typeface.body(15))
              .foregroundStyle(Palette.inkMuted)
          }
          ForEach(list) { item in
            Button {
              reviewing = item
            } label: {
              HStack(spacing: 12) {
                RecipePhoto(recipeId: item.id, image: item.image, width: 56)
                  .frame(width: 56, height: 56)
                  .clipShape(RoundedRectangle(cornerRadius: Radius.control, style: .continuous))
                VStack(alignment: .leading, spacing: 2) {
                  Text(item.title).font(Typeface.subtitle).foregroundStyle(Palette.ink)
                  Text(item.count == 1 ? "1 suggestion" : "\(item.count) suggestions")
                    .font(Typeface.caption)
                    .foregroundStyle(Palette.inkMuted)
                }
              }
            }
          }
        }
      }
      .listRowBackground(Palette.paper)
    }
    .scrollContentBackground(.hidden)
    .canvasBackground()
    .navigationTitle("Review")
    .refreshable { await load() }
    .task { await load() }
    .task(id: status?.pending) {
      // While Check all runs, look again every few seconds
      guard let pending = status?.pending, pending > 0 else { return }
      try? await Task.sleep(for: .seconds(4))
      if !Task.isCancelled { await load() }
    }
    .sheet(item: $reviewing, onDismiss: reload) { item in
      FlagsView(recipeId: item.id, title: item.title)
    }
  }

  private func reload() {
    Task { await load() }
  }

  private func load() async {
    do {
      let api = app.api
      async let checks = api.checksStatus()
      async let list = api.reviewList()
      status = try await checks
      recipes = .ready(try await list, offline: false)
    } catch is CancellationError {
    } catch {
      recipes = .failed(app.handle(error))
    }
  }

  private func checkAll() async {
    do {
      let started = try await app.api.checkAll()
      run = started.queued
      status = started
    } catch {
      app.show("Couldn't start the checks", app.handle(error), tone: .error)
    }
  }
}

/// What Wee Chef flagged on one recipe.
struct FlagsView: View {
  let recipeId: Int64
  let title: String
  @Environment(AppModel.self) private var app
  @Environment(\.dismiss) private var dismiss
  @State private var checks: RecipeChecks?
  @State private var loading = true

  var body: some View {
    NavigationStack {
      List {
        if let checks {
          ForEach(checks.flags) { flag in
            VStack(alignment: .leading, spacing: 8) {
              Text(RecipeText.flagText(flag)).font(Typeface.body(16)).foregroundStyle(Palette.ink)
              if flag.state == "review" {
                Button("Keep as is") { Task { await dismissFlag(flag) } }
                  .buttonStyle(.crumbSecondaryCompact)
              }
            }
            .padding(.vertical, 4)
          }
          if checks.canUndo {
            Button("Undo Wee Chef's changes", systemImage: "arrow.uturn.backward") { Task { await undo() } }
          }
        } else if !loading {
          Text("Wee Chef hasn't checked this recipe.")
        }
      }
      .overlay { if loading { ProgressView() } }
      .navigationTitle(title)
      .navigationBarTitleDisplayMode(.inline)
      .toolbar {
        ToolbarItem(placement: .confirmationAction) { Button("Done") { dismiss() } }
        ToolbarItem(placement: .bottomBar) {
          Button("Open recipe") {
            dismiss()
            app.open(.recipe(recipeId), in: .recipes)
          }
        }
      }
      .task {
        checks = try? await app.api.recipeChecks(id: recipeId)
        loading = false
      }
    }
  }

  private func dismissFlag(_ flag: Flag) async {
    do {
      checks = try await app.api.dismissFlag(recipeId: recipeId, flagId: flag.id)
    } catch {
      app.show("Couldn't keep it", app.handle(error), tone: .error)
    }
  }

  private func undo() async {
    do {
      checks = try await app.api.undoChecks(id: recipeId).checks
      app.show("Put back as it was", tone: .success)
    } catch {
      app.show("Couldn't undo", app.handle(error), tone: .error)
    }
  }
}
