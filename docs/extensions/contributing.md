# Contributing an Extension

Published extensions live in the
[`muxy-app/extensions`](https://github.com/muxy-app/extensions) repository and
appear in **Settings → Extensions → Browse**. You publish by opening a pull
request there; CI builds, checks, and lists your extension. The repository's
`CONTRIBUTING.md` has the current validation steps.

## 1. Check out your fork

Fork `muxy-app/extensions`, then check out only the tooling and your folder:

```bash
git clone --filter=blob:none --sparse https://github.com/<you>/extensions
cd extensions
git sparse-checkout set extensions/my-extension scripts
```

## 2. Build it

Create `extensions/my-extension/`. The folder name must equal the package
`name`. Follow [Get started](get-started.md), and load the folder with **Load
Unpacked…** to test it.

Your `build` script must produce `dist/` with `package.json` inside, because
only `dist/` is published.

## 3. Add a listing

Add a `marketplace` block under `muxy`:

```json
"marketplace": {
  "author": "Your Name",
  "github": "your-handle",
  "categories": ["productivity"],
  "icon": "assets/icon.svg",
  "screenshots": ["assets/screenshot-1.png"]
}
```

Add a `README.md` that says what the extension does and why it needs each
permission.

## 4. Open a pull request

Commit your source, not `dist/`, push to your fork, and open a pull request
against `muxy-app/extensions`. Once merged, the extension is published.

A published `name@version` never changes. Bump `version` to ship an update;
users install updates from **Settings → Extensions**.

## Guidelines

- Ask for the fewest [permissions](permissions.md) you need.
- Keep bundles small and the source readable. Minified or obfuscated code is
  flagged for review.
- Use lowercase names (`a-z`, `0-9`, `.`, `-`) so pages load.
- Test on the latest Muxy 2 release.
