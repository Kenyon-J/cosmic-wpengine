## Language name (this catalog's own autonym, used to build the language picker)

language-name = Italiano

## Tray menu (src/modules/tray.rs)

tray-title = COSMIC Wallpaper
tray-open-settings = Apri impostazioni...
tray-quit-engine = Esci dal motore

## First-run desktop integration (src/modules/gui/bootstrap.rs)

general-launcher-issue = L'app non è ancora registrata nel launcher del desktop. Di norma viene configurata automaticamente al primo avvio di Impostazioni; se continua a mancare, verifica che ~/.local/share/applications sia scrivibile, poi riavvia Impostazioni.

## Status line (src/modules/gui/mod.rs) - the caption at the bottom of every page

status-ready = Pronto.
status-blocked-unsafe-theme-name = Nome del tema bloccato per sicurezza: { $name }
status-saved-theme-live = { $name } salvato: il desktop mostra già la modifica.
status-saved-theme-inactive = { $name } salvato. Applicalo per vederlo sul desktop.
status-error-saving-theme = Errore durante il salvataggio del tema: { $error }
status-error-serialising-theme = Errore durante la serializzazione del tema: { $error }
status-nothing-usable-dropped = Non è stato rilasciato nulla di utilizzabile: solo file MP4 o WebM.
status-importing-files = { $count ->
    [one] Importazione di { $count } file...
   *[other] Importazione di { $count } file...
}
status-detecting-location = Rilevamento della posizione...
status-location-detected = Posizione rilevata.
status-could-not-detect-location = Impossibile rilevare la posizione: { $error }
status-no-videos-imported = Nessun video importato: sono supportati solo file MP4, WebM, MKV, MOV e AVI.
status-imported-videos = { $n ->
    [one] Importato { $n } video.
   *[other] Importati { $n } video.
}
status-imported-skipped = Importati { $n }, saltati { $s } (non sono file video).
status-invalid-theme = Tema non valido: { $error }
status-imported-themes = { $imported ->
    [one] Importato { $imported } tema.
   *[other] Importati { $imported } temi.
}
status-nothing-imported-themes = Nessun elemento importato: rilascia file di tema .toml.
status-engine-starting = Avvio del motore in corso...
status-failed-to-start-engine = Impossibile avviare il motore: { $error }
status-could-not-find-engine-binary = Impossibile trovare il file binario cosmic-wallpaper accanto a Impostazioni.
status-killed-by-signal = terminato da un segnale
status-exit-code = codice di uscita { $code }
status-engine-exited-immediately = Il motore si è chiuso immediatamente ({ $code }).
status-engine-exited-immediately-detail = Il motore si è chiuso immediatamente ({ $code }): { $headline }
status-engine-running = Il motore è in esecuzione.
status-engine-did-not-start = Il motore non si è avviato: controlla i log.
status-engine-stopping = Arresto del motore in corso...
status-engine-still-running = Il motore è ancora in esecuzione.
status-engine-stopped = Motore arrestato.
status-error-applying-theme = Errore durante l'applicazione del tema: { $error }
status-applied-theme = Tema applicato: «{ $theme }»
status-select-theme-to-apply = Seleziona un tema da applicare.
status-created-theme = { $file_name } creato
status-theme-already-exists = Il tema «{ $name }» esiste già.
status-error-creating-theme = Errore durante la creazione del tema: { $error }
status-fetching-patch-notes = Recupero delle note di rilascio...
status-xdg-open-not-found = Impossibile aprire il link: xdg-open non trovato
status-xdg-open-folder-not-found = Impossibile aprire la cartella: xdg-open non trovato
status-diagnostics-copied = Diagnostica copiata negli appunti.
status-downloading = Download di { $tag } in corso...
status-update-failed = Aggiornamento non riuscito: { $error }
status-updated-restart-needed = Aggiornato a { $tag }! Il motore degli sfondi si è riavviato automaticamente; riavvia anche Impostazioni per usare la nuova versione.
status-select-theme-to-export = Seleziona un tema da esportare.
status-exported-pack = Pacchetto esportato in { $path }
status-error-exporting-pack = Errore durante l'esportazione del pacchetto: { $error }
status-pack-too-large = «{ $name }» è troppo grande per essere un pacchetto valido (limite { $limit } MB): saltato.
status-could-not-read-dropped-file = Impossibile leggere uno dei file rilasciati.
status-skip-one-shader-at-a-time = «{ $name }» saltato: importa un solo pacchetto con shader alla volta.
status-pack-renamed-on-import = «{ $original }» esisteva già: importato come «{ $written }».
status-pack-import-error = «{ $name }»: { $error }
status-not-a-valid-pack = Pacchetto non valido: { $error }
status-imported-packs = { $imported ->
    [one] Importato { $imported } pacchetto.
   *[other] Importati { $imported } pacchetti.
}
status-pack-includes-shader-review = Un pacchetto include uno shader personalizzato: verificalo qui sopra.
status-nothing-imported-packs = Nessun elemento importato: rilascia file di pacchetto .cwtheme.
status-pack-detail-video-shader = con video e uno shader personalizzato
status-pack-detail-shader = con uno shader personalizzato
status-imported-pack-named = Pacchetto «{ $name }» importato ({ $detail }).
status-pack-renamed-with-detail = «{ $name }» esisteva già: importato come «{ $written }» ({ $detail }).
status-error-importing-pack = Errore durante l'importazione del pacchetto: { $error }
status-cancelled-nothing-imported = Annullato: non è stato importato nulla.
status-applied-pack-with-video = Pacchetto «{ $name }» applicato insieme al video di sfondo.
status-applied-pack-video-missing = Pacchetto «{ $name }» applicato: il video di sfondo è mancante; reimporta il pacchetto per ripristinarlo.
status-applied-pack-theme-missing = Pacchetto «{ $name }» applicato: il file del tema è mancante, quindi sono stati usati valori predefiniti generici.
status-applied-pack = Pacchetto «{ $name }» applicato.
status-error-applying-pack = Errore durante l'applicazione del pacchetto: { $error }

## Shared across multiple pages (src/modules/gui/view.rs)

common-active = Attivo
common-active-now = Attivo ora.
common-align-center = Centro
common-align-left = Sinistra
common-align-right = Destra
common-apply = Applica
common-copied-to-clipboard = Copiato negli appunti
common-copy = Copia
common-copy-to-clipboard = Copia negli appunti
common-create = Crea
common-hide = Nascondi
common-open-folder = Apri cartella
common-recent-colours = Colori recenti
common-reset = Ripristina
common-retry = Riprova
common-shape-circular = Circolare
common-shape-linear = Lineare
common-shape-square = Quadrata
common-show = Mostra
common-start = Avvia
common-stop = Arresta

## Wallpaper page

wallpaper-mode-frosted-glass = Vetro smerigliato
wallpaper-mode-transparent = Trasparente
wallpaper-mode-album-art = Copertina dell'album
wallpaper-mode-album-colour = Colore dell'album
wallpaper-mode-live-wallpaper = Sfondo animato
text-color-mode-automatic = Automatico
text-color-mode-custom = Personalizzato
wallpaper-preview-none = Nessuno
wallpaper-preview-sample-title = Ancora, e ancora, e ancora, e ancora
wallpaper-preview-sample-caption = Riesco a sentire l'adrenalina, riesco a sentire il rumore
wallpaper-style-title = Stile
wallpaper-frosted-glass-title = Vetro smerigliato
wallpaper-blur-amount = Intensità della sfocatura
wallpaper-blur-amount-desc = Quanto viene sfocato lo sfondo.
wallpaper-live-wallpaper-title = Sfondo animato
wallpaper-video-item = Video
wallpaper-video-item-desc = Scegli e gestisci i video nella pagina Sfondi animati
wallpaper-text-title = Testo
wallpaper-text-colour = Colore del testo
wallpaper-text-colour-desc = La modalità automatica sceglie un colore che rimane leggibile sul tuo sfondo.
wallpaper-page-title = Sfondo
wallpaper-page-summary = Le opzioni dello stile selezionato compaiono qui sotto.

## Live Wallpapers page

live-wallpapers-drop-release = Rilascia per aggiungerlo alla libreria
live-wallpapers-drop-prompt = Rilascia qui i file video per aggiungerli (MP4, WebM)
live-wallpapers-library-title = Libreria
live-wallpapers-no-videos = Ancora nessun video
live-wallpapers-no-videos-desc = Rilascia i file qui sopra, oppure usa Apri cartella per aggiungerli manualmente
live-wallpapers-playback-title = Riproduzione
live-wallpapers-prefer-canvas = Preferisci Canvas di Spotify
live-wallpapers-prefer-canvas-desc = Se il brano in riproduzione ha un loop Canvas, mostralo al posto dello sfondo.
live-wallpapers-library-folder = Cartella della libreria
live-wallpapers-library-folder-desc = I video si trovano in ~/.config/cosmic-wallpaper/videos.
live-wallpapers-page-title = Sfondi animati
live-wallpapers-page-summary = Video in loop riprodotti come sfondo. Fai clic su una miniatura per impostarla.

## Themes page

theme-element-album-art = Copertina dell'album
theme-element-track-info = Informazioni brano
theme-element-lyrics = Testo del brano
theme-element-visualiser = Visualizzatore
theme-element-weather = Meteo
theme-element-effects = Effetti
theme-align = Allineamento
theme-position-x = Posizione X
theme-position-y = Posizione Y
theme-size = Dimensione
theme-text-size = Dimensione del testo
theme-shape = Forma
theme-docked = Agganciata
theme-docked-desc = Mentre la musica è in riproduzione, la copertina si posiziona dentro il visualizzatore circolare e ne segue posizione e dimensione. Disattiva l'aggancio nella scheda Visualizzatore per usare questi controlli.
theme-band-order = Ordine delle bande
theme-rotation = Rotazione
theme-amplitude = Ampiezza
theme-bar-width = Larghezza delle barre
theme-cap-roundness = Arrotondamento delle estremità
theme-glow = Bagliore
theme-reflection = Riflesso
theme-led-segments = Segmenti LED
theme-peak-hold = Picchi con trattenuta
theme-peak-hold-desc = peak_hold - una piccola punta luminosa che trattiene il picco recente di ogni barra e poi scende per gravità
theme-dock-art = Aggancia la copertina nell'anello
theme-dock-art-desc = dock_art - la copertina segue posizione e dimensione dell'anello
theme-lyric-bounce = Rimbalzo del testo
theme-spring-stiffness = Rigidità della molla
theme-spring-damping = Smorzamento della molla
theme-beat-pulse = Pulsazione a ritmo
theme-reset-section = Ripristina questa sezione
theme-reset-section-desc = Ripristina { $element } ai valori predefiniti.
theme-page-theme-title = Tema
theme-editing = Modifica
theme-editing-live-desc = Questo tema è attivo: le modifiche appaiono sul desktop non appena le apporti.
theme-editing-inactive-desc = Non è il tema attivo: le modifiche vengono salvate nel suo file; applicalo per vederle.
theme-manage-title = Gestisci
theme-create-new = Crea nuovo tema
theme-create-new-desc = Parte da un modello completo e interamente commentato.
theme-name-placeholder = Nome del tema
theme-name-empty-error = Il nome del tema non può essere vuoto
theme-name-exists-error = Esiste già un tema con questo nome
theme-import = Importa
theme-import-desc = Rilascia i file di tema .toml in un punto qualsiasi di questa pagina per aggiungerli.
theme-page-title = Temi di layout
theme-page-summary = Dove si trova ogni elemento sullo schermo. Sposta un valore e osserva il tuo desktop seguirlo.

## Packs page

packs-your-packs-title = I tuoi pacchetti
packs-none-yet = Ancora nessun pacchetto importato
packs-none-yet-desc = Rilascia qui sotto un file .cwtheme per vederlo comparire.
packs-includes-video-active = Include un video di sfondo: attivo ora.
packs-includes-video = Include un video di sfondo.
packs-layout-only = Solo layout.
packs-export-title = Esporta
packs-theme-to-bundle = Tema da includere
packs-export-desc-with-video = Include il layout di questo tema, il suo shader personalizzato se impostato, e il video di sfondo attualmente attivo ({ $file }) - i video non sono legati a un tema specifico, quindi verifica che sia quello che vuoi condividere.
packs-export-desc-no-video = Include il layout di questo tema e il suo shader personalizzato, se presente. Nessun video di sfondo è attualmente attivo, quindi il pacchetto non ne includerà uno.
packs-export-pack = Esporta pacchetto
packs-folder = Cartella dei pacchetti
packs-folder-desc = I file .cwtheme esportati vengono salvati in ~/.config/cosmic-wallpaper/packs.
packs-import = Importa
packs-import-desc = Rilascia un file .cwtheme in un punto qualsiasi di questa pagina per aggiungerlo.
packs-page-title = Pacchetti
packs-page-summary = Condividi un intero look - layout, video di sfondo e uno shader del visualizzatore personalizzato - in un solo file.

## Now Playing page

now-playing-album-art-title = Copertina dell'album
now-playing-show-album-art = Mostra copertina dell'album
now-playing-show-album-art-desc = La copertina attuale, posizionata secondo il tema di layout attivo.
now-playing-lyrics-text-title = Testo e didascalie
now-playing-show-lyrics = Mostra il testo del brano
now-playing-show-lyrics-desc = Testo sincronizzato del brano in riproduzione, quando disponibile.
now-playing-font-family = Famiglia del carattere
now-playing-page-title = In riproduzione
now-playing-page-summary = Cosa compare durante la riproduzione della musica: copertina, informazioni sul brano e testo.

## Visualiser page

visualiser-audio-response-title = Risposta audio
visualiser-bands = Bande
visualiser-bands-desc = Quante barre disegna il visualizzatore.
visualiser-smoothing = Attenuazione
visualiser-smoothing-desc = Più alto è più calmo; più basso è più reattivo.
visualiser-page-title = Visualizzatore
visualiser-page-summary = Barre che si muovono seguendo ciò che sta suonando.

## Weather page

weather-unit-celsius = Celsius
weather-unit-fahrenheit = Fahrenheit
weather-poll-5min = 5 minuti
weather-poll-15min = 15 minuti
weather-poll-30min = 30 minuti
weather-poll-1hour = 1 ora
weather-page-title = Meteo
weather-show-weather = Mostra il meteo
weather-show-weather-desc = Condizioni attuali sul desktop.
weather-hide-effects = Nascondi effetti animati
weather-hide-effects-desc = Disattiva le animazioni di pioggia e neve per risparmiare energia.
weather-units = Unità
weather-location = Posizione
weather-location-desc = Latitudine e longitudine per le previsioni. «Usa la mia posizione» le stima dal tuo indirizzo IP tramite ipapi.co.
weather-latitude-placeholder = Latitudine
weather-longitude-placeholder = Longitudine
weather-use-my-location = Usa la mia posizione
weather-update-every = Aggiorna ogni
weather-page-summary = Condizioni ed effetti sovrapposti allo sfondo.

## General page

general-checking-for-updates = Ricerca aggiornamenti in corso...
general-up-to-date = Aggiornato
general-check-for-updates = Cerca aggiornamenti
general-check-failed = Controllo non riuscito: { $reason }
general-update-to = Aggiorna a { $tag }
general-release-page = Pagina della versione { $tag }
general-updating-to = Aggiornamento a { $tag } in corso...
general-installed-restart = { $tag } installato: riavvia l'app
general-engine-title = Motore
general-wallpaper-engine = Motore degli sfondi
general-engine-running = In esecuzione (pid { $pid }).
general-engine-not-running = Non in esecuzione.
general-start-on-login = Avvia all'accesso
general-start-on-login-desc = Avvia il motore degli sfondi quando accedi.
general-frame-rate-limit = Limite fotogrammi
general-frame-rate-limit-desc = Un valore più basso risparmia energia; il motore resta inattivo quando nulla è animato.
general-config-folder = Cartella di configurazione
general-config-folder-desc = Tutta la configurazione del motore si trova qui.
general-about-title = Informazioni
general-version = Versione
general-patch-notes = Note di rilascio
general-patch-notes-desc = Cosa è cambiato nell'ultima versione.
general-diagnostics = Diagnostica
general-diagnostics-desc = Versione, estratto dei log e info sulla GPU, pronti da incollare in una segnalazione.
general-something-broken = Qualcosa non funziona?
general-something-broken-desc = Apre una segnalazione precompilata con gli errori recenti allegati.
general-report-an-issue = Segnala un problema
general-setup-title = Configurazione
general-not-in-launcher = Non presente nel launcher delle applicazioni
general-patch-notes-section-title = Note di rilascio
general-page-title = Generale
general-page-summary = Comportamento del motore e manutenzione.

general-language-title = Lingua
general-language-desc = Sostituisce la lingua del desktop solo per questa app - utile per una lingua che il tuo desktop non offre ancora.
general-language-system-default = Predefinita di sistema
