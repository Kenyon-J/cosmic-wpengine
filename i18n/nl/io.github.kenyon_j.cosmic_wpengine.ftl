## Language name (this catalog's own autonym, used to build the language picker)

language-name = Nederlands

## Tray menu (src/modules/tray.rs)

tray-title = COSMIC Wallpaper
tray-open-settings = Instellingen openen...
tray-quit-engine = Engine afsluiten

## First-run desktop integration (src/modules/gui/bootstrap.rs)

general-launcher-issue = De app is nog niet geregistreerd in de app-starter van je bureaublad. Dit wordt normaal automatisch ingesteld bij de eerste keer dat Instellingen wordt gestart - als het nog steeds ontbreekt, controleer dan of ~/.local/share/applications beschrijfbaar is en start Instellingen opnieuw op.

## Status line (src/modules/gui/mod.rs) - the caption at the bottom of every page

status-ready = Klaar.
status-blocked-unsafe-theme-name = Onveilige themanaam geblokkeerd: { $name }
status-saved-theme-live = { $name } opgeslagen - het bureaublad toont de wijziging al.
status-saved-theme-inactive = { $name } opgeslagen. Pas het toe om het op het bureaublad te zien.
status-error-saving-theme = Fout bij het opslaan van het thema: { $error }
status-error-serialising-theme = Fout bij het serialiseren van het thema: { $error }
status-nothing-usable-dropped = Er is niets bruikbaars losgelaten - alleen MP4- of WebM-bestanden.
status-importing-files = { $count ->
    [one] { $count } bestand wordt geïmporteerd...
   *[other] { $count } bestanden worden geïmporteerd...
}
status-detecting-location = Locatie wordt bepaald...
status-location-detected = Locatie bepaald.
status-could-not-detect-location = Kon locatie niet bepalen: { $error }
status-no-videos-imported = Geen video's geïmporteerd - alleen MP4-, WebM-, MKV-, MOV- en AVI-bestanden worden ondersteund.
status-imported-videos = { $n ->
    [one] { $n } video geïmporteerd.
   *[other] { $n } video's geïmporteerd.
}
status-imported-skipped = { $n } geïmporteerd, { $s } overgeslagen (geen videobestanden).
status-invalid-theme = Ongeldig thema: { $error }
status-imported-themes = { $imported ->
    [one] { $imported } thema geïmporteerd.
   *[other] { $imported } thema's geïmporteerd.
}
status-nothing-imported-themes = Niets geïmporteerd - zet .toml-themabestanden neer.
status-engine-starting = Engine wordt gestart...
status-failed-to-start-engine = Kon de engine niet starten: { $error }
status-could-not-find-engine-binary = Kon het cosmic-wallpaper-programma niet vinden naast Instellingen.
status-killed-by-signal = beëindigd door een signaal
status-exit-code = afsluitcode { $code }
status-engine-exited-immediately = De engine is direct afgesloten ({ $code }).
status-engine-exited-immediately-detail = De engine is direct afgesloten ({ $code }): { $headline }
status-engine-running = Engine draait.
status-engine-did-not-start = De engine is niet gestart - controleer de logs.
status-engine-stopping = Engine wordt gestopt...
status-engine-still-running = De engine draait nog steeds.
status-engine-stopped = Engine gestopt.
status-error-applying-theme = Fout bij het toepassen van het thema: { $error }
status-applied-theme = Thema toegepast: '{ $theme }'
status-select-theme-to-apply = Kies een thema om toe te passen.
status-created-theme = { $file_name } aangemaakt
status-theme-already-exists = Thema '{ $name }' bestaat al!
status-error-creating-theme = Fout bij het aanmaken van het thema: { $error }
status-fetching-patch-notes = Versienotities worden opgehaald...
status-xdg-open-not-found = Kon link niet openen: xdg-open niet gevonden
status-xdg-open-folder-not-found = Kon map niet openen: xdg-open niet gevonden
status-diagnostics-copied = Diagnose gekopieerd naar het klembord.
status-downloading = { $tag } wordt gedownload...
status-update-failed = Bijwerken mislukt: { $error }
status-updated-restart-needed = Bijgewerkt naar { $tag }! De wallpaper-engine is automatisch opnieuw gestart; herstart ook Instellingen om de nieuwe versie te gebruiken.
status-select-theme-to-export = Kies een thema om te exporteren.
status-exported-pack = Pack geëxporteerd naar { $path }
status-error-exporting-pack = Fout bij het exporteren van de pack: { $error }
status-pack-too-large = '{ $name }' is te groot om een geldige pack te zijn (limiet { $limit } MB) - overgeslagen.
status-could-not-read-dropped-file = Kon een neergezet bestand niet lezen.
status-skip-one-shader-at-a-time = '{ $name }' overgeslagen - importeer telkens maar één pack met shader.
status-pack-renamed-on-import = '{ $original }' bestond al - geïmporteerd als '{ $written }'.
status-pack-import-error = '{ $name }': { $error }
status-not-a-valid-pack = Geen geldige pack: { $error }
status-imported-packs = { $imported ->
    [one] { $imported } pack geïmporteerd.
   *[other] { $imported } packs geïmporteerd.
}
status-pack-includes-shader-review = Een pack bevat een aangepaste shader - controleer deze hierboven.
status-nothing-imported-packs = Niets geïmporteerd - zet .cwtheme-packbestanden neer.
status-pack-detail-video-shader = met video en een aangepaste shader
status-pack-detail-shader = met een aangepaste shader
status-imported-pack-named = Pack '{ $name }' geïmporteerd ({ $detail }).
status-pack-renamed-with-detail = '{ $name }' bestond al - geïmporteerd als '{ $written }' ({ $detail }).
status-error-importing-pack = Fout bij het importeren van de pack: { $error }
status-cancelled-nothing-imported = Geannuleerd - er is niets geïmporteerd.
status-applied-pack-with-video = Pack '{ $name }' toegepast, samen met de achtergrondvideo.
status-applied-pack-video-missing = Pack '{ $name }' toegepast - de achtergrondvideo ontbreekt; importeer de pack opnieuw om die te herstellen.
status-applied-pack-theme-missing = Pack '{ $name }' toegepast - het themabestand ontbreekt, dus zijn generieke standaardwaarden gebruikt.
status-applied-pack = Pack '{ $name }' toegepast.
status-error-applying-pack = Fout bij het toepassen van de pack: { $error }

## Shared across multiple pages (src/modules/gui/view.rs)

common-active = Actief
common-active-now = Nu actief.
common-align-center = Midden
common-align-left = Links
common-align-right = Rechts
common-apply = Toepassen
common-copied-to-clipboard = Gekopieerd naar klembord
common-copy = Kopiëren
common-copy-to-clipboard = Kopiëren naar klembord
common-create = Aanmaken
common-hide = Verbergen
common-open-folder = Map openen
common-recent-colours = Recente kleuren
common-reset = Standaardwaarden herstellen
common-retry = Opnieuw proberen
common-shape-circular = Cirkelvormig
common-shape-linear = Lineair
common-shape-square = Vierkant
common-show = Tonen
common-start = Starten
common-stop = Stoppen

## Wallpaper page

wallpaper-mode-frosted-glass = Matglas
wallpaper-mode-transparent = Transparant
wallpaper-mode-album-art = Albumhoes
wallpaper-mode-album-colour = Albumkleur
wallpaper-mode-live-wallpaper = Live-achtergrond
text-color-mode-automatic = Automatisch
text-color-mode-custom = Aangepast
wallpaper-preview-none = Geen
wallpaper-preview-sample-title = En maar door, en door, en door, en door
wallpaper-preview-sample-caption = Ik voel de roes, ik voel het lawaai
wallpaper-style-title = Stijl
wallpaper-frosted-glass-title = Matglas
wallpaper-blur-amount = Vervagingssterkte
wallpaper-blur-amount-desc = Hoe sterk de achtergrond wordt vervaagd.
wallpaper-live-wallpaper-title = Live-achtergrond
wallpaper-video-item = Video
wallpaper-video-item-desc = Kies en beheer video's op de pagina Live-achtergronden
wallpaper-text-title = Tekst
wallpaper-text-colour = Tekstkleur
wallpaper-text-colour-desc = Automatisch kiest een kleur die leesbaar blijft op je achtergrond.
wallpaper-page-title = Achtergrond
wallpaper-page-summary = De opties voor de gekozen stijl verschijnen hieronder.

## Live Wallpapers page

live-wallpapers-drop-release = Loslaten om toe te voegen aan je bibliotheek
live-wallpapers-drop-prompt = Zet hier videobestanden neer om ze toe te voegen (MP4, WebM)
live-wallpapers-library-title = Bibliotheek
live-wallpapers-no-videos = Nog geen video's
live-wallpapers-no-videos-desc = Zet hierboven bestanden neer, of gebruik Map openen om ze handmatig toe te voegen
live-wallpapers-playback-title = Afspelen
live-wallpapers-prefer-canvas = Spotify Canvas verkiezen
live-wallpapers-prefer-canvas-desc = Als het spelende nummer een Canvas-loop heeft, toon deze dan in plaats van je achtergrond.
live-wallpapers-library-folder = Bibliotheekmap
live-wallpapers-library-folder-desc = Video's staan in ~/.config/cosmic-wallpaper/videos.
live-wallpapers-page-title = Live-achtergronden
live-wallpapers-page-summary = Video's in lus die als achtergrond worden afgespeeld. Klik op een tegel om deze in te stellen.

## Themes page

theme-element-album-art = Albumhoes
theme-element-track-info = Nummerinformatie
theme-element-lyrics = Songtekst
theme-element-visualiser = Visualizer
theme-element-weather = Weer
theme-element-effects = Effecten
theme-align = Uitlijning
theme-position-x = Positie X
theme-position-y = Positie Y
theme-size = Grootte
theme-text-size = Tekstgrootte
theme-shape = Vorm
theme-docked = Vastgemaakt
theme-docked-desc = Terwijl er muziek speelt, zit de hoes in de cirkelvormige visualizer en volgt deze positie en grootte. Schakel het vastmaken uit op het tabblad Visualizer om deze regelaars te gebruiken.
theme-band-order = Volgorde van de banden
theme-rotation = Rotatie
theme-amplitude = Amplitude
theme-bar-width = Balkbreedte
theme-cap-roundness = Afronding van de uiteinden
theme-glow = Gloed
theme-reflection = Reflectie
theme-led-segments = LED-segmenten
theme-peak-hold = Piekvasthouders
theme-peak-hold-desc = peak_hold - een kleine felle piek die de recente piek van elke balk vasthoudt en dan onder invloed van zwaartekracht daalt
theme-dock-art = Albumhoes vastmaken in de ring
theme-dock-art-desc = dock_art - de hoes volgt de positie en grootte van de ring
theme-lyric-bounce = Songtekst-stuiter
theme-spring-stiffness = Veerstijfheid
theme-spring-damping = Veerdemping
theme-beat-pulse = Beat-puls
theme-reset-section = Deze sectie herstellen
theme-reset-section-desc = Herstelt { $element } naar de standaardwaarden.
theme-page-theme-title = Thema
theme-editing = Bewerken
theme-editing-live-desc = Dit thema is actief - wijzigingen verschijnen meteen op je bureaublad.
theme-editing-inactive-desc = Niet het actieve thema - wijzigingen worden opgeslagen in het bestand; pas toe om ze te zien.
theme-manage-title = Beheren
theme-create-new = Nieuw thema aanmaken
theme-create-new-desc = Begint met een volledig becommentarieerde sjabloonindeling.
theme-name-placeholder = Themanaam
theme-name-empty-error = De themanaam mag niet leeg zijn
theme-name-exists-error = Er bestaat al een thema met deze naam
theme-import = Importeren
theme-import-desc = Zet .toml-themabestanden ergens op deze pagina neer om ze toe te voegen.
theme-page-title = Lay-outthema's
theme-page-summary = Waar alles zich op het scherm bevindt. Schuif iets en zie je bureaublad meebewegen.

## Packs page

packs-your-packs-title = Jouw packs
packs-none-yet = Nog geen packs geïmporteerd
packs-none-yet-desc = Zet hieronder een .cwtheme-bestand neer om het hier te zien verschijnen.
packs-includes-video-active = Bevat een achtergrondvideo - nu actief.
packs-includes-video = Bevat een achtergrondvideo.
packs-layout-only = Alleen lay-out.
packs-export-title = Exporteren
packs-theme-to-bundle = Te bundelen thema
packs-export-desc-with-video = Bundelt de lay-out van dit thema, de aangepaste shader indien ingesteld, en je momenteel actieve achtergrondvideo ({ $file }) - video's zijn niet aan een specifiek thema gebonden, controleer dus of dit echt de video is die je wilt delen.
packs-export-desc-no-video = Bundelt de lay-out van dit thema en de aangepaste shader, indien aanwezig. Er is momenteel geen achtergrondvideo actief, dus de pack bevat er geen.
packs-export-pack = Pack exporteren
packs-folder = Packmap
packs-folder-desc = Geëxporteerde .cwtheme-bestanden komen terecht in ~/.config/cosmic-wallpaper/packs.
packs-import = Importeren
packs-import-desc = Zet een .cwtheme-bestand ergens op deze pagina neer om het toe te voegen.
packs-page-title = Packs
packs-page-summary = Deel een complete look - lay-out, achtergrondvideo en een aangepaste visualizer-shader - als één bestand.

## Now Playing page

now-playing-album-art-title = Albumhoes
now-playing-show-album-art = Albumhoes tonen
now-playing-show-album-art-desc = De huidige hoes, geplaatst volgens het actieve lay-outthema.
now-playing-lyrics-text-title = Songtekst & tekst
now-playing-show-lyrics = Songtekst tonen
now-playing-show-lyrics-desc = Gesynchroniseerde songtekst voor het huidige nummer, indien beschikbaar.
now-playing-font-family = Lettertype
now-playing-page-title = Nu afspelend
now-playing-page-summary = Wat verschijnt tijdens het afspelen van muziek: albumhoes, nummerinformatie en songtekst.

## Visualiser page

visualiser-audio-response-title = Audiorespons
visualiser-bands = Banden
visualiser-bands-desc = Hoeveel balken de visualizer tekent.
visualiser-smoothing = Afvlakking
visualiser-smoothing-desc = Hoger is rustiger; lager is directer.
visualiser-page-title = Visualizer
visualiser-page-summary = Balken die meebewegen met wat er ook speelt.

## Weather page

weather-unit-celsius = Celsius
weather-unit-fahrenheit = Fahrenheit
weather-poll-5min = 5 minuten
weather-poll-15min = 15 minuten
weather-poll-30min = 30 minuten
weather-poll-1hour = 1 uur
weather-page-title = Weer
weather-show-weather = Weer tonen
weather-show-weather-desc = Actuele omstandigheden op het bureaublad.
weather-hide-effects = Geanimeerde effecten verbergen
weather-hide-effects-desc = Schakelt regen- en sneeuwanimaties uit om energie te besparen.
weather-units = Eenheden
weather-location = Locatie
weather-location-desc = Breedte- en lengtegraad voor de voorspelling. 'Mijn locatie gebruiken' schat deze op basis van je IP-adres via ipapi.co.
weather-latitude-placeholder = Breedtegraad
weather-longitude-placeholder = Lengtegraad
weather-use-my-location = Mijn locatie gebruiken
weather-update-every = Bijwerken elke
weather-page-summary = Omstandigheden en effecten die over de achtergrond worden gelegd.

## General page

general-checking-for-updates = Bijwerkingen controleren...
general-up-to-date = Up-to-date
general-check-for-updates = Controleren op updates
general-check-failed = Controle mislukt: { $reason }
general-update-to = Bijwerken naar { $tag }
general-release-page = Versiepagina voor { $tag }
general-updating-to = Bezig met bijwerken naar { $tag }...
general-installed-restart = { $tag } geïnstalleerd - herstart
general-engine-title = Engine
general-wallpaper-engine = Wallpaper-engine
general-engine-running = Draait (pid { $pid }).
general-engine-not-running = Draait niet.
general-start-on-login = Starten bij aanmelden
general-start-on-login-desc = Start de wallpaper-engine wanneer je je aanmeldt.
general-frame-rate-limit = Framerate-limiet
general-frame-rate-limit-desc = Lager bespaart energie; de engine staat inactief wanneer niets animeert.
general-config-folder = Configuratiemap
general-config-folder-desc = Alle engine-configuratie staat hier.
general-about-title = Over
general-version = Versie
general-patch-notes = Versienotities
general-patch-notes-desc = Wat er is veranderd in de nieuwste versie.
general-diagnostics = Diagnose
general-diagnostics-desc = Versie, logfragment en GPU-info, klaar om in een rapport te plakken.
general-something-broken = Iets kapot?
general-something-broken-desc = Opent een vooraf ingevuld bugrapport met recente fouten bijgevoegd.
general-report-an-issue = Probleem melden
general-setup-title = Instellen
general-not-in-launcher = Niet in je app-starter
general-patch-notes-section-title = Versienotities
general-page-title = Algemeen
general-page-summary = Gedrag van de engine en onderhoud.

general-language-title = Taal
general-language-desc = Overschrijft de taal van het bureaublad alleen voor deze app - handig voor een taal die je bureaublad nog niet aanbiedt.
general-language-system-default = Systeemstandaard
