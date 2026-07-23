## Language name (this catalog's own autonym, used to build the language picker)

language-name = English

## Tray menu (src/modules/tray.rs)

tray-title = COSMIC Wallpaper
tray-open-settings = Open Settings...
tray-quit-engine = Quit Engine

## First-run desktop integration (src/modules/gui/bootstrap.rs)

general-launcher-issue = The app isn't registered with your desktop's launcher yet. This is normally set up automatically the first time Settings runs - if it's still missing, check that ~/.local/share/applications is writable, then restart Settings.

## Status line (src/modules/gui/mod.rs) - the caption at the bottom of every page

status-ready = Ready.
status-blocked-unsafe-theme-name = Blocked unsafe theme name: { $name }
status-saved-theme-live = Saved { $name } - the desktop is showing your change.
status-saved-theme-inactive = Saved { $name }. Apply it to see it on the desktop.
status-error-saving-theme = Error saving theme: { $error }
status-error-serialising-theme = Error serialising theme: { $error }
status-nothing-usable-dropped = Nothing usable was dropped - MP4 or WebM files.
status-importing-files = { $count ->
    [one] Importing { $count } file...
   *[other] Importing { $count } files...
}
status-detecting-location = Detecting location...
status-location-detected = Location detected.
status-could-not-detect-location = Could not detect location: { $error }
status-no-videos-imported = No videos imported - only MP4, WebM, MKV, MOV and AVI files.
status-imported-videos = { $n ->
    [one] Imported { $n } video.
   *[other] Imported { $n } videos.
}
status-imported-skipped = Imported { $n }, skipped { $s } (not video files).
status-invalid-theme = Not a valid theme: { $error }
status-imported-themes = { $imported ->
    [one] Imported { $imported } theme.
   *[other] Imported { $imported } themes.
}
status-nothing-imported-themes = Nothing imported - drop .toml theme files.
status-engine-starting = Engine starting...
status-failed-to-start-engine = Failed to start the engine: { $error }
status-could-not-find-engine-binary = Could not find the cosmic-wallpaper binary next to Settings.
status-killed-by-signal = killed by signal
status-exit-code = exit { $code }
status-engine-exited-immediately = The engine exited immediately ({ $code }).
status-engine-exited-immediately-detail = The engine exited immediately ({ $code }): { $headline }
status-engine-running = Engine running.
status-engine-did-not-start = The engine did not start - check the logs.
status-engine-stopping = Engine stopping...
status-engine-still-running = The engine is still running.
status-engine-stopped = Engine stopped.
status-error-applying-theme = Error applying theme: { $error }
status-applied-theme = Applied theme: '{ $theme }'
status-select-theme-to-apply = Select a theme to apply.
status-created-theme = Created { $file_name }
status-theme-already-exists = Theme '{ $name }' already exists!
status-error-creating-theme = Error creating theme: { $error }
status-fetching-patch-notes = Fetching patch notes...
status-xdg-open-not-found = Failed to open link: xdg-open not found
status-xdg-open-folder-not-found = Failed to open folder: xdg-open not found
status-diagnostics-copied = Diagnostics copied to clipboard.
status-downloading = Downloading { $tag }...
status-update-failed = Update failed: { $error }
status-updated-restart-needed = Updated to { $tag }! The wallpaper engine restarted automatically; restart Settings to use the new version too.
status-select-theme-to-export = Select a theme to export.
status-exported-pack = Exported pack to { $path }
status-error-exporting-pack = Error exporting pack: { $error }
status-pack-too-large = '{ $name }' is too large to be a valid pack (limit { $limit } MB) - skipped.
status-could-not-read-dropped-file = Could not read a dropped file.
status-skip-one-shader-at-a-time = Skipped '{ $name }' - import one shader pack at a time.
status-pack-renamed-on-import = '{ $original }' already existed - imported as '{ $written }'.
status-pack-import-error = '{ $name }': { $error }
status-not-a-valid-pack = Not a valid pack: { $error }
status-imported-packs = { $imported ->
    [one] Imported { $imported } pack.
   *[other] Imported { $imported } packs.
}
status-pack-includes-shader-review = A pack includes a custom shader - review it above.
status-nothing-imported-packs = Nothing imported - drop .cwtheme pack files.
status-pack-detail-video-shader = with video and a custom shader
status-pack-detail-shader = with a custom shader
status-imported-pack-named = Imported pack '{ $name }' ({ $detail }).
status-pack-renamed-with-detail = '{ $name }' already existed - imported as '{ $written }' ({ $detail }).
status-error-importing-pack = Error importing pack: { $error }
status-cancelled-nothing-imported = Cancelled - nothing was imported.
status-applied-pack-with-video = Applied pack '{ $name }' and its background video.
status-applied-pack-video-missing = Applied pack '{ $name }' - its background video is missing; re-import the pack to restore it.
status-applied-pack-theme-missing = Applied pack '{ $name }' - its theme file is missing, so generic defaults were used instead.
status-applied-pack = Applied pack '{ $name }'.
status-error-applying-pack = Error applying pack: { $error }

## Shared across multiple pages (src/modules/gui/view.rs)

common-active = Active
common-active-now = Active now.
common-align-center = Center
common-align-left = Left
common-align-right = Right
common-apply = Apply
common-copied-to-clipboard = Copied to clipboard
common-copy = Copy
common-copy-to-clipboard = Copy to clipboard
common-create = Create
common-hide = Hide
common-open-folder = Open Folder
common-recent-colours = Recent colours
common-reset = Reset
common-retry = Retry
common-shape-circular = Circular
common-shape-linear = Linear
common-shape-square = Square
common-show = Show
common-start = Start
common-stop = Stop

## Wallpaper page

wallpaper-mode-frosted-glass = Frosted Glass
wallpaper-mode-transparent = Transparent
wallpaper-mode-album-art = Album Art
wallpaper-mode-album-colour = Album Colour
wallpaper-mode-live-wallpaper = Live Wallpaper
text-color-mode-automatic = Automatic
text-color-mode-custom = Custom
wallpaper-preview-none = None
wallpaper-preview-sample-title = On, and on, and on, and on
wallpaper-preview-sample-caption = I can feel the rush, I can feel the noise
wallpaper-style-title = Style
wallpaper-frosted-glass-title = Frosted Glass
wallpaper-blur-amount = Blur amount
wallpaper-blur-amount-desc = How strongly the wallpaper is blurred.
wallpaper-live-wallpaper-title = Live Wallpaper
wallpaper-video-item = Video
wallpaper-video-item-desc = Pick and manage videos on the Live Wallpapers page
wallpaper-text-title = Text
wallpaper-text-colour = Text colour
wallpaper-text-colour-desc = Automatic picks a colour that stays readable on your wallpaper.
wallpaper-page-title = Wallpaper
wallpaper-page-summary = Options for the selected style appear below it.

## Live Wallpapers page

live-wallpapers-drop-release = Release to add to your library
live-wallpapers-drop-prompt = Drop video files here to add them (MP4, WebM)
live-wallpapers-library-title = Library
live-wallpapers-no-videos = No videos yet
live-wallpapers-no-videos-desc = Drop files above, or use Open Folder to add them by hand
live-wallpapers-playback-title = Playback
live-wallpapers-prefer-canvas = Prefer Spotify Canvas
live-wallpapers-prefer-canvas-desc = When the playing track has a Canvas loop, show it instead of your wallpaper.
live-wallpapers-library-folder = Library folder
live-wallpapers-library-folder-desc = Videos live in ~/.config/cosmic-wallpaper/videos.
live-wallpapers-page-title = Live Wallpapers
live-wallpapers-page-summary = Looping videos that play as your background. Click a tile to set it.

## Themes page

theme-element-album-art = Album Art
theme-element-track-info = Track Info
theme-element-lyrics = Lyrics
theme-element-visualiser = Visualiser
theme-element-weather = Weather
theme-element-effects = Effects
theme-align = Align
theme-position-x = Position X
theme-position-y = Position Y
theme-size = Size
theme-text-size = Text size
theme-shape = Shape
theme-docked = Docked
theme-docked-desc = While music plays, the art sits inside the circular visualiser and follows its position and size. Turn off docking on the Visualiser tab to use these controls.
theme-band-order = Band order
theme-rotation = Rotation
theme-amplitude = Amplitude
theme-bar-width = Bar width
theme-cap-roundness = Cap roundness
theme-glow = Glow
theme-reflection = Reflection
theme-led-segments = LED segments
theme-peak-hold = Peak-hold caps
theme-peak-hold-desc = peak_hold - a small bright cap that holds each bar's recent peak and falls under gravity
theme-dock-art = Dock album art in the ring
theme-dock-art-desc = dock_art - the art follows the ring's position and size
theme-lyric-bounce = Lyric bounce
theme-spring-stiffness = Spring stiffness
theme-spring-damping = Spring damping
theme-beat-pulse = Beat pulse
theme-reset-section = Reset this section
theme-reset-section-desc = Restores { $element } to its default values.
theme-page-theme-title = Theme
theme-editing = Editing
theme-editing-live-desc = This theme is live - changes appear on your desktop as you make them.
theme-editing-inactive-desc = Not the active theme - changes save to its file; Apply to see them.
theme-manage-title = Manage
theme-create-new = Create new theme
theme-create-new-desc = Starts from a fully-commented template layout.
theme-name-placeholder = Theme name
theme-name-empty-error = Theme name cannot be empty
theme-name-exists-error = A theme with this name already exists
theme-import = Import
theme-import-desc = Drop .toml theme files anywhere on this page to add them.
theme-page-title = Layout Themes
theme-page-summary = Where everything sits on screen. Slide something and watch your desktop follow.

## Packs page

packs-your-packs-title = Your Packs
packs-none-yet = No packs imported yet
packs-none-yet-desc = Drop a .cwtheme file below to see it here.
packs-includes-video-active = Includes a background video - active now.
packs-includes-video = Includes a background video.
packs-layout-only = Layout only.
packs-export-title = Export
packs-theme-to-bundle = Theme to bundle
packs-export-desc-with-video = Bundles this theme's layout, its custom shader if set, and your currently active background video ({ $file }) - videos aren't tied to a specific theme, so double check this is the one you want to share.
packs-export-desc-no-video = Bundles this theme's layout and its custom shader, if set. No background video is currently active, so the pack won't include one.
packs-export-pack = Export Pack
packs-folder = Packs folder
packs-folder-desc = Exported .cwtheme files land in ~/.config/cosmic-wallpaper/packs.
packs-import = Import
packs-import-desc = Drop a .cwtheme file anywhere on this page to add it.
packs-page-title = Packs
packs-page-summary = Share a full look - layout, background video, and a custom visualiser shader - as one file.

## Now Playing page

now-playing-album-art-title = Album Art
now-playing-show-album-art = Show album art
now-playing-show-album-art-desc = The current cover, placed by the active layout theme.
now-playing-lyrics-text-title = Lyrics & Text
now-playing-show-lyrics = Show lyrics
now-playing-show-lyrics-desc = Synced lyrics for the current track, when available.
now-playing-font-family = Font family
now-playing-page-title = Now Playing
now-playing-page-summary = What appears when music plays: album art, track info, and lyrics.

## Visualiser page

visualiser-audio-response-title = Audio Response
visualiser-bands = Bands
visualiser-bands-desc = How many bars the visualiser draws.
visualiser-smoothing = Smoothing
visualiser-smoothing-desc = Higher is calmer; lower is snappier.
visualiser-page-title = Visualiser
visualiser-page-summary = Bars that move with whatever is playing.

## Weather page

weather-unit-celsius = Celsius
weather-unit-fahrenheit = Fahrenheit
weather-poll-5min = 5 minutes
weather-poll-15min = 15 minutes
weather-poll-30min = 30 minutes
weather-poll-1hour = 1 hour
weather-page-title = Weather
weather-show-weather = Show weather
weather-show-weather-desc = Current conditions on the desktop.
weather-hide-effects = Hide animated effects
weather-hide-effects-desc = Turns off rain and snow animations to save power.
weather-units = Units
weather-location = Location
weather-location-desc = Latitude and longitude for the forecast. "Use my location" estimates these from your IP address via ipapi.co.
weather-latitude-placeholder = Latitude
weather-longitude-placeholder = Longitude
weather-use-my-location = Use my location
weather-update-every = Update every
weather-page-summary = Conditions and effects layered over the wallpaper.

## General page

general-checking-for-updates = Checking for updates...
general-up-to-date = Up to date
general-check-for-updates = Check for Updates
general-check-failed = Couldn't check: { $reason }
general-update-to = Update to { $tag }
general-release-page = { $tag } release page
general-updating-to = Updating to { $tag }...
general-installed-restart = { $tag } installed - restart
general-engine-title = Engine
general-wallpaper-engine = Wallpaper engine
general-engine-running = Running (pid { $pid }).
general-engine-not-running = Not running.
general-start-on-login = Start on login
general-start-on-login-desc = Launches the wallpaper engine when you log in.
general-frame-rate-limit = Frame rate limit
general-frame-rate-limit-desc = Lower saves power; the engine idles when nothing animates.
general-config-folder = Config folder
general-config-folder-desc = All engine configuration lives here.
general-about-title = About
general-version = Version
general-patch-notes = Patch notes
general-patch-notes-desc = What changed in the latest release.
general-diagnostics = Diagnostics
general-diagnostics-desc = Version, log tail and GPU info, for pasting into a report.
general-something-broken = Something broken?
general-something-broken-desc = Opens a pre-filled bug report with recent errors attached.
general-report-an-issue = Report an Issue
general-setup-title = Setup
general-not-in-launcher = Not in your app launcher
general-patch-notes-section-title = Patch Notes
general-page-title = General
general-page-summary = Engine behaviour and housekeeping.

general-language-title = Language
general-language-desc = Overrides the desktop's own language for just this app - useful for a language your desktop doesn't offer yet.
general-language-system-default = System default
