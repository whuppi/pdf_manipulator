# Prerelease Changelog

<!--
  PRERELEASE VERSIONS ONLY. Stable releases go in CHANGELOG.md.

  How to add a prerelease:
  1. Add a new ## heading at the top with the prerelease version (e.g. 1.1.0-dev.0)
  2. Write a summary of what changed SINCE THE PREVIOUS ENTRY IN THIS FILE
     - First prerelease after a stable: changes since the last stable version
     - Subsequent prereleases: changes since the previous prerelease
  3. Run: dart run tool/commits.dart v<PREVIOUS_VERSION>
     (e.g. dart run tool/commits.dart v1.1.0-dev.0)
     Copy the <details> block it prints and paste it at the end of your entry
  4. Commit and push to dev
  5. CI reads the version from the top ## heading, tags, and publishes as prerelease

  Format:
    ## X.Y.Z-dev.N
    
    Human-written summary of what's new in this prerelease.
    
    ### Features / Bug Fixes / Breaking Changes / Performance
    - Description of each change since last entry in this file
    
    <details><summary>Commits since PREV (N)</summary>
    
    - abc1234 feat: thing
    
    </details>

  Rules:
  - Version in ## heading is the source of truth for the prerelease version
  - Each entry covers changes since the PREVIOUS entry in THIS file (not CHANGELOG.md)
  - pub.dev shows this file as CHANGELOG.md for the prerelease version
    (CI copies this file to CHANGELOG.md in the published tarball)
  - When the stable release ships, write the full summary in CHANGELOG.md
    covering everything since the last stable — this file is not consulted
  - Entries here are permanent history — don't delete old entries
-->
