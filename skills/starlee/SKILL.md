---
name: starlee
description: >
  Query the user's personal Starlee knowledge corpus — their captured articles,
  YouTube transcripts, and notes. Use when the user asks what they know about a
  topic, wants to recall something they read, asks about their reading history,
  wants to find connections between ideas, asks "what have I learned about X",
  wants to save content into Starlee, or needs Starlee setup/capture troubleshooting.
---

# Starlee

Starlee is the user's local-first digital brain. It stores captures as Markdown
in `~/Starlee/vault`, keeps a disposable SQLite hybrid/vector search index in
`~/Starlee/index.db`, and exposes MCP tools for Codex.

Use Starlee for:

- asking questions about the user's saved articles, YouTube transcripts, notes,
  and reading history;
- finding connections between a new URL/title/idea and the existing library;
- saving or capturing articles, web pages, YouTube transcripts, pasted notes, or
  ideas;
- listing recent captures or retrieving a complete record;
- exporting or ingesting `.starlee` share bundles;
- troubleshooting browser capture.

Do not use Starlee for coding, file management, or topics unrelated to the
user's personal knowledge base.

## Operating rules

- Prefer Starlee MCP tools when they are available.
- Use the default home `~/Starlee` unless the user explicitly asks for another
  vault.
- Do not reveal or paste the capture token.
- Do not commit or share vault data, config files, model caches, local
  databases, logs, or build artifacts.
- For paid, metered, or ambiguous web pages, use the browser sensor/extension
  capture path. Do not fetch and store gated content by URL-only capture.
- Starlee never performs synthesis with an external inference API. Codex does
  the reasoning; Starlee provides retrieval and citations.

## Session start

When a new Starlee-focused session starts, call `starlee_corpus_overview` first
if the tool is available. Use the result to:

- tell the user how many captures are in their brain and the date range;
- suggest 3 query topics based on `top_topics`;
- note if the corpus is sparse, especially with fewer than 10 captures, and
  suggest capturing more.

Skip the overview only when the user is clearly asking for setup/install,
troubleshooting, code work, or a non-Starlee task.

## Answering questions about the corpus

When the user asks what they know about a topic, asks to recall something, asks
about their reading history, or asks "what have I learned about X":

1. Call `starlee_query` with `question` set to the user's query.
2. If `relevance_floor_hit` is true and `chunks` is empty, tell the user their
   corpus does not have enough on this topic yet. Suggest related topics from
   the last corpus overview when available. Do not synthesize from training
   data.
3. If chunks are returned, synthesize a prose answer from only those chunks.
   Cite factual claims with inline markers like `[1]`.
4. At the end, include:

   ```text
   Sources:
   [1] {title} — {url or domain} — captured {captured_at}
   [2] ...
   ```

5. Never make a claim that is not supported by a returned chunk. If uncertain,
   say so and reference the specific chunk.

Tone: respond as if you are the user's own memory — confident, direct, and
cited. Prefer "you've been reading about..." over "the search returned...".

## Gap detection

If `starlee_query` returns sparse results or `relevance_floor_hit: true`, be
explicit:

- "I don't see enough in your Starlee corpus to answer that yet."
- "The closest saved item is..."
- "Topics that are better represented in your library are..."

Never fill gaps with general model knowledge unless the user explicitly asks
for a broader answer outside Starlee.

## "What connects to this?" pattern

When the user pastes a URL, title, or short passage and asks what it connects to
in their library:

1. Call `starlee_query` with `question` describing the connection task and
   `context` set to the pasted URL/title/text.
2. Synthesize the top 3-5 related captures in prose.
3. Explain the connection; do not merely list results.
4. Cite sources with the same inline marker and Sources section format.

## Recent reading

When the user asks what they have read lately or recently:

1. Prefer `recent` for a compact list of current captures, or call
   `starlee_query` with `question` set to "recent captures last 14 days" when a
   thematic synthesis is requested.
2. Group the response by theme rather than mechanically by date.
3. Cite specific captures.

## Setup and install

When the user asks "set yourself up", "install Starlee", or similar:

- Treat this as a setup task, not a query task.
- Direct fresh installs to run `./scripts/install.sh` from the Starlee repo.
- If the CLI is already installed, `starlee setup` initializes the local vault.
- After setup, run `starlee doctor` and make sure all gates are green.
- Do not call `starlee_query` or `starlee_corpus_overview` until setup is
  complete and `starlee doctor` is healthy.

## Connect integrations

When the user asks "set yourself up", "connect my Spotify", "connect YouTube",
"connect my integrations", or similar, extend the setup workflow with these
steps after the install/setup check and before the final doctor confirmation:

1. Run `starlee doctor` first.
2. Read the doctor output to determine which integrations are already
   configured.
3. Skip any integration that is already green. Handle Spotify and YouTube
   independently:
   - If Spotify is configured, do not run Spotify setup again.
   - If YouTube is configured, do not run YouTube setup again.
   - If Spotify is configured but YouTube is missing, only configure YouTube.
   - If YouTube is configured but Spotify is missing, only configure Spotify.
4. Before opening any browser-based authorization page, explain the next step in
   plain English. Do not say "OAuth" unless the user asks for technical detail.
5. Run the appropriate Starlee CLI command yourself in the terminal. Do not ask
   the user to type terminal commands.
6. Guide the user through the browser approval screen in plain English.
7. If an integration command fails, stop that integration's setup, explain what
   went wrong in non-technical language, and continue checking any other
   requested integration that can still be configured safely.
8. End by running `starlee doctor` again.
9. Report which integrations are green and which are still missing or blocked.

Spotify configuration:

1. Detect Spotify status from `starlee doctor`. Treat a `spotify_oauth`
   check with `"ok": true` as green.
2. If Spotify is missing, say:

   ```text
   I'm going to connect your Spotify account now. A browser window will open
   and ask you to approve Starlee's access to your listening history. It only
   requests read access — it cannot change anything in your Spotify account.
   ```

3. Run `starlee configure-spotify`.
4. If the command asks for a Spotify client id or reports that a Spotify app is
   required, explain:

   ```text
   Starlee needs a Spotify app client id before it can open the approval page.
   This is a Spotify requirement for connecting your account. I can't finish
   Spotify until that client id is available.
   ```

5. If a browser approval page opens, tell the user to approve the request and
   return to Codex when the browser says the connection is complete.
6. After the command finishes, run `starlee sync-spotify`.
7. Verify with `starlee list` and `starlee doctor`.

YouTube configuration:

1. Detect YouTube status from `starlee doctor`. Treat `youtube_oauth:
   configured` as green. If the installed Starlee version reports YouTube under
   another green doctor/status field such as configured YouTube metadata, treat
   that as already configured and do not rerun setup.
2. If YouTube is missing, say:

   ```text
   I'm going to connect your YouTube account now. A browser window will open
   and ask you to approve Starlee's read access. Starlee uses this to save and
   understand videos or transcripts you choose to add; it cannot post videos,
   change your account, or edit anything on YouTube.
   ```

3. Run `starlee configure-youtube`.
4. If the installed Starlee version expects an API key instead of browser
   approval, explain:

   ```text
   This Starlee version supports YouTube through a local API key instead of a
   browser approval screen. I can't complete YouTube automatically until that
   key is available, but I can still finish the rest of setup.
   ```

5. If a browser approval page opens, tell the user to approve the request and
   return to Codex when the browser says the connection is complete.
6. After the command finishes, verify with `starlee doctor`.

OAuth failure handling:

- If the browser is closed, say the connection was cancelled and the user can
  retry later.
- If Starlee reports an expired, denied, or invalid authorization, say the
  approval did not complete and rerun the relevant `starlee configure-*`
  command once if retrying is safe.
- If a provider account, client id, redirect URI, or quota approval is missing,
  explain the missing requirement plainly and mark only that integration as
  blocked.
- Never paste access tokens, refresh tokens, client secrets, capture tokens, or
  local config contents into the chat.

Final integration report:

```text
Starlee setup check:
- Spotify: connected | missing | blocked — {plain-English reason}
- YouTube: connected | missing | blocked — {plain-English reason}
- Doctor: healthy | needs attention — {one-sentence summary}
```

## Capture workflows

To save pasted text, use the `capture` MCP tool with `source_type: "note"` or
`source_type: "article"`.

To save the current webpage, prefer the user's installed browser sensor:

- The macOS Starlee menu-bar icon should create a local capture request.
- The Safari or Chromium extension should extract the visible page and send it
  only to `http://127.0.0.1:47291`.
- If capture fails, run `doctor` and check local service, extension assets,
  Safari extension installation, and extension handshake.

If the page button says the local engine is unreachable, ask the user to run:

```sh
~/.local/bin/starlee serve
```

If the page button asks for setup, ask the user to run:

```sh
~/.local/bin/starlee setup
```
