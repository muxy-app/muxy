# Remote Methods

An extension can serve named API methods to the Muxy mobile app. The mobile app calls a stable endpoint on the desktop; Muxy decides whether the request belongs to an extension and, if so, proxies it into the extension's background script, awaits the result, and returns it to the device. The mobile app never needs to know which extensions are installed.

Requires the [`remote:serve`](permissions.md) permission, and every call prompts the user for [consent](permissions.md#runtime-consent) (remembered per action) before the handler runs.

```json
{
  "permissions": ["remote:serve"],
  "remoteMethods": [
    { "id": "forecast", "description": "Return the weather forecast for a city." }
  ]
}
```

## Declaration fields

| Field | Type | Required | Notes |
| --- | --- | --- | --- |
| `id` | string | yes | Unique within the extension. The action name the device addresses. |
| `description` | string | no | Human-readable summary for documentation. |

The manifest is the source of truth for which actions exist: an action that is not declared, or an extension that lacks `remote:serve`, is rejected by Muxy before the background script is ever reached.

## Serving a method

Register a handler in your background script with `muxy.remote.handle`. The handler receives the request payload and returns a JSON-serializable value (or a `Promise` of one).

```js
muxy.remote.handle('forecast', async (payload) => {
  const city = payload.city;
  const res = await muxy.exec(['curl', '-s', `https://api.example.com/${city}`]);
  muxy.notifications.notify({ title: 'Forecast requested', body: city });
  return JSON.parse(res.stdout);
});
```

The background script can surface a notification with `muxy.notifications.notify({ title, body })` (requires the `notifications:write` permission).

- The payload is the JSON the device sent (any JSON value).
- The return value is JSON-encoded and sent back to the device.
- Throwing (or rejecting) returns an error to the device.
- `muxy.remote.unhandle(id)` removes a handler.
- The first call to an action prompts the user; choosing "Allow & remember" whitelists that action. A denied call returns `403` to the device.

Remote handlers run in the same background JavaScript context as your event handlers and are serialized with them.

## How the device reaches it

The mobile app sends one request over the [remote server](../remote-server/methods.md):

```json
{
  "method": "extensionRequest",
  "params": { "extension": "<extension-id>", "action": "forecast", "payload": { "city": "Berlin" } }
}
```

The device must already be paired and authenticated. On success the response carries the handler's value; on failure it carries a `MuxyError`:

| Code | Meaning |
| --- | --- |
| 404 | Extension not loaded, or the action is not declared in `remoteMethods`. |
| 403 | The extension lacks the `remote:serve` permission, or the user denied consent. |
| 503 | The extension is not running / has no live background script. |
| 502 | The handler threw, rejected, or is not registered. |
| 504 | The handler did not reply in time. |
