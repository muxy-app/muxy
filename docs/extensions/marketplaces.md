# Marketplaces

**Settings → Extensions → Browse** lists extensions from marketplaces. A
marketplace is a public GitHub repository that publishes extensions as GitHub
Releases and lists them in an index file. The official one,
[`muxy-app/extensions`](https://github.com/muxy-app/extensions), is always
there. You can add others next to it, such as your team's own.

## Add or remove a marketplace

At the bottom of **Browse**, under **Marketplaces**:

1. Enter the repository as `owner/repo`, or paste its
   `https://github.com/owner/repo` address.
2. Choose **Add Marketplace** and confirm.

**Browse** then lists the extensions of every marketplace together. A listing
from an added marketplace shows **from owner/repo**. **Remove** takes a
marketplace out of **Browse**.

- Muxy reviews only the official marketplace. An extension from another one can
  ask for any permission it lists, so add only repositories you trust.
- An extension updates only from the marketplace it was installed from. One
  marketplace can't replace an extension installed from another, even with the
  same name: uninstall it first.
- Removing a marketplace keeps the extensions installed from it. They get no
  updates until you add it again.
- If a marketplace can't be loaded, **Browse** says so and shows the others.

Muxy keeps the list in `extension-sources.json` in the
[profile folder](../features/server.md#files), along with which marketplace
each installed extension came from.

## Host a marketplace

The easiest start is to fork `muxy-app/extensions` and keep its `scripts/` and
`.github/workflows/publish.yml`. On every merge to `main` they build each
changed extension, publish a release, and update the index. The same workflow
runs daily to refresh download counts. Edit `official.json` to `[]`: Muxy shows
**Official** only for the official marketplace anyway.

To build your own pipeline instead, publish this layout.

### Releases

Each published version is a GitHub Release tagged `<name>@<version>`, such as
`notes@1.2.0`. `<name>` is the extension's package name and `<version>` its
semver version. A published version should never change: ship a new version
instead. The release carries these assets:

| Asset | Contents |
| --- | --- |
| `<name>-<version>.zip` | The extension's build output, with `package.json` at the root or in one top-level folder |
| `icon.svg` or `icon.png` | The listing icon |
| `screenshot-1.png` … | Screenshots, up to six |

### Index

The index is `index.json` at the root of a branch named `index`. Muxy reads it
from
`https://raw.githubusercontent.com/<owner>/<repo>/index/index.json`, caches it
for five minutes, and accepts up to 16 MB.

```json
{
  "schema": 1,
  "extensions": [
    {
      "name": "notes",
      "description": "Project notes in a side panel",
      "version": "1.2.0",
      "permissions": ["panels:write", "storage:read", "storage:write"],
      "author": {"name": "Ada", "github": "ada"},
      "homepage": null,
      "repository": "https://github.com/ada/notes",
      "categories": ["productivity"],
      "downloads": 120,
      "published_at": "2026-11-26T17:30:50Z",
      "sha256": "<64 hex characters>",
      "size": 48211,
      "download_url": "https://github.com/<owner>/<repo>/releases/download/notes%401.2.0/notes-1.2.0.zip",
      "icon_url": "https://github.com/<owner>/<repo>/releases/download/notes%401.2.0/icon.svg",
      "screenshot_urls": ["https://github.com/<owner>/<repo>/releases/download/notes%401.2.0/screenshot-1.png"]
    }
  ]
}
```

- `schema` must be `1`. Muxy refuses any other value and asks the user to
  update Muxy.
- `name` is required. Muxy skips an entry without it, and keeps the first entry
  when two share a name.
- `version` is the current version. It must be newer than the installed one for
  Muxy to offer an update.
- `permissions` must be exactly the permissions in the package's manifest. An
  entry without them can't be installed.
- `sha256` and `size` describe the zip. Muxy refuses a download that doesn't
  match them, or a package whose name, version, or permissions differ from the
  entry.
- `description`, `author` (a string or an object with `name`), `homepage`,
  `repository`, `categories`, `downloads`, and `published_at` are optional.
  `downloads` sorts **Popular** and `published_at` sorts **Recent**.
- `official` is ignored for every marketplace but the official one.

### Where files may come from

Every URL in an entry must be a release download of that same repository:
`https://github.com/<owner>/<repo>/releases/download/<tag>/<file>`. Muxy drops
icons and screenshots from anywhere else, and refuses to download a package
from anywhere else. Downloads follow GitHub's redirects only to its release
asset hosts.
