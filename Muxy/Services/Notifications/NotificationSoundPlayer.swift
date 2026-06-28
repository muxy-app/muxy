import AppKit

@MainActor
final class NotificationSoundPlayer {
    static let shared = NotificationSoundPlayer()

    private var sounds: [NotificationSound: NSSound] = [:]

    func play(_ sound: NotificationSound) {
        guard let player = player(for: sound) else { return }
        player.stop()
        player.play()
    }

    private func player(for sound: NotificationSound) -> NSSound? {
        if let cached = sounds[sound] {
            return cached
        }

        guard let player = NSSound(named: .init(sound.rawValue)) else { return nil }
        sounds[sound] = player
        return player
    }
}
