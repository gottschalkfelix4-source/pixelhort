import SwiftUI

struct PeopleView: View {
    @Environment(AppSession.self) private var session
    @State private var people: [PersonDto] = []
    @State private var showHidden = false
    @State private var isLoading = false
    @State private var errorMessage: String?

    private let columns = [GridItem(.adaptive(minimum: 96), spacing: 16)]

    var body: some View {
        ScrollView {
            if !isLoading && people.isEmpty {
                StateView(kind: .empty("people.empty", "person.crop.circle.badge.questionmark"))
                    .padding(.top, 60)
            }
            LazyVGrid(columns: columns, spacing: 20) {
                ForEach(people) { person in
                    NavigationLink {
                        PersonDetailView(person: person)
                    } label: {
                        VStack(spacing: 6) {
                            PersonAvatar(personId: person.id, diameter: 90)
                                .opacity(person.isHidden == true ? 0.4 : 1)
                            Text(verbatim: person.displayName ?? "")
                                .font(.subheadline)
                                .lineLimit(1)
                            if let count = person.faceCount {
                                Text("people.count \(count)")
                                    .font(.caption)
                                    .foregroundStyle(.secondary)
                            }
                        }
                    }
                    .buttonStyle(.plain)
                }
            }
            .padding()
        }
        .overlay { if isLoading && people.isEmpty { ProgressView() } }
        .navigationTitle("library.people")
        .toolbar {
            ToolbarItem(placement: .topBarTrailing) {
                Toggle(isOn: $showHidden) {
                    Image(systemName: showHidden ? "eye" : "eye.slash")
                }
                .toggleStyle(.button)
                .accessibilityLabel(Text("people.showHidden"))
            }
        }
        .task(id: showHidden) { await load() }
        .refreshable { await load() }
        .errorAlert($errorMessage)
    }

    private func load() async {
        guard let api = session.api else { return }
        isLoading = true
        defer { isLoading = false }
        do {
            people = try await api.people(withHidden: showHidden).people
        } catch {
            errorMessage = error.localizedDescription
        }
    }
}

struct PersonDetailView: View {
    @Environment(AppSession.self) private var session
    @State private var person: PersonDto
    @State private var showRename = false
    @State private var newName = ""
    @State private var errorMessage: String?

    init(person: PersonDto) {
        _person = State(initialValue: person)
    }

    var body: some View {
        AssetTimelineView(context: .person(id: person.id)) {
            HStack(spacing: 16) {
                PersonAvatar(personId: person.id, diameter: 72)
                VStack(alignment: .leading, spacing: 4) {
                    if let name = person.displayName {
                        Text(verbatim: name).font(.title2.bold())
                    } else {
                        Button("people.addName") {
                            newName = ""
                            showRename = true
                        }
                        .font(.title3)
                    }
                    if let count = person.faceCount {
                        Text("people.count \(count)")
                            .font(.footnote)
                            .foregroundStyle(.secondary)
                    }
                }
                Spacer()
            }
            .padding()
        }
        .navigationTitle(Text(verbatim: person.displayName ?? ""))
        .navigationBarTitleDisplayMode(.inline)
        .toolbar {
            ToolbarItem(placement: .topBarTrailing) {
                Menu {
                    Button {
                        newName = person.name ?? ""
                        showRename = true
                    } label: {
                        Label("people.rename", systemImage: "pencil")
                    }
                    Button {
                        Task { await setHidden(!(person.isHidden ?? false)) }
                    } label: {
                        if person.isHidden == true {
                            Label("people.unhide", systemImage: "eye")
                        } else {
                            Label("people.hide", systemImage: "eye.slash")
                        }
                    }
                } label: {
                    Image(systemName: "ellipsis.circle")
                }
            }
        }
        .alert("people.rename", isPresented: $showRename) {
            TextField("people.name", text: $newName)
            Button("common.save") { Task { await rename() } }
            Button("common.cancel", role: .cancel) {}
        }
        .errorAlert($errorMessage)
    }

    private func rename() async {
        guard let api = session.api else { return }
        do {
            person = try await api.updatePerson(id: person.id, UpdatePersonRequest(name: newName.trimmingCharacters(in: .whitespaces)))
        } catch {
            errorMessage = error.localizedDescription
        }
    }

    private func setHidden(_ hidden: Bool) async {
        guard let api = session.api else { return }
        do {
            person = try await api.updatePerson(id: person.id, UpdatePersonRequest(isHidden: hidden))
        } catch {
            errorMessage = error.localizedDescription
        }
    }
}
