import SwiftUI

/// Backup status: totals, current file, recent uploads, failures.
struct BackupProgressView: View {
    @Environment(BackupService.self) private var backup
    @State private var failed: [UploadRecord] = []

    private var fraction: Double {
        guard backup.totalAssets > 0 else { return 0 }
        return Double(backup.backedUpAssets) / Double(backup.totalAssets)
    }

    var body: some View {
        List {
            Section {
                VStack(spacing: 14) {
                    ZStack {
                        Circle()
                            .stroke(Color.secondary.opacity(0.2), lineWidth: 12)
                        Circle()
                            .trim(from: 0, to: fraction)
                            .stroke(Color.accentColor, style: StrokeStyle(lineWidth: 12, lineCap: .round))
                            .rotationEffect(.degrees(-90))
                            .animation(.easeInOut, value: fraction)
                        VStack(spacing: 2) {
                            Text(verbatim: "\(Int((fraction * 100).rounded())) %")
                                .font(.title.bold())
                            Text("backup.progress.count \(backup.backedUpAssets) \(backup.totalAssets)")
                                .font(.caption)
                                .foregroundStyle(.secondary)
                        }
                    }
                    .frame(width: 150, height: 150)

                    statusLabel
                }
                .frame(maxWidth: .infinity)
                .padding(.vertical)
            }

            if let fileName = backup.currentFileName {
                Section("backup.progress.current") {
                    VStack(alignment: .leading, spacing: 6) {
                        Text(verbatim: fileName).lineLimit(1)
                        ProgressView(value: backup.currentProgress)
                    }
                }
            }

            Section {
                LabeledContent("backup.progress.remaining") { Text(verbatim: "\(backup.remainingAssets)") }
                LabeledContent("backup.progress.failed") { Text(verbatim: "\(backup.failedCount)") }
                if let last = backup.lastRunAt {
                    LabeledContent("backup.progress.lastRun") {
                        Text(last, style: .relative)
                    }
                }
                Button {
                    if backup.isRunning {
                        backup.stop()
                    } else {
                        backup.start(.manual)
                    }
                } label: {
                    Label(backup.isRunning ? LocalizedStringKey("backup.stop") : LocalizedStringKey("backup.startNow"),
                          systemImage: backup.isRunning ? "stop.circle" : "arrow.clockwise.icloud")
                }
                .disabled(!backup.settings.isEnabled)
            }

            if !backup.recentUploads.isEmpty {
                Section("backup.progress.recent") {
                    ForEach(backup.recentUploads) { item in
                        HStack {
                            Image(systemName: item.duplicate ? "doc.on.doc" : "checkmark.circle.fill")
                                .foregroundStyle(item.duplicate ? Color.secondary : Color.green)
                            Text(verbatim: item.fileName).lineLimit(1)
                            Spacer()
                            Text(item.date, style: .time)
                                .font(.caption)
                                .foregroundStyle(.secondary)
                        }
                    }
                }
            }

            if !failed.isEmpty {
                Section("backup.progress.errors") {
                    ForEach(failed.prefix(20), id: \.localIdentifier) { record in
                        VStack(alignment: .leading, spacing: 2) {
                            Text(verbatim: record.lastError ?? "–")
                                .font(.footnote)
                            Text(verbatim: record.updatedAt.formatted(date: .abbreviated, time: .shortened))
                                .font(.caption2)
                                .foregroundStyle(.secondary)
                        }
                    }
                }
            }
        }
        .navigationTitle("backup.title")
        .task {
            await backup.refreshCounts()
            failed = await UploadStateStore.shared.failedRecords()
        }
        .refreshable {
            await backup.refreshCounts()
            failed = await UploadStateStore.shared.failedRecords()
        }
    }

    @ViewBuilder
    private var statusLabel: some View {
        switch backup.status {
        case .idle:
            Label(backup.remainingAssets == 0 ? LocalizedStringKey("backup.status.done") : LocalizedStringKey("backup.status.idle"),
                  systemImage: "checkmark.icloud")
        case .scanning:
            Label("backup.status.scanning", systemImage: "magnifyingglass")
        case .uploading:
            Label("backup.status.uploading", systemImage: "icloud.and.arrow.up")
        case .waitingForWiFi:
            Label("backup.status.wifi", systemImage: "wifi.exclamationmark")
        case .noPermission:
            Label("backup.status.noPermission", systemImage: "exclamationmark.triangle")
                .foregroundStyle(.orange)
        case .disabled:
            Label("backup.status.disabled", systemImage: "icloud.slash")
                .foregroundStyle(.secondary)
        case let .failed(message):
            Label {
                Text(verbatim: message)
            } icon: {
                Image(systemName: "exclamationmark.icloud")
            }
            .foregroundStyle(.orange)
        }
    }
}
