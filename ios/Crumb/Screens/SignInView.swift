import CrumbKit
import SwiftUI

/// Your Crumb's address and its password. The password is sent once and never stored; the
/// session cookie the server returns goes into the Keychain.
struct SignInView: View {
  @Environment(AppModel.self) private var app
  @State private var server = ""
  @State private var password = ""
  @State private var busy = false
  @State private var error: String?
  @FocusState private var focus: Field?

  private enum Field { case server, password }

  var body: some View {
    ScrollView {
      VStack(spacing: 18) {
        Image("Mark")
          .resizable()
          .scaledToFit()
          .frame(width: 88, height: 88)
          .clipShape(RoundedRectangle(cornerRadius: Radius.card, style: .continuous))
          .accessibilityHidden(true)
        Text("Welcome back")
          .font(Typeface.hand(32))
          .foregroundStyle(Palette.primary)
        Text("Crumb")
          .font(Typeface.display)
          .foregroundStyle(Palette.ink)

        VStack(alignment: .leading, spacing: 14) {
          field("Your Crumb address") {
            TextField("crumb.example.com", text: $server)
              .textContentType(.URL)
              .keyboardType(.URL)
              .textInputAutocapitalization(.never)
              .autocorrectionDisabled()
              .submitLabel(.next)
              .focused($focus, equals: .server)
              .onSubmit { focus = .password }
              .accessibilityIdentifier("server")
          }
          field("Password") {
            SecureField("Password", text: $password)
              .textContentType(.password)
              .submitLabel(.go)
              .focused($focus, equals: .password)
              .onSubmit(submit)
              .accessibilityIdentifier("password")
          }
          if let error {
            Text(error)
              .font(Typeface.body(15))
              .foregroundStyle(Palette.error)
              .accessibilityIdentifier("sign-in-error")
          }
          Button(action: submit) {
            HStack(spacing: 8) {
              if busy { ProgressView().tint(Palette.onButter) }
              Text(busy ? "Signing in…" : "Sign in")
            }
          }
          .buttonStyle(.crumbPrimary)
          .disabled(busy || server.trimmingCharacters(in: .whitespaces).isEmpty)
          .accessibilityIdentifier("sign-in")
        }
        .textFieldStyle(CrumbFieldStyle())
        .padding(20)
        .paperCard()

        Text("Crumb keeps your recipes on your own server. Your password isn't stored on this iPhone.")
          .font(Typeface.caption)
          .foregroundStyle(Palette.inkMuted)
          .multilineTextAlignment(.center)
      }
      .padding(24)
      .frame(maxWidth: 460)
      .frame(maxWidth: .infinity)
    }
    .scrollDismissesKeyboard(.interactively)
    .canvasBackground()
    .onAppear {
      if server.isEmpty, let last = app.sessions.lastServer, let url = URL(string: last) {
        server = Session(server: url, cookie: nil).displayServer
      }
    }
  }

  private func field<Content: View>(_ label: String, @ViewBuilder content: () -> Content) -> some View {
    VStack(alignment: .leading, spacing: 6) {
      Text(label).font(Typeface.small).foregroundStyle(Palette.inkMuted)
      content()
    }
  }

  private func submit() {
    guard !busy else { return }
    busy = true
    error = nil
    Task {
      let message = await app.signIn(server: server, password: password)
      busy = false
      error = message
      if message == nil { password = "" }
    }
  }
}
