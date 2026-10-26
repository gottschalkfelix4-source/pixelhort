import Foundation
import Network

/// Tracks whether the current network path is "expensive" (cellular / personal hotspot),
/// used by the "Wi-Fi only" backup option.
final class NetworkMonitor: @unchecked Sendable {
    static let shared = NetworkMonitor()

    private let monitor = NWPathMonitor()
    private let queue = DispatchQueue(label: "app.pixelhort.network-monitor")
    private let lock = NSLock()
    private var _isExpensive = false
    private var _isConnected = true

    /// Called when the path changes (on a background queue).
    var onChange: (@Sendable (_ isConnected: Bool, _ isExpensive: Bool) -> Void)?

    var isExpensive: Bool { lock.withLock { _isExpensive } }
    var isConnected: Bool { lock.withLock { _isConnected } }

    private init() {
        monitor.pathUpdateHandler = { [weak self] path in
            guard let self else { return }
            let connected = path.status == .satisfied
            let expensive = path.isExpensive
            self.lock.withLock {
                self._isConnected = connected
                self._isExpensive = expensive
            }
            self.onChange?(connected, expensive)
        }
        monitor.start(queue: queue)
    }
}
