import SwiftUI
import ContextDBiOSFeature

@main
struct ContextDBiOSApp: App {
    var body: some Scene {
        WindowGroup {
            ContentView()
                .environment(\.contextdbClient, .live())
        }
    }
}
