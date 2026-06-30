# Starlee Capture Chrome Extension Privacy Policy

Last updated: 2026-06-30

Starlee Capture is a Chrome-only browser extension for saving the article or
YouTube transcript the user chooses into the local Starlee app running on that
user's Mac.

## Data The Extension Handles

When the user saves a page, Starlee Capture may process:

- article text from the current rendered page;
- selected text from the current page;
- page metadata such as title, URL, canonical URL, author, publisher, and access
  classification;
- YouTube video metadata visible on the page;
- YouTube transcript segments visible or available to the rendered page;
- local connection settings needed to reach the user's Starlee service.

Starlee Capture does not collect or transmit full browsing history. It acts only
when the user chooses to save the current page through the Chrome toolbar,
in-page Starlee button, or Starlee macOS menu-bar app.

## How Data Is Used

Captured article text, selected text, metadata, and YouTube transcript data are
sent only to the Starlee service running on the user's own computer at
`http://127.0.0.1`. The local Starlee app stores captures in `~/Starlee` and
builds a local search index so the user can search and retrieve their own saved
knowledge.

The extension does not use captured content for advertising, analytics, user
profiling, credit decisions, or resale.

## Sharing And Transmission

Starlee Capture does not sell, share, upload, or transmit article bodies,
transcripts, browsing history, vault data, or capture tokens to Starlee servers.
The extension communicates with the loopback address `127.0.0.1`, which is the
user's own computer.

Starlee Capture does not send captured article bodies, selected text, YouTube
transcripts, vault data, or capture tokens to third parties.

## Local Storage And Security

The extension stores only the local connection information it needs to talk to
the user's Starlee service, including the local port, local bearer token, and
redacted connection state. The bearer token is generated locally by Starlee and
is used only to authenticate requests to the loopback service on the same Mac.

The Chrome Web Store package does not include `starlee-config.json`, capture
tokens, local vault data, local config files, source maps, model files,
`node_modules`, or unexpected remote URLs.

## Retention And Deletion

Captured content is stored locally in `~/Starlee` by the Starlee app. The user
controls that local folder. To delete saved captures, delete the relevant files
from `~/Starlee` or remove the Starlee local data folder. To remove extension
connection state, uninstall the Chrome extension or clear the extension's local
Chrome storage.

## Human Access

Starlee Capture does not provide Starlee developers with access to captured
article bodies, selected text, YouTube transcripts, browsing history, vault data,
or capture tokens. This data stays on the user's Mac unless the user separately
chooses to share files from their local Starlee folder.

## Limited Use Statement

The use of information received from Google APIs will adhere to the Chrome Web
Store User Data Policy, including the Limited Use requirements.

## Contact

For privacy or support questions, use the support contact listed on the Starlee
Chrome Web Store listing.
