# Browser Permissions Audit

Last updated: 2026-06-29

Starlee's Chrome, Safari, and Firefox extensions currently use broad `http://*/*` and `https://*/*` content-script matches so a native menu-bar capture can reach the active page after the user clicks Starlee outside the browser. The extension posts captured content only to the local `127.0.0.1` Starlee service and diagnostics must not include tokens, cookies, passwords, raw page bodies, selected text, article text, or transcript text.

## Current Warning

Safari and Chromium-family browsers may describe broad page access with scary wording such as reading sensitive fields on websites. That warning is a browser-level description of host access, not Starlee's diagnostic behavior. Starlee still avoids collecting passwords, credit cards, cookies, auth headers, and tokens.

## Safer Permission Options

- `activeTab`: useful for toolbar-click capture inside the browser, but not sufficient by itself for native menu-bar capture because the native app cannot grant the browser's active-tab permission.
- Optional host permissions: can reduce first-install warning text, but menu-bar capture would need a permission request flow before capture on each ungranted site. That is a larger UX and recovery change.
- User-triggered injection with `scripting.executeScript`: promising for Chrome/Firefox, but it requires replacing the current preloaded content-script readiness/probe path and auditing Safari Web Extension behavior separately.
- Narrow host permissions: not viable for general article/newsletter/member-site capture because Starlee intentionally supports arbitrary user-chosen article and YouTube pages.

## Decision For This PR

This PR does not narrow host permissions. Doing so safely requires a new browser-side permission UX and a user-triggered injection architecture so menu-bar requests remain reliable without falling back to a shared broadcast queue. The safe change in this PR is request routing: menu-bar requests are browser-targeted, and each extension only polls requests for its own browser.

## Follow-Up

Design and implement a permission-reduced capture path:

1. Keep local bridge access to `http://127.0.0.1/*`.
2. Use `activeTab` plus explicit user-triggered injection for toolbar captures.
3. Add optional host permission prompts for native menu-bar capture when the active site is not already granted.
4. Preserve diagnostics redaction and keep permission-denied states actionable rather than red failure states.
