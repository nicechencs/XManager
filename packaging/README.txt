XManager desktop build

This zip is unsigned. That is expected for a first GitHub Release.

Credentials
-----------
Copy .env.example to .env and fill the four OAuth 1.0a fields from
https://console.x.com → your App → Keys and tokens.

The app looks for .env in this order:
  1. Next to the program (this folder, or next to XManager.app on macOS)
  2. XMANAGER_DATA_DIR, if set
  3. The current working directory (and two parents)
  4. User config dir:
       Windows  %APPDATA%\XManager
       macOS    ~/Library/Application Support/XManager

Logs and CSV/JSON exports go under the same data root (logs/ and exports/).

Windows
-------
1. Copy .env.example to .env and fill it.
2. Double-click XManager.exe.
3. If SmartScreen says "Windows protected your PC":
     More info → Run anyway
4. CLI: xmanager-cli.exe --help

macOS
-----
1. Copy .env.example to .env (same folder as XManager.app, or the user
   config dir above).
2. Open XManager.app. You can drag it to /Applications.
3. If macOS says the developer cannot be verified:
     xattr -cr XManager.app
     open XManager.app
   or Control-click the app → Open.
4. CLI: ./xmanager-cli --help

This build is not notarized. The xattr step is required until an Apple
Developer ID is used.

Upgrading
---------
Download the new release zip from GitHub Releases and replace the
program files only:

  Windows: overwrite XManager.exe (and xmanager-cli.exe)
  macOS:   replace XManager.app in /Applications

Keep your `.env` in place - credentials, logs, and exported backups live
outside the program and survive upgrades untouched.

Newer builds (v0.2.0+) can update themselves: the app checks GitHub
Releases on startup and offers "download and install" in-app. Updating
from an older portable build still requires the manual steps above once.

Deleting tweets
---------------
Library "删除选中" asks once, writes a CSV backup, then calls the X API.
HTTP 429 stops further deletes. Keep the backup.
