# Privacy Policy

_Effective date: the date this document was first published at its public URL._
_Last updated: November 14, 2026._

Muxy is a developer tool made of a desktop app, a server that runs on your own computer (with the `muxy` command), and phone apps for iOS and Android that connect to that computer. This policy describes what data each part handles, what is sent to `muxy.app`, and how to turn it off.

Muxy is open source, so you can check everything described here in the code.

## Summary

- No account, no sign-up, no email required.
- No advertising, no third-party analytics or tracking SDKs, and no data sold or shared.
- The desktop app sends one anonymous ping a day to `muxy.app` with only the app version, operating system, and processor type. It is on by default, and you can turn it off.
- Push notifications to your phone go through a relay at `muxy.app` that cannot read them. You can turn them off on the phone or on the computer.
- Your terminals, prompts, code, and files never leave the computers and phones you connect.

## The desktop app

### Anonymous daily ping

Installed releases of the desktop app send one request a day to `https://muxy.app/api/ping`, so we can count active installs per version. The request contains only:

```json
{"version": "2.6.0", "os": "macos", "arch": "aarch64"}
```

There is no install ID, user ID, or account, and nothing about your projects, terminals, or how you use Muxy. The server, the `muxy` command, and the phone apps never send it, and neither do development builds.

Like any web request, the ping comes from your IP address. Here is what `muxy.app` keeps:

- One row per day per sender, holding the version, operating system, processor type, the country Cloudflare derives from the IP address, and the time of the last ping.
- The IP address is never stored. The row holds a keyed hash of it instead (HMAC-SHA-256 with a secret key), so one computer pinging twice on the same day counts once, and the hash can't be turned back into the address without that key.
- Rows are deleted after 90 days.

`muxy.app` is served through Cloudflare, which handles the request on our behalf like any other web traffic to the site.

**To turn it off**, use any one of these:

- **Settings → General → Send an anonymous daily ping**
- `muxy config set privacy.usage_ping false`
- Start Muxy with the environment variable `DO_NOT_TRACK=1`

Once it's off, nothing more is sent.

### Other connections the desktop app makes

- **Update checks.** The app reads the latest release information from GitHub (`github.com/muxy-app/muxy`) and downloads updates from there. GitHub's own privacy policy applies to those requests.
- **Extensions.** Browsing and installing extensions talks to `muxy.app`. Each download adds to the extension's public download count and records the extension, its version, the country, and a keyed hash of the IP address. Those records are deleted after 90 days.
- **Things you open yourself.** Web pages, searches, Git remotes, SSH hosts, and AI agents you run in Muxy connect wherever you point them. Muxy doesn't add anything to those connections.

## Push notifications

Push notifications let a paired phone know when an agent on your computer needs your attention or finishes working while you're away from the Muxy app.

### How a notification travels

1. **Your computer writes and locks it.** The Muxy server on your computer writes the notification, such as "Claude Code · my-app — Needs your attention", and encrypts it with AES-256-GCM using a key that only your computer and your phone have. Your computer gave the phone that key directly when you turned notifications on. It never passes through `muxy.app`.
2. **The `muxy.app` relay passes it on.** The relay never has the key, so it can't open the message. It sees only that a notification was sent, whether it is an "attention" or a "finished" notification, and scrambled identifiers that group notifications on your phone and mean nothing without the key.
3. **Apple or Google delivers it.** The relay hands the locked message to Apple Push Notification service (iOS) or Firebase Cloud Messaging (Android), which see the same locked message as the relay.
4. **Your phone unlocks and shows it.** The Muxy app opens the message with its key and shows it.

Terminal output, prompts, code, and file contents are never included in a notification.

Muxy waits about 20 seconds before sending. If you look at that terminal on any device in that time, or the Muxy app is open and connected, nothing is sent.

### What the relay stores

When a phone turns notifications on, your computer registers it with the relay. For each registration the relay stores:

- The phone's push token from Apple or Google, encrypted.
- A hash of the secret your computer uses to send to that phone, not the secret itself.
- Which push service and environment it uses (for example, App Store or development build), and when it was last used and last delivered to.

It also keeps daily totals per push service (registered, delivered, failed) with no per-device detail. The relay does not store your computer's IP address or any notification content.

Registrations no one has used for 90 days are deleted, and ones Apple or Google reject are deleted 30 days later.

### Turning notifications off

You can turn them off on either side:

- **On the phone.** Notifications are off until you allow them. Turn them off for a computer in the Muxy app, or for the whole app in your phone's system settings. Turning them off for a computer deletes its registration at the relay.
- **On the computer.** Notifications are allowed by default. Pause them for every phone with **Push notifications** under **Settings → Servers → Clients**, or with `muxy mobile push off`. The relay forgets every phone at once, and nothing more is sent to `muxy.app`. Your computer keeps each phone's push token and key locally for up to 90 days, so turning notifications back on resumes them without pairing again. After that, it forgets them.
- **For one phone.** **Reset Notifications** next to the phone, or `muxy mobile push reset <id>`, gives it a new key without unpairing it. Revoking the phone (**Revoke**, or `muxy mobile revoke <id>`) deletes its registration.

Phones connected over SSH don't get push notifications.

## The phone apps

### What the app stores on your phone

The app stores the following on your phone. None of it is sent to Muxy or any third party.

- **Pairing credentials.** When you pair with a computer, it gives the app a credential: the computer's name, its addresses and port, the fingerprint of its security certificate, an ID for your device, and an access token. The credential is kept in your phone's secure storage (the iOS Keychain, on this device only, or the Android Keystore) and is used to sign in to that computer. The computer keeps only a hash of the token.
- **SSH connections.** If you connect over SSH, the connection details and credentials you choose to save are stored only on your phone.
- **Notification keys.** If you turn on notifications for a computer, that computer gives the app a key for decrypting its notifications. The keys are kept in secure storage, shared with the app's notification extension on iOS.
- **Preferences.** Display settings such as the terminal font size.
- **Diagnostic log (in memory only).** While the app is running, it keeps a short rolling log of connection events (timestamps, the address and port you are connecting to, and request identifiers) to help you troubleshoot connection problems. The log is held in memory, cleared when the app exits, and never sent anywhere. If a connection error occurs, the app shows the log so you can copy or share it yourself if you choose to.

You can remove a saved computer at any time. Uninstalling the app removes its data, though iOS can keep Keychain items after an app is deleted. To be sure a paired phone has no more access, revoke it on the computer, under **Settings → Servers → Clients** or with `muxy mobile revoke <id>`. Revoking does not end SSH access; that is controlled by the computer's SSH settings.

### What the app sends over the network

The app connects directly to the computer you choose, in one of two ways:

- **Paired.** An encrypted TLS connection to the address and port from the pairing code. The app trusts only the certificate named in that code. When pairing, it sends the device name you choose, which the computer shows in its list of paired devices.
- **SSH.** An SSH connection to the computer, using the login details you provide.

The app sends only what is needed to sign in, show terminal output, send your input, and perform the project, file, and version-control actions you start (such as staging, committing, pushing, pulling, switching branches, managing worktrees, or opening pull requests).

The phone app itself never contacts `muxy.app` or any third-party server, apart from Apple's or Google's push service when you turn on notifications. When you do, the app sends its push token to your computer over the paired connection, and your computer registers it with the relay as described above. The app does no background networking.

### Permissions

- **Local Network.** Required on iOS so the app can reach a computer on your local network.
- **Notifications.** Optional. Used only to tell you when an agent on your computer needs your attention or finishes working.
- **Camera.** Used only to scan a pairing code. Camera images are not stored or sent.

## What Muxy does not collect

- No personal information, account, or email address.
- No contacts, photos, location, or microphone data.
- No terminal output, prompts, code, or file contents.
- No usage analytics beyond the anonymous daily ping, and no crash reports.
- No advertising identifiers.
- No data sold or shared with third parties.

## Children

Muxy is a developer tool and is not directed to children under 13.

## Changes to this policy

If this policy changes, the updated version will be posted at this URL with a new "Last updated" date.

## Contact

Questions about this policy: sa.vaziry@gmail.com
