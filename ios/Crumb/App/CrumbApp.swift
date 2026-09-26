import CrumbKit
import SwiftUI
import UIKit

@main
struct CrumbApp: App {
  @State private var app = AppModel()
  @Environment(\.scenePhase) private var scenePhase

  init() {
    Self.styleSystemBars()
  }

  var body: some Scene {
    WindowGroup {
      RootView()
        .environment(app)
        .onOpenURL { app.handle(url: $0) }
        .onAppear {
          app.alarms.onOpenRecipe = { [app] id in app.open(.recipe(id), in: .recipes) }
        }
    }
    .onChange(of: scenePhase) { _, phase in
      // The Share extension may have signed in or changed the theme meanwhile
      if phase == .active { app.sessions.reload() }
    }
  }

  /// The web's dark tab bar with a butter active item; navigation bars on the canvas.
  private static func styleSystemBars() {
    let tabs = UITabBarAppearance()
    tabs.configureWithOpaqueBackground()
    tabs.backgroundColor = UIColor(Palette.nav)
    tabs.shadowColor = .clear
    let item = UITabBarItemAppearance()
    let font = UIFont(name: "NunitoSans-Bold", size: 11) ?? .systemFont(ofSize: 11, weight: .bold)
    item.normal.iconColor = UIColor(Palette.navInk)
    item.normal.titleTextAttributes = [.foregroundColor: UIColor(Palette.navInk), .font: font]
    item.selected.iconColor = UIColor(Palette.butter)
    item.selected.titleTextAttributes = [.foregroundColor: UIColor(Palette.butter), .font: font]
    tabs.stackedLayoutAppearance = item
    tabs.inlineLayoutAppearance = item
    tabs.compactInlineLayoutAppearance = item
    UITabBar.appearance().standardAppearance = tabs
    UITabBar.appearance().scrollEdgeAppearance = tabs

    let bars = UINavigationBarAppearance()
    bars.configureWithTransparentBackground()
    bars.backgroundColor = UIColor(Palette.canvas)
    let ink = UIColor(Palette.ink)
    bars.titleTextAttributes = [
      .foregroundColor: ink, .font: UIFont(name: "NunitoSans-Bold", size: 17) ?? .boldSystemFont(ofSize: 17),
    ]
    bars.largeTitleTextAttributes = [
      .foregroundColor: ink, .font: UIFont(name: "DMSerifDisplay-Regular", size: 34) ?? .boldSystemFont(ofSize: 34),
    ]
    UINavigationBar.appearance().standardAppearance = bars
    UINavigationBar.appearance().scrollEdgeAppearance = bars
    UINavigationBar.appearance().compactAppearance = bars
    UINavigationBar.appearance().tintColor = UIColor(Palette.primary)
  }
}

/// Sign-in until there's a session, then the tabs. Applies the theme to the whole window.
struct RootView: View {
  @Environment(AppModel.self) private var app
  @State private var decision = ThemeDecision(dark: nil)

  var body: some View {
    ZStack(alignment: .top) {
      if app.session == nil {
        SignInView()
          .transition(.opacity)
      } else {
        MainTabs()
          .transition(.opacity)
      }
      ToastView()
    }
    .animation(.easeInOut(duration: 0.25), value: app.session == nil)
    .tint(Palette.primary)
    .task(id: themeKey) { await followTheme() }
  }

  private var themeKey: String {
    "\(app.theme.mode.rawValue)|\(app.theme.location?.lat ?? .nan)|\(app.theme.location?.lng ?? .nan)"
  }

  /// Light, dark or system for every window (sheets and alerts too). In "Sunrise & sunset"
  /// it looks again at the next sunrise or sunset, and at least hourly.
  private func followTheme() async {
    while !Task.isCancelled {
      decision = app.theme.decide()
      let style: UIUserInterfaceStyle =
        switch decision.dark {
        case .some(true): .dark
        case .some(false): .light
        case .none: .unspecified
        }
      for scene in UIApplication.shared.connectedScenes {
        for window in (scene as? UIWindowScene)?.windows ?? [] {
          window.overrideUserInterfaceStyle = style
        }
      }
      guard let recheck = decision.recheckAt else { return }
      try? await Task.sleep(for: .seconds(max(1, recheck.timeIntervalSinceNow)))
    }
  }
}

/// The five tabs, each with its own navigation stack, plus the Add sheet, cook mode and
/// the running timers above the tab bar.
struct MainTabs: View {
  @Environment(AppModel.self) private var app

  var body: some View {
    @Bindable var app = app
    TabView(selection: $app.tab) {
      ForEach(AppTab.allCases, id: \.self) { tab in
        NavigationStack(path: pathBinding(tab)) {
          screen(tab)
            .navigationDestination(for: Route.self) { route in
              switch route {
              case .recipe(let id): RecipeView(id: id)
              case .cookbook(let id): CookbookView(id: id)
              }
            }
        }
        .safeAreaInset(edge: .bottom, spacing: 0) { TimerDock() }
        .tabItem { Label(tab.label, systemImage: tab.systemImage) }
        .tag(tab)
        .accessibilityIdentifier("tab-\(tab.rawValue)")
      }
    }
    .sheet(isPresented: $app.showAdd) {
      AddView(initial: app.addDraft)
    }
    .fullScreenCover(item: $app.cooking) { recipe in
      CookView(recipe: recipe)
    }
    .task { await app.loadConnector() }
  }

  private func pathBinding(_ tab: AppTab) -> Binding<[Route]> {
    Binding(get: { app.paths[tab] ?? [] }, set: { app.paths[tab] = $0 })
  }

  @ViewBuilder
  private func screen(_ tab: AppTab) -> some View {
    switch tab {
    case .home: HomeView()
    case .recipes: RecipesView()
    case .shelf: ShelfView()
    case .review: ReviewView()
    case .more: MoreView()
    }
  }
}

/// The current toast, sliding in from the top for a few seconds.
struct ToastView: View {
  @Environment(AppModel.self) private var app

  var body: some View {
    Group {
      if let toast = app.toast {
        HStack(alignment: .top, spacing: 10) {
          Image(systemName: icon(toast.tone))
            .foregroundStyle(toast.tone == .error ? Palette.error : Palette.primary)
          VStack(alignment: .leading, spacing: 2) {
            Text(toast.title).font(Typeface.label).foregroundStyle(Palette.ink)
            if let message = toast.message {
              Text(message).font(Typeface.caption).foregroundStyle(Palette.inkMuted)
            }
          }
          Spacer(minLength: 0)
        }
        .padding(14)
        .background(RoundedRectangle(cornerRadius: Radius.card, style: .continuous).fill(Palette.paper))
        .overlay(RoundedRectangle(cornerRadius: Radius.card, style: .continuous).strokeBorder(Palette.line))
        .shadow(color: .black.opacity(0.12), radius: 12, y: 4)
        .padding(.horizontal, 16)
        .padding(.top, 8)
        .transition(.move(edge: .top).combined(with: .opacity))
        .onTapGesture { app.toast = nil }
        .task(id: toast.id) {
          try? await Task.sleep(for: .seconds(toast.tone == .error ? 6 : 3.5))
          if app.toast?.id == toast.id { app.toast = nil }
        }
        .accessibilityElement(children: .combine)
        .accessibilityIdentifier("toast")
      }
    }
    .animation(.spring(duration: 0.35), value: app.toast)
  }

  private func icon(_ tone: Toast.Tone) -> String {
    switch tone {
    case .info: return "info.circle.fill"
    case .success: return "checkmark.circle.fill"
    case .error: return "exclamationmark.triangle.fill"
    }
  }
}

/// Running kitchen timers, counting down above the tab bar on every screen.
struct TimerDock: View {
  @Environment(AppModel.self) private var app

  var body: some View {
    if !app.timers.timers.isEmpty {
      TimelineView(.periodic(from: .now, by: 1)) { context in
        ScrollView(.horizontal, showsIndicators: false) {
          HStack(spacing: 8) {
            ForEach(app.timers.timers) { timer in
              chip(timer, now: context.date)
            }
          }
          .padding(.horizontal, 12)
          .padding(.vertical, 8)
        }
        .background(Palette.canvas.opacity(0.96))
        .overlay(alignment: .top) { Rectangle().fill(Palette.line).frame(height: 1) }
      }
      .accessibilityIdentifier("timer-dock")
    }
  }

  private func chip(_ timer: KitchenTimer, now: Date) -> some View {
    let done = timer.isDone(at: now)
    return HStack(spacing: 8) {
      Image(systemName: done ? "bell.and.waves.left.and.right.fill" : "timer")
        .symbolEffect(.pulse, options: .repeating, isActive: done)
      VStack(alignment: .leading, spacing: 0) {
        Text(done ? "Time's up" : timer.clock(at: now))
          .font(Typeface.body(15, weight: .bold).monospacedDigit())
        Text(timer.label).font(Typeface.caption).lineLimit(1)
      }
      Menu {
        Button("Add a minute", systemImage: "plus") { app.timers.addMinute(timer.id) }
        if let id = timer.recipeId {
          Button("Open recipe", systemImage: "book") { app.open(.recipe(id), in: .recipes) }
        }
        Button(done ? "Dismiss" : "Stop timer", systemImage: "xmark", role: .destructive) {
          app.timers.dismiss(timer.id)
        }
      } label: {
        Image(systemName: "ellipsis.circle").font(.system(size: 18))
      }
      .accessibilityLabel("Timer options")
    }
    .foregroundStyle(done ? Palette.onButter : Palette.onTile)
    .padding(.leading, 12)
    .padding(.trailing, 8)
    .padding(.vertical, 6)
    .background(Capsule().fill(done ? Palette.butter : Palette.tile))
    .accessibilityElement(children: .contain)
  }
}
