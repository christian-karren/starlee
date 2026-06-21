# Spotify Passive Sync

Status: blocked pending product decision.

The PRD-01a goal is directionally right: Starlee should remember podcast episodes the user actually listened to, especially on phone and CarPlay. The proposed implementation path, however, depends on Spotify returning podcast episodes from the recently played endpoint.

Spotify's current Web API documentation for `GET /me/player/recently-played` says it gets recently played tracks and "currently doesn't support podcast episodes." That means Starlee cannot faithfully implement "episodes listened to since last check" using hourly polling of that endpoint.

What is implemented now:

- Spotify episode vault schema:
  - `type: spotify_episode`
  - stable id `spotify:episode:{episode_id}`
  - deterministic filename `{YYYY-MM-DD}-spotify-{episode_id}.md`
  - podcast-specific frontmatter fields
  - restricted access by default for future Spotify captures
- `starlee sync-status`
- `starlee sync-spotify`
- MCP tool `starlee_spotify_sync_status`
- `doctor` checks that make the Spotify API limitation visible

What remains unresolved:

- How Starlee should learn about completed podcast episodes.

Viable product paths:

1. Current-playback sampling
   - Poll `GET /me/player` or `GET /me/player/currently-playing` with `additional_types=episode`.
   - This can see an episode while it is actively playing and includes progress.
   - It cannot reconstruct history if Starlee was asleep or not polling during playback.
   - It needs a more frequent sampler than hourly if we want reliable completion inference.

2. Mobile companion
   - An iOS app or shortcut/share-sheet flow records local playback intent or completion.
   - Best path for phone-first behavior, but bigger product surface.

3. User-triggered capture
   - User shares an episode from Spotify to Starlee or clicks a Starlee action.
   - Much easier to make robust, but not passive.

4. Spotify data export/import
   - User imports Spotify account data exports.
   - Useful for backfill, not real-time.

Until the product chooses one of those paths, Starlee should not claim true passive Spotify podcast history sync.
