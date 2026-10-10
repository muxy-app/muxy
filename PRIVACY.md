# Privacy Policy

_Effective date: the date this document was first published at its public URL._
_Last updated: October 10, 2026._

Muxy ("the app") is a developer tool that lets your iPhone or iPad connect to Muxy running on your own computer, such as a Mac or a Linux server. It connects over your local network, a private VPN, an address you add to the pairing code, or SSH. This policy describes what data the app handles and what it does not.

## Summary

- No account, no sign-up, no email required.
- No analytics, advertising, or third-party tracking SDKs.
- The app communicates only with the computers you pair with or connect to over SSH.
- All data stays on your devices.

## What the app stores on your device

The app stores the following locally on your iOS device. None of it is transmitted to Muxy or any third party.

- **Pairing credentials.** When you pair with a computer, it gives the app a credential: the computer's name, its addresses and port, the fingerprint of its security certificate, an ID for your device, and an access token. The credential is stored in the iOS Keychain (this device only) and is used to sign in to that computer. The computer keeps only a hash of the token.
- **SSH connections.** If you connect over SSH, the connection details and credentials you choose to save are stored only on your device.
- **Preferences.** Display settings such as the terminal font size.
- **Diagnostic log (in memory only).** While the app is running, it keeps a short rolling log of connection events (timestamps, the address and port you are connecting to, and request identifiers) to help you troubleshoot connection problems. This log is held in memory, is cleared when the app exits, and is never sent anywhere. If a connection error occurs, the app shows the log so you can copy or share it yourself if you choose to.

You can remove a saved computer at any time. Uninstalling the app removes its data, but iOS can keep Keychain items after an app is deleted. To end a phone's access for certain, revoke it on the computer, in **Settings → Mobile** or with `muxy mobile revoke`.

## What the app sends over the network

The app connects directly to the computer you choose, in one of two ways:

- **Paired.** An encrypted TLS connection to the address and port from the pairing code. The app trusts only the certificate named in that code. When pairing, it sends the device name you choose, which the computer shows in its list of paired devices.
- **SSH.** An SSH connection to the computer, using the login details you provide.

The app sends only what is needed to sign in, show terminal output, send your input, and perform the project, file, and version-control actions you start (such as staging, committing, pushing, pulling, switching branches, managing worktrees, or opening pull requests).

The app does not contact any Muxy-operated server. It does not contact any third-party server. It does not perform background networking.

## What the app does not collect

- No personal information.
- No contacts, photos, location, or microphone data.
- No usage analytics or crash analytics.
- No advertising identifiers.
- No data sold or shared with third parties.

## Permissions

- **Local Network.** Required by iOS so the app can reach a computer on your LAN.
- **Camera.** Used only to scan a pairing code. Camera images are not stored or sent.

## Children

The app is a developer tool and is not directed to children under 13.

## Changes to this policy

If this policy changes, the updated version will be posted at this URL with a new "Last updated" date.

## Contact

Questions about this policy: sa.vaziry@gmail.com
