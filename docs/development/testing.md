# Testing policy

## Automated verification

Automated checks must be headless. They may compile bundles, verify bundle inventories and signatures, run Rust unit and integration tests, exercise protocol helpers, and use isolated filesystem or socket fixtures.

Automated tests and verifier scripts must never launch `Muxy.app`, `MuxyTests.app`, or an app executable from a bundle on the user's computer. They must not use `open`, `osascript`, or direct bundle executable invocation to drive an app lifecycle.

The app-launching E2E entry points are disabled. Do not add replacements or new staged-app launch modes.

## Manual native verification

Visual presentation, focus behavior, accessibility, native window lifecycle, real OS integration, and other behavior that requires a running app belong to manual user verification.

When a change needs native or visual acceptance:

1. Build and verify the bundle without launching it.
2. Tell the user exactly what needs verification.
3. Ask the user to launch the app and report the result.
4. Record the user's observed result separately from automated evidence.
5. Leave anything the user did not check explicitly unverified.

Agents and automated tools must not launch the app on the user's behalf.

## Allowed alternatives

Prefer deterministic Rust tests around portable state machines, coordinators, protocol framing, persistence, process identity, and view models. Use source and bundle inventory checks for packaging contracts. Keep helper-process tests isolated and bounded as long as they do not launch an app bundle.
