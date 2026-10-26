import XCTest

/// End-to-end flow against a running Pixelhort server (started by CI):
/// connect → log in → enable backup → back up now → server has the photos →
/// they appear in the app's timeline.
final class BackupFlowUITests: XCTestCase {
    private var env: [String: String] { ProcessInfo.processInfo.environment }
    private var server: String { env["PH_TEST_SERVER"] ?? "http://localhost:2342" }
    private var email: String { env["PH_TEST_EMAIL"] ?? "ios@example.com" }
    private var password: String { env["PH_TEST_PASSWORD"] ?? "iostester1" }
    private var minAssets: Int { Int(env["PH_TEST_MIN_ASSETS"] ?? "") ?? 3 }

    override func setUp() {
        continueAfterFailure = false
    }

    func test1_LoginBackupAndTimeline() throws {
        let app = XCUIApplication()
        app.launchArguments += ["-AppleLanguages", "(en)", "-AppleLocale", "en_US"]
        addUIInterruptionMonitor(withDescription: "System alerts") { alert in
            for label in ["Allow Full Access", "Allow Access to All Photos", "Allow", "OK", "Continue"] {
                let button = alert.buttons[label]
                if button.exists {
                    button.tap()
                    return true
                }
            }
            return false
        }
        app.launch()

        // Server
        let url = app.textFields["server.url"]
        XCTAssertTrue(url.waitForExistence(timeout: 30), "server URL field missing")
        url.tap()
        url.typeText(server)
        app.buttons["server.connect"].tap()

        // Login
        let emailField = app.textFields["login.email"]
        XCTAssertTrue(emailField.waitForExistence(timeout: 30), "login form missing")
        emailField.tap()
        emailField.typeText(email)
        let passwordField = app.secureTextFields["login.password"]
        passwordField.tap()
        passwordField.typeText(password)
        app.buttons["login.submit"].tap()

        let tabBar = app.tabBars.firstMatch
        XCTAssertTrue(tabBar.waitForExistence(timeout: 30), "main tabs missing after login")
        dismissSavePassword(app)
        attach(app, "after-login")

        // Settings → backup options → enable
        tabBar.buttons.element(boundBy: 4).tap()
        let options = app.descendants(matching: .any)["settings.backupOptions"]
        XCTAssertTrue(options.waitForExistence(timeout: 10))
        options.tap()
        let toggle = app.switches["backup.enabledToggle"]
        XCTAssertTrue(toggle.waitForExistence(timeout: 10))
        toggle.coordinate(withNormalizedOffset: CGVector(dx: 0.95, dy: 0.5)).tap()
        allowPhotoAccess(app)
        XCTAssertTrue(waitFor { (toggle.value as? String) == "1" }, "backup could not be enabled")
        attach(app, "backup-enabled")
        app.navigationBars.buttons.element(boundBy: 0).tap()

        // Back up now
        let progress = app.descendants(matching: .any)["settings.backupProgress"]
        XCTAssertTrue(progress.waitForExistence(timeout: 10))
        progress.tap()
        let start = app.buttons["backup.startNow"]
        XCTAssertTrue(start.waitForExistence(timeout: 10))
        XCTAssertTrue(waitFor { start.isEnabled }, "backup button disabled")
        start.tap()

        // The server must receive the photos.
        let token = try serverLogin()
        var total = 0
        XCTAssertTrue(waitFor(timeout: 180) {
            total = (try? self.assetCount(token: token)) ?? 0
            return total >= self.minAssets
        }, "server received only \(total) assets")
        attach(app, "backup-done")

        // And the timeline shows them.
        app.navigationBars.buttons.element(boundBy: 0).tap()
        tabBar.buttons.element(boundBy: 0).tap()
        let firstCell = app.descendants(matching: .any).matching(identifier: "timeline.asset").firstMatch
        if !firstCell.waitForExistence(timeout: 5) {
            // Pull to refresh when the timeline was loaded before the upload.
            let top = app.windows.firstMatch.coordinate(withNormalizedOffset: CGVector(dx: 0.5, dy: 0.25))
            top.press(forDuration: 0.1, thenDragTo: app.windows.firstMatch.coordinate(withNormalizedOffset: CGVector(dx: 0.5, dy: 0.85)))
        }
        XCTAssertTrue(firstCell.waitForExistence(timeout: 30), "timeline shows no assets")
        attach(app, "timeline")
        let cells = app.descendants(matching: .any).matching(identifier: "timeline.asset").count
        XCTAssertGreaterThanOrEqual(cells, min(minAssets, 3))

        // Open the first photo in the viewer.
        firstCell.tap()
        sleep(2)
        attach(app, "viewer")
    }

    // MARK: helpers

    /// Signed builds trigger iOS' "Save Password?" sheet after the login form.
    private func dismissSavePassword(_ app: XCUIApplication) {
        let springboard = XCUIApplication(bundleIdentifier: "com.apple.springboard")
        for source in [app, springboard] {
            for label in ["Not Now", "Never for This Website", "Not now"] {
                let button = source.buttons[label]
                if button.waitForExistence(timeout: 2) && button.isHittable {
                    button.tap()
                    return
                }
            }
        }
        // The sheet is a remote view without accessible buttons: tap "Not Now" by position.
        // Without the sheet this hits the empty "No photos yet" area of the timeline.
        sleep(2)
        app.coordinate(withNormalizedOffset: CGVector(dx: 0.32, dy: 0.626)).tap()
        sleep(1)
    }

    /// iOS 18 shows the photo permission as a full-screen sheet owned by SpringBoard,
    /// which UI interruption monitors do not catch.
    private func allowPhotoAccess(_ app: XCUIApplication) {
        let springboard = XCUIApplication(bundleIdentifier: "com.apple.springboard")
        let labels = ["Allow Full Access", "Allow Access to All Photos", "Allow"]
        let deadline = Date().addingTimeInterval(15)
        while Date() < deadline {
            for source in [springboard, app] {
                for label in labels {
                    let button = source.buttons[label]
                    if button.exists && button.isHittable {
                        button.tap()
                        // iOS may ask for confirmation of full access.
                        let confirm = springboard.buttons["Allow Full Access"]
                        if confirm.waitForExistence(timeout: 2) { confirm.tap() }
                        return
                    }
                }
            }
            RunLoop.current.run(until: Date().addingTimeInterval(0.5))
        }
    }

    private func waitFor(timeout: TimeInterval = 20, _ condition: () -> Bool) -> Bool {
        let deadline = Date().addingTimeInterval(timeout)
        while Date() < deadline {
            if condition() { return true }
            RunLoop.current.run(until: Date().addingTimeInterval(1))
        }
        return condition()
    }

    private func attach(_ app: XCUIApplication, _ name: String) {
        let shot = XCTAttachment(screenshot: app.screenshot())
        shot.name = name
        shot.lifetime = .keepAlways
        add(shot)
    }

    private func request(_ path: String, method: String = "GET", token: String? = nil, body: [String: Any]? = nil) throws -> Any {
        var req = URLRequest(url: URL(string: server + path)!)
        req.httpMethod = method
        if let token { req.setValue("Bearer \(token)", forHTTPHeaderField: "Authorization") }
        if let body {
            req.setValue("application/json", forHTTPHeaderField: "Content-Type")
            req.httpBody = try JSONSerialization.data(withJSONObject: body)
        }
        let done = expectation(description: path)
        var result: Result<Data, Error> = .failure(URLError(.unknown))
        URLSession.shared.dataTask(with: req) { data, _, error in
            result = data.map { .success($0) } ?? .failure(error ?? URLError(.badServerResponse))
            done.fulfill()
        }.resume()
        wait(for: [done], timeout: 30)
        return try JSONSerialization.jsonObject(with: result.get())
    }

    private func serverLogin() throws -> String {
        let json = try request("/api/auth/login", method: "POST", body: ["email": email, "password": password]) as? [String: Any]
        return try XCTUnwrap(json?["accessToken"] as? String)
    }

    private func assetCount(token: String) throws -> Int {
        let json = try request("/api/assets/statistics", token: token) as? [String: Any]
        return json?["total"] as? Int ?? 0
    }
}
