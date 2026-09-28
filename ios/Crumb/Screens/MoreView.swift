import CoreLocation
import CrumbKit
import SwiftUI

/// Your Crumb (open on the web, sign out), the theme, and a backup of the whole box.
struct MoreView: View {
  @Environment(AppModel.self) private var app
  @State private var exporting: ExportedFile?
  @State private var locating = LocationOnce()

  var body: some View {
    @Bindable var theme = app.theme
    List {
      Section("Your Crumb") {
        if let session = app.session {
          LabeledContent("Server", value: session.displayServer)
          Link(destination: session.server) {
            Label("Open on the web", systemImage: "safari")
          }
        }
        Button("Sign out", systemImage: "rectangle.portrait.and.arrow.right", role: .destructive) {
          Task { await app.signOut() }
        }
        .accessibilityIdentifier("sign-out")
      }
      .listRowBackground(Palette.paper)

      Section {
        Picker("Theme", selection: $theme.mode) {
          ForEach(ThemeMode.allCases) { mode in
            VStack(alignment: .leading) {
              Text(mode.label)
              Text(mode.detail).font(Typeface.caption).foregroundStyle(Palette.inkMuted)
            }
            .tag(mode)
          }
        }
        .pickerStyle(.inline)
        .labelsHidden()
        if theme.mode == .sun {
          if theme.location == nil {
            Button("Use my location", systemImage: "location") {
              locating.request { location in
                theme.location = SunLocation(lat: location.latitude, lng: location.longitude)
              }
            }
            Text("Until then, sunrise and sunset are estimated from your time zone.")
              .font(Typeface.caption)
              .foregroundStyle(Palette.inkMuted)
          } else {
            Button("Forget my location", systemImage: "location.slash") { theme.location = nil }
          }
        }
      } header: {
        Text("Theme")
      }
      .listRowBackground(Palette.paper)

      Section("Backup") {
        Button("Export every recipe", systemImage: "square.and.arrow.down") { Task { await export() } }
        Text("A Crumb backup file, the same as the web's. Import it into any Crumb.")
          .font(Typeface.caption)
          .foregroundStyle(Palette.inkMuted)
      }
      .listRowBackground(Palette.paper)

      Section {
        Text("Crumb for iPhone \(Self.version)")
          .font(Typeface.caption)
          .foregroundStyle(Palette.inkMuted)
      }
      .listRowBackground(Color.clear)
    }
    .scrollContentBackground(.hidden)
    .canvasBackground()
    .navigationTitle("More")
    .sheet(item: $exporting) { file in
      ShareSheet(items: [file.url])
    }
  }

  static var version: String {
    let info = Bundle.main.infoDictionary
    let short = info?["CFBundleShortVersionString"] as? String ?? "?"
    let build = info?["CFBundleVersion"] as? String ?? "?"
    return "\(short) (\(build))"
  }

  private func export() async {
    do {
      let file = try await app.api.exportAll()
      let url = FileManager.default.temporaryDirectory.appending(path: file.fileName)
      try file.data.write(to: url, options: .atomic)
      exporting = ExportedFile(url: url)
    } catch {
      app.show("Couldn't export", app.handle(error), tone: .error)
    }
  }
}

struct ExportedFile: Identifiable {
  let id = UUID()
  var url: URL
}

/// One location fix for "Sunrise & sunset", then stop. Coarse accuracy is plenty.
@Observable
final class LocationOnce: NSObject, CLLocationManagerDelegate {
  @ObservationIgnored private let manager = CLLocationManager()
  @ObservationIgnored private var done: ((CLLocationCoordinate2D) -> Void)?

  func request(_ completion: @escaping (CLLocationCoordinate2D) -> Void) {
    done = completion
    manager.delegate = self
    manager.desiredAccuracy = kCLLocationAccuracyReduced
    switch manager.authorizationStatus {
    case .notDetermined: manager.requestWhenInUseAuthorization()
    case .authorizedWhenInUse, .authorizedAlways: manager.requestLocation()
    default: break
    }
  }

  func locationManagerDidChangeAuthorization(_ manager: CLLocationManager) {
    if done != nil, [.authorizedWhenInUse, .authorizedAlways].contains(manager.authorizationStatus) {
      manager.requestLocation()
    }
  }

  func locationManager(_ manager: CLLocationManager, didUpdateLocations locations: [CLLocation]) {
    guard let location = locations.last else { return }
    done?(location.coordinate)
    done = nil
  }

  func locationManager(_ manager: CLLocationManager, didFailWithError error: Error) {
    done = nil
  }
}
