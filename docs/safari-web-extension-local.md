# Local Safari Web Extension setup

Starlee's Safari path reuses the same browser sensor source as Chrome. The
extension extracts rendered article text, metadata, selected text, and YouTube
transcripts from pages you choose to save, then sends the payload only to the
local Starlee service at `http://127.0.0.1:47291`.

## What this does and does not require

For your own Mac, you can run a Safari Web Extension locally without App Store
review. Safari Web Extensions are packaged inside a small macOS app, so local
development uses Xcode and Apple's `safari-web-extension-converter`.

This repo can build the reusable WebExtension package with Node alone. To
generate and run the Safari macOS wrapper, install full Xcode and select it with
`xcode-select`; Apple's converter is not included with Command Line Tools alone.

Apple's Safari extension docs describe this architecture: web extensions from
other browsers can be converted into an Xcode project containing a macOS app and
Safari extension, and distribution for other users goes through Apple's
extension distribution paths.

## Build the local Safari package

```sh
./scripts/package-safari-extension.sh
```

The script:

1. builds the shared Starlee sensor;
2. stages a clean Safari Web Extension source folder;
3. strips source maps and generated local config;
4. writes `release/safari-extension/starlee-safari-web-extension-0.1.0.zip`;
5. if Apple's converter exists, generates an Xcode project at
   `release/safari-extension/StarleeSafari`.

Inspect the package:

```sh
./scripts/inspect-safari-extension-package.sh \
  release/safari-extension/starlee-safari-web-extension-0.1.0.zip
```

The inspection gate fails if it finds local configuration, vault data, model
files, source maps, obvious bearer tokens, or unexpected remote fetch
destinations.

## If the converter is missing

Install full Xcode from Apple, open it once, then select it:

```sh
sudo xcode-select -s /Applications/Xcode.app/Contents/Developer
```

Then rerun:

```sh
./scripts/package-safari-extension.sh
```

## Run locally in Safari

After the Xcode project is generated:

1. Open `release/safari-extension/StarleeSafari` in Xcode.
2. Select the macOS app target.
3. Build and run it.
4. Open Safari.
5. Enable the extension in Safari Settings > Extensions.
6. Make sure Starlee's local service is running:

   ```sh
   starlee serve
   ```

7. Open an article in Safari and use the Starlee extension button or the in-page
   Starlee button.

Verify from Starlee:

```sh
starlee doctor
starlee recent
starlee search "words from the article you saved"
```

## Product tradeoff

Safari is a good fit for Starlee's Mac-first identity, but the extension is
still a browser permission surface. The advantage is that it shares most of the
same JavaScript sensor code as Chrome, so Starlee does not need separate article
extraction engines per browser.
