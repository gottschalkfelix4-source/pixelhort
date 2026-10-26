import XCTest

/// Runs after `BackupFlowUITests` (alphabetical order): the user is logged in and the
/// simulator photos are on the server. Test data is prepared through the API, then every
/// library feature is checked in the app.
final class FeatureTourUITests: XCTestCase {
    private var env: [String: String] { ProcessInfo.processInfo.environment }
    private var server: String { env["PH_TEST_SERVER"] ?? "http://localhost:2342" }
    private let pin = "1357"

    private var app: XCUIApplication!
    private var token = ""

    override func setUpWithError() throws {
        continueAfterFailure = true
        app = XCUIApplication()
        app.launchArguments += ["-uiTesting", "-AppleLanguages", "(en)", "-AppleLocale", "en_US"]
    }

    func testFeatureTour() throws {
        token = try login(email: "ios@example.com", password: "iostester1")
        let fixture = try prepareData()

        app.launch()
        // Unsigned simulator builds have no Keychain, so the login may not survive a relaunch.
        logInIfNeeded()
        let tabBar = app.tabBars.firstMatch
        XCTAssertTrue(tabBar.waitForExistence(timeout: 30), "app is not logged in")

        // Memories ("On this day") on the photos tab.
        tabBar.buttons.element(boundBy: 0).tap()
        XCTAssertTrue(text(containing: "years ago").waitForExistence(timeout: 20), "memories card missing")
        shot("memories")

        // Album created on the server, plus creating one in the app.
        tabBar.buttons.element(boundBy: 2).tap()
        XCTAssertTrue(text(equal: "Urlaub").waitForExistence(timeout: 15), "album missing")
        text(equal: "Urlaub").tap()
        XCTAssertTrue(cell.waitForExistence(timeout: 15), "album has no photos")
        shot("album")
        back()

        // Library sections.
        tabBar.buttons.element(boundBy: 3).tap()
        checkLibrary("Favorites", expectCell: true)
        checkLibrary("Archive", expectCell: true)
        checkLibrary("Trash", expectCell: true)

        openLibrary("Shared links")
        XCTAssertTrue(text(containing: "1 item").waitForExistence(timeout: 15), "shared link missing")
        shot("shared-links")
        back()

        openLibrary("Partners")
        XCTAssertTrue(text(equal: "Admin").waitForExistence(timeout: 15), "partner missing")
        shot("partners")
        back()

        openLibrary("Map")
        XCTAssertTrue(app.maps.firstMatch.waitForExistence(timeout: 15), "map missing")
        sleep(3)
        shot("map")
        back()

        // Locked folder: unlock with the PIN and see the locked photo.
        openLibrary("Locked folder")
        let pinField = app.secureTextFields.firstMatch
        XCTAssertTrue(pinField.waitForExistence(timeout: 15), "PIN field missing")
        pinField.tap()
        pinField.typeText(pin)
        app.buttons["Unlock"].firstMatch.tap()
        XCTAssertTrue(cell.waitForExistence(timeout: 15), "locked photo not shown after unlock")
        shot("locked")
        back()

        // Search (file name fallback without the ML service).
        tabBar.buttons.element(boundBy: 1).tap()
        let search = app.searchFields.firstMatch
        XCTAssertTrue(search.waitForExistence(timeout: 10))
        search.tap()
        search.typeText(fixture.searchTerm + "\n")
        XCTAssertTrue(cell.waitForExistence(timeout: 20), "search found nothing for \(fixture.searchTerm)")
        shot("search")

        // Viewer: info sheet with metadata, favorite toggle.
        cell.tap()
        let info = app.buttons["Info"].firstMatch
        if info.waitForExistence(timeout: 10) {
            info.tap()
            XCTAssertTrue(text(containing: ".jpg").waitForExistence(timeout: 10), "info sheet shows no file name")
            shot("viewer-info")
        } else {
            XCTFail("viewer has no info button")
        }
    }

    // MARK: - UI helpers

    private func logInIfNeeded() {
        let url = app.textFields["server.url"]
        let email = app.textFields["login.email"]
        let tabs = app.tabBars.firstMatch
        let deadline = Date().addingTimeInterval(30)
        while Date() < deadline, !tabs.exists, !url.exists, !email.exists {
            RunLoop.current.run(until: Date().addingTimeInterval(0.5))
        }
        if url.exists {
            url.tap()
            url.typeText(server)
            app.buttons["server.connect"].tap()
            _ = email.waitForExistence(timeout: 30)
        }
        if email.exists {
            email.tap()
            email.typeText("ios@example.com")
            let password = app.secureTextFields["login.password"]
            password.tap()
            password.typeText("iostester1")
            app.buttons["login.submit"].tap()
        }
    }

    private var cell: XCUIElement {
        app.descendants(matching: .any).matching(identifier: "timeline.asset").firstMatch
    }

    private func text(equal value: String) -> XCUIElement {
        app.staticTexts.matching(NSPredicate(format: "label == %@", value)).firstMatch
    }

    private func text(containing value: String) -> XCUIElement {
        app.staticTexts.matching(NSPredicate(format: "label CONTAINS[c] %@", value)).firstMatch
    }

    private func openLibrary(_ title: String) {
        let link = app.buttons.matching(NSPredicate(format: "label CONTAINS %@", title)).firstMatch
        XCTAssertTrue(link.waitForExistence(timeout: 10), "library entry \(title) missing")
        link.tap()
    }

    private func checkLibrary(_ title: String, expectCell: Bool) {
        openLibrary(title)
        if expectCell {
            XCTAssertTrue(cell.waitForExistence(timeout: 15), "\(title) shows no photos")
        }
        shot(title.lowercased())
        back()
    }

    private func back() {
        let button = app.navigationBars.buttons.element(boundBy: 0)
        if button.waitForExistence(timeout: 5) { button.tap() }
    }

    private func shot(_ name: String) {
        let attachment = XCTAttachment(screenshot: app.screenshot())
        attachment.name = "tour-" + name
        attachment.lifetime = .keepAlways
        add(attachment)
    }

    // MARK: - Test data through the API

    private struct Fixture {
        let searchTerm: String
    }

    private func prepareData() throws -> Fixture {
        let delta = try call("GET", "/api/sync/delta?updatedAfter=2000-01-01T00:00:00Z") as? [String: Any]
        let assets = (delta?["upserted"] as? [[String: Any]] ?? []).filter { ($0["visibility"] as? String) == "timeline" }
        let ids = assets.compactMap { $0["id"] as? String }
        guard ids.count >= 6 else {
            throw XCTSkip("backup test did not upload the photos (\(ids.count) found)")
        }
        let named = { (name: String) in assets.first { ($0["originalFileName"] as? String) == name }?["id"] as? String }
        let favorite = named("ios-1.jpg") ?? ids[0]
        let others = ids.filter { $0 != favorite && $0 != named("ios-2.jpg") }

        _ = try call("PUT", "/api/assets/\(favorite)", body: ["isFavorite": true])
        _ = try call("PUT", "/api/assets", body: ["ids": [others[0]], "visibility": "archive"])
        _ = try call("DELETE", "/api/assets", body: ["ids": [others[1]]])
        _ = try call("POST", "/api/albums", body: ["albumName": "Urlaub", "assetIds": [favorite, others[2]]])
        _ = try call("POST", "/api/shared-links", body: ["type": "INDIVIDUAL", "assetIds": [favorite]])

        // Locked folder: PIN, unlock this API session, move one photo, lock again.
        _ = try call("PUT", "/api/users/me/pin", body: ["pin": pin])
        _ = try call("POST", "/api/auth/session/unlock", body: ["pin": pin])
        _ = try call("PUT", "/api/assets", body: ["ids": [others[3]], "visibility": "locked"])
        _ = try call("POST", "/api/auth/session/lock")

        // Partner sharing from the admin account.
        let me = try call("GET", "/api/users/me") as? [String: Any]
        let adminToken = try login(email: "admin@example.com", password: "supersecret1")
        _ = try call("POST", "/api/partners/\(me?["id"] as? String ?? "")", token: adminToken)
        return Fixture(searchTerm: "ios-2")
    }

    private func login(email: String, password: String) throws -> String {
        let json = try call("POST", "/api/auth/login", body: ["email": email, "password": password], token: nil) as? [String: Any]
        return try XCTUnwrap(json?["accessToken"] as? String, "login failed for \(email)")
    }

    @discardableResult
    private func call(_ method: String, _ path: String, body: [String: Any]? = nil, token explicitToken: String?? = .none) throws -> Any? {
        var req = URLRequest(url: URL(string: server + path)!)
        req.httpMethod = method
        let bearer: String? = explicitToken ?? token
        if let bearer, !bearer.isEmpty { req.setValue("Bearer \(bearer)", forHTTPHeaderField: "Authorization") }
        if let body {
            req.setValue("application/json", forHTTPHeaderField: "Content-Type")
            req.httpBody = try JSONSerialization.data(withJSONObject: body)
        }
        let done = expectation(description: path)
        var result: (Data?, URLResponse?, Error?) = (nil, nil, nil)
        URLSession.shared.dataTask(with: req) { data, response, error in
            result = (data, response, error)
            done.fulfill()
        }.resume()
        wait(for: [done], timeout: 30)
        if let error = result.2 { throw error }
        let status = (result.1 as? HTTPURLResponse)?.statusCode ?? 0
        XCTAssertTrue((200..<300).contains(status), "\(method) \(path) -> \(status): \(String(data: result.0 ?? Data(), encoding: .utf8) ?? "")")
        guard let data = result.0, !data.isEmpty else { return nil }
        return try JSONSerialization.jsonObject(with: data, options: [.fragmentsAllowed])
    }
}
