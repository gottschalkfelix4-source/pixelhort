import Foundation
#if canImport(FoundationNetworking)
import FoundationNetworking
#endif

/// Owns the background `URLSession` used for asset uploads. Upload tasks are created from a
/// multipart body file so the system can continue them while the app is suspended or terminated.
final class BackupUploader: NSObject, @unchecked Sendable {
    static let shared = BackupUploader()
    static let sessionIdentifier = "app.pixelhort.backup.upload"

    struct UploadFailure: LocalizedError {
        let message: String
        var errorDescription: String? { message }
    }

    /// Set by the AppDelegate when the system relaunches the app for background session events.
    var backgroundCompletionHandler: (() -> Void)? {
        get { lock.withLock { _backgroundCompletionHandler } }
        set { lock.withLock { _backgroundCompletionHandler = newValue } }
    }

    /// Progress callback (bytes sent / expected) for the task currently awaited in the foreground.
    var onProgress: (@Sendable (String, Double) -> Void)?

    private let lock = NSLock()
    private var _backgroundCompletionHandler: (() -> Void)?
    private var responseData: [Int: Data] = [:]
    private var continuations: [Int: CheckedContinuation<UploadResponse, Error>] = [:]

    private lazy var session: URLSession = {
        let configuration = URLSessionConfiguration.background(withIdentifier: Self.sessionIdentifier)
        #if os(iOS)
        configuration.sessionSendsLaunchEvents = true
        #endif
        configuration.isDiscretionary = false
        configuration.allowsCellularAccess = true // narrowed per request (Wi-Fi only option)
        configuration.timeoutIntervalForResource = 60 * 60 * 24
        let queue = OperationQueue()
        queue.maxConcurrentOperationCount = 1
        queue.name = "app.pixelhort.backup.delegate"
        return URLSession(configuration: configuration, delegate: self, delegateQueue: queue)
    }()

    static var bodyDirectory: URL {
        let base = FileManager.default.urls(for: .cachesDirectory, in: .userDomainMask).first
            ?? FileManager.default.temporaryDirectory
        return base.appendingPathComponent("upload-bodies", isDirectory: true)
    }

    /// Re-creates the session so pending events from a previous process are delivered.
    func activate() {
        _ = session
    }

    /// `localIdentifier`s of uploads still running in the background session.
    func activeLocalIdentifiers() async -> Set<String> {
        let tasks: [URLSessionTask] = await withCheckedContinuation { continuation in
            session.getAllTasks { continuation.resume(returning: $0) }
        }
        var identifiers = Set<String>()
        for task in tasks where task.state == .running || task.state == .suspended {
            if let context = UploadTaskContext.decode(task.taskDescription) {
                identifiers.insert(context.localIdentifier)
            }
        }
        return identifiers
    }

    func cancelAll() {
        session.getAllTasks { tasks in
            tasks.forEach { $0.cancel() }
        }
    }

    /// Writes the multipart body and enqueues the upload.
    ///
    /// - Parameter waitForCompletion: when false the task is only enqueued (used from short
    ///   background windows); the completion is persisted by the delegate.
    func upload(api: APIClient, resource: ExportedResource, fields: [(name: String, value: String)],
                context: UploadTaskContext, allowsCellular: Bool, waitForCompletion: Bool) async throws -> UploadResponse? {
        let writer = MultipartFormWriter()
        let bodyURL = URL(fileURLWithPath: context.bodyFilePath)
        try writer.write(fields: fields,
                         files: [.init(fieldName: "assetData", fileURL: resource.fileURL,
                                       fileName: resource.fileName, mimeType: resource.mimeType)],
                         to: bodyURL)

        let request = api.uploadRequest(contentType: writer.contentType, checksum: resource.checksum,
                                        allowsCellular: allowsCellular)
        let task = session.uploadTask(with: request, fromFile: bodyURL)
        task.taskDescription = context.encoded()

        guard waitForCompletion else {
            task.resume()
            return nil
        }

        return try await withTaskCancellationHandler {
            try await withCheckedThrowingContinuation { (continuation: CheckedContinuation<UploadResponse, Error>) in
                lock.withLock { continuations[task.taskIdentifier] = continuation }
                task.resume()
            }
        } onCancel: {
            task.cancel()
        }
    }

    // MARK: - Completion handling

    private func finish(task: URLSessionTask, error: Error?) {
        let identifier = task.taskIdentifier
        let (data, continuation) = lock.withLock { () -> (Data?, CheckedContinuation<UploadResponse, Error>?) in
            let data = responseData.removeValue(forKey: identifier)
            let continuation = continuations.removeValue(forKey: identifier)
            return (data, continuation)
        }
        let context = UploadTaskContext.decode(task.taskDescription)
        if let context {
            try? FileManager.default.removeItem(atPath: context.bodyFilePath)
        }

        let result: Result<UploadResponse, Error>
        if let error {
            if (error as? URLError)?.code == .cancelled {
                result = .failure(CancellationError())
            } else {
                result = .failure(UploadFailure(message: error.localizedDescription))
            }
        } else if let http = task.response as? HTTPURLResponse {
            do {
                try APIClient.validate(status: http.statusCode, data: data ?? Data())
                let response = try JSONCoding.makeDecoder().decode(UploadResponse.self, from: data ?? Data())
                result = .success(response)
            } catch {
                result = .failure(error)
            }
        } else {
            result = .failure(APIError.invalidResponse)
        }

        // Persist so completions that happen while the app is suspended are not lost.
        let store = UploadStateStore.shared
        Task {
            if let context {
                switch result {
                case let .success(response):
                    switch context.role {
                    case .primary:
                        await store.markUploaded(context.localIdentifier, checksum: context.checksum,
                                                 remoteAssetId: response.id, modificationDate: context.modificationDate)
                    case .livePhotoVideo:
                        await store.setLivePhotoVideo(context.localIdentifier, remoteId: response.id,
                                                      checksum: context.checksum)
                    }
                case let .failure(error):
                    if !(error is CancellationError) {
                        await store.markFailed(context.localIdentifier, error: error.localizedDescription)
                    }
                }
                await store.flush()
            }
            continuation?.resume(with: result)
        }
    }
}

extension BackupUploader: URLSessionDataDelegate {
    func urlSession(_ session: URLSession, dataTask: URLSessionDataTask, didReceive data: Data) {
        lock.withLock {
            responseData[dataTask.taskIdentifier, default: Data()].append(data)
        }
    }

    func urlSession(_ session: URLSession, task: URLSessionTask, didSendBodyData bytesSent: Int64,
                    totalBytesSent: Int64, totalBytesExpectedToSend: Int64) {
        guard totalBytesExpectedToSend > 0,
              let context = UploadTaskContext.decode(task.taskDescription) else { return }
        onProgress?(context.fileName, Double(totalBytesSent) / Double(totalBytesExpectedToSend))
    }

    func urlSession(_ session: URLSession, task: URLSessionTask, didCompleteWithError error: Error?) {
        finish(task: task, error: error)
    }

    func urlSessionDidFinishEvents(forBackgroundURLSession session: URLSession) {
        let handler = backgroundCompletionHandler
        backgroundCompletionHandler = nil
        DispatchQueue.main.async {
            handler?()
        }
    }
}
