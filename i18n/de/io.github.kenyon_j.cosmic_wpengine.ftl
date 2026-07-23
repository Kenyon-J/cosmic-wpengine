## Language name (this catalog's own autonym, used to build the language picker)

language-name = Deutsch

## Tray menu (src/modules/tray.rs)

tray-title = COSMIC Wallpaper
tray-open-settings = Einstellungen öffnen...
tray-quit-engine = Engine beenden

## First-run desktop integration (src/modules/gui/bootstrap.rs)

general-launcher-issue = Die App ist noch nicht im Anwendungsstarter deines Desktops registriert. Das wird normalerweise automatisch beim ersten Start der Einstellungen eingerichtet - falls sie weiterhin fehlt, prüfe, ob ~/.local/share/applications beschreibbar ist, und starte die Einstellungen neu.

## Status line (src/modules/gui/mod.rs) - the caption at the bottom of every page

status-ready = Bereit.
status-blocked-unsafe-theme-name = Unsicherer Themenname blockiert: { $name }
status-saved-theme-live = { $name } gespeichert - der Desktop zeigt die Änderung bereits.
status-saved-theme-inactive = { $name } gespeichert. Anwenden, um es auf dem Desktop zu sehen.
status-error-saving-theme = Fehler beim Speichern des Themes: { $error }
status-error-serialising-theme = Fehler beim Serialisieren des Themes: { $error }
status-nothing-usable-dropped = Nichts Verwendbares abgelegt - nur MP4- oder WebM-Dateien.
status-importing-files = { $count ->
    [one] { $count } Datei wird importiert...
   *[other] { $count } Dateien werden importiert...
}
status-detecting-location = Standort wird ermittelt...
status-location-detected = Standort ermittelt.
status-could-not-detect-location = Standort konnte nicht ermittelt werden: { $error }
status-no-videos-imported = Keine Videos importiert - nur MP4-, WebM-, MKV-, MOV- und AVI-Dateien werden unterstützt.
status-imported-videos = { $n ->
    [one] { $n } Video importiert.
   *[other] { $n } Videos importiert.
}
status-imported-skipped = { $n } importiert, { $s } übersprungen (keine Videodateien).
status-invalid-theme = Kein gültiges Theme: { $error }
status-imported-themes = { $imported ->
    [one] { $imported } Theme importiert.
   *[other] { $imported } Themes importiert.
}
status-nothing-imported-themes = Nichts importiert - .toml-Theme-Dateien ablegen.
status-engine-starting = Engine wird gestartet...
status-failed-to-start-engine = Engine konnte nicht gestartet werden: { $error }
status-could-not-find-engine-binary = Die Datei cosmic-wallpaper wurde nicht neben den Einstellungen gefunden.
status-killed-by-signal = durch Signal beendet
status-exit-code = Exit-Code { $code }
status-engine-exited-immediately = Die Engine wurde sofort beendet ({ $code }).
status-engine-exited-immediately-detail = Die Engine wurde sofort beendet ({ $code }): { $headline }
status-engine-running = Engine läuft.
status-engine-did-not-start = Die Engine ist nicht gestartet - Protokolle prüfen.
status-engine-stopping = Engine wird beendet...
status-engine-still-running = Die Engine läuft noch.
status-engine-stopped = Engine beendet.
status-error-applying-theme = Fehler beim Anwenden des Themes: { $error }
status-applied-theme = Theme angewendet: „{ $theme }“
status-select-theme-to-apply = Wähle ein Theme zum Anwenden aus.
status-created-theme = { $file_name } erstellt
status-theme-already-exists = Theme „{ $name }“ existiert bereits!
status-error-creating-theme = Fehler beim Erstellen des Themes: { $error }
status-fetching-patch-notes = Versionshinweise werden abgerufen...
status-xdg-open-not-found = Link konnte nicht geöffnet werden: xdg-open nicht gefunden
status-xdg-open-folder-not-found = Ordner konnte nicht geöffnet werden: xdg-open nicht gefunden
status-diagnostics-copied = Diagnose in die Zwischenablage kopiert.
status-downloading = { $tag } wird heruntergeladen...
status-update-failed = Aktualisierung fehlgeschlagen: { $error }
status-updated-restart-needed = Auf { $tag } aktualisiert! Die Wallpaper-Engine wurde automatisch neu gestartet; starte auch die Einstellungen neu, um ebenfalls die neue Version zu verwenden.
status-select-theme-to-export = Wähle ein Theme zum Exportieren aus.
status-exported-pack = Pack nach { $path } exportiert
status-error-exporting-pack = Fehler beim Exportieren des Packs: { $error }
status-pack-too-large = „{ $name }“ ist zu groß, um ein gültiges Pack zu sein (Limit { $limit } MB) - übersprungen.
status-could-not-read-dropped-file = Eine abgelegte Datei konnte nicht gelesen werden.
status-skip-one-shader-at-a-time = „{ $name }“ übersprungen - importiere jeweils nur ein Pack mit Shader.
status-pack-renamed-on-import = „{ $original }“ existierte bereits - importiert als „{ $written }“.
status-pack-import-error = „{ $name }“: { $error }
status-not-a-valid-pack = Kein gültiges Pack: { $error }
status-imported-packs = { $imported ->
    [one] { $imported } Pack importiert.
   *[other] { $imported } Packs importiert.
}
status-pack-includes-shader-review = Ein Pack enthält einen benutzerdefinierten Shader - oben überprüfen.
status-nothing-imported-packs = Nichts importiert - .cwtheme-Pack-Dateien ablegen.
status-pack-detail-video-shader = mit Video und einem benutzerdefinierten Shader
status-pack-detail-shader = mit einem benutzerdefinierten Shader
status-imported-pack-named = Pack „{ $name }“ importiert ({ $detail }).
status-pack-renamed-with-detail = „{ $name }“ existierte bereits - importiert als „{ $written }“ ({ $detail }).
status-error-importing-pack = Fehler beim Importieren des Packs: { $error }
status-cancelled-nothing-imported = Abgebrochen - nichts wurde importiert.
status-applied-pack-with-video = Pack „{ $name }“ samt Hintergrundvideo angewendet.
status-applied-pack-video-missing = Pack „{ $name }“ angewendet - das Hintergrundvideo fehlt; importiere das Pack erneut, um es wiederherzustellen.
status-applied-pack-theme-missing = Pack „{ $name }“ angewendet - die Theme-Datei fehlt, daher wurden generische Standardwerte verwendet.
status-applied-pack = Pack „{ $name }“ angewendet.
status-error-applying-pack = Fehler beim Anwenden des Packs: { $error }

## Shared across multiple pages (src/modules/gui/view.rs)

common-active = Aktiv
common-active-now = Jetzt aktiv.
common-align-center = Mitte
common-align-left = Links
common-align-right = Rechts
common-apply = Anwenden
common-copied-to-clipboard = In die Zwischenablage kopiert
common-copy = Kopieren
common-copy-to-clipboard = In die Zwischenablage kopieren
common-create = Erstellen
common-hide = Ausblenden
common-open-folder = Ordner öffnen
common-recent-colours = Zuletzt verwendete Farben
common-reset = Zurücksetzen
common-retry = Erneut versuchen
common-shape-circular = Kreisförmig
common-shape-linear = Linear
common-shape-square = Quadratisch
common-show = Anzeigen
common-start = Starten
common-stop = Stoppen

## Wallpaper page

wallpaper-mode-frosted-glass = Mattglas
wallpaper-mode-transparent = Transparent
wallpaper-mode-album-art = Album-Cover
wallpaper-mode-album-colour = Albumfarbe
wallpaper-mode-live-wallpaper = Live-Hintergrundbild
text-color-mode-automatic = Automatisch
text-color-mode-custom = Benutzerdefiniert
wallpaper-preview-none = Keine
wallpaper-preview-sample-title = Und weiter, und weiter, und weiter, und weiter
wallpaper-preview-sample-caption = Ich spüre den Rausch, ich spüre den Lärm
wallpaper-style-title = Stil
wallpaper-frosted-glass-title = Mattglas
wallpaper-blur-amount = Unschärfe
wallpaper-blur-amount-desc = Wie stark das Hintergrundbild geweichzeichnet wird.
wallpaper-live-wallpaper-title = Live-Hintergrundbild
wallpaper-video-item = Video
wallpaper-video-item-desc = Videos auf der Seite „Live-Hintergrundbilder“ auswählen und verwalten
wallpaper-text-title = Text
wallpaper-text-colour = Textfarbe
wallpaper-text-colour-desc = Automatisch wählt eine Farbe, die auf deinem Hintergrundbild lesbar bleibt.
wallpaper-page-title = Hintergrundbild
wallpaper-page-summary = Die Optionen des gewählten Stils erscheinen darunter.

## Live Wallpapers page

live-wallpapers-drop-release = Loslassen, um es zur Bibliothek hinzuzufügen
live-wallpapers-drop-prompt = Videodateien hier ablegen, um sie hinzuzufügen (MP4, WebM)
live-wallpapers-library-title = Bibliothek
live-wallpapers-no-videos = Noch keine Videos
live-wallpapers-no-videos-desc = Dateien oben ablegen oder über „Ordner öffnen“ manuell hinzufügen
live-wallpapers-playback-title = Wiedergabe
live-wallpapers-prefer-canvas = Spotify Canvas bevorzugen
live-wallpapers-prefer-canvas-desc = Wenn der aktuelle Titel eine Canvas-Schleife hat, diese anstelle des Hintergrundbilds anzeigen.
live-wallpapers-library-folder = Bibliotheksordner
live-wallpapers-library-folder-desc = Videos befinden sich in ~/.config/cosmic-wallpaper/videos.
live-wallpapers-page-title = Live-Hintergrundbilder
live-wallpapers-page-summary = Videos in Dauerschleife, die als Hintergrund abgespielt werden. Auf eine Kachel klicken, um sie zu aktivieren.

## Themes page

theme-element-album-art = Album-Cover
theme-element-track-info = Titelinformationen
theme-element-lyrics = Songtext
theme-element-visualiser = Visualizer
theme-element-weather = Wetter
theme-element-effects = Effekte
theme-align = Ausrichtung
theme-position-x = Position X
theme-position-y = Position Y
theme-size = Größe
theme-text-size = Textgröße
theme-shape = Form
theme-docked = Angedockt
theme-docked-desc = Während Musik läuft, sitzt das Cover im kreisförmigen Visualizer und folgt dessen Position und Größe. Deaktiviere das Andocken im Tab „Visualizer“, um diese Regler zu verwenden.
theme-band-order = Reihenfolge der Bänder
theme-rotation = Drehung
theme-amplitude = Amplitude
theme-bar-width = Balkenbreite
theme-cap-roundness = Rundung der Enden
theme-glow = Leuchten
theme-reflection = Spiegelung
theme-led-segments = LED-Segmente
theme-peak-hold = Peak-Hold-Spitzen
theme-peak-hold-desc = peak_hold - eine kleine helle Spitze, die den jüngsten Peak jedes Balkens hält und dann der Schwerkraft folgend absinkt
theme-dock-art = Album-Cover im Ring andocken
theme-dock-art-desc = dock_art - das Cover folgt der Position und Größe des Rings
theme-lyric-bounce = Songtext-Bounce
theme-spring-stiffness = Federsteifigkeit
theme-spring-damping = Federdämpfung
theme-beat-pulse = Beat-Puls
theme-reset-section = Diesen Abschnitt zurücksetzen
theme-reset-section-desc = Setzt { $element } auf die Standardwerte zurück.
theme-page-theme-title = Theme
theme-editing = Bearbeitung
theme-editing-live-desc = Dieses Theme ist aktiv - Änderungen erscheinen sofort auf deinem Desktop.
theme-editing-inactive-desc = Nicht das aktive Theme - Änderungen werden in seiner Datei gespeichert; zum Ansehen anwenden.
theme-manage-title = Verwalten
theme-create-new = Neues Theme erstellen
theme-create-new-desc = Beginnt mit einer vollständig kommentierten Vorlage.
theme-name-placeholder = Themenname
theme-name-empty-error = Der Themenname darf nicht leer sein
theme-name-exists-error = Ein Theme mit diesem Namen existiert bereits
theme-import = Importieren
theme-import-desc = .toml-Theme-Dateien irgendwo auf dieser Seite ablegen, um sie hinzuzufügen.
theme-page-title = Layout-Designs
theme-page-summary = Wo sich alles auf dem Bildschirm befindet. Etwas verschieben und zusehen, wie der Desktop folgt.

## Packs page

packs-your-packs-title = Deine Packs
packs-none-yet = Noch keine Packs importiert
packs-none-yet-desc = Eine .cwtheme-Datei hier unten ablegen, um sie hier zu sehen.
packs-includes-video-active = Enthält ein Hintergrundvideo - jetzt aktiv.
packs-includes-video = Enthält ein Hintergrundvideo.
packs-layout-only = Nur Layout.
packs-export-title = Exportieren
packs-theme-to-bundle = Zu bündelndes Theme
packs-export-desc-with-video = Bündelt das Layout dieses Themes, seinen benutzerdefinierten Shader (falls gesetzt) und dein aktuell aktives Hintergrundvideo ({ $file }) - Videos sind an kein bestimmtes Theme gebunden, prüfe also, ob es wirklich das ist, das du teilen möchtest.
packs-export-desc-no-video = Bündelt das Layout dieses Themes und seinen benutzerdefinierten Shader, falls vorhanden. Derzeit ist kein Hintergrundvideo aktiv, das Pack wird also keines enthalten.
packs-export-pack = Pack exportieren
packs-folder = Pack-Ordner
packs-folder-desc = Exportierte .cwtheme-Dateien landen in ~/.config/cosmic-wallpaper/packs.
packs-import = Importieren
packs-import-desc = Eine .cwtheme-Datei irgendwo auf dieser Seite ablegen, um sie hinzuzufügen.
packs-page-title = Packs
packs-page-summary = Teile einen kompletten Look - Layout, Hintergrundvideo und einen benutzerdefinierten Visualizer-Shader - als eine einzige Datei.

## Now Playing page

now-playing-album-art-title = Album-Cover
now-playing-show-album-art = Album-Cover anzeigen
now-playing-show-album-art-desc = Das aktuelle Cover, platziert nach dem aktiven Layout-Design.
now-playing-lyrics-text-title = Songtext & Text
now-playing-show-lyrics = Songtext anzeigen
now-playing-show-lyrics-desc = Synchronisierter Songtext zum aktuellen Titel, sofern verfügbar.
now-playing-font-family = Schriftart
now-playing-page-title = Wird wiedergegeben
now-playing-page-summary = Was während der Musikwiedergabe erscheint: Album-Cover, Titelinformationen und Songtext.

## Visualiser page

visualiser-audio-response-title = Audioreaktion
visualiser-bands = Bänder
visualiser-bands-desc = Wie viele Balken der Visualizer zeichnet.
visualiser-smoothing = Glättung
visualiser-smoothing-desc = Höher ist ruhiger; niedriger ist reaktionsfreudiger.
visualiser-page-title = Visualizer
visualiser-page-summary = Balken, die sich zur aktuellen Wiedergabe bewegen.

## Weather page

weather-unit-celsius = Celsius
weather-unit-fahrenheit = Fahrenheit
weather-poll-5min = 5 Minuten
weather-poll-15min = 15 Minuten
weather-poll-30min = 30 Minuten
weather-poll-1hour = 1 Stunde
weather-page-title = Wetter
weather-show-weather = Wetter anzeigen
weather-show-weather-desc = Aktuelle Bedingungen auf dem Desktop.
weather-hide-effects = Animierte Effekte ausblenden
weather-hide-effects-desc = Schaltet Regen- und Schnee-Animationen aus, um Energie zu sparen.
weather-units = Einheiten
weather-location = Standort
weather-location-desc = Breiten- und Längengrad für die Vorhersage. „Meinen Standort verwenden“ schätzt diese anhand deiner IP-Adresse über ipapi.co.
weather-latitude-placeholder = Breitengrad
weather-longitude-placeholder = Längengrad
weather-use-my-location = Meinen Standort verwenden
weather-update-every = Aktualisieren alle
weather-page-summary = Bedingungen und Effekte, die über das Hintergrundbild gelegt werden.

## General page

general-checking-for-updates = Nach Updates wird gesucht...
general-up-to-date = Aktuell
general-check-for-updates = Nach Updates suchen
general-check-failed = Prüfung fehlgeschlagen: { $reason }
general-update-to = Auf { $tag } aktualisieren
general-release-page = Versionsseite für { $tag }
general-updating-to = Wird auf { $tag } aktualisiert...
general-installed-restart = { $tag } installiert - neu starten
general-engine-title = Engine
general-wallpaper-engine = Wallpaper-Engine
general-engine-running = Läuft (PID { $pid }).
general-engine-not-running = Läuft nicht.
general-start-on-login = Bei Anmeldung starten
general-start-on-login-desc = Startet die Wallpaper-Engine bei der Anmeldung.
general-frame-rate-limit = Bildratenlimit
general-frame-rate-limit-desc = Niedriger spart Energie; die Engine läuft im Leerlauf, wenn nichts animiert wird.
general-config-folder = Konfigurationsordner
general-config-folder-desc = Die gesamte Engine-Konfiguration liegt hier.
general-about-title = Über
general-version = Version
general-patch-notes = Versionshinweise
general-patch-notes-desc = Was sich in der neuesten Version geändert hat.
general-diagnostics = Diagnose
general-diagnostics-desc = Version, Protokollauszug und GPU-Infos, bereit zum Einfügen in einen Bericht.
general-something-broken = Etwas funktioniert nicht?
general-something-broken-desc = Öffnet einen vorausgefüllten Fehlerbericht mit den letzten Fehlern im Anhang.
general-report-an-issue = Problem melden
general-setup-title = Einrichtung
general-not-in-launcher = Nicht im Anwendungsstarter
general-patch-notes-section-title = Versionshinweise
general-page-title = Allgemein
general-page-summary = Verhalten der Engine und Wartung.

general-language-title = Sprache
general-language-desc = Überschreibt die Sprache des Desktops nur für diese App - nützlich für eine Sprache, die dein Desktop noch nicht anbietet.
general-language-system-default = Systemstandard
