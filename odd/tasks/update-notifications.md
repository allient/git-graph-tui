# Update notifications

## Goal

Tell users when a newer stable release is available without delaying startup or changing their installation. Provide release notes and upgrade guidance for each supported installation method.

## Acceptance criteria

- Check releases in the background at most daily and cache the result, including failed attempts.
- Never block the UI or show errors for ordinary network failures; allow automatic checks to be disabled.
- Compare stable semantic versions, ignoring prereleases; do not offer downgrades.
- Show a dismissible, keyboard and mouse accessible notice and upgrade guidance.
- Preserve Git as the only external runtime dependency.
- Cover version parsing, caching, notification interaction and rendering with tests.
