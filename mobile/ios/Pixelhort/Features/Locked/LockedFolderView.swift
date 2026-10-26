import SwiftUI

/// Locked folder: unlocked server-side with the PIN (`POST /api/auth/session/unlock`, 15 min).
/// Face ID is a local convenience gate that releases the PIN stored in the Keychain.
struct LockedFolderView: View {
    enum Phase: Equatable {
        case checking
        case needsPinSetup
        case locked
        case unlocked
    }

    @Environment(AppSession.self) private var session
    @AppStorage("locked.useBiometrics") private var useBiometrics = false
    @State private var phase: Phase = .checking
    @State private var pin = ""
    @State private var newPin = ""
    @State private var confirmPin = ""
    @State private var isWorking = false
    @State private var errorMessage: String?

    var body: some View {
        Group {
            switch phase {
            case .checking:
                ProgressView()
            case .needsPinSetup:
                pinSetupForm
            case .locked:
                unlockForm
            case .unlocked:
                AssetTimelineView(context: .locked) {
                    Label("locked.hint", systemImage: "lock.shield")
                        .font(.footnote)
                        .foregroundStyle(.secondary)
                        .padding()
                }
                .toolbar {
                    ToolbarItem(placement: .topBarLeading) {
                        Button {
                            Task { await lock() }
                        } label: {
                            Image(systemName: "lock")
                        }
                        .accessibilityLabel(Text("locked.lockNow"))
                    }
                }
            }
        }
        .navigationTitle("library.locked")
        .task { await check() }
        .onDisappear {
            // Re-lock when leaving the folder.
            if phase == .unlocked, let api = session.api {
                Task { try? await api.lockSession() }
            }
        }
        .errorAlert($errorMessage)
    }

    // MARK: Forms

    private var unlockForm: some View {
        Form {
            Section {
                VStack(spacing: 10) {
                    Image(systemName: "lock.shield.fill")
                        .font(.system(size: 52))
                        .foregroundStyle(.tint)
                    Text("locked.enterPin")
                        .font(.headline)
                }
                .frame(maxWidth: .infinity)
                .padding(.vertical)
            }
            .listRowBackground(Color.clear)

            Section {
                SecureField("locked.pin", text: $pin)
                    .keyboardType(.numberPad)
                    .textContentType(.oneTimeCode)
                Button {
                    Task { await unlock(with: pin, remember: useBiometrics) }
                } label: {
                    HStack {
                        Spacer()
                        if isWorking { ProgressView() } else { Text("locked.unlock").bold() }
                        Spacer()
                    }
                }
                .disabled(pin.count < 4 || isWorking)
            }

            if BiometricAuth.isAvailable {
                Section {
                    Toggle(isOn: $useBiometrics) {
                        Text("locked.useBiometrics \(BiometricAuth.biometryName)")
                    }
                    if useBiometrics, KeychainStore.string(for: .lockedFolderPin) != nil {
                        Button {
                            Task { await unlockWithBiometrics() }
                        } label: {
                            Label {
                                Text("locked.unlockWith \(BiometricAuth.biometryName)")
                            } icon: {
                                Image(systemName: "faceid")
                            }
                        }
                    }
                } footer: {
                    Text("locked.biometrics.footer")
                }
            }
        }
    }

    private var pinSetupForm: some View {
        Form {
            Section {
                SecureField("locked.newPin", text: $newPin)
                    .keyboardType(.numberPad)
                SecureField("locked.confirmPin", text: $confirmPin)
                    .keyboardType(.numberPad)
            } header: {
                Text("locked.setup.title")
            } footer: {
                Text("locked.setup.footer")
            }
            Section {
                Button("locked.setup.save") { Task { await setupPin() } }
                    .disabled(!isValidPin(newPin) || newPin != confirmPin || isWorking)
            }
        }
    }

    // MARK: Logic

    private func isValidPin(_ value: String) -> Bool {
        (4...8).contains(value.count) && value.allSatisfy(\.isNumber)
    }

    private func check() async {
        guard let api = session.api else { return }
        do {
            let info = try await api.sessionInfo()
            if info.isUnlocked {
                phase = .unlocked
            } else if info.hasPin == false {
                phase = .needsPinSetup
            } else {
                phase = .locked
                if useBiometrics, KeychainStore.string(for: .lockedFolderPin) != nil {
                    await unlockWithBiometrics()
                }
            }
        } catch {
            phase = .locked
            errorMessage = error.localizedDescription
        }
    }

    private func unlockWithBiometrics() async {
        let reason = NSLocalizedString("locked.biometrics.reason", comment: "")
        guard await BiometricAuth.authenticate(reason: reason),
              let stored = KeychainStore.string(for: .lockedFolderPin) else { return }
        await unlock(with: stored, remember: true)
    }

    private func unlock(with value: String, remember: Bool) async {
        guard let api = session.api else { return }
        isWorking = true
        defer { isWorking = false }
        do {
            _ = try await api.unlockSession(pin: value)
            if remember {
                KeychainStore.set(value, for: .lockedFolderPin)
            }
            pin = ""
            phase = .unlocked
        } catch {
            if remember {
                // Stored PIN is outdated.
                KeychainStore.set(nil, for: .lockedFolderPin)
            }
            errorMessage = error.localizedDescription
        }
    }

    private func setupPin() async {
        guard let api = session.api else { return }
        isWorking = true
        defer { isWorking = false }
        do {
            try await api.setPin(newPin, currentPin: nil)
            let value = newPin
            newPin = ""
            confirmPin = ""
            phase = .locked
            await unlock(with: value, remember: useBiometrics)
        } catch {
            errorMessage = error.localizedDescription
        }
    }

    private func lock() async {
        guard let api = session.api else { return }
        try? await api.lockSession()
        phase = .locked
    }
}
