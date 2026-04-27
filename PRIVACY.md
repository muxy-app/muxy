# Privacy Policy

_Effective date: the date this document was first published at its public URL._

Muxy ("the app") is a developer tool that lets your iPhone or iPad connect to a Mac running the Muxy desktop application over your local network or a private VPN. This policy describes what data the app handles and what it does not.

## Summary

- No account, no sign-up, no email required.
- No analytics, advertising, or third-party tracking SDKs.
- The app communicates only with the Mac you choose to pair it with.
- All data stays on your devices.

## What the app stores on your device

The app stores the following locally on your iOS device. None of it is transmitted to Muxy or any third party.

- **Pairing credentials.** A random device ID and token are generated on first launch and stored in the iOS Keychain (device-locked, this device only). They are used to authenticate the app to a Mac you have paired with.
- **Saved devices.** The names, hostnames, and ports of Macs you have added are stored in the app's local preferences (UserDefaults). Credentials are not stored here.
- **Preferences.** Terminal font size and Nerd Font toggle.
- **Diagnostic log (in memory only).** While the app is running, it keeps a short rolling log of connection events (timestamps, the hostname and port you are connecting to, and request identifiers) to help you troubleshoot connection problems. This log is held in memory, is cleared when the app exits, and is never sent anywhere. If a connection error occurs, the app shows the log inside an error sheet so you can copy or share it yourself if you choose to.

You can remove a saved device at any time from the device list. Uninstalling the app removes all locally stored data.

## What the app sends over the network

When you connect to a Mac, the app opens a direct WebSocket connection to the address and port you entered. It sends only the messages required to authenticate, view terminal output, control panes, and perform the version-control actions you initiate (such as staging, committing, pushing, pulling, switching branches, managing worktrees, or opening pull requests).

The app does not contact any Muxy-operated server. It does not contact any third-party server. It does not perform background networking.

## What the app does not collect

- No personal information.
- No contacts, photos, location, microphone, or camera data.
- No usage analytics or crash analytics.
- No advertising identifiers.
- No data sold or shared with third parties.

## Permissions

- **Local Network.** Required by iOS so the app can reach the Mac you pair with on your LAN or VPN.

## Children

The app is a developer tool and is not directed to children under 13.

## Changes to this policy

If this policy changes, the updated version will be posted at this URL with a new "Last updated" date.

## Contact

Questions about this policy: sa.vaziry@gmail.com
