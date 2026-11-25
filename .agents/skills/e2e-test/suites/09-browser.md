# S09 Browser

The built-in browser shows a page served in the VM, the CLI automates it,
and the address bar and Back work. On Linux the page is drawn by WebKitGTK.

Start: run `$E g $VM reset`, then `$E g $VM http-start` (prints
`serving http://127.0.0.1:8765/`). Tests continue from each other in order.

## T09.1 Open a page from the CLI

1. Run `$E g $VM muxy browser open 'http://127.0.0.1:8765/'`. It prints a tab id; note it.
2. Run `$E g $VM muxy browser wait <id> --text 'Hello from Muxy E2E' --timeout-ms 30000`.
   Check: prints `true`.
3. Wait: `sleep 2`. **Shot** `t09-1-page`. Check: a tab titled "E2E Home" with a
   globe icon; an address bar with `http://127.0.0.1:8765/`; a light yellow page
   with the heading "Hello from Muxy E2E", a text field, a "Save" button and the
   link "Go to the second page".

## T09.2 Automate the page

1. Run `$E g $VM muxy browser read <id>`. Check: the output includes `E2E Home`
   and `Hello from Muxy E2E`.
2. Run `$E g $VM muxy browser fill <id> '#name' Robo`.
3. Run `$E g $VM muxy browser click <id> '#go'`.
4. Run `$E g $VM muxy browser wait <id> --text 'Saved Robo' --timeout-ms 10000`. Check: `true`.
5. **Shot** `t09-2-saved`. Check: the field shows `Robo` and the text
   "Saved Robo" appears under it.

## T09.3 Address bar and Back

1. Press address bar `$E press $VM address-bar`.
2. `$E type $VM '127.0.0.1:8765/next.html'`, `$E key $VM Return`. Wait: `sleep 3`.
3. **Shot** `t09-3-next`. Check: a light blue page with "Second page reached";
   the tab title is "E2E Second".
4. Run `$E g $VM muxy browser list`. Check: the line for `<id>` contains `next.html`.
5. Press back `$E press $VM browser-back`. Wait: `sleep 3`.
6. **Shot** `t09-3-back`. Check: the yellow "Hello from Muxy E2E" page is back.

## T09.4 Close the tab from the CLI

1. Run `$E g $VM muxy browser close <id>`.
2. Wait: `sleep 2`. **Shot** `t09-4-closed`. Check: the browser tab is gone.
3. Run `$E g $VM muxy browser list`. Check: no line for `<id>`.
4. Run `$E g $VM http-stop`.
