import Foundation
import Security

/// Minimal generic-password Keychain wrapper.
enum KeychainStore {
    private static let service = "app.pixelhort"

    enum Key: String {
        case accessToken = "accessToken"
        case deviceId = "deviceId"
        case lockedFolderPin = "lockedFolderPin"
    }

    /// Access token and device id must be readable by background tasks while the device is
    /// locked (after first unlock). The locked-folder PIN is only accessible while unlocked.
    private static func accessibility(for key: Key) -> CFString {
        switch key {
        case .lockedFolderPin:
            return kSecAttrAccessibleWhenUnlockedThisDeviceOnly
        case .accessToken, .deviceId:
            return kSecAttrAccessibleAfterFirstUnlockThisDeviceOnly
        }
    }

    private static func baseQuery(_ key: Key) -> [String: Any] {
        [
            kSecClass as String: kSecClassGenericPassword,
            kSecAttrService as String: service,
            kSecAttrAccount as String: key.rawValue,
        ]
    }

    static func string(for key: Key) -> String? {
        var query = baseQuery(key)
        query[kSecReturnData as String] = true
        query[kSecMatchLimit as String] = kSecMatchLimitOne
        var result: AnyObject?
        let status = SecItemCopyMatching(query as CFDictionary, &result)
        guard status == errSecSuccess, let data = result as? Data else { return nil }
        return String(data: data, encoding: .utf8)
    }

    @discardableResult
    static func set(_ value: String?, for key: Key) -> Bool {
        let query = baseQuery(key)
        guard let value else {
            let status = SecItemDelete(query as CFDictionary)
            return status == errSecSuccess || status == errSecItemNotFound
        }
        let data = Data(value.utf8)
        let attributes: [String: Any] = [
            kSecValueData as String: data,
            kSecAttrAccessible as String: accessibility(for: key),
        ]
        let updateStatus = SecItemUpdate(query as CFDictionary, attributes as CFDictionary)
        if updateStatus == errSecSuccess { return true }
        guard updateStatus == errSecItemNotFound else { return false }
        var addQuery = query
        addQuery.merge(attributes) { _, new in new }
        return SecItemAdd(addQuery as CFDictionary, nil) == errSecSuccess
    }
}

/// Stable per-installation device id (survives reinstall thanks to the Keychain).
enum DeviceIdentity {
    static var deviceId: String {
        if let existing = KeychainStore.string(for: .deviceId) {
            return existing
        }
        if let cached = UserDefaults.standard.string(forKey: "deviceId") {
            KeychainStore.set(cached, for: .deviceId)
            return cached
        }
        let created = UUID().uuidString
        KeychainStore.set(created, for: .deviceId)
        UserDefaults.standard.set(created, forKey: "deviceId")
        return created
    }
}
